# Testing Strategy

Choose a test boundary that exposes the behavior being changed. Isolated tests give
quick feedback on logic and errors; database and service tests verify interactions;
browser tests verify that a user can complete a workflow through the application.
A test's dependencies matter more than its location: a Rust `#[tokio::test]` inside
a library can still require PostgreSQL or HTTP services.

## Choosing a layer

- **Isolated logic tests** exercise functions, validation, transformations, and
  permission checks without a running application. Use `#[test]` or
  `#[tokio::test]` as appropriate. The
  [GraphQL authorization tests](../../backend/crates/econ-graph-graphql/src/graphql/authorization_tests.rs)
  use a pool that never connects to distinguish role rejection from later database
  access; they do not prove that an authorized database operation succeeds.
- **Database and service tests** exercise interactions against real PostgreSQL and,
  where needed, local HTTP services. The
  [dataset tests](../../backend/crates/econ-graph-core/src/models/dataset/tests.rs)
  verify stored dataset relationships using unique rows. The
  [MCP integration tests](../../backend/crates/econ-graph-mcp/tests/mcp_server_integration_tests.rs)
  include assertions that tools return stored series and date-filtered observations,
  rather than merely returning a successful wrapper around an error.
- **Browser workflow tests** exercise the release frontend through the backend and
  database, including sign-in through Keycloak. Use the
  [release E2E suite](../../frontend/tests/e2e/release/README.md) for workflows such
  as search, chart transformations, CSV downloads, and signed-in annotations.

Keep detailed input and error permutations close to the component that owns them.
Use browser coverage to check that the application exposes the expected outcome
and recovery path across component boundaries. The
[failure behavior matrix](../testing/failure-behavior-matrix.md) connects backend
and frontend failure checks; the release suite's `failures/` specs sample failures
and recovery through the UI.

## Release E2E boundary and upstream data

In fixture mode, the release suite runs a frontend release build against a real
backend on PostgreSQL 18. Authentication uses a local Keycloak container. The seed
tool runs crawler adapters against local mock upstreams serving recorded responses;
it refuses requests to external data sources. This checks application communication,
persistence, transformations, and authentication with reproducible inputs, without
requiring provider API keys or depending on a provider's availability.

The seed applies data plane migrations, and backend startup applies app plane
migrations. Both planes use the same disposable database in this suite. Follow the
[release runner instructions](../../frontend/tests/e2e/release/README.md#run-it)
for dependencies, port conflicts, database overrides, and selecting an area or
browser project. An override of `RELEASE_E2E_DATABASE_URL` must name a disposable
test database: the runner migrates, seeds, and writes to it.

The map specs exercise different boundaries.
[`world-map-mocked.spec.ts`](../../frontend/tests/e2e/release/world-map/world-map-mocked.spec.ts)
intercepts GraphQL with Playwright's `page.route` to check map interactions against
controlled responses.
[`world-atlas.spec.ts`](../../frontend/tests/e2e/release/world-map/world-atlas.spec.ts)
checks that outlines load from the app's own origin, then aborts the
`countries-50m` asset request in a separate case to verify the error and Reload
action. It does not intercept GraphQL. The
[`world-map-every-country.spec.ts`](../../frontend/tests/e2e/release/world-map/world-map-every-country.spec.ts)
fixture-mode check instead compares every seeded WDI country and indicator with
recorded World Bank responses and the seed manifest. A mocked browser test does
not establish backend correctness or live provider coverage.

Live-provider validation is a separate activity: recorded responses cannot prove
that an upstream API still accepts a request or returns current data. Live requests
have provider-specific credentials, rate limits, availability, and timeout needs.
Use the [release QA guidance](../release/v4.0.0-qa.md) for those checks. The browser
suite also supports a
[deployed-build mode](../../frontend/tests/e2e/release/README.md#against-a-deployed-build-rel-qa-check-3)
with live-data assertions and separately configured authentication. Running that
mode checks the deployed application's data and workflows; it does not turn every
browser action into a direct provider API test.

## Database setup and isolation

Use real, disposable PostgreSQL databases for database-backed tests. The
[database testing guide](../testing/DATABASE_TESTING.md) explains target selection,
setup, and troubleshooting; the
[core helpers](../../backend/crates/econ-graph-core/src/test_utils.rs) define the
actual lifecycle.

`TestContainer::new()` exposes its pool through `pool()` but does not apply
migrations or create fixtures. In core unit tests it starts a PostgreSQL 18
container even when `DATABASE_URL` supplies an external pool, so Docker is still
required. When core is compiled as a dependency, including for other crates and
integration-test targets, it connects to `DATABASE_URL`, falling back to
`postgres://localhost/econ_graph_test`, without starting a container.

`get_test_db()` and the compatibility function `db_test()` return a shared
`Arc<TestContainer>` from a `OnceCell` per test binary. Sharing the helper does not
give each test its own database or transaction. Follow the target module's migration,
fixture, and locking convention. For example, the dataset tests apply migrations
once and create a unique data source per test without dropping the schema.

`clean_database()` must be called explicitly and its `Result` checked. It drops the
pool connection's entire `public` schema with `CASCADE`, recreates the schema, and
runs migrations. The migration URL comes from `DATABASE_URL` or the localhost
fallback, rather than from the pool's mapped container port. Set `DATABASE_URL` to
the disposable database used by the pool before using this cleanup path so both
operations target the same database. The
[queue service tests](../../backend/crates/econ-graph-services/src/services/queue_service.rs)
show explicit cleanup followed by fixture creation, with `#[serial]` on tests that
share the database.

Coordinate destructive cleanup with every test using the same database. A module's
lock or `serial_test` group does not protect unrelated code that does not acquire
it. `--test-threads=1` serializes functions within a test binary; it does not
coordinate separate Cargo processes. Cleanup is neither an automatic after-test
hook nor a rollback transaction.

For tests needing an empty database or separate app and data databases,
`ScratchDatabase::create(tag)` creates a uniquely named database on the
`DATABASE_URL` server, using the same localhost fallback. The server role needs
permission to create databases. The helper exposes `url` and `connect()`; tests
arrange migrations and fixtures themselves. Its drop handler attempts to drop the
database with `FORCE` and ignores cleanup errors, so scope exit is best-effort
cleanup. The
[migration tests](../../backend/crates/econ-graph-core/src/database.rs) demonstrate
using separate scratch databases to check the two migration sets.

## Writing reproducible assertions

Name tests for the behavior, scenario, and expected outcome. A name such as
`test_user_authentication_invalid_password_returns_error` is more useful than a
numbered case. Keep setup, the operation under test, and assertions visible; explain
unusual prerequisites or fixture choices in comments rather than documenting every
line of familiar test code.

Use the smallest fixture that establishes the behavior, adding realistic data when
its structure matters. For release specs, follow the
[area contribution guidance](../../frontend/tests/e2e/release/README.md#add-specs-for-your-area)
to extend recorded responses, the seed manifest, and shared expectations. Tests
that write rows need independent identities unless they deliberately share state
within a serial group. Control time when testing timing logic, and seed random
generators when random input is needed so failures can be reproduced.

Assert the observable result, including errors and recovery. A request returning
HTTP success is insufficient when its body can contain GraphQL errors or missing
data. Cover rejected input and unavailable dependencies at the layer that owns the
response. A selected dependency check should fail clearly when its prerequisite
is missing; returning success early would hide the missing verification. Rust
`#[ignore]` requires explicit selection with `--ignored`, regardless of whether
the test runs locally or in CI.

The existing [crawler soak target](../../backend/crates/econ-graph-crawler/tests/soak.rs)
checks queue completion, duplicate claims, retry spacing, and request-rate limits
against fixture-serving mock upstreams with injected failures. Its ignored soak
case requires a dedicated disposable PostgreSQL database and deletes every
`crawl_queue` row. Its source documents explicit invocation and policy overrides;
it is separate from the default tests and does not measure production throughput.

## Running checks and interpreting CI

Run Rust commands from `backend/`. Start with the relevant existing module or
integration-test target; a name filter selects tests but does not arrange their
services, migrations, or fixtures. Database-backed targets need reachable
disposable PostgreSQL, container-backed targets need Docker, and schema validation
also invokes the Diesel CLI. See the
[database test commands](../testing/DATABASE_TESTING.md#running-tests) for concrete
package and target examples.

[Core CI](../../.github/workflows/ci-core.yml) runs workspace library, binary,
integration, and documentation tests, plus feature-specific checks such as the
SEC crawler's `xbrl-parser` tests. The workspace's database-backed test invocation
uses `--test-threads=1` because tests can drop and recreate the shared schema.
Other database groups receive separate databases. The
[CI/CD guide](../development/CI_CD_PIPELINE.md) explains job selection and required
checks; use the workflow's service setup and feature flags when reproducing a CI
failure.

The [Release E2E workflow](../../.github/workflows/release-e2e.yml) runs the
fixture-backed browser suite for matching pull-request paths and also supports
GitHub's **Run workflow** button. Its
[runner](../../scripts/release-e2e.sh) owns the local build, database, Keycloak,
and seed setup. A passing isolated test, workspace test, or browser spec provides
evidence for its own boundary; review the selected jobs and assertions before
using the result to assess a broader change.
