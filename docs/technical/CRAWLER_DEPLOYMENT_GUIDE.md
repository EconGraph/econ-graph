# Crawler Deployment Guide

All data collection goes through the Postgres `crawl_queue`. Nothing crawls on a schedule by
itself: jobs are enqueued, and one long-running `crawler-worker` drains the queue.

```
GraphQL triggerCrawl (admin) ─┐
crawler enqueue / discover  ──┼─> crawl_queue ──> crawler-worker ──> source adapters ──> Postgres
SEC enqueue_filings         ──┘                     (HttpFetcher: rate limits, retries)
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
| `FRED_API_KEY`, `BLS_API_KEY`, `BEA_API_KEY`, `CENSUS_API_KEY` | unset | from Secret `crawler-api-keys` (all optional) |

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
