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
  digits after it, and its text at most 128 characters. Text that isn't a plain decimal
  (`NaN`, `inf`, `1,234` after the adapter's own clean-up) is rejected. The worker checks every
  fetched value against the same limits before persisting
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
  a new release since the last crawl, the re-fetch stores it as a new vintage.
- **BLS, BEA, Census, FHFA** store one current value per date (revision date = observation
  date), which the re-fetch rewrites with the source's current value, as any refresh does.
  BLS history beyond the adapter's 20-year window is not re-read.
- A series the source no longer publishes fails with `not_found` and keeps its stored values.

### Running it

Run from a checkout with `kubectl` access to the cluster. The script runs the crawler CLI in the
`crawler-worker` pod (`/app/crawler`), which uses the worker's own database URL:

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
time filled in. The underlying commands are `crawler refetch` and `crawler refetch-status` (see
`crawler --help`); `--json` gives machine-readable output.

- **Which series.** The series the refresh scheduler refreshes: active `economic_series` rows
  and active discovered `series_metadata` rows not fetched yet, without the `legacy` dataset or
  Census ids the adapter can't fetch. `--include-inactive` adds retired series.
- **Idempotent.** A series with a pending, processing or retrying `refetch_series` job is not
  enqueued again; re-running the command after an interruption only adds what is missing.
- **Priority.** Jobs default to priority 3, below scheduled refreshes (5), so regular crawling
  continues while the re-fetch drains. `--priority` overrides it.
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

The migration that made the columns unconstrained
(`2026-10-25-000100_observation_values_unconstrained_numeric`) changes only the catalog: no table
rewrite and no index rebuild, under a brief `ACCESS EXCLUSIVE` lock. Its `down.sql` narrows the
columns back to `DECIMAL(20,6)`, which rewrites both tables and rebuilds their indexes. It refuses,
changing nothing, while any stored value needs more than 14 integer or 6 fractional digits, or
while any `refetch_series` queue row exists; it never rounds or deletes values itself.
