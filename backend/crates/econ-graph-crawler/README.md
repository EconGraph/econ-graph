# EconGraph Crawler

Data collection for EconGraph. Background crawling uses the Postgres `crawl_queue`:
GraphQL's admin-only `triggerCrawl`, the `crawler` CLI, and the refresh scheduler enqueue
jobs; `crawler-worker` claims and executes them. The worker runs source adapters through the
shared rate-limited `HttpFetcher`, persists results, and retries or fails jobs by error kind.
The worker lives in the `econ-graph-crawler-worker` crate and is deployed by
[`crawler-worker.yaml`](../../../k8s/manifests/crawler-worker.yaml).

The queue supports `fetch_series` to download one series, `refetch_series` to force a
full-history re-fetch, and `discover_catalog` to write a source's series metadata.
`fetch_filing` jobs are handled separately by `econ-graph-sec-crawler` for SEC filings.
For debugging, `crawler fetch` runs an adapter and persists its result directly in the CLI
process, bypassing the queue.

## Refresh scheduler

The [refresh scheduler](src/scheduler.rs) runs inside `crawler-worker` and only enqueues work.
It is enabled by default; disable it with `--scheduler false` or `CRAWLER_SCHEDULER=false`.
Set the time between ticks in seconds with `--scheduler-interval-secs` or
`CRAWLER_SCHEDULER_INTERVAL_SECS`; the default is 300.

Each tick enqueues at most 500 `fetch_series` jobs at priority 5. Candidates include active
`economic_series` rows that are due and active `series_metadata` rows from discovery that
have no `economic_series` row yet. The two groups take turns in the batch, each oldest first,
so an initial discovery cannot starve refreshes. Series refresh requires a registered source
with an enabled `data_sources` row and excludes static catalogs and SEC. World Bank and BEA
both support series refresh.

An existing series is due if it has never been crawled or its `last_crawled_at` is at least
one refresh interval old. The interval depends on frequency: daily and business daily use
1 day; weekly, biweekly, and monthly use 7 days; quarterly uses 14 days; annual and semiannual
use 30 days; anything else uses 7 days. A series with `crawl_status = 'failed'` waits twice
its interval from the later of its last successful crawl and last attempt. A series with
`crawl_status = 'not_found'`, meaning the source confirmed it does not exist, waits ten times
its interval from that same reference time.

A discovered series with no `economic_series` row yet uses its last failed `fetch_series`
queue row for the twice-interval delay. This history lasts only as long as queue retention
(14 days by default), so purging can shorten that delay for quarterly and annual series.
Disabling purging preserves the full delay. If a fetch returns NotFound before an
`economic_series` row exists, the worker instead deactivates the `series_metadata` row with `persist::deactivate_metadata_conn`.
That series is no longer a scheduling candidate until catalog discovery lists it again.

Catalog discovery considers registered sources other than SEC, including static catalogs,
unless an existing `data_sources` row is disabled. Neither refresh nor discovery includes a
[licence-gated](#licence-gate) source. A source with no retained discovery
history is eligible immediately. After a completed or cancelled discovery, the scheduler
waits 7 days. After a terminal failure, once worker retries are exhausted, it waits 1 hour;
consecutive failed discovery jobs double that delay up to 1 day. Successful discovery resets
the failure backoff. Discovery history also comes from retained queue rows, so purging that
history resets the backoff.

Active jobs (`pending`, `processing`, or `retrying`) suppress new discovery jobs for the same
source and new series refreshes for a series with a `fetch_series` or `refetch_series` job.
The queue's unique index rejects concurrent duplicates, so no leader election is needed.

## `crawler` CLI

Reads `DATA_DATABASE_URL` (or `--database-url`; `DATABASE_URL` is not read) and source API keys from `FRED_API_KEY`, `BLS_API_KEY`,
`BEA_API_KEY`, `CENSUS_API_KEY`.

```bash
crawler enqueue  --source FRED --series GDP,UNRATE [--priority 1-10]   # fetch_series jobs
crawler discover --source WORLD_BANK [--priority 1-10]                 # discover_catalog job
crawler status   [--json]                                              # queue-derived status
crawler sources                                                        # sources, policies, key state
crawler fetch    --source FRED --series GDP [--full]                   # one fetch now, bypassing the queue
```

Run from `backend/` (use `cd backend` from the repository root):
`cargo run -p econ-graph-crawler --bin crawler -- <command>`; the worker:
`cargo run -p econ-graph-crawler-worker --bin crawler-worker`.

Before queue commands or worker startup, export `DATA_DATABASE_URL` and apply the data
migrations with `crawler migrate` (`--dry-run` lists the pending versions and changes nothing);
see the [local setup](../../README.md#development-workflow). `crawler migrate` is the only
command that applies them: the worker applies no migrations, and the backend applies only the
app migrations. Export the relevant provider API keys in the
worker's environment as well as in the CLI's environment for direct `fetch` calls.

Use `crawler --help` for the full command list. For queue-backed full-history re-fetches and
progress reporting, see the [re-fetch guide](../../../docs/technical/OBSERVATION_PRECISION_REFETCH.md).

## Licence gate

Some providers' terms of use forbid the way the crawler would use them. The
[licence gate](src/licence.rs) keeps those sources from being crawled unless a build enables
them. Each gated source has a `crawl_<source>` build flag in
[`flags.flagd.json`](../../../config/flags/flags.flagd.json), off in both the `release` and
`dev` profiles:

| Source | Flag | Restriction | Review |
|---|---|---|---|
| IMF | `crawl_imf` | Bulk download by automated technology needs explicit permission; commercial reuse needs permission | [ECO-175](https://linear.app/econgraph/issue/ECO-175) |
| BOJ | `crawl_boj` | Reproduction for commercial purposes needs the Bank's prior permission | [ECO-743](https://linear.app/econgraph/issue/ECO-743) |
| SNB | `crawl_snb` | Use is permitted for non-commercial purposes only | [ECO-742](https://linear.app/econgraph/issue/ECO-742) |
| UN_STATS | `crawl_un_stats` | The SDG Global Database states no terms; the generic UN terms allow personal, non-commercial use only | [ECO-182](https://linear.app/econgraph/issue/ECO-182) |

The verdicts come from the
[data source terms of use](https://linear.app/econgraph/document/data-source-terms-of-use-42433ce643db)
register. Sources that aren't listed are not gated. While a source's flag is off:

- `crawler-worker` never claims its jobs. Jobs already queued stay `pending`, untouched: they
  aren't failed, retried or deleted. The worker logs why once per process, and `crawler status`
  marks the source `disabled: licence (<ticket>); not claimed`.
- The refresh scheduler neither refreshes nor re-discovers it.
- `crawler enqueue`, `discover`, `fetch`, `refetch` and `record-reference-seeds`, and GraphQL
  `triggerCrawl`, refuse it with the restriction and the ticket link.
- `crawler sources` shows `disabled: licence (<ticket>)` in its `CRAWL` column.

To crawl a gated source, first settle its licence question (written permission, or a revised
verdict in the register). Then set the flag's `defaultVariant` to `on` in
[`flags.flagd.json`](../../../config/flags/flags.flagd.json), or in `dev.flagd.json` for
development builds only, and run `node scripts/flags-merge`
(see [feature flags](../../../config/flags/README.md)). The flag is compiled in, so the
change takes effect on the next build.

## Development-only static catalogs

Ten hardcoded catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO) register behind
the `static_catalogs` build flag; see [feature flags](../../../config/flags/README.md).
They are on in the `dev` flag profile used by local Cargo builds and tests, and off in the
`release` flag profile used by the Dockerfile and deployments. Cargo's `--release` option
does not select the flag profile. To include static catalogs explicitly, use `FLAGS_PROFILE=dev cargo build --release -p
econ-graph-crawler-worker`, or `--build-arg FLAGS_PROFILE=dev` to `backend/Dockerfile`. Their
discovery writes a fixed series list with no HTTP and they can't fetch data. The BOJ, SNB and
UN_STATS catalogs are licence-gated like any other adapter for those sources, so the queue
never runs their discovery while the [licence gate](#licence-gate) flags are off.

## Tests

```bash
cargo test -p econ-graph-crawler --all-features -- --test-threads=1
```

Adapter tests use a local mock upstream rather than provider APIs; see the
[testkit](src/testkit/mod.rs). DB-backed tests skip when `DATABASE_URL` is unset. To run them,
set `DATABASE_URL` to a disposable PostgreSQL test database: the test helper applies migrations,
and tests delete queue rows. Migration tests also create and drop separate scratch databases
on the same server, so the role in `DATABASE_URL` must be allowed to create databases
(for example, have `CREATEDB`). This test setting is separate from the runtime
`DATA_DATABASE_URL`. See [database testing](../../../docs/testing/DATABASE_TESTING.md) for setup.

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the [repository license](../../../LICENSE) for complete terms and conditions.
