# Developer Guide: Database Setup and Migrations

This guide covers local PostgreSQL setup, schema changes, and database tests for the EconGraph backend. The [database migration guide](../development/MIGRATIONS.md) defines migration ownership and release rules; [database integration testing](../testing/DATABASE_TESTING.md) explains the test infrastructure in detail.

## Database targets

The app plane stores users and the state they create, such as annotations and preferences. Its migrations live in `backend/migrations/app` and run against `APP_DATABASE_URL`. The data plane stores crawled and reference data, including sources, series, observations, and SEC data. Its migrations live in `backend/migrations/data` and run against `DATA_DATABASE_URL`. Both URLs may point to one physical database holding both planes' tables, as they do in the root Compose setup.

Backend startup applies only app plane migrations. Use `crawler migrate` to apply data plane migrations; starting the backend or crawler worker does not apply them. The Rust migration runner maps these responsibilities to `MigrationSet::App` and `MigrationSet::Data` and holds a database advisory lock during each run. The test-oriented `run_migrations()` applies data migrations followed by app migrations to one database under that lock.

Runtime configuration and development tools read different variables:

- The backend requires `APP_DATABASE_URL`. `DATA_DATABASE_URL` is optional for deployments without a local data plane; when absent, data fields answer `DATA_SERVED_REMOTELY`.
- Data-only binaries such as `crawler` require `DATA_DATABASE_URL` (the crawler CLI also accepts `--database-url`). Runtime configuration rejects `DATABASE_URL` as a fallback for a missing required plane URL.
- The Diesel CLI and shared core test helper use `DATABASE_URL`. Setting that variable alone does not configure application binaries. Some test modules also support `TEST_DATABASE_URL`; check the module being run.

## Local database setup

Use the checked-in [root Compose file](../../docker-compose.yml) for this example. Its `postgres` service creates `econ_graph` on host port 5432 with development credentials `postgres` / `password`. These commands run from the repository root and retain existing volumes:

```bash
docker compose up -d --wait postgres
docker compose exec postgres pg_isready -U postgres -d econ_graph
```

`--wait` waits for the service's health check instead of assuming PostgreSQL will be ready after a fixed sleep. Starting PostgreSQL alone does not apply the nested app and data migrations. With a **new, empty** `econ_graph` database, switch to the backend workspace and apply both planes explicitly:

```bash
cd backend
export APP_DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph
export DATA_DATABASE_URL="$APP_DATABASE_URL"
export DATABASE_URL="$APP_DATABASE_URL"

diesel migration list --migration-dir migrations/data
diesel migration list --migration-dir migrations/app
diesel migration run --migration-dir migrations/data
diesel migration run --migration-dir migrations/app
```

These host URLs use `localhost`; Compose application services use the hostname `postgres`. Compose's `crawler-migrate` service applies the data migrations before its backend services start, and the backend applies app migrations at startup. For a host-run crawler, from `backend/`, use `cargo run --bin crawler -- migrate` with `DATA_DATABASE_URL` set to the intended target.

For a database created from the single v4.0.0 baseline, do not use the fresh-database CLI sequence to bypass the app baseline guard. Start the backend once to perform its supported app-baseline adoption check, then apply pending data migrations with `crawler migrate`. The runner verifies the recorded data baseline and all app baseline tables before recording the app baseline. See [the v4.0.0 baseline split](../development/MIGRATIONS.md#the-v400-baseline-split) for the exact conditions.

The Compose test profile has a separate `test-postgres` service on host port 5433. It is not started by the postgres-only command above. Container-backed tests instead use a dynamically mapped port; there is no universal test port or database URL. Keep test targets disposable and separate from databases holding development work or crawled data.

## Changing the schema

Run Diesel and Cargo commands below from `backend/`. [diesel.toml](../../backend/diesel.toml) defaults to `migrations/app`; specify the migration directory to make the target plane clear. A migration changes only the tables owned by its plane, and app migrations must not add foreign keys to data tables.

```bash
# Choose the plane that owns the changed tables.
diesel migration generate --migration-dir migrations/app add_feature_state
# For a data change instead:
diesel migration generate --migration-dir migrations/data add_source_metadata
```

Give the migration a version later than every existing version in both directories. Versions must be unique across both planes because a shared database records them in the same `__diesel_schema_migrations` table. Check the layout and version uniqueness with:

```bash
python3 scripts/check_migration_order.py
```

### App and data migration validation

For a **new reversible incremental app migration**, test up, down, and up again on a disposable database. First set `DATABASE_URL` to that database, apply the prerequisite migrations, and confirm the new migration will be the last app migration before reverting:

```bash
diesel migration run --migration-dir migrations/app
diesel migration revert --migration-dir migrations/app
diesel migration run --migration-dir migrations/app
```

`revert` acts on the last applied migration in the selected directory, not the last migration across both planes. `redo` likewise reverts and reapplies within that directory. Neither baseline can be reverted.

Data migrations are forward-only: fix mistakes with a new migration rather than `revert` or `redo`. They preserve existing rows, using defaults, backfills, or conversions where needed. A change requiring crawled rows to be dropped or fetched again needs the maintainer's sign-off, a verified retained backup, and a recrawl plan. See [data migrations](../development/MIGRATIONS.md#data-migrations).

Released migrations and both baselines remain unchanged. The migration guide documents the narrow [reference-code seed exception](../development/MIGRATIONS.md#reference-code-seed-migrations) and the [app migration squash process](../development/MIGRATIONS.md#squashing-at-a-release); neither permits deleting migration files to repair an unexplained schema mismatch.

### Regenerate the shared schema and update models

After changing SQL, regenerate [the committed Diesel schema](../../backend/crates/econ-graph-core/src/schema.rs) from a disposable database containing **both** planes with all migrations applied. Set `DATABASE_URL` to that database, then run from `backend/`:

```bash
diesel print-schema > crates/econ-graph-core/src/schema.rs
```

`diesel.toml` deliberately omits `print_schema.file`, so migrating does not regenerate this artifact. Generating it from an app-only or data-only database would remove the other plane's tables from the shared schema. The [schema validation integration test](../../backend/crates/econ-graph-core/tests/schema_validation_test.rs) compares the committed schema with `diesel print-schema` output.

Update the affected models in [the core models directory](../../backend/crates/econ-graph-core/src/models), their insert/update structs, initializers, and tests. For example, a `data_sources` change can require updating `DataSource`, `NewDataSource`, and `UpdateDataSource` in [data_source.rs](../../backend/crates/econ-graph-core/src/models/data_source.rs), along with callers that initialize them. Other tables require their own model changes.

These SQL patterns illustrate adding a column and backfilling values; replace `table_name`, `column_name`, and `TYPE` with the intended identifiers and type:

```sql
ALTER TABLE table_name ADD COLUMN column_name TYPE;
COMMENT ON COLUMN table_name.column_name IS 'Description';
UPDATE table_name SET column_name = 'default_value' WHERE column_name IS NULL;
```

For deployment, follow [applying data migrations](../development/MIGRATIONS.md#applying-the-data-set-crawler-migrate) and the [Kubernetes database guidance](../../k8s/README.md#databases). The deployment script runs a `crawler-migrate` Job before applying backend and worker services in namespaces that own a data database. A default `diesel migration run` command covers only app migrations and is not a complete deployment procedure.

## Database tests

### Construction and migration ownership

The [shared core test utilities](../../backend/crates/econ-graph-core/src/test_utils.rs) behave differently depending on how the core crate is compiled:

- In core unit tests (`cfg(test)`), `TestContainer::new()` starts a PostgreSQL 18 container. With `DATABASE_URL`, the pool uses that external URL, but construction still starts a separate container, so Docker is required. Without the variable, the pool uses the container's dynamically mapped port.
- When core is compiled as a dependency (`cfg(not(test))`), including integration tests and other crates' tests, construction uses `DATABASE_URL` or the fallback `postgres://localhost/econ_graph_test`. It does not start a container.

Neither constructor runs migrations or creates fixture rows. Each module must arrange those steps. `get_test_db()` stores one helper in a `OnceCell` per test binary; it does not create a separate database for every test.

Before running database tests, identify the selected module's pool URL, migration setup, fixture setup, and locking convention. Use a disposable target. For example, the dataset module uses its own external pool and runs migrations once. Set `TEST_DATABASE_URL` to a disposable database, or set `DATABASE_URL` to one whose name contains `test`, then run from `backend/`:

```bash
cargo test -p econ-graph-core --lib models::dataset::tests -- --nocapture
```

A test-name filter selects tests; it does not provision or migrate their database. See [database integration testing](../testing/DATABASE_TESTING.md#running-tests) for package selection and prerequisites.

### Cleanup errors and target alignment

The existing `TestContainer::clean_database()` returns `Result<(), Box<dyn std::error::Error>>`. It drops the pool connection's entire `public` schema with `CASCADE`, recreates it, grants permissions, and invokes `run_migrations()` to apply both planes. It does not merely truncate `countries` or roll back a test transaction.

The migration URL comes from `DATABASE_URL`, falling back to `postgres://localhost/econ_graph_test`; it is not derived from the pool's mapped container port. Before using cleanup, ensure this URL and the pool name the **same disposable database**. Otherwise, cleanup can drop one database's schema and migrate another database.

Utilities should propagate cleanup failures to their callers. At the test call site, stop before writing fixtures or making assertions if cleanup fails:

```rust
container.clean_database().await.expect("Failed to clean database");
```

`expect` here fails the test with the cleanup error. Ignoring the `Result`, including with `let _ =`, allows the test to continue after a failed reset and can turn that failure into misleading missing-column or duplicate-key errors.

### Shared fixtures and country codes

Tests sharing a database must coordinate cleanup and writes using the target module's locking convention. `#[serial]` coordinates participating tests; it does not protect against unrelated locks, other test binaries, or separate Cargo processes. `--test-threads=1` only serializes tests within one binary. Prefer unique fixture rows when a module can avoid destructive cleanup; the [dataset tests](../../backend/crates/econ-graph-core/src/models/dataset/tests.rs) demonstrate one-time migrations and separate rows per test.

Use full UUIDs in fixture fields that accept them. Country-code fields have a finite namespace: `iso_code` is limited to three characters and `iso_code_2` to two, and both are unique. A value such as `format!("T{}", &uuid_text[..1])` has only 16 possible suffixes, so slicing a UUID does not prevent country-code collisions.

For country fixtures, reserve distinct codes of the valid lengths within the module, account for seeded countries, and coordinate their allocation and cleanup under the same lock. Check for existing rows before assigning a reserved code. Serial execution alone does not remove retained rows, and random short codes are not a uniqueness guarantee.

## Troubleshooting

### Connection or port failures

From the repository root, inspect the Compose service and connect to its actual database:

```bash
docker compose ps postgres
docker compose logs postgres
docker compose exec postgres pg_isready -U postgres -d econ_graph
docker compose exec postgres psql -U postgres -d econ_graph
```

If port 5432 is occupied, identify the owner with `docker ps` or the host's socket tools. Choose another host port in Compose if necessary and update host-side URLs to match. Container-side URLs continue to use `postgres:5432`.

### Missing columns or inconsistent migration history

Inspect the failing process's actual pool target and the SQL for the affected plane before changing anything. From `backend/`, with `DATABASE_URL` set to that target:

```bash
diesel migration list --migration-dir migrations/data
diesel migration list --migration-dir migrations/app
diesel print-schema
```

A column's existence does not prove that every statement in its migration ran. Do not delete a migration file or manually record its version as a general recovery step. Use the supported baseline-adoption procedure for a matching legacy database. Other existing databases need a reviewed repair based on their complete schema and migration history. Recreating and replaying migrations is an option only after confirming the target is disposable; removing a shared volume is not a routine diagnostic step.

For test failures, inspect the module's own migration setup rather than checking an unrelated development database. For duplicate country-code errors, check cleanup results, existing fixture rows, and coordination among all writers before changing the code generator.

## Schema reference

The [generated schema](../../backend/crates/econ-graph-core/src/schema.rs) is the current column reference. Key data plane tables include:

- `data_sources`: external API configuration. `api_key_name` is a nullable environment-variable name for an API key; `api_key_required` is the boolean requirement flag.
- `economic_series`: time series from external sources.
- `data_points`: individual observations for those series.
- `series_metadata`: metadata for discovered series.

## Additional resources

- [Diesel getting started: migrations](https://diesel.rs/guides/getting-started)
- [Diesel CLI configuration](https://diesel.rs/guides/configuring-diesel-cli)
- [PostgreSQL documentation](https://www.postgresql.org/docs/)
- [Docker Compose reference](https://docs.docker.com/compose/)
- [Testcontainers Rust](https://github.com/testcontainers/testcontainers-rs)
