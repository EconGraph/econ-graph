# Selected observation backend for compatibility reads

The public observation page is not the only observation consumer. Direct series
fields, batched summaries and cross-sections also use the durable storage
selection. `services::observation_compatibility` implements the retained
compatibility shapes against PostgreSQL and real Iceberg/DataFusion reads.
Postgres shadow rows are never a fallback for a selected Iceberg read.

| Surface                             | PostgreSQL path                                       | Iceberg path                                                                              |
| ----------------------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Series `dataPoints`                 | Filtered ordered query                                | Bounded immutable series scan and exact revision selection                                |
| `recentDataPoints` / batched points | Batched latest-revision SQL                           | Same legacy latest-revision selection over decoded exact observations                     |
| Direct / batched `dataPointCount`   | One grouped count, all revisions                      | Count logical observation IDs in one pinned snapshot, all revisions                       |
| Batched `latestObservation`         | Indexed lateral `LIMIT 1` per series in one statement | Latest date/vintage, with non-original then UUID tie-break; current null remains null     |
| `crossSection`                      | Existing batched lateral current-vintage query        | Matching SQL series and table pointer from the same snapshot, then bounded physical reads |

Every compatibility call checks `ObservationStorage` against the durable recovery
target **inside** its repeatable-read snapshot. Iceberg series/dataset identities
and metadata pointers are read in that same snapshot. Object reads use the pinned
immutable metadata pointer and open no nested SQL pool connection. Cross-sections
can therefore run on a pool with only one connection. An existing caller-owned
transaction must already use repeatable-read or serializable isolation; a
read-committed caller receives an explicit error rather than a mixed snapshot.
PostgreSQL retains its optimized batching instead of reading every observation to
calculate a count or latest summary. Its cross-section adds constant snapshot and
selection-check overhead, not a query per key.

Observation loaders batch within a request but do not cache observations or
counts for a process-lifetime schema. Reusing a loader after a backend transition
must revalidate selection and fail for stale process configuration. Catalog
authorization remains at the existing API boundary before these operations;
denied series remain indistinguishable from missing series.

The direct `dataPoints` shape has no public pagination. It preserves All as the
untransformed default and Latest as the transformation default, inclusive date
bounds, explicit Original/AsOf combinations, exact decimal strings, nullable
values, stored IDs and attributes. It does not silently return a page when a
query is too large. Compatibility vectors are bounded to 100000 observations
and 2000 requested series. The bound is applied in PostgreSQL or DataFusion
**before** result collection, with one extra row used to detect overflow. An
exceeded budget returns an availability error so callers can narrow the query.
Iceberg's configured row and memory budgets also apply; production capacity and
latency approval remain separate cutover gates.

Revision selection preserves each existing surface's specific meaning. Explicit
series revision modes exclude superseded synthetic originals, checking all
original vintages even when the real original is newer than an AsOf cutoff.
Legacy recent/latest summaries retain their existing synthetic-row treatment.
`latestObservation` can return a null value at the newest date; `crossSection`
with `latest: true` instead returns the newest **non-null current-vintage** date
for each key. A null revision never resurrects an older revision of that date.

`selected_compatibility` runs the production GraphQL schema with a caller obtained
through real signed-token verification, separate app/data databases, real Iceberg
files and DataFusion. Each backend case deliberately changes the unselected
store so a bypass cannot pass by reading identical shadow rows. It covers direct
fields, batched loaders, exact/null values, revision and synthetic semantics,
attributes, catalog denial, scan overflow, stale loader refusal and a one-pool-
connection cross-section pinned across a concurrent catalog publication.

```sh
DATABASE_URL=postgres://... cargo test --manifest-path backend/Cargo.toml \
  -p econ-graph-graphql --test selected_compatibility -- --test-threads=1
```

The role needs permission to create disposable scratch databases. Child test
processes isolate backend/warehouse settings; the test never changes the parent
process's environment or uses a live provider. Local-file correctness does not
claim object-store deployment, production-scale latency or cutover acceptance.
