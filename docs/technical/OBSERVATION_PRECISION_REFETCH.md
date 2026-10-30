# Observation precision and full-history re-fetch

## How observation values are stored

`data_points.value` and `global_indicator_data.value` are unconstrained PostgreSQL `NUMERIC`:
each value is stored exactly as the source published it, with every digit and the source's own
scale (`1.50` stays `1.50`). The path from the source to the user keeps those digits:

- **Ingestion.** Every adapter parses value text with
  `econ_graph_crawler::value::parse_observation_value` into a `BigDecimal`, never through a
  binary float. World Bank values are JSON numbers; the adapter reads each one's text from the
  response (`serde_json::value::RawValue`) instead of letting the JSON parser turn it into an
  `f64`. BEA's `UNIT_MULT` scaling is an exact decimal multiplication.
- **Limits.** A value may have at most 40 digits before the decimal point and 30 significant
  digits after it. Its text may contain at most 128 characters, with at most 128 digits after
  the decimal point and an exponent of at most 40. The exponent limit also applies when the
  value is zero: `0e999999999` is rejected.

  Text that isn't a plain decimal (`NaN`, `inf`, `1,234` after the adapter's own
  clean-up) is rejected. A source's missing-value marker (FRED's `.`, BLS and BEA markers
  without digits, a Census letter flag) is a missing value; anything else that fails to parse
  is an error, never a missing value that would overwrite a stored one. The worker checks
  every fetched value against the same limits before persisting
  (`value::check_fetched_values`); a series with a value outside them fails with a parse error
  and nothing of it is written.
- **API.** GraphQL serializes values as decimal strings (`BigDecimal`). Derived values are
  computed from those decimals, except the log-difference transformation, which is computed in
  `f64` (it is a displayed growth rate, not a stored value).
- **CSV download.** The series page's CSV writes each value's decimal string as the API sent it.
  Charts plot the nearest `f64`.

## Recovering precision lost before

Values stored while the column was `DECIMAL(20,6)` were rounded to six fractional digits, and
values of 10^14 or more (World Bank aggregates such as `NY.GDP.MKTP.PP.CD` for `WLD`) could not
be stored at all. The stored values don't show which ones were rounded. A full-history re-fetch
reads every series again from its source and rewrites any stored value that differs.

### What a re-fetch does

The crawler CLI enqueues one `refetch_series` job per series. The crawler worker processes it
like a scheduled `fetch_series` job (same queue, same per-source rate limits, concurrency
limits, retries and circuit breaker), except that it:

- passes no `since` and no known vintage, so the adapter requests the series' whole history
  (FRED: every ALFRED vintage, through the normal FRED fetch, which reads the vintage dates
  and requests them in real-time windows of at most 1,000;
  BLS: the adapter's full window of 20 years);
- sets `CrawlCtx::bypass_validators`, so the adapter ignores the validators stored with the
  series (FRED `last_updated`, World Bank `lastupdated`, FHFA `ETag`/`Last-Modified`/content
  hash) and fetches even when they say nothing changed. The new validators are stored as usual.

Points are upserted on the `data_points` key `(series_id, date, revision_date,
is_original_release)`:

- Series keep their ids, metadata, annotations and relationships. Nothing is deleted.
- A stored value that differs from the source's is updated in place (same row); an identical
  one is not rewritten.
- A vintage the source still serves is rewritten under its own revision date. A vintage it no
  longer serves keeps its stored value; it is never replaced with the current estimate.

### What can and cannot be recovered

Only precision the source still serves can be recovered:

- **FRED** serves every vintage through ALFRED, so all stored vintages are corrected.
- **World Bank** serves only the current database release. Points carry the release's
  `lastupdated` as their revision date: points of the current release are corrected; points of
  earlier releases keep their stored (possibly rounded) values. If the World Bank has published
  a new release since the last crawl, the re-fetch stores its new and changed values as a new
  vintage; a value equal to the newest stored one is not written again.
- **BLS, BEA, Census, FHFA** store one current value per date (revision date = observation
  date), which the re-fetch rewrites with the source's current value, as any refresh does.
  BLS history beyond the adapter's 20-year window is not re-read.
- A series the source no longer publishes fails with `not_found` and keeps its stored values.

### Deploying the migration

Migration `2026-10-25-000100_observation_values_unconstrained_numeric` (version
`20261025000100`) is a data plane migration (`backend/migrations/data`). Only `crawler migrate`
applies it: `scripts/deploy/deploy.sh` runs it as the Job `crawler-migrate` before it applies
the backend and `crawler-worker`, and stops the deploy if the Job fails (see
[database migrations](../development/MIGRATIONS.md#applying-the-data-set-crawler-migrate)). The
backend never applies it. A database that has it recorded already (from a release whose single
migration directory held it, under the same version) has nothing pending.

The forward migration rewrites neither observation table and keeps `data_points`' indexes.
It rebuilds only `global_indicator_data`'s partial index `idx_global_data_date_value`, whose
predicate reads the column. It takes `ACCESS EXCLUSIVE` locks on `crawl_queue`, `data_points`
and `global_indicator_data`, in the crawler worker's order, and holds them until commit.

Lock acquisition is bounded: each lock waits at most 10 seconds (`lock_timeout`), and the
statement taking all three locks has a total limit of 10 seconds (`statement_timeout`). If
other transactions hold the tables, the migration gives up after about 10 seconds with
nothing changed. Once it has the locks, it sets `statement_timeout = 0`, so the alterations
and partial-index rebuild have no statement time limit.

Deploy in the order below. Define `KC` in your shell to run the commands with the kind
cluster's kubectl and context:

```bash
KC() {
  KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph \
    ~/.cache/econgraph/bin/kubectl --context kind-econ-graph "$@"
}
```

1. **Pick the time.** The whole deploy, from the check below until `crawler-migrate` has
   completed, stays clear of 08:00 UTC, when the nightly `postgres-backup` CronJob starts and
   holds a long transaction on the data database.
2. **Check the database, immediately before deploying.** Run
   `scripts/deploy/precheck-db-migration.sh --stop-crawler` (first with `--dry-run`, which
   changes nothing) right before `deploy.sh`, so nothing new starts in between. The precheck
   script uses the kind cluster's kubectl and the `kind-econ-graph` context, matching `deploy.sh`.
   It exits without proceeding if a `postgres-backup` or `crawler-migrate` Job is running.
   Otherwise, it scales `crawler-worker` to 0 and waits for its pods to stop; a dry run reports
   this action without performing it. It then lists transactions on `econ_graph` that have
   been open for more than 5 seconds or are idle in a transaction, and exits non-zero if any
   remain. The precheck script never ends a session: find the owner it names, stop or finish
   that work, and run it again until it passes. If a check fails after it stopped the worker,
   it prints the command that scales the worker back.
3. **Deploy and confirm.** Run `scripts/deploy/deploy.sh`. The `crawler-migrate` Job applies
   the migration, and `deploy.sh` applies `crawler-worker.yaml` only once the Job has completed,
   so the worker stays stopped while the migration runs. Then check that the migration is
   recorded:
   `KC -n econ-graph exec postgresql-0 -c postgresql -- psql -U postgres -d econ_graph -Atc "SELECT version FROM __diesel_schema_migrations WHERE version = '20261025000100'"`
   prints `20261025000100`.

   If other transactions prevent lock acquisition, the Job's pod fails after about 10 seconds
   with a lock or statement timeout, without applying this migration. The Job tries once more
   (`backoffLimit: 1`). If the tables are still held, the Job fails. `deploy.sh` prints the
   Job's log and stops before applying the backend or `crawler-worker`. The worker stays
   stopped and the previous backend keeps serving.

   Run `scripts/deploy/precheck-db-migration.sh --dry-run` to list the sessions holding
   transactions open. Deal with their owner, then run `deploy.sh` again; it replaces the failed
   Job.
4. **Crawler back on the new image.** After `crawler-migrate` completes, `deploy.sh` applies
   `crawler-worker.yaml`, which brings the worker back at 1 replica with the new image. Check
   with `KC -n econ-graph get deployment crawler-worker -o jsonpath='{.status.readyReplicas} {..image}'`;
   if the migration was applied some other way, `KC -n econ-graph scale deployment/crawler-worker --replicas=1`.
5. **Only then re-fetch,** as below.

### Running it

Enqueue only after every `crawler-worker` pod runs an image that knows the `refetch_series`
kind (check `KC -n econ-graph get pods -l app=crawler-worker -o
jsonpath='{..image}'`, `KC` as above). A worker from an earlier image fails each `refetch_series` job it claims
as an unknown job kind; running the command again after the rollout re-enqueues those series.

Run from a checkout on the host of the kind cluster. The script runs the crawler CLI in the
`crawler-worker` pod (`/app/crawler`) through the kind cluster's own kubectl on context
`kind-econ-graph`, and the CLI uses the worker's own data database URL (`DATA_DATABASE_URL`).
Before the crawler runs, the script prints the cluster's context, `kube-system` UID and
production marker:

```bash
# What would be enqueued, per source (writes nothing).
scripts/deploy/refetch-full-history.sh --all --dry-run

# Enqueue every source, or some.
scripts/deploy/refetch-full-history.sh --all
scripts/deploy/refetch-full-history.sh --source WORLD_BANK,FRED

# Progress per source: series counted by their latest re-fetch job, with failed series listed.
scripts/deploy/refetch-full-history.sh --status --all --since 2026-10-26T00:00:00Z --failures 50

# Resume: enqueue what is left (failed series included), skipping series completed since then.
scripts/deploy/refetch-full-history.sh --all --completed-since 2026-10-26T00:00:00Z
```

After enqueuing, the script prints the matching progress and resume commands with the start
time filled in; the resume command keeps `--include-inactive` and `--priority N`. The underlying commands are `crawler refetch` and `crawler refetch-status` (see
`crawler --help`); `--json` gives machine-readable output.

- **Which series.** The series the refresh scheduler refreshes: active `economic_series` rows
  and active discovered `series_metadata` rows not fetched yet, without the `legacy` dataset or
  Census ids the adapter can't fetch. `--include-inactive` adds retired series.
- **Idempotent.** A series with a pending, processing or retrying `refetch_series` job is not
  enqueued again; re-running the command after an interruption only adds what is missing.
- **Priority.** Jobs default to priority 3, below scheduled refreshes (5), so regular crawling
  continues while the re-fetch drains. `--priority` overrides it.
- **Scheduled refreshes.** While a series has an active `refetch_series` job, the refresh
  scheduler doesn't enqueue a `fetch_series` job for it. A refresh already queued when the
  re-fetch is enqueued still runs; both write the source's current values.
- **Resumable.** Jobs live in `crawl_queue` and survive worker restarts. A job that failed for
  good stays `failed` until the command is run again.
- **Progress window.** Finished queue rows are purged after `CRAWLER_QUEUE_RETENTION_DAYS`
  (14 days by default), so check progress within that window. Each attempt is also recorded in
  `crawl_attempts`.
- **Rate limits.** The re-fetch adds load only through the queue; each source's limiter decides
  how fast it drains. A full World Bank pass is one request per indicator (jobs are enqueued in
  external-id order so an indicator's series batch together); a full FRED pass is one series
  request plus the paged vintage requests per series.

## Rolling back the column type

The migration's `down.sql` narrows the columns back to `DECIMAL(20,6)`, which rewrites both tables and
rebuilds their indexes. It locks the tables before checking them, and refuses, changing
nothing, while any stored value needs more than 14 integer or 6 fractional digits, or while any
`refetch_series` queue row exists; it never rounds or deletes values itself.
