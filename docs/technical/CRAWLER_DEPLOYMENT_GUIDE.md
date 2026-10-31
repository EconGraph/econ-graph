# Crawler Deployment Guide

Enqueued data collection goes through the Postgres `crawl_queue`: jobs are enqueued (by the
built-in refresh scheduler, `triggerCrawl`, or the CLI) and one long-running `crawler-worker`
drains the queue. `crawler fetch` is the one exception: it fetches and persists a single series
directly, bypassing the queue, for debugging (see below).

```
GraphQL triggerCrawl (admin) ─┐
crawler enqueue / discover  ──┼─> crawl_queue ──> crawler-worker ──> source adapters ──> Postgres
RefreshScheduler (in-worker) ─┤                     (HttpFetcher: rate limits, retries)
SEC enqueue_filings         ──┘
```

Code: `backend/crates/econ-graph-crawler` (adapters, worker, CLI) and
`backend/crates/econ-graph-crawler-worker` (the deployed binary; adds the SEC filing handler).

## Enqueue work

- **GraphQL**: the `triggerCrawl` mutation (admin only) enqueues jobs and returns immediately.
- **CLI** (`crawler`, reads `DATA_DATABASE_URL`):

```bash
crawler enqueue  --source FRED --series GDP,UNRATE   # fetch_series jobs
crawler discover --source BLS                        # discover_catalog job (writes series metadata)
crawler refetch  --all [--dry-run]                   # refetch_series jobs: forced full-history re-fetch
crawler refetch-status --all [--failures N]          # per-source progress of those jobs
crawler status   [--json]                            # queue-derived status
crawler sources                                      # known sources, policies, API key state
crawler fetch    --source FRED --series GDP          # one fetch now, bypassing the queue (debugging)
crawler migrate  [--dry-run]                         # apply (or list) pending data migrations
crawler check-selection [--source FRED,BEA] [--json] # curated lists and rules vs live catalogs (no database)
```

From a checkout, run Cargo commands from `backend/` (`cd backend` from the repository
root): `cargo run -p econ-graph-crawler --bin crawler -- <command>`. First export
`DATA_DATABASE_URL` for the CLI and worker and apply the data migrations with
`crawler migrate`, following the [local setup](../../backend/README.md#development-workflow).
Export any required
provider API keys in the worker's environment and in the CLI's environment for
direct `fetch` calls.

An item is enqueued only if no active (pending / processing / retrying) item exists for the
same `(source, series_id, kind)`.

A `refetch_series` job re-reads a series' whole history, ignoring stored validators and
incremental bounds, and rewrites stored values in place; see
[Observation precision and full-history re-fetch](OBSERVATION_PRECISION_REFETCH.md) and
`scripts/deploy/refetch-full-history.sh`.

## Run the worker

Locally, from `backend/` with `DATA_DATABASE_URL` and provider API keys exported:
`cargo run -p econ-graph-crawler-worker --bin crawler-worker`.

Kubernetes: `k8s/manifests/crawler-worker.yaml` (one replica, `Recreate`, image
`econ-graph-crawler-worker:<version>` built from `backend/Dockerfile --target crawler-worker`).
`scripts/deploy/build-images.sh` and `scripts/deploy/deploy.sh` build and apply it. The worker
does not run migrations. Before applying it, `deploy.sh` runs the Job `crawler-migrate`
(`k8s/manifests/crawler-migrate-job.yaml`, `/app/crawler migrate` from the same image), which
applies the data migrations and must complete. When any data migration is pending, `deploy.sh`
first stops the running worker (`precheck-db-migration.sh --stop-crawler`), so it doesn't write
while they run; see
[database migrations](../development/MIGRATIONS.md#applying-the-data-set-crawler-migrate).

Configuration (flags or environment):

| Variable | Default | Meaning |
|---|---|---|
| `DATA_DATABASE_URL` | required | the data database (Postgres); `DATABASE_URL` alone is a startup error |
| `CRAWLER_CONCURRENCY` | 4 | claim loops (per-source limits still apply) |
| `CRAWLER_SOURCES` | all | comma-separated source filter |
| `CRAWLER_WORKER_ID` | hostname-based | lock owner written to `crawl_queue.locked_by` |
| `CRAWLER_POLL_INTERVAL_SECS` | 5 | idle poll interval |
| `CRAWLER_STUCK_AFTER_SECS` | 1800 | release items locked longer than this |
| `CRAWLER_PAUSE_AFTER` / `CRAWLER_PAUSE_SECS` | 5 / 300 | pause a source after N consecutive rate-limit/auth errors |
| `CRAWLER_HTTP_TIMEOUT_SECS` | 30 | ordinary HTTP request timeout; source reference-file requests use at least 120 seconds |
| `CRAWLER_DATA_DIR` | `/app/data` in the image | crawler selection files and dataset definitions, described below |
| `REFERENCE_DATA_DIR` | `/app/reference` in the image | shared country data and backend search abbreviations, described below |
| `FRED_API_KEY`, `BLS_API_KEY`, `BEA_API_KEY`, `CENSUS_API_KEY`, `WTO_API_KEY` | unset | from Secret `crawler-api-keys`; the worker starts without them, but FRED, BEA and Census jobs fail with an auth error when their key is missing. Without `WTO_API_KEY` the worker leaves the WTO source out entirely (no discovery, refreshes or coverage gauges) and logs a warning at startup |
| `CRAWLER_SCHEDULER` | true | enqueue due refreshes and catalog discovery in the background (see below); `false` disables it, leaving `triggerCrawl` / the CLI as the only way to enqueue work |
| `CRAWLER_SCHEDULER_INTERVAL_SECS` | 300 | seconds between scheduler ticks |

The crawler reads its selection files and dataset definitions from `CRAWLER_DATA_DIR`,
which defaults to `backend/crates/econ-graph-crawler/data/` in a source checkout.
`bls_series.csv` selects BLS series to crawl; BLS supplies their titles, units and frequency.
`wdi_indicators.csv` selects World Bank indicators and their units.
`catalog_selection/fred.toml` and `catalog_selection/bea.toml` hold the rules that select FRED
and BEA series from the upstream catalogs. Dataset definitions live in `datasets/<source>.toml`;
the production adapters use `fred.toml`, `bls.toml`, `bea.toml`, `census.toml`, `fhfa.toml` and
`world_bank.toml`.

Before processing jobs, the worker loads the BLS list and the FRED and BEA selection rules,
and validates dataset definitions for every registered adapter. Missing or invalid files in
these startup checks stop the worker. The World Bank indicator list is loaded when its
adapter first needs it. These loaders cache their results, including errors, for the
process lifetime; restart the worker after correcting a file.

`REFERENCE_DATA_DIR` defaults to `backend/crates/econ-graph-core/data/` in a source checkout.
Both the worker and backend load `countries.csv` at startup; the backend also loads
`search_abbreviations.csv`. Missing or invalid files in those checks stop the corresponding
service. These shipped files serve a different purpose from the provider-published labels
refreshed into the database under [Source reference data](#source-reference-data).

## Scheduler

With `CRAWLER_SCHEDULER` on (the default), the worker runs a background scheduler
(`econ_graph_crawler::scheduler::RefreshScheduler`) that only **enqueues** `crawl_queue` jobs;
fetching still goes through the same worker loop as a manual `crawler enqueue`. Every tick
(`CRAWLER_SCHEDULER_INTERVAL_SECS`, default 300s):

1. **Series refresh** — enqueues a `fetch_series` job for every active series that is due, up to
   500 per tick. A series becomes due this long after its last successful crawl:

   | frequency | interval |
   |---|---|
   | daily, business daily | 1 day |
   | weekly, biweekly | 7 days |
   | monthly | 7 days |
   | quarterly | 14 days |
   | annual, semiannual | 30 days |
   | anything else | 7 days |

   A series that has never been crawled is due immediately. A series whose `crawl_status`
   is `failed` backs off to twice its interval, measured from the latest of its last crawl
   and its last attempt. A persisted series marked `not_found` backs off to ten times its
   interval instead, as does one marked `absent` (a complete BEA response omitted it).

   A discovered series without an `economic_series` row has no attempt history, so its last
   failed `fetch_series` queue row supplies the failure time. Finished queue rows are purged
   after `CRAWLER_QUEUE_RETENTION_DAYS` (14 by default), limiting this backoff to about 14 days
   for a failed quarterly or annual discovered series. With purging disabled, the full
   twice-the-interval backoff applies. If the provider reports a metadata-only series as
   not found, the worker deactivates its `series_metadata` row instead; it becomes a candidate
   again only when catalog discovery lists that id again.

   Sources that track publication lifecycle (FRED and BEA) keep the row active instead and
   count confirmed NotFound results (and, separately, BEA responses that omit the series). A
   series with a publication state other than active is not a candidate, except one retired
   only by those fetch results (three NotFound, or six omissions in a row): it is
   probed once every 12 intervals, and a successful fetch makes it active again (see the
   [crawler coverage runbook](../monitoring/crawler-coverage-runbook.md#catalog-selection-and-retained-series)).

   Only sources in `CRAWLER_SOURCES` (default: all) are considered, and only series whose
   `data_sources` row is enabled and whose own `is_active` flag is set; for Census, only ids
   matching `fetchable_id_regex()` (national and per-state BDS ids).

   This also covers **newly discovered series**: catalog discovery writes `series_metadata` rows,
   and any such row without an `economic_series` row yet is treated as due like a never-crawled
   series, so it is picked up by the next scheduler tick with no separate enqueue step.

2. **Catalog discovery** — enqueues a `discover_catalog` job when a source is due and has
   no active discovery job. A completed or cancelled discovery delays the next job for 7 days
   (`DISCOVERY_INTERVAL`). After a job exhausts its worker retries and fails, the scheduler
   waits one hour, doubling the delay after consecutive failed jobs up to a one-day cap.
   Successful discovery resets that failure backoff. The scheduler derives this history from
   retained queue rows (14 days by default); purging history can reset the backoff, never
   lengthen it. Sources whose `data_sources` row is disabled are excluded.

   This applies to every adapter in `default_registry()`: FRED, BLS, BEA, Census, FHFA, WTO,
   Bank of Canada and World Bank with the release flag profile. The `static_catalogs` build
   flag also registers eight static catalogs (BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS). It is enabled
   by `FLAGS_PROFILE=dev`, the local Cargo default, and disabled by `FLAGS_PROFILE=release`,
   the image default. Cargo’s `--release` option controls optimization independently; static
   catalogs are selected through [build flags](../../config/flags/README.md#how-the-backend-reads-it),
   not `--features static-catalogs`. Static catalogs follow the same discovery schedule but
   never get a refresh (`fetch_series` is unimplemented for them). SEC filings are not covered
   by the scheduler; enqueue them with `sec-crawler enqueue --cik <CIK>[,<CIK>...]`
   (`econ-graph-sec-crawler`’s binary, reads `DATA_DATABASE_URL`).

Over a seven-day observation window, a source that completes discovery near the start is
not due again until about the end. A failed discovery can produce additional attempts within
the window under the failure backoff above. Daily series should refresh roughly every day,
subject to queue throughput (at most 500 `fetch_series` jobs enqueued per tick, shared with
first-time discovery) and each source’s rate limits. Watch queue depth as well as the schedule.
A weekly or monthly series crawled at the start becomes due again at about the seven-day mark,
so it may see at most one further refresh, or none if the window ends sooner. Quarterly
(14-day) and annual (30-day) series are not expected to refresh again within that window.
Use [monitoring](#monitoring) to check whether jobs are actually being enqueued and completed
on this cadence.

## Source reference data

Provider-published labels, such as Census state names and BLS code labels, are crawled
rather than shipped as data files. The worker refreshes each selected adapter’s code lists
in the background at startup, logging failures without holding up queue processing. Each
catalog discovery refreshes those lists again before discovering series.

Refresh requests use the validators stored in `reference_file_cache`: `If-None-Match` from
the `ETag` and `If-Modified-Since` from `Last-Modified`. A response whose body hashes to the
stored copy also counts as unchanged, covering sources that honor neither header.

A seed migration can populate a new database with those labels before its first crawl.
These migrations live in `backend/migrations/data/<timestamp>_seed_<source>_reference_codes/`
and load the labels with the validators they were downloaded with (`ETag`, `Last-Modified`
and body SHA-256). A migration loads a reference file only if that database does not already
contain it. Record one from a machine that can reach the source:

```bash
cd backend && cargo run -p econ-graph-crawler --bin crawler -- record-reference-seeds --source CENSUS
```

It replaces that source's previous seed migration. If any list fails to download or parse, it writes nothing and names every failure; `--skip-failed` writes the rest and leaves the failed lists for the crawl. A refused (401, 403) or throttled (429) request stops it at once. Never write or edit a seed by hand: made-up validators would let the crawl keep wrong labels as unchanged.

## Check curated lists and selection rules

`crawler check-selection` checks each source's curated list or catalog selection rules against
the source's live upstream catalog, without a database and without writing anything (its
database pool cannot connect, so a read or write would fail the check). With no `--source` it
checks FRED, BEA, BLS, World Bank, Census and FHFA, one source at a time; what each check
downloads and reports is in [Data sources](../data-sources/README.md#checking-selections-against-the-live-catalogs).

Its requests follow the worker's per-source policies and retries, at `--rate-percent` (default
25) of each policy's rate, burst and concurrency. Its rate limiter is its own, so beside a
running worker on the same API keys the two together send up to 125% of each policy rate. For
FRED, whose policy is its 120-a-minute limit, that is up to 150 a minute (the fetcher backs off
on 429s). BEA also caps data volume per minute and locks a key out for an hour when exceeded.
To stay within the limits, run the check while the worker is idle.

```bash
crawler check-selection --source FRED,BEA        # text report
crawler check-selection --json                   # every source, as JSON
```

| Exit status | Meaning |
|---|---|
| 0 | every checked source is clean |
| 1 | a source has a problem (missing, discontinued, restricted, over-broad, over-cap, over-quota) |
| 2 | a source's check could not finish (download, parse or authentication failure) |
| 3 | nothing worse, but a source was skipped because its API key is not set |
| 64 | the command line was refused: a parse error, or a source `check-selection` doesn't check or that is licence-gated |

The API keys live only in the cluster's `crawler-api-keys` Secret, so run the check there:

```bash
scripts/deploy/check-selection.sh --namespace econ-graph [--source FRED] [--json]
scripts/deploy/check-selection.sh --namespace econ-graph-br-<slug> --image econ-graph-crawler-worker:<tag>
```

The script requires `--namespace`. It runs the check as a one-off Job with the namespace's
`crawler-worker` image (or `--image`) and that namespace's optional key references, so keys never
appear on a command line or in output. It waits, prints the Job's log, deletes the Job and exits
with the check's status (its own usage errors exit 64, a Job that doesn't finish 4). It refuses
to start (exit 5) while another `crawler-check-selection` Job is running in the namespace, and
warns, naming the combined rate, when the namespace's `crawler-worker` is running; it never
scales the worker. A branch namespace without `crawler-api-keys` reports the keyed sources as
skipped.

## Retries

| Error | Queue transition |
|---|---|
| rate limited (429, 503 + Retry-After) | retry after Retry-After or backoff; does not count as an attempt |
| transient (timeout, 5xx, DB error) | retry with exponential backoff; fails at `max_retries` |
| not found, auth, parse, other 4xx | fail |

Per-source rate limits and backoff are in [CRAWLER_POLITENESS.md](./CRAWLER_POLITENESS.md).

## Monitoring

Use `crawler status` or the GraphQL crawler status query to inspect queue-derived status,
and the worker’s logs (`RUST_LOG`) to diagnose failures. The `crawler_queue_*` and
`crawler_coverage_*` Prometheus metrics in `econ-graph-metrics` show queue activity and source
coverage; see the [crawler coverage and freshness runbook](../monitoring/crawler-coverage-runbook.md)
for queries and alerts.
