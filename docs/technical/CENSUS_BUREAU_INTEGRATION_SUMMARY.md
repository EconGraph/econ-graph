# Census Bureau Integration - Developer Summary

> **Status (2026):** the Census Business Dynamics Statistics (BDS) integration lives in the Census
> adapter, `backend/crates/econ-graph-crawler/src/sources/census.rs` (ported from the removed
> `series_discovery::census` module; the `catalog_crawler` binary is gone too). It runs through the
> crawl queue: `crawler discover --source CENSUS` / `crawler enqueue --source CENSUS --series <id>`,
> drained by `crawler-worker` (see [CRAWLER_DEPLOYMENT_GUIDE.md](./CRAWLER_DEPLOYMENT_GUIDE.md)).
> The module docs in `census.rs` are the authoritative reference; this page is a summary.

## Overview

- **Data source**: U.S. Census Bureau Business Dynamics Statistics (BDS)
- **API root**: `https://api.census.gov/data` (`census::DEFAULT_BASE_URL`), dataset path `/timeseries/bds`
- **Authentication**: required. The Census Data API rejects requests without a key, so
  `CENSUS_API_KEY` (`ctx.keys.census`) is sent as `key=` on every request. Without it, discovery
  and fetching fail with `Auth` before any request. Keys are free at
  <https://api.census.gov/data/key_signup.html>.
- **Data type**: annual establishment, firm and job creation/destruction statistics (units "Count")
- **Rate policy**: `SourcePolicy::default_for(SourceId::Census)` in
  `backend/crates/econ-graph-crawler/src/policy.rs` (40 requests/min); enforced by the shared
  `HttpFetcher` / `SourceRateLimiter`, with queue-level retries and backoff.

## Components (`econ-graph-crawler/src/sources/census.rs`)

### `CensusAdapter` (implements `SourceAdapter`, `SourceId::Census`, stored as `CENSUS`)

**Discovery** (`discover`) makes two requests, both required:

1. `GET {base}/timeseries/bds/variables.json` -> `{"variables": {NAME: {"label": ..}, ..}}`
2. `GET {base}/timeseries/bds/geography.json` -> `{"fips": [{"name": .., "geoLevelDisplay": ..}, ..]}`

Variables are filtered by the keyword rules in `is_economic_variable` (establishments, firms, jobs,
employment, creation/destruction, ...) and crossed with the geography levels that give single
series:

- `us`: one national series per variable, `bds/national..{VARIABLE}` (e.g. `bds/national..ESTAB`);
- `state`: one series per state and DC, `bds/state.{FIPS}.{VARIABLE}` with the two-digit
  state FIPS code (e.g. `bds/state.06.ESTAB` for California), 51 per variable. The states
  come from the Census Bureau's state FIPS file
  (<https://www2.census.gov/geo/docs/reference/state.txt>), which the crawler downloads on every
  crawl; FIPS codes `01` to `56` are the states and DC, and the territories are dropped.

Finer levels (county, metro area) have thousands of areas and are skipped.

Every series belongs to the `bds` dataset, defined in
`backend/crates/econ-graph-crawler/data/datasets/census.toml`. The dataset is stored long: each
variable is its own series. Its dimensions, in id order, are `geo_level` (`national` or `state`),
`state` (the FIPS code, empty for national series) and `variable`. The external id is the
canonical dataset key `bds/{geo_level}.{state}.{variable}`, and each series row stores the same
values in its `dimensions`.

**Fetching** (`fetch_series`) is supported for both kinds of id. Any other id (including ids from
earlier versions, e.g. `CENSUS_BDS_*`) is a `Permanent` error, and the refresh scheduler never
enqueues it. The request is
`GET {base}/timeseries/bds?get={VARIABLE},YEAR&for={us:*|state:FIPS}&key=KEY` (all years; the
old comma-separated `YEAR=` list hit the API's "204 No Content" limitation for multi-year queries).
`since` is applied client-side by year. Each row becomes a point dated January 1 of its `YEAR`.
Rows of the wrong width or with an unparseable year are skipped; empty or non-numeric values are
missing observations.

### Error mapping (`classify_census_error` and the row parser)

| Upstream response | `CrawlError` |
|---|---|
| HTTP 400 with "unknown variable" | `NotFound` |
| HTTP 200 HTML page saying "Invalid Key" | `Auth` |
| Empty body (HTTP 204) | `NotFound` |
| Any other non-JSON body | `Parse` |
| 429 / 5xx / timeouts | handled by the shared `HttpFetcher` (`RateLimited` / `Transient`) |

## Database

The data source row ("U.S. Census Bureau") is visible and enabled. Its seed row
in the v4.0.0 baseline migration (`2026-10-02-000500_v4_0_baseline`) marks it as requiring `CENSUS_API_KEY`. Discovered
series are written as `EconomicSeries` rows keyed by the external IDs above when `crawler-worker`
runs the discovery job.

## Testing

Unit and contract tests are in the `tests` module of `census.rs`, against a wiremock upstream
serving the fixtures in `backend/crates/econ-graph-crawler/tests/fixtures/census/`
(`variables.json`, `geography.json`, `bds_estab_us.json`):

```bash
cd backend
cargo test -p econ-graph-crawler --all-features census -- --test-threads=1
```

## Usage

```bash
# Discover Census series (series metadata is written when crawler-worker runs the job)
crawler discover --source CENSUS
# Queue one series for the worker
crawler enqueue --source CENSUS --series bds/national..ESTAB
# Or fetch one series now, bypassing the queue
crawler fetch --source CENSUS --series bds/state.06.ESTAB
```

## Known API Limitations

1. **Multi-year queries**: explicit year lists may return 204 No Content; the adapter requests all
   years and filters locally.
2. **Geography**: a geography level returns one value per area per year, so the adapter makes one
   series per area: national and each state plus DC. County and metro-area levels (thousands of
   areas) are not discovered or fetched.
3. **Data availability**: some variables are not available for every geography level; values
   may be suppressed (stored as missing observations).
4. **Publication lag**: BDS data typically lags 1-2 years.

## Debugging Tips

1. `RUST_LOG=debug` shows the shared fetcher's per-request logs (API keys are redacted).
2. Verify raw responses with `curl` before changing the parser; update the fixtures to match.
3. A 400 "unknown variable" means the variable name in the external ID is wrong, not a transient error.
