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
- **CLI** (`crawler`, reads `DATABASE_URL`):

```bash
crawler enqueue  --source FRED --series GDP,UNRATE   # fetch_series jobs
crawler discover --source BLS                        # discover_catalog job (writes series metadata)
crawler status   [--json]                            # queue-derived status
crawler sources                                      # known sources, policies, API key state
crawler fetch    --source FRED --series GDP          # one fetch now, bypassing the queue (debugging)
```

From a checkout: `cargo run -p econ-graph-crawler --bin crawler -- <command>`.

An item is enqueued only if no active (pending / processing / retrying) item exists for the
same `(source, series_id, kind)`.

## Run the worker

Locally: `DATABASE_URL=... cargo run -p econ-graph-crawler-worker --bin crawler-worker`.

Kubernetes: `k8s/manifests/crawler-worker.yaml` (one replica, `Recreate`, image
`econ-graph-crawler-worker:<version>` built from `backend/Dockerfile --target crawler-worker`).
`scripts/deploy/build-images.sh` and `scripts/deploy/deploy.sh` build and apply it. The worker
does not run migrations; the backend applies them at startup.

Configuration (flags or environment):

| Variable | Default | Meaning |
|---|---|---|
| `DATABASE_URL` | required | Postgres |
| `CRAWLER_CONCURRENCY` | 4 | claim loops (per-source limits still apply) |
| `CRAWLER_SOURCES` | all | comma-separated source filter |
| `CRAWLER_WORKER_ID` | hostname-based | lock owner written to `crawl_queue.locked_by` |
| `CRAWLER_POLL_INTERVAL_SECS` | 5 | idle poll interval |
| `CRAWLER_STUCK_AFTER_SECS` | 1800 | release items locked longer than this |
| `CRAWLER_PAUSE_AFTER` / `CRAWLER_PAUSE_SECS` | 5 / 300 | pause a source after N consecutive rate-limit/auth errors |
| `CRAWLER_HTTP_TIMEOUT_SECS` | 30 | per-request timeout |
| `CRAWLER_DATA_DIR` | `/app/data` in the image | reference data read at runtime (`bls_series.csv`, the BLS series list; and `datasets/<source>.toml` for each adapter that declares datasets, today FRED, BLS, Census and FHFA); the worker exits at startup if any of it is missing |
| `REFERENCE_DATA_DIR` | `/app/reference` in the image | shared reference data (`countries.csv`, econ-graph-core's `data/`); the worker and the backend exit at startup if it is missing |
| `FRED_API_KEY`, `BLS_API_KEY`, `BEA_API_KEY`, `CENSUS_API_KEY` | unset | from Secret `crawler-api-keys`; the worker starts without them, but FRED, BEA and Census jobs fail with an auth error when their key is missing |
| `CRAWLER_SCHEDULER` | true | enqueue due refreshes and catalog discovery in the background (see below); `false` disables it, leaving `triggerCrawl` / the CLI as the only way to enqueue work |
| `CRAWLER_SCHEDULER_INTERVAL_SECS` | 300 | seconds between scheduler ticks |

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

   A series that has never been crawled is due immediately. A series whose last crawl failed
   backs off to twice its interval, measured from the latest of its last crawl and its last
   attempt, before being retried. A discovered series (no `economic_series` row yet) has no
   attempt history, so its last failed `fetch_series` queue row stands in instead — and that row
   is purged after `CRAWLER_QUEUE_RETENTION_DAYS` (14 by default), so a failed quarterly or
   annual discovered series backs off at most ~14 days, not the full twice-the-interval. See the
   `scheduler` module docs for the exact rule.

   Only sources in `CRAWLER_SOURCES` (default: all) are considered, and only series whose
   `data_sources` row is enabled and whose own `is_active` flag is set; for Census, only ids
   matching `fetchable_id_regex()` (national and per-state BDS ids).

   This also covers **newly discovered series**: catalog discovery writes `series_metadata` rows,
   and any such row without an `economic_series` row yet is treated as due like a never-crawled
   series, so it is picked up by the next scheduler tick with no separate enqueue step.

2. **Catalog discovery** — enqueues one `discover_catalog` job per source at most once every 7
   days (`DISCOVERY_INTERVAL`) after the source's last discovery job *finished*, whether it
   completed or failed — so a discovery that fails is not retried for 7 days either.

   Applies to every adapter in a release build's `default_registry()` (FRED, BLS, BEA, Census,
   FHFA, World Bank). A debug build (or `--features static-catalogs`) also registers ten static
   catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO); those get weekly discovery
   but never a refresh (`fetch_series` is unimplemented for them). SEC filings are not covered by
   the scheduler at all; enqueue them with `sec-crawler enqueue --cik <CIK>[,<CIK>...]`
   (`econ-graph-sec-crawler`'s binary, reads `DATABASE_URL`).

**For QA's seven-day run:** with the defaults above, expect at most one catalog discovery per
source over seven days (and none at all for a source whose discovery fails early in the run).
Daily series should refresh roughly every day, subject to queue throughput (at most 500
`fetch_series` jobs enqueued per tick, shared with any large first-time discovery) and each
source's own rate limits — watch queue depth, not just the schedule. A weekly or monthly series
crawled at the start of the run becomes due again only at about the 7-day mark, so it may see at
most one refresh in the window, or none if the run is shorter. Quarterly (14-day) and annual
(30-day) series are not expected to refresh again inside a single seven-day run. `crawler status`
and the `crawler_queue_*` / `crawler_coverage_*` Prometheus metrics (see Monitoring below) show
whether jobs are actually being enqueued and completed on this cadence.

## Source reference data

Labels a source publishes itself (Census state names, BLS code labels) are crawled, never shipped as data files: the worker refreshes each adapter's code lists at startup and before each catalog discovery, with a conditional GET on the `ETag` stored in `reference_file_cache`.

A new database can get those labels before the first crawl from a seed migration, `backend/migrations/<timestamp>_seed_<source>_reference_codes/`, which loads them with the `ETag` they were downloaded with, and only if this database doesn't have the file yet. Record one from a machine that can reach the source:

```bash
cd backend && cargo run -p econ-graph-crawler --bin crawler -- record-reference-seeds --source CENSUS
```

It replaces that source's previous seed migration. If any list fails to download or parse, it writes nothing and names every failure; `--skip-failed` writes the rest and leaves the failed lists for the crawl. A refused (401, 403) or throttled (429) request stops it at once. Never write or edit a seed by hand: a made-up `ETag` would let a `304` keep wrong labels.

## Retries

| Error | Queue transition |
|---|---|
| rate limited (429, 503 + Retry-After) | retry after Retry-After or backoff; does not count as an attempt |
| transient (timeout, 5xx, DB error) | retry with exponential backoff; fails at `max_retries` |
| not found, auth, parse, other 4xx | fail |

Per-source rate limits and backoff are in [CRAWLER_POLITENESS.md](./CRAWLER_POLITENESS.md).

## Monitoring

`crawler status` or the GraphQL crawler status query (both derived from `crawl_queue`), the
worker's logs (`RUST_LOG`), and the crawler Prometheus metrics in `econ-graph-metrics`.
