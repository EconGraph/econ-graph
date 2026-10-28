# EconGraph GraphQL

This crate defines the GraphQL schema, query and mutation resolvers, and DataLoaders
that expose EconGraph's economic data and application state. The hosting
[backend](../econ-graph-backend/src/main.rs) authenticates bearer tokens and supplies
request context. This crate uses the caller in
[`GraphQLContext`](src/graphql/context.rs) to enforce resolver permissions.

## Schema behavior

The [schema builder](src/graphql/schema.rs) limits query depth to 15 and complexity
to 1000, counting one per selected field. Standard introspection queries are
permitted. [DataLoaders](src/graphql/dataloaders.rs) batch related database reads.

[Authorization checks](src/graphql/context.rs) enforce fine-grained roles.
[Audit handling](src/graphql/audit.rs) records admin mutation attempts and
signed-in callers' permission denials. Audit writes are best effort: failures are
logged and counted without failing the request. The
[error sanitizer](src/graphql/errors.rs) keeps internal database details out of
GraphQL error responses.

## Schema snapshot

[`schema.graphql`](schema.graphql) is the SDL the server serves, printed by
`econ_graph_graphql::graphql::sdl()`. The
[`schema_snapshot` test](tests/schema_snapshot.rs), run in CI by the
`backend-smoke-tests` job, "Backend Smoke Tests (Fast)", fails when the two differ.
The frontend's [operation validation test](../../../frontend/src/__tests__/graphql-operations.test.ts)
validates frontend GraphQL operations against this file, except those on its skip list.

A PR that changes the schema regenerates the file and commits it. From `backend/`:

```bash
UPDATE_SCHEMA=1 cargo test -p econ-graph-graphql --test schema_snapshot
```

If a frontend operation stops validating, fix the operation. Operations that are
known to fail are listed in the [frontend skip list](../../../frontend/src/__tests__/graphqlOperationSkipList.ts);
remove an entry in the PR that fixes or deletes its operation.

## Testing

The tests cover different parts of the API:

- The [SDL snapshot test](tests/schema_snapshot.rs) compares the runtime schema
  with the committed file without needing a database.
- [Query-limit tests](src/graphql/schema.rs) check depth and complexity boundaries
  and permitted introspection. They use deliberately unreachable pools; the
  at-the-limit complexity case can wait for a connection timeout.
- [Context tests](src/graphql/context.rs) and [authorization tests](src/graphql/authorization_tests.rs)
  check caller permissions. Authorization coverage includes database-backed user
  administration regressions; [audit tests](src/graphql/audit.rs) check recorded outcomes.
- Database-backed tests cover [annotation visibility](src/graphql/annotation_visibility_tests.rs),
  [annotation idempotency](src/graphql/annotation_idempotency_tests.rs),
  [error sanitization](src/graphql/error_sanitizing_db_tests.rs), and
  [separate app and data databases](src/graphql/split_databases_tests.rs).
- [Loader behavior tests](src/graphql/n_plus_one_tests.rs) check missing-ID results
  and successful query execution. They do not assert SQL query counts, timing, or
  resource thresholds.

Run these commands from `backend/`. To check the schema without PostgreSQL:

```bash
cargo test -p econ-graph-graphql --test schema_snapshot
```

Before running database-backed tests, set `DATABASE_URL` to a reachable disposable
PostgreSQL database; see [database testing](../../../docs/testing/DATABASE_TESTING.md)
for setup and cleanup behavior. The core `get_test_db()` helper, compiled as a
dependency here, connects to that URL or falls back to
`postgres://localhost/econ_graph_test`; it does not start a container or apply
migrations. Test modules arrange their own migrations and fixtures, and some drop
or delete existing data. Split-database tests also need permission to create and
drop scratch databases on that server.

Some modules, including annotation visibility, error sanitization, and
split-database tests, skip when `DATABASE_URL` is absent. Others require it or use
the helper's fallback, so an unset variable does not make the full suite database-free.

```bash
# Run this crate's full suite after preparing the disposable database.
cargo test -p econ-graph-graphql

# Select existing library test modules.
cargo test -p econ-graph-graphql --lib graphql::authorization_tests
cargo test -p econ-graph-graphql --lib graphql::annotation_visibility_tests
cargo test -p econ-graph-graphql --lib graphql::split_databases_tests
```

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL).
See the [backend license](../../LICENSE) for complete terms and conditions.
