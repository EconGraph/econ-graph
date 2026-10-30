# Observation contract

Train 3 extracts the current Postgres read behavior into `TimeSeriesStore`. Feature consumers
can use `ObservationRequest` and `ObservationPage` from `econ_graph_core::models` without
selecting a physical storage format. `PostgresStore::new(&data_pool)` implements the contract.
`series_service::get_series_data` remains the compatibility adapter for existing callers.
It translates legacy filters into explicit modes and converts the result back without changing
values, stored UUIDs, timestamps, pagination, or transformation inputs.

```rust,ignore
use econ_graph_services::observation_store::{
    ObservationRequest, PostgresStore, RevisionMode, TimeSeriesStore,
};
let page = PostgresStore::new(&pool).read_page(ObservationRequest {
    series_id,
    start_date: None,
    end_date: None,
    revisions: RevisionMode::Latest,
    limit: Some(100),
    offset: Some(0),
    context: None,
}).await?;
```

A read is one repeatable-read, read-only transaction. Count, page, dataset identity and context
are selected in that transaction. Dates are inclusive bounds. Ordering is observation date,
revision date, then stored observation UUID, ascending. A page defaults to 10,000 rows and is
clamped to 0–10,000; negative offsets normalize to zero. The total is computed after revision
selection and before pagination. Context uses the same filters and selection, and is absent
when the offset is zero or the page is empty. `PreviousPoint` returns one predecessor;
`Days(n)` returns all earlier rows within a nonnegative, representable duration window
(invalid windows return BadRequest); `FirstPoint` returns the earliest usable
non-null, non-zero predecessor. Dataset identity is absent for an unknown series.

The store does not grant access. API callers apply request-scoped catalog authorization before
reading it, including for counts and remote target checks. An absent dataset in the store is
not a public distinction between missing and inaccessible data.

## Revision selection and values

- `All` preserves every stored revision, including historical synthetic vintage rows.
- `Latest` picks the greatest revision date per observation date. At the same vintage, a
  non-original row wins; any remaining tie goes to the greatest UUID.
- `AsOf(day)` applies the same selection among revisions published on or before `day`.
- `Original` selects rows marked original. Existing legacy combinations remain expressible
  as `OriginalLatest` and `OriginalAsOf(day)`; `asOf` takes precedence over the latest flag.

Every selection other than `All` excludes a synthetic original whose revision date equals its
observation date when another original at a different vintage exists for that series/date.
A genuine same-day original with no competing original stays. `Original` deliberately preserves
the existing behavior when multiple originals remain rather than inventing a new selection rule.
The shared SQL predicates remain in `models::data_point` for the optimized compatibility paths.

Untransformed public reads default to `All`. The existing GraphQL boundary chooses `Latest`
for transformations only when the caller did not explicitly choose a mode. Nulls stay null;
no superseded non-null revision replaces a current null.

An observation value is `Option<BigDecimal>`, with no Decimal128 or binary floating point
restriction. The transport serializes it as an exact decimal string or JSON null. PostgreSQL's
unconstrained `NUMERIC` is the current physical representation; a future adapter must preserve
the same precision. Observation IDs and series IDs are the stored identities, including legacy
IDs, rather than hashes recomputed from a date/value. The page carries its dataset identity.
The additive `attributes` map is empty in the current Postgres adapter; no attribute storage
migration is introduced. Legacy DataPoint conversion intentionally has no attributes field.

## Current read and write inventory

The following are explicit Postgres compatibility adapters. They remain optimized or preserve
an existing uncapped API shape; they are not a promise that swapping one page adapter changes
every storage read. A later backend must implement these operations before cutover.

| Consumer | Route and semantics | Regression coverage |
| --- | --- | --- |
| GraphQL `seriesData`; MCP `get_series_data` | `series_service::get_series_data` → `PostgresStore`; page/count/context together | `observation_contract`, GraphQL `series_data_tests` |
| GraphQL series `dataPoints` | Direct uncapped Postgres read in `types.rs`; explicit modes, transformations default latest | GraphQL `series_data_tests` revision/default regressions and API contracts |
| GraphQL series `recentDataPoints` | Direct descending latest-revision read, existing caller limit | GraphQL API contract suite |
| GraphQL series `dataPointCount` | Direct count of all stored revisions, deliberately independent of latest summaries | GraphQL API contract suite |
| `DataPointsBySeriesBatcher` | Batched latest revision reads, descending dates; uses shared revision predicate | `test_data_points_by_series_loader_with_real_data` |
| `DataPointCountBatcher` | Batched counts of all stored rows, zero for missing | `test_data_point_count_loader_with_real_data` |
| `LatestObservationBatcher` | Lateral join picks latest date then latest revision; current null stays null | GraphQL latest-observation regressions |
| `cross_section_service::cross_section` | Caller-owned connection, dataset dimension selection and latest/as-of lateral read | `cross_section_service::tests` including same-vintage ties and synthetic rows |
| Core `DataPoint::{find_all, find_by_series_and_date_range, get_by_series_and_date}` | Legacy raw ORM helpers with no current production consumer; no authorization or revision contract | Existing model tests; excluded from feature-consumer API |
| Core `DataPoint::{create, create_batch}` | Legacy fixture helpers; not the crawler commit unit | Backend integration and GraphQL fixtures |
| crawler `persist_series_conn` / worker commit | Writes observations, attempt, and queue completion in the worker's transaction under its lease lock | Existing crawler persistence/lease-fencing regressions |

No pool-based observation write operation is exposed by `TimeSeriesStore`. The transaction-bound
write unit remains the existing worker transaction, including lease locking, attempt persistence,
observation writes and queue completion. A future write adapter must borrow that transaction or
provide an equivalent fenced atomic unit; opening an independent pool transaction would violate
the contract. No crawler write is rewired by this extraction.

## Executable adapter checks

`econ-graph-services/tests/observation_contract.rs` runs the same consumer assertions against a
small independent in-memory adapter and the working Postgres adapter. It covers exact and null
values, synthetic vintages, all explicit selections, same-vintage revision ties, stable UUIDs,
dataset identity, inclusive bounds, page/count/context and unknown series. The Postgres case also
runs a concurrent writer while checking that count, page and context describe one population.
Serialization and legacy-filter precedence have independent tests.

```sh
cargo test --manifest-path backend/Cargo.toml -p econ-graph-services --test observation_contract
# Against an explicitly disposable, already migrated data database:
DATA_DATABASE_URL=postgres://... cargo test --manifest-path backend/Cargo.toml \
  -p econ-graph-services --test observation_contract -- --include-ignored
```

The explicit Postgres test fails if its URL is missing; ordinary unit runs report it ignored.
The split-plane compatibility runner includes it. Fixtures use their own source and series,
derive observation UUIDs within that series namespace, and remove their source after success.
No provider HTTP access or production writes are needed.
