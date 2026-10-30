# Data access contract, version 1

Train 3 freezes the current data API so company research, workspace and storage
workstreams can use the same operations while Postgres remains the implementation.
The checked-in [consumer operations](operations/train3-data.graphql) are executed by
`backend/crates/econ-graph-graphql/tests/data_api_contract.rs`, against both separate
app/data databases and a combined database. They use explicit request authorization
contexts against the production library, including its error sanitization.

## Transport and observation selection

`schema.graphql` in `backend/crates/econ-graph-graphql` is the complete public schema.
The operations file is a small consumer contract, rather than a replacement schema.
An observation response contains UUID `id` and `seriesId`, ISO observation and
revision dates, `isOriginalRelease`, and a nullable **decimal string** `value`.
Consumers must retain decimal strings or parse arbitrary-precision decimals. A null
value means an observation was published without a number; it is distinct from an
empty page or a missing series. There is no Float64 or Decimal128 narrowing rule.
UUIDs come from stored rows, including legacy non-derived IDs.

The typed request consists of `seriesId: ID!`, optional `DataFilter`, optional
`DataTransformation`, `first: Int`, and `after: String`. The executable Rust consumer
example deserializes `seriesData` into `ObservationPage`, with `Vec<Observation>` and
`PageInfo`; its `Observation.value` is `Option<String>`. For example:

```json
{
  "seriesId": "8933cde1-0db6-454c-8675-805b4386f301",
  "filter": { "asOf": "2024-02-15" },
  "first": 1
}
```

The corresponding first point is the January revision, with `value: "12.5"`,
`revisionDate: "2024-02-15"`, and `isOriginalRelease: false`; `totalCount` is 2.
The second point has `value: null`. The fixture also exercises a decimal with more
than 38 significant digits, without serialization loss.

| Requested selection | Existing GraphQL filter                 | Store meaning                                   |
| ------------------- | --------------------------------------- | ----------------------------------------------- |
| All                 | Omitted, or `latestRevisionOnly: false` | Every stored revision                           |
| Latest              | `latestRevisionOnly: true`              | Newest revision of each observation date        |
| As of               | `asOf: "YYYY-MM-DD"`                    | Newest revision published on or before that day |
| Original            | `originalOnly: true`                    | Only rows marked original release               |

`asOf` takes precedence over `latestRevisionOnly`. `originalOnly` is an additional
restriction, so combinations retain their existing compatibility semantics. Store
revision selection also preserves synthetic-vintage handling. An untransformed
`seriesData` defaults to All. A transformation defaults to Latest unless the caller
explicitly chooses a revision mode. Pagination count, page and transformation
predecessors share one repeatable-read snapshot through the Postgres store. A
continued transformed page must match the corresponding unpaged values.

Bounds are inclusive. `first` defaults to and is capped at 10000; zero returns an
empty page with the matching total. Negative page sizes and malformed cursors are
errors. Cursors are nonnegative offsets encoded as strings. An empty page retains
its offset in `endCursor`; an out-of-range page must not reset the cursor. The
connection count counts the selected revision mode before pagination. In contrast,
`series.dataPointCount` counts all stored revisions. `latestObservation` and
`recentDataPoints` use latest revisions; `crossSection(latest: true)` deliberately
selects each key's latest non-null value. Those summaries have different meanings.

## Read-path and authorization inventory

| Surface                                          | Current implementation and compatibility coverage                                                                                                               |
| ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Series UUID / source-external-ID lookup          | `query.rs`; `catalog_authorization` compares missing and denied results                                                                                         |
| Dataset lookup / list / labelled dimensions      | `query.rs`, `datasets.rs`; consumer operations and catalog regressions                                                                                          |
| Observation page, count and transforms           | `series_service` compatibility entry and `observation_store`; consumer operations exercise defaults, explicit filters and continuation context                  |
| Series `dataPoints`                              | Direct uncapped Postgres compatibility adapter in `types.rs`; consumer operations check exact/null values, All/Latest/Original/AsOf and transformation defaults |
| Latest summary, recent points, raw count         | `types.rs` and `dataloaders.rs` compatibility adapters; consumer operations preserve IDs, units and counts                                                      |
| Cross-section                                    | `cross_section.rs` and service adapter; consumer operations cover the current revision and catalog regressions cover denial                                     |
| Discovery, search, source series/count           | Catalog-scoped SQL before count/offset/limit; catalog regressions and reused-schema consumer test                                                               |
| Annotation target, annotation and comment access | `catalogs::series_visible`, `SeriesCheck::readable_series_exists`; catalog regressions exercise local and remote validation                                     |

The effective visibility rule is the intersection of the data plane's grants and
the current caller's catalog grants. Signed-in callers use their own roles; they
do not inherit anonymous grants. Unknown catalog names deny. Allowed source IDs
are cached only in that request's `GraphQLContext`. Create a fresh context for
every request; do not cache an authorized response or a context in a shared schema.
The executable reused-schema test alternates allowed, denied and allowed callers.

Future per-series licence policy composes at this same read boundary as
`catalog_visible && licence_visible`. Its storage predicate must restrict rows
**before** discovery pagination, counts and summaries, and its target predicate
must also restrict `SeriesCheck`/annotation access. Neither policy can grant access
denied by the other. Shared caches must key every relevant authorization input or
remain request-scoped. No licence class migration or new grant behavior is part of
this version; adding one requires extending this common compatibility suite before
downstream consumers use it.

## Empty, missing, denied and unavailable results

| Situation                                             | Caller-visible result                                                                                   |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Observation with no numeric value                     | Point exists with `value: null`                                                                         |
| Valid filter with no observations                     | Empty `nodes`, matching count zero                                                                      |
| Missing or unreadable series UUID                     | `series: null`, `seriesExists: false`, `seriesCatalog: null`; indistinguishable empty observation pages |
| Discovered series with no observations                | `series: null`, but readable `seriesExists: true`                                                       |
| Missing or unreadable dataset                         | `dataset: null`; cross-section uses the same not-found error for both                                   |
| Explicit source access without its grant              | `FORBIDDEN`, with no internal identifiers                                                               |
| Data field on app-only schema                         | `DATA_SERVED_REMOTELY`; same schema shape, no local-data fallback                                       |
| Remote annotation target lookup unavailable/malformed | `DATA_PLANE_UNAVAILABLE`; write fails closed and persists nothing                                       |
| Missing or denied annotation target                   | Same not-found result; no existence oracle                                                              |

The remote `SeriesCheck` operation is `seriesCatalog(id)` and then caller-grant
intersection; an HTTP success with GraphQL errors, a malformed response or a
transport failure is an unavailable result. A remote URL does not cause local data
fields to become a GraphQL proxy. Existing trust configuration alone controls
whether a verified caller token can be forwarded to that endpoint.

## Evolution and required checks

Consumers can request subsets of these operations. Add optional fields and new
operations additively; do not change existing nullability, scalar encoding,
revision defaults, privacy behavior, cursor meaning or authorization scope behind
the same contract. A future storage adapter implements the shared store contract;
a future gateway preserves these public operations and errors. Neither change
requires company or workspace callers to know physical table names or data URLs.
Intentional breaking changes require a new contract version with migration
examples and tests for both supported versions.

Run these from `backend` with `DATABASE_URL` on a disposable Postgres 18 server
whose test role can create scratch databases:

```sh
cargo test -p econ-graph-graphql --test schema_snapshot
cargo test -p econ-graph-graphql --test data_api_contract -- --test-threads=1
cargo test -p econ-graph-graphql --test catalog_authorization -- --test-threads=1
```

The fixtures create no provider requests. The unavailable-target test permits only
its in-process mock endpoint and checks that no annotation was stored. Scratch
databases are scoped to each fixture and dropped afterward. These checks prove
the current contract; they do not claim downstream Iceberg, gateway, licence or
OAuth implementations are complete.
