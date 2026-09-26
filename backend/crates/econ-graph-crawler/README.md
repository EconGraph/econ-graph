# EconGraph Crawler

Data collection for EconGraph. All crawling goes through the Postgres `crawl_queue`:

1. **Enqueue** jobs, either from GraphQL (`triggerCrawl`, admin only; it only enqueues) or with the
   `crawler` CLI below.
2. **`crawler-worker`** (crate `econ-graph-crawler-worker`, deployed by `k8s/manifests/crawler-worker.yaml`)
   claims jobs, runs the source adapter (`src/sources/*`) through the shared rate-limited `HttpFetcher`,
   persists the result and retries or fails the job by error kind.

3. **Refresh scheduler** (`src/scheduler.rs`, runs inside `crawler-worker`; disable with
   `--scheduler false` / `CRAWLER_SCHEDULER=false`, tick every `--scheduler-interval-secs` /
   `CRAWLER_SCHEDULER_INTERVAL_SECS`, default 300) only enqueues: each tick it adds `fetch_series` jobs
   (priority 5, at most 500, oldest first) for active series that are due, and a `discover_catalog` job for
   each registered source whose last discovery finished more than 7 days ago. A series is due if it was
   never crawled or `last_crawled_at` is older than its frequency's interval: daily 1 day, weekly and
   monthly 7 days, quarterly 14 days, annual/semiannual 30 days, anything else 7 days. Series with
   `crawl_status = 'failed'` wait twice as long (from their last attempt). Skipped: static catalogs,
   sources whose `fetch_series` isn't implemented (WORLD_BANK, IMF, BEA), SEC, and disabled `data_sources`.
   Duplicates of active jobs are rejected by the queue's unique index, so no leader election is needed.

Job kinds: `fetch_series` (download one series), `discover_catalog` (write a source's series metadata),
`fetch_filing` (SEC, handled by `econ-graph-sec-crawler`).

## `crawler` CLI

Reads `DATABASE_URL` (or `--database-url`) and source API keys from `FRED_API_KEY`, `BLS_API_KEY`,
`BEA_API_KEY`, `CENSUS_API_KEY`.

```bash
crawler enqueue  --source FRED --series GDP,UNRATE [--priority 1-10]   # fetch_series jobs
crawler discover --source WORLD_BANK [--priority 1-10]                 # discover_catalog job
crawler status   [--json]                                              # queue-derived status
crawler sources                                                        # sources, policies, key state
crawler fetch    --source FRED --series GDP [--full]                   # one fetch now, bypassing the queue
```

From the repo: `cargo run -p econ-graph-crawler --bin crawler -- <command>`; the worker:
`cargo run -p econ-graph-crawler-worker --bin crawler-worker`.

## Tests

```bash
cargo test -p econ-graph-crawler --all-features -- --test-threads=1   # DB-backed tests use DATABASE_URL
```

Adapters are tested against a local mock upstream (`testkit`, see `src/testkit`); no network needed.

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.
