# EconGraph Backend

`econ-graph-backend` is the HTTP application entry point for EconGraph. It serves
`/graphql`, builds the authentication context for each request, and exposes
liveness (`/livez`), readiness (`/readyz`), and Prometheus metrics (`/metrics`).

The binary assembles the GraphQL schema from `econ-graph-graphql`, verifies bearer
tokens through `econ-graph-auth`, and uses database pools and migrations from
`econ-graph-core`. Services in `econ-graph-services` provide application operations,
including checks against a remote data plane when configured. Crawling runs in the
separate `crawler-worker` process; the HTTP backend enqueues jobs for that worker.

At startup, the server opens the required app database and applies app plane
migrations. It also opens the data database when configured, but leaves data plane
migrations to `crawler migrate`. The two planes can share a database and pool.
See the [backend setup and configuration](../../README.md#development-workflow)
for database URLs, server commands, and environment settings.

## Testing

The tests cover several parts of the server:

- [Integration tests](src/integration_tests.rs) exercise database models, queue
  status, connection checks, and startup failure handling.
- [Metrics tests](src/metrics.rs) check metric registration, recording, and
  Prometheus output.
- [Binary tests](src/main.rs) cover routing, CORS, GraphQL request bodies and size
  limits, liveness, readiness, and request authentication. Most route checks use
  Warp's in-process test requests; some checks use a local HTTP server.
  [OIDC HTTP tests](src/request_auth_tests/oidc_http_tests.rs) exercise bearer-token
  handling through the production routes with a mock identity provider.
- [Database startup tests](src/databases_tests.rs) check app plane migrations,
  optional data access, shared pools, and readiness for each plane. They also
  compare local database and remote data-plane checks for series existence.

The `test_database_connection_test_performance` test asserts that one connection
check against the local test database completes in under one second. It does not
measure request load, memory use, or overall server throughput.

### Running tests

For database-backed tests, set `DATABASE_URL` to an existing, disposable PostgreSQL
database. This test setting is separate from the server's `APP_DATABASE_URL` and
`DATA_DATABASE_URL`. The integration tests use the core crate's `TestContainer`
helper as a dependency: it gives each test a migrated scratch database of its own on
the server `DATABASE_URL` names (falling back to
`postgres://localhost/econ_graph_test`) and does not start a container. The database
startup tests and database-backed authentication tests create scratch databases on
that server too, so the role must be able to create and drop databases, and the
server should not hold data you need. See [database testing](../../../docs/testing/DATABASE_TESTING.md)
for helper behavior and isolation requirements.

The scratch-database tests return early when `DATABASE_URL` is absent. Integration
tests using `TestContainer` instead try the fallback URL and fail if it is
unreachable. A passing command without `DATABASE_URL` therefore does not establish
that the scratch-database assertions ran.

Run these commands from `backend/`, after setting up the disposable database for
commands that need it:

```bash
# Run this package's tests.
cargo test -p econ-graph-backend

# Select database-backed integration tests in the library target.
cargo test -p econ-graph-backend --lib integration_tests

# Select metrics tests without a database.
cargo test -p econ-graph-backend --lib metrics

# Select binary route tests or database startup tests.
cargo test -p econ-graph-backend --bin econ-graph-backend route_tests
cargo test -p econ-graph-backend --bin econ-graph-backend databases_tests
```

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL).
See the [license](../../LICENSE) for complete terms and conditions.
