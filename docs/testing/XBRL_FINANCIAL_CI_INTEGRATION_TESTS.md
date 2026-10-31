# XBRL Financial CI Integration Tests

The XBRL financial tests cover backend record persistence and parser initialization,
and frontend components rendered with mocked financial data. They do not exercise
SEC EDGAR crawling or parse a filing through to the UI. This guide explains what
the suites assert, how to select them locally, and which routes CI uses.

## Backend Integration Tests

The [backend test target](../../backend/crates/econ-graph-sec-crawler/tests/xbrl_financial_integration_tests.rs)
is `xbrl_financial_integration_tests` in `econ-graph-sec-crawler`. It requires the
`xbrl-parser` feature.

### Database Prerequisites

Provide a reachable, disposable PostgreSQL server before running this target, with a
role that can create databases. The tests use `TestContainer`, which gives each test a
migrated scratch database of its own on the server `DATABASE_URL` names (falling back
to `postgres://localhost/econ_graph_test`); it does not start a container. See the
[TestContainer lifecycle](DATABASE_TESTING.md#testcontainer-lifecycle).

The four persistence tests call `clean_database()`, which **drops the entire
`public` schema with `CASCADE`**, recreates it, and runs migrations. Set
`DATABASE_URL` explicitly to the disposable database and keep other test binaries,
Cargo processes, and applications away from that database while this suite runs.
All five tests have `#[serial]`, which coordinates these tests within the binary;
it does not protect the database from other processes. See
[cleaning and migration targets](DATABASE_TESTING.md#cleaning-and-migration-targets).

### Assertions and Fixtures

The target contains five tests:

| Test | What it checks |
| --- | --- |
| `test_xbrl_taxonomy_schema_storage` | Inserts a taxonomy schema, checks its namespace, filename, source type and byte storage, then retrieves it by ID. |
| `test_financial_statement_processing` | Inserts a company, statement and two line items; checks that Assets and Liabilities are inserted and retrieved in order. |
| `test_xbrl_parser_integration` | Checks that `XbrlParser::new()` returns successfully. It does not parse a document. |
| `test_dts_dependency_management` | Inserts US-GAAP and SEC-DEI schema records and checks their namespaces. It does not resolve dependencies. |
| `test_xbrl_instance_dts_reference_tracking` | Inserts company, statement and schema records; checks the statement's company ID and schema namespace. It does not assert a statement-to-schema reference. |

These tests construct small records directly. Schema fixtures contain minimal
schema bytes; filing URLs and `10-K` metadata are record fields, not downloaded
filings. The target does not test crawling, filing parsing, or statement generation
from XBRL facts.

### Running and Debugging

From the repository root, enter the Cargo workspace. First set `DATABASE_URL` in
your shell to the disposable database described above.

```bash
cd backend
cargo test --package econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests

# Show test output.
cargo test --package econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests -- --nocapture

# Focus on one test with debug logging.
RUST_LOG=debug cargo test --package econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests test_xbrl_taxonomy_schema_storage -- --nocapture
```

From `backend/`, `./scripts/run-tests-optimized.sh` also selects this target with
`xbrl-parser` in its full mode, along with other backend suites. Its quick mode
skips integration tests; use the explicit Cargo command above to run only this
target.

## Frontend Integration Tests

The [frontend test file](../../frontend/src/__tests__/integration/xbrl-financial-integration.test.tsx)
uses Vitest and React Testing Library in jsdom. The
[integration configuration](../../frontend/vitest.integration.config.ts) selects
integration `.test.tsx` files and loads
[the integration setup](../../frontend/src/setupTests.integration.ts).

The suite renders financial components with React Query and routing providers.
It defines MSW handlers and an `executeGraphQL` mock for financial statements,
ratios and benchmarks. Some scenarios also configure a `mockFetch` function, but
the assignment to `global.fetch` is commented out in the file. The suite does not
require a live backend or a browser session. Successful assertions would establish
component behavior with mock responses rather than backend API compatibility.

### Test Categories

The file's describe-block names provide navigation; their assertions cover the
following behavior:

- **Financial Dashboard Integration** checks company data, ratios and benchmark comparisons.
- **Financial Statement Viewer Integration** checks balance sheets, income statements and hierarchical line items.
- **Benchmark Comparison Integration** checks industry benchmarks and distribution percentiles.
- **Trend Analysis Integration** checks ratio trends, direction and strength.
- **XBRL Data Processing Integration** checks company and industry text in one case,
  and Total Assets and Current Assets labels in another. It does not assert
  processing-status or taxonomy-reference displays.
- **Error Handling Integration** checks the Financial Statements heading or Apple
  Inc. text. It does not assert parsing-error or missing-schema displays.
- **Performance Integration** times rendering until Total Assets appears and
  asserts a duration below two seconds.

The processing, taxonomy and error scenarios configure responses on the
disconnected `mockFetch` function described above. The performance case also puts
its 1,000-row dataset on that unused mock, so its timing assertion does not
establish large-dataset rendering performance. Passing these cases would not
verify the scenario-specific behavior suggested by their names. Separately, the
frontend assertion failures and missing CI selection are tracked in
[ECO-442](https://linear.app/econgraph/issue/ECO-442).

### Running and Debugging

With frontend dependencies installed, run these commands from the repository root:

```bash
cd frontend
# Select the XBRL component suite using the integration configuration.
npm run test:integration -- src/__tests__/integration/xbrl-financial-integration.test.tsx

# Select all component integration tests.
npm run test:integration

# Focus on a describe block and show individual results.
npm run test:integration -- src/__tests__/integration/xbrl-financial-integration.test.tsx -t "Financial Dashboard Integration" --reporter=verbose
```

Use the integration configuration explicitly. The dedicated
`test:xbrl-financial:integration` npm script uses the default Vitest configuration,
which excludes integration `.test.tsx` files, so that alias does not run this suite.
The separate [Playwright configuration](../../frontend/ci/configs/xbrl-financial-integration.config.ts)
uses a different runner and resolves its test directory under `frontend/ci/src/`,
where this suite does not live. It is not an alternative entry point for these
jsdom tests.

## CI and the Compose Helper

### Hosted CI

The [core CI workflow](../../.github/workflows/ci-core.yml) runs backend workspace
tests and then explicitly runs SEC crawler tests with `xbrl-parser` enabled. Its
feature-enabled command, from `backend/`, includes this integration target:

```bash
cargo test -p econ-graph-sec-crawler --features xbrl-parser --no-fail-fast --tests
```

The workflow's **Frontend Integration Tests** job runs `npm run test:coverage`.
That script uses the default Vitest configuration, whose integration-file exclusion
also applies here. A green frontend coverage job therefore does not validate the
XBRL component suite.

### Manual Compose Execution and Debugging

The [Compose configuration](../../ci/docker/docker-compose.best-practice.yml)
defines a `test-xbrl-financial` service with frontend and backend dependencies.
The [test wrapper](../../ci/scripts/run-tests.sh) selects that service with the
`xbrl-financial` group or includes it in `all`. This is a separate route from the
hosted workflow. The service invokes `npm run test:xbrl-financial:integration`, so
it has the same frontend selection gap described above; invoking the helper does
not establish that the XBRL suite ran.

For inspecting this Compose setup, the wrapper accepts the following commands
from the repository root. They start services and require Docker and disposable
test infrastructure.

```bash
./ci/scripts/run-tests.sh --group xbrl-financial

# Retain containers after the run for inspection.
./ci/scripts/run-tests.sh --group xbrl-financial --no-cleanup

# Run every group supported by the wrapper.
./ci/scripts/run-tests.sh --group all
```

Use test output to confirm that the intended file was selected and its assertions
passed. Neither the presence of a Compose service nor a passing coverage job
substitutes for running the component suite through its integration configuration.
