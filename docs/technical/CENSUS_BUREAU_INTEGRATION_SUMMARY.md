# Census Bureau Integration - Developer Summary

The `CensusAdapter` integrates U.S. Census Bureau Business Dynamics Statistics (BDS) with the
crawl queue. Use `crawler discover --source CENSUS` to enqueue catalog discovery, or
`crawler enqueue --source CENSUS --series <id>` to enqueue a series fetch. `crawler-worker`
processes these jobs; see the [Crawler Deployment Guide](./CRAWLER_DEPLOYMENT_GUIDE.md).
The [Census adapter source](../../backend/crates/econ-graph-crawler/src/sources/census.rs) is the
authoritative implementation reference; this page summarizes its behavior.

## Overview

- **Data source**: U.S. Census Bureau Business Dynamics Statistics (BDS)
- **API root**: `https://api.census.gov/data` (`census::DEFAULT_BASE_URL`), dataset path `/timeseries/bds`
- **Authentication**: the adapter requires `CENSUS_API_KEY` (`ctx.keys.census`). Without it,
  discovery and fetching fail with `Auth` before making requests. The adapter sends `key=` on
  BDS metadata and observation requests; the public state reference file is requested without
  a key. This adapter requirement also applies to metadata that Census makes publicly
  accessible without a key. Keys are free at <https://api.census.gov/data/key_signup.html>.
- **Data type**: annual establishment, firm and job creation/destruction statistics, including
  counts and rates. The adapter assigns `Annual` frequency and `Count` units to discovered metadata.
- **Rate policy**: `SourcePolicy::default_for(SourceId::Census)` in
  [policy.rs](../../backend/crates/econ-graph-crawler/src/policy.rs) sets 40 requests/min,
  enforced by the shared `HttpFetcher` and `SourceRateLimiter`, with queue-level retries and backoff.

## Adapter behavior

### `CensusAdapter` (implements `SourceAdapter`, `SourceId::Census`, stored as `CENSUS`)

**Discovery** (`discover`) requires both BDS metadata files:

1. `GET {base}/timeseries/bds/variables.json?key=KEY` -> `{"variables": {NAME: {"label": ..}, ..}}`
2. `GET {base}/timeseries/bds/geography.json?key=KEY` -> `{"fips": [{"name": .., "geoLevelDisplay": ..}, ..]}`

The adapter filters variables with the keyword rules in `is_economic_variable` (establishments,
firms, jobs, employment, creation/destruction, ...) and generates series for the supported
geography levels present in `geography.json`:

- `us`: one national series per variable, `bds/national..{VARIABLE}` (e.g. `bds/national..ESTAB`);
- `state`: one series per state and DC, `bds/state.{FIPS}.{VARIABLE}` with the two-digit
  state FIPS code (e.g. `bds/state.06.ESTAB` for California), 51 per variable.

When `geography.json` lists `state`, discovery also downloads the Census Bureau's
[state FIPS file](https://www2.census.gov/geo/docs/reference/state.txt) to obtain state codes and
names. It keeps codes `01` to `56` and drops territories; the file must contain exactly the 50
states and DC. Discovery reads this file directly rather than relying on stored reference data.
The separate reference-data refresh uses conditional GETs with cached validators to update the
`bds` dataset's state labels at worker startup and before scheduled discovery. Observation
fetching does not need the state file. See the
[reference-file implementation](../../backend/crates/econ-graph-crawler/src/reference_file.rs).

Finer levels (county, metro area) have thousands of areas and are skipped.

Every series belongs to the `bds` dataset, defined in
[census.toml](../../backend/crates/econ-graph-crawler/data/datasets/census.toml). The dataset is
stored long: each variable is its own series. Its dimensions, in id order, are `geo_level`
(`national` or `state`), `state` (the FIPS code, empty for national series) and `variable`.
The external id is the
canonical dataset key `bds/{geo_level}.{state}.{variable}`. Discovery stores these values in
`series_metadata.dimensions`; fetching stores them in `economic_series.dimensions`.

**Fetching** (`fetch_series`) is supported for both kinds of id. Any other id (including ids from
earlier versions, e.g. `CENSUS_BDS_*`) is a `Permanent` error, and the refresh scheduler never
enqueues it. The request is
`GET {base}/timeseries/bds?get={VARIABLE},YEAR&for={us:*|state:FIPS}&key=KEY` (all years; the
old comma-separated `YEAR=` list hit the API's "204 No Content" limitation for multi-year queries).
`since` is applied client-side by year. Each row becomes a point dated January 1 of its `YEAR`.
Rows of the wrong width or with an unparseable year are skipped. Empty cells and suppression
markers without ASCII digits become missing observations. Cells containing digits must pass
`parse_observation_value`: malformed or out-of-bounds numeric text fails the fetch with `Parse`,
preventing an invalid value from overwriting a stored observation with NULL. See
[observation precision and limits](./OBSERVATION_PRECISION_REFETCH.md#how-observation-values-are-stored).

### Error mapping (`classify_census_error` and the row parser)

| Upstream response | `CrawlError` |
|---|---|
| HTTP 400 with "unknown variable" | `NotFound` |
| HTTP 200 HTML page saying "Invalid Key" | `Auth` |
| Empty observation body (including HTTP 204) | `NotFound` |
| Empty or otherwise invalid metadata JSON | `Parse` |
| Any other non-JSON body | `Parse` |
| 429 / 5xx / timeouts | handled by the shared `HttpFetcher` (`RateLimited` / `Transient`) |

## Database

The [v4.0.0 data plane baseline migration](../../backend/migrations/data/2026-10-21-000100_v4_0_baseline/up.sql)
seeds "U.S. Census Bureau" as visible and enabled, requiring `CENSUS_API_KEY`.

When `crawler-worker` runs a discovery job, it upserts catalog entries in `series_metadata`,
keyed by the source and the external IDs above, including dataset membership and dimensions.
A successful later fetch creates or updates `economic_series` and stores observations in
`data_points`. The fetched series takes its title, units and frequency from the catalog entry
because the BDS fetch response carries no metadata. See the
[persistence implementation](../../backend/crates/econ-graph-crawler/src/persist.rs).

## Testing

Unit and contract tests are in the `tests` module of the
[Census adapter](../../backend/crates/econ-graph-crawler/src/sources/census.rs), against a wiremock
upstream serving the [Census fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/census/)
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
# Or fetch and persist one series now, bypassing the queue
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
