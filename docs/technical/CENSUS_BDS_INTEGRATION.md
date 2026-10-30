# Census Bureau BDS Integration

The Census adapter retrieves annual Business Dynamics Statistics (BDS) on establishments,
firms, employment, and job flows from the U.S. Census Bureau Data API.

Use `crawler discover --source CENSUS` to enqueue catalog discovery and
`crawler enqueue --source CENSUS --series <id>` to enqueue a series fetch.
`crawler-worker` processes both kinds of job. See the [Crawler Deployment Guide](./CRAWLER_DEPLOYMENT_GUIDE.md)
for database setup, worker configuration, and queue operations.

## API and Source Configuration

The seeded source is **U.S. Census Bureau**, identified by `CENSUS` in crawler commands.
Its API root is `https://api.census.gov/data`; the adapter appends `/timeseries/bds`
for BDS requests. The source is enabled and visible, and requires `CENSUS_API_KEY`.

The adapter uses these endpoints:

| Endpoint under the API root | Purpose |
|---|---|
| `/timeseries/bds/variables.json` | Variable names and labels for discovery |
| `/timeseries/bds/geography.json` | Geographic levels available for discovery |
| `/timeseries/bds?get={VARIABLE},YEAR&for=us:*&key={KEY}` | All years of one national variable |
| `/timeseries/bds?get={VARIABLE},YEAR&for=state:{FIPS}&key={KEY}` | All years of one state variable |

The key is also sent on the two metadata requests. State names and FIPS codes come from
[the Census state reference file](https://www2.census.gov/geo/docs/reference/state.txt).

`CENSUS_API_KEY` is required by the adapter: without it, discovery and series fetching
fail with an auth error before making any request. The BDS data endpoint rejects keyless
requests, although [public variable metadata](https://api.census.gov/data/timeseries/bds/variables/ESTAB.json)
is available without a key.
Obtain a free key through [Census API key signup](https://api.census.gov/data/key_signup.html)
and configure it in the worker's environment.

## Discovery, Datasets, and Series IDs

[`CensusAdapter`](../../backend/crates/econ-graph-crawler/src/sources/census.rs) implements
`SourceAdapter`. Its `discover(&ctx)` method reads the variable and geography metadata,
selects the measures, and crosses them with
the supported geographic levels. When the metadata lists `us`, discovery produces one
national series per selected variable. When it lists `state`, discovery reads the state
reference file and produces one series per variable for each of the 50 states and DC.
County, metropolitan-area, and other geographic levels are skipped. A measure is a
variable with a numeric `predicateType` (`int` or `float`) whose name or label matches an
economic keyword. Classification codes such as `EAGE` and `EMPSZES` are `string`
variables and never become series; see the
[Census BDS data-source page](../data-sources/census-bds.md#what-we-fetch-today).

Discovery generates canonical external IDs and returns `DiscoveredSeries` metadata; the
worker records that catalog in `series_metadata`. A later fetch supplies observations,
and the worker creates or updates the corresponding `economic_series` row and stores its
points. Because Census fetches carry no series metadata, the worker uses the discovered
metadata for titles, units, and frequency. Discovery alone does not fetch observations.

Every series belongs to the `bds` dataset, defined in
[`census.toml`](../../backend/crates/econ-graph-crawler/data/datasets/census.toml).
The ID encodes its dimensions in the order `geo_level`, `state`, and `variable`:
`bds/{geo_level}.{state}.{variable}`. National series have an empty `state` dimension,
so `bds/national..ESTAB` identifies national establishment counts. State and DC series
use a two-digit state FIPS code; `bds/state.06.ESTAB` identifies California's establishment
counts. The worker refreshes state labels in the dataset's `state` dimension from the
reference file at startup and before scheduled discovery.

### Economic Variables

Discovery reads the available variables rather than using a fixed indicator list.
Examples in the [Census BDS variable catalog](https://api.census.gov/data/timeseries/bds/variables.html)
include:

| Variable | Measure |
|---|---|
| `ESTAB` | Establishments |
| `FIRM` | Firms |
| `EMP` | Employees |
| `JOB_CREATION` | Jobs created |
| `JOB_DESTRUCTION` | Jobs destroyed |
| `ESTABS_ENTRY` | Establishment births |
| `ESTABS_EXIT` | Establishment exits |

### Time Coverage

BDS publishes annual observations beginning in 1978; consult the
[BDS data releases](https://www.census.gov/programs-surveys/bds.html) for publication coverage.
The adapter does not impose an end year. Each fetch requests all years returned by Census,
without a `YEAR` filter. If the caller supplies `since`, the adapter retains observations
from that year onward locally. This request shape avoids the comma-separated `YEAR` query
that the earlier implementation found could return HTTP 204.

## Usage Examples

With the CLI installed and the data database configured as described in the deployment guide:

```bash
# Enqueue discovery; the worker records the BDS catalog
crawler discover --source CENSUS

# Enqueue observation fetches; the worker processes each series
crawler enqueue --source CENSUS --series bds/national..ESTAB
crawler enqueue --source CENSUS --series bds/state.06.ESTAB   # California
```

The adapter accepts national and per-state IDs in the canonical format above. Unsupported
geographic levels, malformed IDs, and older `CENSUS_BDS_*` IDs fail as `Permanent` without
an API request. A state ID must contain a two-digit FIPS code between `01` and `56`;
Census rejects unused codes within that range.

## Fetching and Response Parsing

The worker calls `fetch_series(&ctx, external_id, since)` through the adapter registry.
The result is a [`FetchedSeries`](../../backend/crates/econ-graph-crawler/src/adapter.rs)
containing dataset dimensions and `FetchedPoint` observations. Census fetches return no
additional series metadata or cache validators.

Code that already has an initialized `CrawlCtx` can obtain the adapter through the default
registry. These calls return results to the caller; the worker handles persistence for
queued jobs:

```rust
use econ_graph_crawler::source::SourceId;

let registry = econ_graph_crawler::sources::default_registry();
let census = registry.get(SourceId::Census).expect("census adapter");
let discovered = census.discover(&ctx).await?;
let fetched = census.fetch_series(&ctx, "bds/national..ESTAB", None).await?;
```

The Census API returns a JSON array with a header row followed by data rows. For example,
this abbreviated response illustrates the duplicated `YEAR` header the parser supports:

```json
[
  ["ESTAB", "YEAR", "YEAR", "us"],
  ["7206748", "2020", "2020", "1"]
]
```

The parser locates the requested variable and the first `YEAR` column by name. It dates
each observation January 1 of its year, sets `revision_date` to that date, and marks it
as an original release. Rows with the wrong number of columns or an invalid year are
skipped; duplicate years keep the first row.

Values use the crawler's decimal observation type. Nulls, empty cells, and suppression
markers without digits become missing observations. A cell containing digits must parse
as a valid observation value; malformed or out-of-range numeric text fails the series
with `Parse`. See [observation precision](./OBSERVATION_PRECISION_REFETCH.md) for the value limits.

## Request Limits and Errors

The shared HTTP fetcher limits Census traffic to 40 requests per minute, with burst 1 and
at most two requests in flight per worker process. These are crawler defaults, not a
statement of Census's published quota. Each series fetch requests one variable and one
geography for all years, even when `since` later removes older points.

The fetcher already makes up to two extra attempts for rate-limit and transient errors,
using exponential backoff with jitter and honoring `Retry-After`. If those attempts fail,
the worker applies the queue retry policy. See [Crawler Politeness](./CRAWLER_POLITENESS.md)
for limits and tuning, and [Retries](./CRAWLER_DEPLOYMENT_GUIDE.md#retries) for queue transitions.

The adapter reports failures using `CrawlError`:

| Error | Census condition | Queue behavior |
|---|---|---|
| `Auth` | Missing key, HTTP 401/403, or an HTML “Invalid Key” page | Fail without retry |
| `NotFound` | HTTP 404/410, HTTP 400 saying “unknown variable,” or an empty response such as HTTP 204 | Fail without retry |
| `Permanent` | Unsupported ID or other HTTP 4xx response | Fail without retry |
| `Parse` | Unexpected response structure or invalid numeric value | Fail without retry |
| `RateLimited` | HTTP 429, or HTTP 503 with `Retry-After` | Reschedule without counting a failed attempt |
| `Transient` | Timeout, connection failure, HTTP 408, or other HTTP 5xx | Retry with backoff up to `max_retries` |

## Testing

Tests in `sources/census.rs` use a mocked Census API alongside the shared adapter contract
tests. They cover discovery from both metadata files, national and state IDs, key handling,
local `since` filtering, error classification, and row parsing. These fixtures verify the
adapter's behavior; they do not establish current live API availability or publication coverage.

Run from `backend/`:

```bash
# Run the Census adapter tests against the mock server
cargo test -p econ-graph-crawler --all-features sources::census

# Run a specific request and parsing test
cargo test -p econ-graph-crawler --all-features sources::census::tests::fetch_parses_rows_and_sends_key
```

## Deployment and Troubleshooting

Apply data plane migrations with `crawler migrate`. The
[`2026-10-21-000100_v4_0_baseline` migration](../../backend/migrations/data/2026-10-21-000100_v4_0_baseline/up.sql)
seeds the Census source with the API root and `CENSUS_API_KEY` requirement.

Use `crawler status`, worker logs, and crawler metrics to distinguish failed jobs from
jobs waiting for rate-limit permits or retries. See [crawler monitoring](./CRAWLER_DEPLOYMENT_GUIDE.md#monitoring).

- For `Auth`, check that the worker has a valid `CENSUS_API_KEY`.
- For `NotFound` or HTTP 204, check the variable in the Census catalog and confirm it is
  available for the requested geography. The queue commands do not select a year range;
  the adapter already requests all available years.
- For `Parse`, inspect the error for missing columns, invalid values, or an unexpected
  response format, and check Census service announcements.
- For slow responses, inspect timeout and retry logs before changing the existing
  [request policy](./CRAWLER_POLITENESS.md#tuning).

For a local worker, enable debug logging when launching it from `backend/`:

```bash
RUST_LOG=debug cargo run -p econ-graph-crawler-worker --bin crawler-worker
```

For a deployed worker, configure `RUST_LOG` in its environment; see
[worker configuration](./CRAWLER_DEPLOYMENT_GUIDE.md#run-the-worker).

## References

- [Census Data API Documentation](https://www.census.gov/data/developers/data-sets.html)
- [BDS Dataset Information](https://www.census.gov/programs-surveys/bds.html)
- [Census API User Guide](https://www.census.gov/data/developers/guidance/api-user-guide.html)
