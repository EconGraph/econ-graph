# EconGraph Crawler

Data collection for EconGraph. All crawling goes through the Postgres `crawl_queue`:

1. **Enqueue** jobs, either from GraphQL (`triggerCrawl`, admin only; it only enqueues) or with the
   `crawler` CLI below.
2. **`crawler-worker`** (crate `econ-graph-crawler-worker`, deployed by `k8s/manifests/crawler-worker.yaml`)
   claims jobs, runs the source adapter (`src/sources/*`) through the shared rate-limited `HttpFetcher`,
   persists the result and retries or fails the job by error kind.

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
