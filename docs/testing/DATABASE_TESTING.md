# Database Integration Testing

Database-backed tests use real PostgreSQL. The backend is a Cargo workspace; run the
commands below from `backend/`, and use a disposable test database.

## Current Infrastructure

- [Core test utilities](../../backend/crates/econ-graph-core/src/test_utils.rs) provide
  `TestContainer`, `get_test_db()`, and `db_test()`.
- [Dataset tests](../../backend/crates/econ-graph-core/src/models/dataset/tests.rs) show
  a separate pool helper, one-time migrations, and unique rows per test.
- [GraphQL migration helper](../../backend/crates/econ-graph-graphql/src/graphql/test_db.rs)
  runs migrations once per test binary.
- [Schema validation integration test](../../backend/crates/econ-graph-core/tests/schema_validation_test.rs)
  starts its own container, applies migrations, and compares Diesel's generated schema.
- [Workspace dependencies](../../backend/Cargo.toml) declare the database and
  testcontainers versions.

There is no `db_test!` macro in the current utility. `db_test()` is an async function
returning the same shared `Arc<TestContainer>` as `get_test_db()`.

## TestContainer Lifecycle

The public helper signatures are:

```rust
pub async fn get_test_db() -> Arc<TestContainer>;
pub async fn db_test() -> Arc<TestContainer>;

impl TestContainer {
    pub async fn new() -> Self;
    pub fn pool(&self) -> &DatabasePool;
    pub async fn clean_database(&self) -> Result<(), Box<dyn std::error::Error>>;
}
```

These are signature summaries, not a standalone Rust implementation.
`DatabaseTestExt` only exposes `test_pool(&self) -> &DatabasePool`; it does not
provide count-query or table-inspection methods.

Construction behaves differently depending on how the core crate is compiled:

- In core unit tests (`cfg(test)`), `TestContainer::new()` starts PostgreSQL 18 with
  testcontainers. Without `DATABASE_URL`, its pool connects to that container.
  With `DATABASE_URL`, its pool connects to that URL, but construction still starts
  a separate container; Docker is therefore required in either case.
- When core is compiled as a dependency, including for integration tests and other
  crates' tests, construction connects to `DATABASE_URL`, falling back to
  `postgres://localhost/econ_graph_test`. It does not start a container.
- Construction does **not** run migrations or create seed rows. Each test module
  must arrange its own migration and fixture setup. There is no
  `seed_test_data()` helper.

`get_test_db()` stores one container/pool in a global `OnceCell` per test binary.
It does not give each test a separate database.

### Cleaning and Migration Targets

`clean_database()` drops the pool connection's entire `public` schema with
`CASCADE`, recreates it, grants schema permissions, and then calls
`run_migrations()`. It does not truncate tables or wrap each test in a rollback
transaction.

The migration URL comes from `DATABASE_URL`, falling back to
`postgres://localhost/econ_graph_test`; it is not derived from the container's
mapped port. If using this helper, set `DATABASE_URL` to the disposable database
used by the pool so cleaning and migrations target the same database.

Tests sharing that database must coordinate destructive cleanup and fixture writes.
Some modules use `serial_test`, while others use their own database locks or unique
rows. Neither the shared helper nor an unrelated module's lock guarantees isolation.
`--test-threads=1` serializes a test binary's test functions; it does not coordinate
separate Cargo processes.

For a new database test, follow the target module's existing setup and locking
convention. Prefer unique fixture IDs when cleanup is unnecessary. See the dataset
tests for an example that runs migrations once and avoids dropping the schema.

## Running Tests

Prerequisites are Rust, a reachable disposable PostgreSQL instance for tests using
external pools, and a running Docker daemon for container-backed tests. Schema
validation additionally invokes the Diesel CLI's `print-schema`.

From the repository root:

```bash
cd backend
# Set DATABASE_URL to your disposable test database before DB-backed tests.
# Modules supporting TEST_DATABASE_URL may use that variable instead.

# Run a specific core unit-test module.
cargo test -p econ-graph-core --lib models::dataset::tests -- --nocapture

# Run core unit tests sequentially.
cargo test -p econ-graph-core --lib -- --test-threads=1

# Run the existing schema validation integration-test target.
cargo test -p econ-graph-core --test schema_validation_test -- --nocapture

# Verify the committed GraphQL SDL matches the runtime schema.
cargo test -p econ-graph-graphql --test schema_snapshot

# Run the workspace's default-feature suite, including integration-test targets.
cargo test --workspace
```

There is no integration-test target named `integration`. Select an existing target
with `--test <filename-without-.rs>` and its owning package. A test-name filter does
not arrange migrations, start application services, or seed a database.

`TEST_DATABASE_URL` support is module-specific; the shared core `TestContainer`
reads `DATABASE_URL`. The workspace suite can include external-service tests, so
consult each module's prerequisites before running it. Feature-gated checks and
required CI commands are documented in the [CI/CD guide](../development/CI_CD_PIPELINE.md).

## Debugging

Use output and logging for the selected test:

```bash
RUST_LOG=debug cargo test -p econ-graph-core --lib models::dataset::tests -- --nocapture
docker ps
docker logs <container_id>
```

Inspect the test's pool, migration setup, environment-variable handling, and lock
before assuming a failure is caused by PostgreSQL or Docker. Test utilities redact
database URLs when logging; keep credentials out of additional diagnostic output.

## CI

The repository's actual job setup and test selection live in
[ci-core.yml](../../.github/workflows/ci-core.yml) and the
[CI/CD guide](../development/CI_CD_PIPELINE.md). Use those sources for service setup,
environment variables, package selection, and feature flags rather than copying a
separate illustrative workflow.
