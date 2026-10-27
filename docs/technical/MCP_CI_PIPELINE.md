# MCP CI pipeline

[Core CI](../../.github/workflows/ci-core.yml) checks MCP through metadata smoke tests, database-backed Rust tests, and a separate Chart API suite. The dedicated MCP job also starts and health-checks the backend and Chart API, but its Rust tests call MCP methods in process. A passing job therefore does not establish an authenticated HTTP MCP request followed by successful chart generation.

## When the checks run

Core CI runs for pull requests, pushes to `main`, `develop`, `optimize/ci-core-primary` and `release/**`, and workflow dispatch. Changes confined to its ignored paths, including Markdown and `docs/**`, do not trigger push or PR runs. Within a run, `changes` selects the backend build and its dependent tests unless the change is frontend-only. Pushes to `main`, `develop` and `release/**` bypass that path filter. See the [CI pipeline guide](../development/CI_CD_PIPELINE.md#triggers-and-selection) for the full selection rules.

The MCP-related dependencies are:

```text
changes
  └─ backend-build-cache
       ├─ backend-smoke-tests
       │    └─ chart-api-integration-tests
       │         └─ backend-mcp-integration-tests
       └─ backend-workspace-tests
```

`backend-mcp-integration-tests` also directly requires `backend-smoke-tests`. The build-cache job compiles backend and test targets and saves the shared Rust cache; dependent test jobs restore it. Compilation in the cache job does not execute tests. A skipped or failed prerequisite can prevent downstream checks from running, so inspect dependency results before treating a missing test result as a test failure.

Browser E2E is a separate dependency chain. In Core CI, `e2e-comprehensive-tests` requires `backend-smoke-tests`, `frontend-integration-tests`, `backend-integration-tests` and `e2e-container-build-optimized`; it does not depend on Chart API tests. It runs only when workflow dispatch enables `run_e2e_tests` and selects `all` or `comprehensive` (the condition also accepts an empty suite value). The container build is opt-in too. These browser tests do not run alongside MCP tests in every ordinary PR run.

## What each job checks

| Job | MCP-relevant scope | Job timeout |
| --- | --- | --- |
| `backend-smoke-tests` | Formatting, linting, rustdoc, selected database-free tests, and release flag checks | 60 minutes |
| `chart-api-integration-tests` | Chart service lint, build script, startup health check, and Jest coverage run | 90 minutes |
| `backend-mcp-integration-tests` | Service startup checks and the `mcp_server_integration_tests` Rust target | 60 minutes |
| `backend-workspace-tests` | Workspace library, binary, integration and documentation tests, including MCP | 120 minutes |

These are configured timeout limits, not measured runtimes. Use the selected run's step durations to distinguish compilation, service startup and test execution costs.

### Metadata smoke and library tests

The smoke job explicitly selects `mcp_server::tests::test_get_available_tools` and `mcp_server::tests::test_get_available_resources`, alongside other fast test filters. It unsets database URL variables. Those two MCP tests check advertised tool names and resource URIs without constructing a database-backed server; they do not call the tools or test HTTP endpoints.

The broader [MCP library tests](../../backend/crates/econ-graph-mcp/src/mcp_server.rs) include server construction, tool argument handling, custom limits and date filters, resource queries, and Warp handler requests for tools, resources, malformed JSON and unknown methods. These tests use the database test helper. The handler tests exercise the MCP handler inside the test process; they do not replace tests of the backend's authenticated `/mcp` route.

The workspace job runs library, binary and integration tests with `--test-threads=1`, then runs doctests separately. Its broad test steps propagate failures. This job includes the MCP library group; the separate `backend-db-tests` job selects other library groups and does not select MCP. Keep the workspace job in view when investigating MCP failures: the smoke selection is only a small subset of the library suite.

Smoke also checks `FLAGS_PROFILE=release`, where the backend's `/mcp` route is compiled out. The ordinary local flag profile is `dev`, where the route is enabled. The [build flag configuration](../../config/flags/README.md) explains this distinction; Cargo's `--release` optimization setting alone does not select the release flag profile.

### Chart API checks

The chart job runs `npm ci`, linting, the build script and `npm run test:coverage` in `chart-api-service/`. It starts `npm start` on port 3001 and requires `GET /health` to succeed before continuing. The Node service's build script currently performs no compilation.

The [core chart tests](../../chart-api-service/tests/chartApi.test.js) check configuration generation for line, bar and scatter charts, datasets, options, metadata and invalid input. The [server tests](../../chart-api-service/tests/server.test.js) use Supertest to exercise HTTP responses, including `POST /api/chart/generate`. These validate chart configuration generation, not browser rendering. Inspect Jest's coverage output for statement, branch, function and line coverage from that run; an aggregate percentage is not a measure of MCP endpoint coverage.

The workflow's additional chart-generation curl probe still uses `/api/chart` and the old `data` payload, then tolerates failure with `|| echo`. It cannot establish that the actual generation endpoint works. The local diagnostic below uses the service's current route and request shape.

### Dedicated MCP integration target

The MCP job first prepares PostgreSQL, applying data plane migrations followed by app plane migrations. It starts the Chart API and builds the backend binary and MCP integration target together in debug mode. It then starts the backend with `APP_DATABASE_URL` and `DATA_DATABASE_URL` pointing to the same CI database and requires its `/health` check to pass before running the Rust target.

The [integration tests](../../backend/crates/econ-graph-mcp/tests/mcp_server_integration_tests.rs) construct `EconGraphMcpServer` directly. Each test starts its own PostgreSQL testcontainer, applies both planes' migrations and passes that database pool to the server. The tests do not use the backend listener or the job's fixed-port database for their MCP method calls.

The target checks database queries, invalid arguments, concurrent searches, resource access and visualization responses. For example, `mcp_queries_return_real_series_and_date_filtered_observations` inserts a series with observations, then asserts that a date-filtered query returns the expected series ID and date and that search and catalog responses contain the inserted series. These assertions establish more than a successful response wrapper.

Visualization tests have narrower guarantees. Several accept a fallback visualization when the Chart API call fails. `test_mcp_server_call_private_chart_api_success_integration` logs an unavailable service without failing the test, while the failure test accepts an error for invalid input. The job also configures `CHART_API_SERVICE_URL` with the obsolete `/api/chart` route. Health checks and these fallback-tolerant assertions therefore do not establish a successful MCP-to-Chart-API round trip.

## Runner, process and database isolation

Each job runs on its own `ubuntu-latest` runner. Reusing port 3001 in the chart and MCP jobs does not connect their services or cause a cross-job collision. Within the MCP job, PostgreSQL is a service container; the Chart API and backend are background processes on the runner.

| Environment | Port | Purpose |
| --- | --- | --- |
| Chart API job | 3001 | Chart service process and health probe |
| MCP job | 5445 → container 5432 | PostgreSQL for CI migrations and the started backend |
| MCP job | 9877 | Started backend and its health probe |
| MCP job | 3001 | A separately started Chart API process |
| MCP integration test helper | Dynamically mapped PostgreSQL port | Disposable database for each test's in-process MCP server |
| Workspace job | 5446 → container 5432 | Job-level PostgreSQL used by tests that read its database URL |

The integration helper always creates its own container even when `DATABASE_URL` is set. Other library tests use the shared database helper and may use a configured disposable database. See [database testing](../testing/DATABASE_TESTING.md) before changing local database settings.

## Run focused checks locally

Run each command from the repository root. Use the repository Rust toolchain and Node version in [`.nvmrc`](../../.nvmrc). These commands reproduce selected checks rather than the entire CI environment.

For the two MCP metadata checks selected by smoke, no database or service startup is needed:

```bash
(
  cd backend || exit 1
  unset DATABASE_URL APP_DATABASE_URL DATA_DATABASE_URL
  cargo test -p econ-graph-mcp --lib -- \
    mcp_server::tests::test_get_available_tools \
    mcp_server::tests::test_get_available_resources
)
```

For the Chart API suite, install its locked dependencies and run the same lint and coverage scripts as CI. The Jest server tests manage their HTTP requests through Supertest; they do not require a separately started backend or PostgreSQL.

```bash
(
  cd chart-api-service || exit 1
  npm ci && npm run lint && npm run test:coverage
)
```

The dedicated Rust integration target requires a working Docker daemon and access to its PostgreSQL image. Its helper creates and migrates disposable databases. It can exercise fallback behavior without a running Chart API; starting that service does not strengthen the assertions that tolerate failures.

```bash
(
  cd backend || exit 1
  cargo test -p econ-graph-mcp --test mcp_server_integration_tests -- --nocapture
)
```

To investigate the broader MCP library group, use `cargo test -p econ-graph-mcp --lib` from `backend/` with the disposable database prerequisites in the database testing guide. Do not treat that command as database-free smoke testing. Root Compose is not a prerequisite for the standalone integration helper and does not reproduce the MCP job's service layout.

## Diagnose a failure

Start with the first failed step in the affected job. Use the checked-in [workflow](../../.github/workflows/ci-core.yml) for the exact environment and startup commands rather than copying a partial workflow into a local file. Job timeouts, process startup waits and test execution are separate failure points; increasing a timeout does not repair a failed migration or wrong endpoint.

For a skipped job, trace the prerequisites and selection rules above. For a test failure, read the assertion and test name before deciding whether it concerns metadata, database contents, handler behavior, authentication or Chart API availability. To compare runs, inspect step durations and logs in Actions; the MCP jobs do not provide a general automatic retry mechanism.

### Service startup and port conflicts

On the CI runner or a local environment configured with the same ports, inspect listeners and probe the services:

```bash
lsof -i :3001
lsof -i :5445
lsof -i :9877
curl --fail --show-error http://localhost:3001/health
curl --fail --show-error http://localhost:9877/health
```

Read the workflow step output for the background Node/backend processes. Use `docker logs CONTAINER_ID` for a PostgreSQL container after identifying the relevant container; Node and backend output is not in that container's logs. A failure in a testcontainer database needs its dynamically mapped port, not an assumption that the test used 5445.

For database errors, confirm which test helper supplied the connection and whether both migration sets were applied. The CI backend uses one database for both planes. A bare `diesel migration run` defaults to app plane migrations and is insufficient for that setup. Follow [applying both sets to one database](../development/MIGRATIONS.md#applying-both-sets-to-one-database) against an explicitly selected disposable database.

### Chart generation diagnostic

For a locally started Chart API on port 3001, this request checks the current generation contract:

```bash
curl --fail --show-error http://127.0.0.1:3001/api/chart/generate \
  -H 'Content-Type: application/json' \
  -d '{"chartType":"line","title":"Test Chart","seriesData":[{"id":"test-series","name":"Test Series","dataPoints":[{"date":"2023-01-01","value":100},{"date":"2023-02-01","value":105}]}]}'
```

A successful response contains `success: true`, `chartConfig`, `chartData` and metadata. This checks the Chart API independently; it does not test MCP transport or browser rendering.

### MCP HTTP diagnostic prerequisites

Use the actual backend address and port for the environment being investigated; 9877 is the dedicated CI job's backend port. `/mcp` must be compiled in, and a caller must present a valid bearer token for an active account holding `api:mcp`. A successful `/health` probe does not establish those prerequisites. See the [MCP API guide](MCP_SERVER.md#api-endpoints) for requests and the [route implementation](../../backend/crates/econ-graph-backend/src/mcp_routes.rs) for access errors. For a release flag build, route absence is expected and is checked separately by smoke.
