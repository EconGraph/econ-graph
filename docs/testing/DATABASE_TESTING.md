# Database Integration Testing

Database-backed tests use real PostgreSQL. The backend is a Cargo workspace; run the
commands below from `backend/`, and use a disposable test database.

## Current Infrastructure

- [Core test utilities](../../backend/crates/econ-graph-core/src/test_utils.rs) provide
  `TestContainer`, `get_test_db()`, `db_test()`, and `ScratchDatabase`.
- [Dataset tests](../../backend/crates/econ-graph-core/src/models/dataset/tests.rs) show
  a separate pool helper, one-time migrations, and unique rows per test.
- [GraphQL migration helper](../../backend/crates/econ-graph-graphql/src/graphql/test_db.rs)
  runs migrations once per test binary.
- [Schema validation integration test](../../backend/crates/econ-graph-core/tests/schema_validation_test.rs)
  starts its own container, applies migrations, and compares Diesel's generated schema.
- [Workspace dependencies](../../backend/Cargo.toml) declare the database and
  testcontainers versions.

There is no `db_test!` macro in the current utility. `db_test()` is an async function
that returns what `get_test_db()` does.

## TestContainer Lifecycle

The public helper signatures are:

```rust
pub async fn get_test_db() -> Arc<TestContainer>;
pub async fn db_test() -> Arc<TestContainer>;

impl TestContainer {
    pub async fn new() -> Self;
    pub fn pool(&self) -> &DatabasePool;
    pub fn database_url(&self) -> &str;
    pub async fn clean_database(&self) -> Result<(), Box<dyn std::error::Error>>;
}
```

These are signature summaries, not a standalone Rust implementation.
`DatabaseTestExt` only exposes `test_pool(&self) -> &DatabasePool`; it does not
provide count-query or table-inspection methods.

`TestContainer::new()` gives the calling test a database of its own:

- With `DATABASE_URL` set, and whenever core is compiled as a dependency (other crates'
  tests and integration-test targets, which fall back to
  `postgres://localhost/econ_graph_test`), it creates a scratch database named
  `eg_scratch_test_<uuid>` on that server, applies both migration sets, and opens its pool
  there. The database is dropped (`DROP DATABASE ... WITH (FORCE)`) when the last handle
  to it is dropped. No container is started, but the role must be allowed to create
  databases (for example, with `CREATEDB`).
- In core's own unit tests (`cfg(test)`) without `DATABASE_URL`, it starts PostgreSQL 18
  with testcontainers instead, which needs Docker. That database is not migrated; tests
  migrate it with `clean_database()`.

`database_url()` returns the URL of the test's database, for a test that needs a pool or
connection of its own; never build one from `DATABASE_URL` beside a `TestContainer`,
which would reach a different database. Construction creates no seed rows beyond what the
migrations insert, and there is no `seed_test_data()` helper.

`get_test_db()` shares one database per test binary: the one `DATABASE_URL` names, as it
is, without migrating it. Each call returns a pool of the caller's own, opened on the
caller's runtime. Tests using it must not drop the schema; they create unique rows
instead, as the graphql tests do.

### Cleaning and Migration Targets

`clean_database()` drops the test database's entire `public` schema with `CASCADE`,
recreates it, grants schema permissions, and applies both migration sets to that same
database (its `database_url()`). It does not truncate tables or wrap each test in a
rollback transaction. Because every `TestContainer::new()` has a database of its own,
cleaning one never affects another test, so test functions can run in parallel; CI runs
the workspace suites in parallel.

A test run that is killed (Ctrl-C, a timeout) leaves its scratch databases behind on a
persistent server. Remove them with:

```bash
psql "$DATABASE_URL" -Atc "SELECT 'DROP DATABASE ' || quote_ident(datname) || ' WITH (FORCE);'
  FROM pg_database WHERE datname LIKE 'eg_scratch_%'" | psql "$DATABASE_URL"
```

For a new database test, use `TestContainer::new()` when the test needs a clean or
migrated database of its own, and `get_test_db()` with unique fixture rows when it only
reads and inserts. See the dataset tests for a module that migrates the shared database
once and avoids dropping the schema.

## ScratchDatabase Lifecycle

Core migration tests use `ScratchDatabase::create()` to create a uniquely named
database on the server named by `DATABASE_URL`, falling back to
`postgres://localhost/econ_graph_test`. `TestContainer::new()` is built on it. The
helper attempts to drop the
scratch database when it goes out of scope; cleanup errors are ignored.

## Running Tests

Prerequisites are Rust, a reachable disposable PostgreSQL instance for tests using
external pools, and a running Docker daemon for container-backed tests. Schema
validation additionally invokes the Diesel CLI's `print-schema`.

Every `TestContainer` test, and the core migration tests, require the `DATABASE_URL` role
to be allowed to create databases on that disposable PostgreSQL server (for example,
with `CREATEDB`). The dataset-only
command uses an existing database and does not require database-creation permission.

From the repository root:

```bash
cd backend
# Set DATABASE_URL to your disposable test database before DB-backed tests.
# Modules supporting TEST_DATABASE_URL may use that variable instead.

# Run a specific core unit-test module.
cargo test -p econ-graph-core --lib models::dataset::tests -- --nocapture

# Run core unit tests.
cargo test -p econ-graph-core --lib

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

`TEST_DATABASE_URL` support is module-specific; the core `TestContainer` and
`get_test_db()` read `DATABASE_URL`. The workspace suite can include external-service tests, so
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
