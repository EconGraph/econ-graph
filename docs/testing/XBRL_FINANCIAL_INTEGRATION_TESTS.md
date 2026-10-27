# XBRL Financial Statement Integration Tests

Use this guide to choose and run the SEC crawler's XBRL integration target and the
financial UI integration tests. The Rust target checks database persistence and
parser construction. The React tests check rendering with mocked financial data.
Together, these suites do not establish an end-to-end flow from SEC EDGAR crawling
to parsed statements or calculated ratios.

## Backend test map

The [SEC crawler integration target](../../backend/crates/econ-graph-sec-crawler/tests/xbrl_financial_integration_tests.rs)
contains five tests. Its [Cargo manifest](../../backend/crates/econ-graph-sec-crawler/Cargo.toml)
requires the `xbrl-parser` feature to select this target. DTS means Discoverable
Taxonomy Set.

| Test | What its assertions establish |
| --- | --- |
| `test_xbrl_taxonomy_schema_storage` | Inserts a taxonomy schema, checks its namespace, filename, source type, byte content storage mode, and matching byte length, then retrieves it and checks its ID and namespace. |
| `test_financial_statement_processing` | Inserts a company, a statement, and two line items. Checks the inserted and retrieved item counts and the ordered concepts `Assets` and `Liabilities`. |
| `test_xbrl_parser_integration` | Checks that `XbrlParser::new()` succeeds. It does not parse an instance document. |
| `test_dts_dependency_management` | Inserts US-GAAP and SEC-DEI schemas and checks their namespaces. It does not create or resolve a dependency between them. |
| `test_xbrl_instance_dts_reference_tracking` | Inserts a company, statement, and schema, then checks the statement's company ID and the schema's namespace. It does not create or validate a statement-to-schema reference. |

The statement fixtures are supplied directly to Diesel. This target does not
extract facts, generate statements, validate the balance-sheet equation, calculate
ratios, or crawl SEC EDGAR.

## Frontend test map

The [frontend integration file](../../frontend/src/__tests__/integration/xbrl-financial-integration.test.tsx)
renders financial components with React Query and router providers. It installs
MSW handlers and directly mocks `executeGraphQL` to return canned GraphQL
responses; chart rendering is also mocked by the
[integration setup](../../frontend/src/setupTests.integration.ts).

| Suite | Observable assertions |
| --- | --- |
| Financial Dashboard Integration | Company name, ticker, and industry appear. Ratio values include ROE `14.7%` and current ratio `104.0%`. Benchmark content includes “Industry Benchmark Analysis” and “Above Average.” |
| Financial Statement Viewer Integration | Balance-sheet and income-statement labels and formatted values appear, including Total Assets, Current Assets, Total Liabilities, Stockholders' Equity, Net Sales, and Net Income. The hierarchy-named test checks labels and values, without asserting parent-child layout. |
| Benchmark Comparison Integration | Benchmark labels, the 75th-percentile ranking, “Above Average,” and supplied P10, P25, median, P75, and P90 values appear. |
| Trend Analysis Integration | The trend heading, “Trend Visualization” heading, and direction and strength labels appear. |
| XBRL Data Processing Integration | Company and industry content or asset labels appear. These assertions do not verify processing status transitions or taxonomy reference resolution. |
| Error Handling Integration | The viewer heading or company name appears. These assertions do not verify parsing-error recovery or missing-taxonomy handling. |
| Performance Integration | Rendering until “Total Assets” appears takes less than two seconds. |

Several tests configure a local `mockFetch` stub, including the status, error, and
performance cases. That stub is not assigned to `global.fetch`, while the GraphQL
mock continues to return its normal fixtures. In particular, the performance case
creates 1,000 line items on the unattached stub. Its elapsed-time assertion is a
fixture-render check, not a benchmark of processing or rendering 1,000 items.
These tests also do not measure memory use or database query efficiency.

### Recognizing the mock data

The frontend file includes this company fixture:

```typescript
const mockCompany = {
  id: 'test-company-1',
  cik: '0000320193',
  name: 'Apple Inc.',
  ticker: 'AAPL',
  industry: 'Computer Hardware',
  sector: 'Technology'
};
```

The dashboard's GraphQL fixture supplies the displayed industry “Technology
Hardware & Equipment.” The local company object above belongs to the separate
`mockFetch` setup, so changing it alone does not change the dashboard response.

The statement display checks expect Total Assets of `$352.76B`, Current Assets of
`$143.57B`, Total Liabilities of `$258.55B`, Stockholders' Equity of `$94.21B`, and
Net Sales of `$383.29B`. The local ratio fixtures include ROE `0.147` (14.7%) and
net profit margin `0.253` (25.3%). These are supplied values, not evidence that the
suite calculates ratios from XBRL input. The benchmark props supply P10 `0.08`,
P25 `0.10`, median `0.12`, P75 `0.15`, and P90 `0.18`.

## Prerequisites

### Backend

Use the repository's Rust toolchain and an already reachable, disposable
PostgreSQL database. Set `DATABASE_URL` explicitly to that database. The database
role must be able to drop and recreate its `public` schema and run migrations.

In this target, core is compiled as a dependency, so `TestContainer::new()` connects
to `DATABASE_URL`; it does not start a container or create a database. Its fallback
is `postgres://localhost/econ_graph_test`. Each of the four database-backed tests
calls `clean_database()`, which **drops the entire `public` schema with `CASCADE`,
recreates it, and runs migrations**. The parser-construction test does not call
this helper. See [TestContainer lifecycle](DATABASE_TESTING.md#testcontainer-lifecycle)
and [cleaning and migration targets](DATABASE_TESTING.md#cleaning-and-migration-targets).

Run the target serially, and do not run other tests against the same database at
the same time. `--test-threads=1` only serializes tests within a test binary; it
does not coordinate separate Cargo processes. The helper applies migrations, so
these commands do not require a separate Diesel CLI migration step.

### Frontend

Use Node.js 26, as specified by [`.nvmrc`](../../.nvmrc), and npm. Install the
frontend dependencies with `npm ci` from `frontend/`. This suite supplies mocked
GraphQL data and does not require a running backend API or a database. Select the
[integration Vitest configuration](../../frontend/vitest.integration.config.ts);
the default configuration excludes this TSX integration file.

## Running the tests

### Backend target

Run this block from `backend/`. Replace the example URL with your disposable test
database before executing the tests; the schema cleanup described above applies.

```bash
export DATABASE_URL='postgres://localhost/econ_graph_test'
cargo test -p econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests -- --nocapture --test-threads=1
```

To select one test, run this command from `backend/`, with `DATABASE_URL` already
set to the same disposable database:

```bash
cargo test -p econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests test_xbrl_taxonomy_schema_storage -- --nocapture --test-threads=1
```

### Frontend file

Run this command from `frontend/`, after installing dependencies:

```bash
npm run test:integration -- src/__tests__/integration/xbrl-financial-integration.test.tsx
```

To select the dashboard suite, run this command from `frontend/`:

```bash
npm run test:integration -- src/__tests__/integration/xbrl-financial-integration.test.tsx -t 'Financial Dashboard Integration'
```

The [`test:integration` script](../../frontend/package.json) selects
`vitest.integration.config.ts`. A test-name filter alone does not override the
integration-file exclusions in the [default Vitest configuration](../../frontend/vitest.config.ts).

## Continuous integration

[Core CI](../../.github/workflows/ci-core.yml) runs the SEC crawler's tests with
`--features xbrl-parser --no-fail-fast --tests -- --test-threads=1` in its workspace
test step, which selects this backend integration target. Its database setup is
part of the workflow; the local commands above require an existing database.

The job named “Frontend Integration Tests” runs `npm run test:coverage`. That
script uses the default Vitest configuration, which excludes
`**/__tests__/integration/**/*.test.tsx`. A passing result from that job therefore
does not establish that this frontend file ran. Use the explicit integration
command above to select it locally.

## Troubleshooting

For database connection or cleanup failures, check that `DATABASE_URL` names the
intended disposable database, PostgreSQL is reachable, and the role can recreate
its schema and apply migrations. Check for another test process using the same
database if fixtures disappear or tables change during a run.

Parser initialization failures belong to `XbrlParser::new()`; this target does
not diagnose XML parsing or DTS resolution from an instance document.

For frontend failures, first confirm that the integration configuration selected
the file. Then check the mocked GraphQL operation names, fixture responses, and
component props against the failing assertion. Changes to the unattached
`mockFetch` stub do not change `executeGraphQL` responses.

The backend commands already include `--nocapture`. For verbose frontend output,
run this command from `frontend/`:

```bash
npm run test:integration -- src/__tests__/integration/xbrl-financial-integration.test.tsx --reporter=verbose
```
