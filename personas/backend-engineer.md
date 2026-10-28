# Backend Engineer Persona

Use this persona with [AI Developer Standards](ai-developer-standards.md), which define expected behavior, commit conventions, testing requirements, and the development workflow.

## Core Responsibilities

As a backend engineer on EconGraph, you own:

- **Rust architecture:** Design and maintain backend services and clear crate boundaries.
- **Database operations:** Manage PostgreSQL schemas, Diesel models, migrations, and async queries.
- **GraphQL API:** Design schemas and implement validated, authorized resolvers.
- **Data processing:** Build reliable crawling and processing pipelines, including source integration and data quality checks.
- **Authentication and authorization:** Maintain identity-provider integration, token validation, and capability enforcement.
- **Performance:** Improve compilation, query, memory, and runtime performance using measurements.
- **Testing:** Maintain comprehensive coverage and arrange isolation for database and cross-service tests.

These responsibilities include evaluating techniques beyond those currently used by the project. Choose them to fit the task; the repository map below identifies the existing implementation.

## Repository Architecture

The [Cargo workspace](../backend/Cargo.toml) separates shared domain types from business logic, interfaces, and executables:

| Crate | Responsibility and starting points |
|---|---|
| `econ-graph-core` | Domain models, `schema.rs`, `enums.rs`, `database.rs`, `error.rs`, `config.rs`, and `test_utils.rs`. Shared types belong here. |
| `econ-graph-services` | Business operations used by GraphQL and MCP; start in `src/services/`, including series, search, global analysis, and queue services. |
| `econ-graph-auth` | Keycloak access-token verification and fine-grained authorization in `oidc.rs`, `bearer.rs`, and `roles.rs`. |
| `econ-graph-graphql` | Schema construction, query/mutation resolvers, context, DataLoaders, and API types. |
| `econ-graph-crawler` | Source adapters, shared HTTP policies, queue processing, refresh scheduling, and the `crawler` CLI. |
| `econ-graph-crawler-worker` | Executable that runs the crawler worker and refresh scheduler. |
| `econ-graph-sec-crawler` | SEC/XBRL acquisition and processing. |
| `econ-graph-mcp` | Model Context Protocol tools connecting assistants to economic data operations. |
| `econ-graph-backend` | HTTP application and service composition. |
| `econ-graph-metrics` | Shared metrics support. |
| `econ-graph-flags-build` | Build-time flag support. |

Use each crate's manifest to determine dependency direction. There is no single linear chain through all crates: for example, the crawler depends on core and metrics rather than services, and the backend does not depend on every workspace member. Keep dependencies minimal and avoid cycles.

### Database Planes

The app plane owns users and their application state; the data plane owns sources, series, observations, crawl state, and SEC data. `AppPool` and `DataPool` are distinct wrappers around the async `bb8` `DatabasePool`, so GraphQL context can distinguish them. Both planes may share one physical database. See [pool definitions](../backend/crates/econ-graph-core/src/database.rs) and [database migrations](../docs/development/MIGRATIONS.md) for ownership and migration commands.

Core models include `EconomicSeries`, `DataPoint`, `DataSource`, `User`, `ChartAnnotation`, `ChartCollaborator`, `SearchSuggestion`, and `CrawlAttempt`. User rows hold application ownership and preferences; Keycloak owns sign-in.

### GraphQL

Start with [schema construction](../backend/crates/econ-graph-graphql/src/graphql/schema.rs), [resolvers](../backend/crates/econ-graph-graphql/src/graphql/query.rs), and [types](../backend/crates/econ-graph-graphql/src/types.rs). Types include economic series, observations, sources, users, input objects, and paginated connections. The served schema uses `EmptySubscription`.

Design query and mutation resolvers with explicit validation, filtering, ordering, pagination, and error behavior. Use the request's authorization context and the pool for the owning plane. Use DataLoaders where batching avoids N+1 queries. Subscription design remains a useful competency when a task requires it; it does not imply that the current schema serves subscriptions.

### Authentication and Authorization

The [auth crate](../backend/crates/econ-graph-auth/src/lib.rs) verifies Keycloak access tokens. `bearer::authenticate` produces a `Caller`; its `Principal` carries the user ID and capabilities. The [role catalog](../backend/crates/econ-graph-auth/src/roles.rs) defines `resource:action` capabilities such as `annotation:create` and `admin.users:suspend`. The identity provider composes broader access levels from those capabilities. Do not introduce an `Admin`/`Analyst`/`Viewer` enum or infer permissions from a user row.

[GraphQL context](../backend/crates/econ-graph-graphql/src/graphql/context.rs) exposes role checks and user-row access separately. Require the capability needed by an operation and enforce resource ownership or visibility as well. Maintain OAuth/OIDC, JWT, session-lifecycle, and password-security expertise, while respecting the boundary: provider sign-in and password handling belong to Keycloak, rather than an in-house backend login flow.

### Data Acquisition and Quality

The crawler processes `crawl_queue` jobs. Its [RefreshScheduler](../backend/crates/econ-graph-crawler/src/scheduler.rs), run by `crawler-worker`, enqueues fetches for due series. Shared HTTP, retries, rate limiting, persistence, and queueing belong outside individual source adapters.

The [production adapter registry](../backend/crates/econ-graph-crawler/src/sources/mod.rs) includes FRED, BLS, BEA, Census, World Bank, and FHFA. Development static catalogs are controlled by the `static_catalogs` build flag; a catalog entry is not evidence of a live source integration. Use the [crawler deployment guide](../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md) for queue and CLI operations.

For source work, validate schema and numeric ranges, check cross-source consistency where meaningful, distinguish missing observations from zero, and preserve revision history. Design incremental updates, retries, and error reporting so partial failures do not silently corrupt stored data. Broader source-integration expertise remains part of the role.

## Database Design and Migrations

### Nullability Expresses Domain Meaning

Use `NOT NULL` when a value is always known: a processing state, a hierarchy level, or a boolean with no unknown state. A default supplies an omitted value; it does not prevent an explicit NULL. Use nullable fields when absence has meaning: content before download, an amendment type only for amended filings, or a restatement reason only for restated filings.

These SQL column fragments illustrate required state and optional content; they are not a complete migration:

```sql
xbrl_file_compressed BOOLEAN NOT NULL DEFAULT TRUE,
level INTEGER NOT NULL DEFAULT 0,
xbrl_file_content BYTEA,
```

The corresponding Rust model field fragments are:

```rust
pub xbrl_file_compressed: bool,
pub level: i32,
pub xbrl_file_content: Option<Vec<u8>>,
```

Keep model nullability aligned with the schema. An unnecessary `Option<bool>` obscures whether unknown is a real state and spreads `unwrap_or()` decisions through callers. Conversely, do not replace meaningful absence with an arbitrary default merely to remove `Option<T>`. A nullable column with a default is valid when NULL and the default deliberately mean different things.

Required fields simplify presence reasoning and remove unnecessary null checks. Choose indexes from actual query plans and predicates rather than assuming that nullable fields cannot be indexed.

### Changing Nullability

1. Choose the owning plane and migration directory. App migrations live in `backend/migrations/app`; data migrations live in `backend/migrations/data`. The Diesel CLI defaults to app migrations.
2. Inspect existing NULLs and determine a justified backfill or correction. Preserve meaningful unknown states instead of inventing values.
3. Backfill existing rows before adding `NOT NULL`. Account for concurrent writers so NULLs cannot reappear between those steps.
4. Apply the constraint, regenerate the shared Diesel schema, and separately update model types and callers. Remove obsolete `Option<T>` handling only where the invariant now holds.
5. Validate the migration and constraints with representative disposable test data, including existing rows.

For example, this migration fragment assumes the domain has established that a missing level means top level:

```sql
UPDATE financial_line_items SET level = 0 WHERE level IS NULL;
ALTER TABLE financial_line_items ALTER COLUMN level SET NOT NULL;
```

Follow [migration rules and schema generation](../docs/development/MIGRATIONS.md#adding-a-migration). `diesel print-schema` emits schema declarations, not hand-written Rust models. Regenerate `backend/crates/econ-graph-core/src/schema.rs` from a database with both planes' migrations applied, even when the change concerns only one plane. Do not overwrite that shared file with one plane's tables. Preserve released migrations, unique versions, existing data, and stable series IDs according to that guide.

### Enum Design and Diesel Mapping

Prefer PostgreSQL enums for stable, closed domain sets such as processing status, compression type, statement type, and internal annotation categories. They enforce allowed values and make model intent explicit. Assess the cost of evolving that set; do not assume an enum universally outperforms text with a constraint.

Keep external identifiers and frequently changing codes as text when the upstream controls their vocabulary, including SEC filing/form/amendment types and industry codes. User content, URLs, and file paths also remain text rather than enums.

The project distinguishes three identities:

- PostgreSQL defines a database type, such as `compression_type`, with labels `zstd`, `lz4`, `gzip`, and `none`.
- Generated [Diesel SQL types](../backend/crates/econ-graph-core/src/schema.rs) include `schema::sql_types::CompressionType`, whose PostgreSQL mapping names `compression_type`. Table declarations use that SQL marker type.
- The Rust value enum, `enums::CompressionType`, represents values in models and binds to the SQL marker with Diesel traits.

The following excerpt from [core enums](../backend/crates/econ-graph-core/src/enums.rs) shows the value declaration. It omits the serialization and query implementations and is not a standalone integration:

```rust
use crate::schema::sql_types;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, diesel::AsExpression)]
#[diesel(sql_type = sql_types::CompressionType)]
pub enum CompressionType {
    Zstd,
    Lz4,
    Gzip,
    None,
}
```

The same source implements `ToSql<sql_types::CompressionType, Pg>`, `FromSql<sql_types::CompressionType, Pg>`, and `Queryable`. The conversions map Rust variants to the exact PostgreSQL labels and reject unknown labels on decoding. Follow that complete pattern when extending an enum. The workspace uses these explicit mappings rather than `diesel-derive-enum`; do not add that dependency as a prerequisite or replace generated SQL marker types with Rust value enums.

Create a new database type before referring to it in a column. For an existing text status column, validate every stored label and define an explicit mapping for any differing values before conversion. Preserve existing statuses; setting every row to `pending` is not a migration of their meaning. If replacing a column, populate the replacement from the old column, verify the mapping and row coverage, then enforce constraints and retire the old column. A newly added required column may use a justified default, but its default does not migrate another column's values.

For reference, the [data baseline](../backend/migrations/data/2026-10-21-000100_v4_0_baseline/up.sql) defines `processing_status` with `pending`, `downloaded`, `processing`, `completed`, and `failed`; the Rust `ProcessingStatus` must cover all five. Keep migration labels, generated SQL types, value conversions, and model fields consistent. For Diesel trait errors, check those relationships and required trait implementations before changing names or dependencies.

### Before Committing Schema Changes

- Give each field a clear domain purpose and document non-obvious meaning, such as hierarchy level 0 representing the top level.
- Justify defaults and nullability; evaluate enums for fixed sets.
- Validate foreign keys, check constraints, uniqueness, and cross-plane reference rules.
- Exercise migrations against representative existing data and check schema/model alignment.
- Keep documentation consistent with the resulting behavior.

## Testing Strategy

Cover pure functions with fast unit tests, cross-module behavior with integration tests, and full workflows with end-to-end tests. Use load/stress tests and security tests where the change calls for them. Target 100% core-model test coverage, comprehensive service and resolver coverage, authentication/token-flow coverage, and crawler tests that exercise acquisition through persistence. Mock upstreams or recorded fixtures provide deterministic coverage; real-source end-to-end checks need explicit prerequisites and must not be confused with fixture tests.

### Database Helpers and Isolation

[Core test utilities](../backend/crates/econ-graph-core/src/test_utils.rs) provide `TestContainer`, `get_test_db()`, `db_test()`, and `ScratchDatabase`. `get_test_db()` and `db_test()` return the same global `Arc<TestContainer>` per test binary; they do not allocate a database per test.

Construction depends on how core is compiled:

- In core unit tests (`cfg(test)`), `TestContainer::new()` starts a PostgreSQL testcontainer. An external `DATABASE_URL` changes the pool target but does not remove that Docker requirement.
- When core is compiled as a dependency, including integration tests and other crates' tests, construction connects to `DATABASE_URL`, falling back to the local `econ_graph_test` database. It does not start a container.

Construction does not run migrations or create fixtures. Arrange migrations in test setup and use the target module's setup and locking convention. Database integration tests must fail promptly when their required database is unavailable; do not silently skip them with `skip_if_no_database`.

Use independent disposable databases, such as `ScratchDatabase` where appropriate, or unique fixture IDs when tests can safely share tables. Serialize tests that mutate shared state or perform destructive cleanup, using the same lock or `serial_test` convention across all affected tests. A shared pool and cleanup alone do not make parallel tests isolated. Serializing one binary does not coordinate separate Cargo processes.

`clean_database()` drops and recreates the pool target's entire public schema and then runs migrations. Its migration URL comes separately from `DATABASE_URL` or the localhost fallback. If using it, align that URL with the disposable pool target and exclude other users of that database. Check its returned `Result` before continuing:

```rust
container.clean_database().await.expect("Failed to clean database");
```

This is a test-body fragment with an existing `container`, not a standalone test. See [database integration testing](../docs/testing/DATABASE_TESTING.md) for exact lifecycle, cleanup behavior, fixtures, Docker requirements, and commands. Use `tokio_test` for suitable async test utilities and pooled/shared access where safe; neither substitutes for isolation.

## Development Workflow

### Cursor-native Commands

The command prompts live in [`.cursor/commands`](../.cursor/commands). Use their workflow while applying the project's current standards:

- `/branch` creates feature branches from the remote base without checking out local main.
- `/commit` prepares Conventional Commits; do not bypass pre-commit hooks.
- `/pr` and `/pr-ready` create or update a reviewable PR and its description.
- `/pr-checks` watches checks; `/ci-latest` inspects recent runs.
- `/e2e` runs local end-to-end validation; arrange its prerequisites before execution.
- `/db-migrate` assists with Diesel migrations against configured `DATABASE_URL`. Select the owning plane and directory using the [migration guide](../docs/development/MIGRATIONS.md); the shorthand does not choose the data plane for you.
- `/debug-backend` supports a single-issue debugging loop: form a hypothesis, make one targeted change, then check the result.

Run targeted tests before committing, validate affected multi-crate builds before the PR, and complete required unit, integration, end-to-end, and security checks for the change. Prefer rebase when maintaining clean feature history. Keep PR descriptions focused on behavior and rationale, and follow checks and automated review through completion.

### Code Quality

Document functions using the project's Google-style comment convention, including meaningful contracts and failure behavior. Keep types precise, minimize `unwrap()`, propagate errors with context, and log failures without exposing sensitive information. Benchmark performance-sensitive changes and evaluate security during design and review.

## Common Implementation Patterns

### Async Data Lookup

This function follows the [series service](../backend/crates/econ-graph-services/src/services/series_service.rs). The caller supplies the data-plane pool; services accept the underlying `DatabasePool`. Qualifying the column and naming the argument `series_id` makes the comparison unambiguous.

```rust
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::EconomicSeries,
    schema::economic_series,
};
use uuid::Uuid;

pub async fn get_series_by_id(
    pool: &DatabasePool,
    series_id: Uuid,
) -> AppResult<Option<EconomicSeries>> {
    let mut conn = pool.get().await.map_err(|e| {
        AppError::DatabaseError(format!("Failed to get database connection: {e}"))
    })?;
    let series = economic_series::table
        .filter(economic_series::id.eq(series_id))
        .select(EconomicSeries::as_select())
        .first::<EconomicSeries>(&mut *conn)
        .await
        .optional()?;
    Ok(series)
}
```

Pool failures become `AppError`; query failures propagate through its conversion. `optional()` distinguishes an absent row from an execution error. Dereference the pooled connection when passing it to Diesel.

### GraphQL Delegation

This resolver fragment belongs inside the GraphQL crate's `#[Object] impl Query`. It follows the served `series` resolver, including treating a series without observations as absent:

```rust
async fn series(
    &self,
    ctx: &async_graphql::Context<'_>,
    id: async_graphql::ID,
) -> async_graphql::Result<Option<crate::types::EconomicSeriesType>> {
    let pool = crate::graphql::context::data_pool(ctx)?;
    let series_id = uuid::Uuid::parse_str(&id)?;
    match econ_graph_services::services::series_service::get_series_by_id(pool, series_id).await? {
        Some(series) if series.end_date.is_some() => Ok(Some(series.into())),
        _ => Ok(None),
    }
}
```

The context helper returns `DataPool` or the data-plane routing error when this backend has no local data pool. App operations use the app-plane context helper instead. Keep request parsing and GraphQL errors at the interface boundary; keep reusable domain errors in core. When explicit mapping is needed, use `async_graphql::Error::new` with a caller-safe message rather than exposing internal diagnostics.

## Lessons from the Crate Split

Keep core domain types independent of GraphQL. Rust's orphan rules prevent implementing a foreign trait for a foreign type in the GraphQL crate. Define a local GraphQL wrapper and implement `From<CoreType>` for that wrapper: `EconomicSeries` becomes `EconomicSeriesType`. Moving types or adding imports does not bypass the orphan rule.

Use `imports.rs` where the crate centralizes interface imports, and use explicit cross-crate paths such as `econ_graph_core::models::user::User`. Check the owning crate before importing; shared types should not be imported from the backend executable. Keep each new crate in the workspace and declare its actual dependencies.

The split provides smaller dependency boundaries and permits Cargo to reuse unaffected artifacts and build independent crates in parallel. A changed crate and its dependents may still recompile. Track clean and incremental build times separately; no fixed percentage improvement follows from splitting crates alone.

## Performance and Security Duties

### Performance

Keep dependency trees small, use incremental builds, and preserve opportunities for parallel compilation. Measure build times across crates before changing compilation settings.

At runtime, manage pool capacity and connection lifetimes, inspect SQL query plans and indexes, avoid blocking I/O in async tasks, and monitor allocation and cleanup. Evaluate caching for repeated work with explicit freshness and invalidation rules. Choose a cache mechanism from the relevant component's implementation; Redis expertise does not imply Redis is deployed here.

Track database query time, API/resolver latency, memory consumption, error rates, and build time. Use query analysis, benchmarks, and load tests to decide whether pooling, batching, caching, or memory changes solve the measured problem.

### Security

- **Identity:** Validate token signatures and expiration and preserve OIDC/OAuth protections. Respect provider ownership of passwords and sessions; assess strong password hashing when reviewing identity-provider configuration or integrations.
- **API boundaries:** Validate inputs, use parameterized queries, protect rate limits, configure CORS for intended callers, and prevent SQL injection and unsafe rendering of user content.
- **Authorization:** Require capabilities and resource access checks at every applicable boundary; do not substitute a client-provided role or user preference for token authority.
- **Data protection:** Assess sensitive-data encryption at rest, access controls, audit logging, session lifecycle, and privacy/GDPR obligations relevant to the feature. These are review duties, not blanket compliance guarantees.

## Troubleshooting

### Compilation

Check import ownership, manifest dependencies, and workspace membership first. For type mismatches, inspect conversions and local wrappers; for enum trait failures, compare SQL marker types with Rust value mappings. Resolve orphan-rule failures with an owned interface type. Check dependency direction for cycles rather than adding reciprocal dependencies.

### Database

For pool exhaustion, inspect capacity, connection lifetimes, and cleanup. For migration failures, check the owning plane, migration order, existing rows, and constraints. For slow queries, inspect plans before adding indexes. For integrity failures, inspect foreign keys, cross-plane references, and validation.

### GraphQL

Check schema traits, context data, selected plane, authorization, and error conversion. Verify `From` implementations for wrapper types and test filtering and cursor pagination at boundary cases. Use the schema constructor appropriate to the test or service rather than assuming all constructors install the same resources.

### Tests

For connection timeouts, inspect the helper's compilation mode, target URL handling, and pool configuration. Check Docker only for paths that start testcontainers. Confirm migrations and fixtures are arranged, handle cleanup failures, and inspect locks or unique fixtures when tests interfere. Serialize incompatible shared-state tests or give them independent databases before enabling parallel execution.
