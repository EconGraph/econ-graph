# GraphQL Response Mocks

These JSON fixtures provide reusable response bodies for frontend tests and mock
handlers. Keeping the data in separate files makes scenarios easy to inspect and
update.

## Directory Structure

Fixture directories use a local snake_case naming convention. Handlers map
GraphQL operation names such as `GetFinancialRatios` to fixture keys such as
`get_financial_ratios`. The collection includes financial, series, search,
annotation, and collaboration fixtures. Financial examples include:

- `get_financial_dashboard/` - Company financial dashboard data
- `get_financial_statement/` - Individual financial statement data
- `get_financial_ratios/` - Financial ratio calculations
- `get_ratio_benchmarks/` - Industry benchmark data
- `get_ratio_explanation/` - Ratio explanation and educational content

A file on disk is available through
[`loadGraphQLResponse`](../graphql-response-loader.ts) only when it has a static
import and an entry in that loader's `RESPONSE_MAP`. Supported scenarios vary by
operation; `getAvailableScenarios(operation)` returns the registered scenario
keys. An unregistered operation or scenario returns an error body with code
`MOCK_NOT_FOUND`.

## Response Types

Scenario filenames describe the response bodies. Inspect the fixture before
using it; operations do not all provide the same scenarios. Examples include:

- `success.json` - Populated response data
- `partial.json` or `partial_data.json` - Some missing or incomplete data
- `empty.json` - A successful response with empty data
- `not_found.json` - An error body for a missing resource
- `error.json` - An error body for a server or processing failure

The `loading.json` bodies also differ by operation: the dashboard contains a
company with empty collections, the statement contains a null statement and a
top-level `loading` flag, and ratios contain an error with code `LOADING`. Loading
one of these bodies does not keep a request pending. To test the UI while a
request is in flight, configure the handler's response timing separately.

## Usage in Tests

The loader is synchronous and accepts extensionless scenario keys. From a module
in this directory, for example:

```typescript
import {
  getAvailableScenarios,
  loadGraphQLResponse,
} from '../graphql-response-loader';

const ratioScenarios = getAvailableScenarios('get_financial_ratios');
// ['success', 'error', 'empty', 'loading']

const dashboardLoadingResponse = loadGraphQLResponse(
  'get_financial_dashboard',
  'loading'
);
const ratiosErrorResponse = loadGraphQLResponse('get_financial_ratios', 'error');
const statementSuccessResponse = loadGraphQLResponse(
  'get_financial_statement',
  'success'
);
```

These calls load response objects. To serve a fixture to a component test,
configure the relevant handler to select its registered scenario and return the
object with `HttpResponse.json(response)`. The
[ratio handlers](../graphql/ratio-mocks.ts) show selection based on query
variables; adjust the import path to the loader for the location of your test or
handler.

The [fixture validation tests](../../__tests__/graphql-schema-validation.test.ts)
check selected field paths, data types, array shapes, and error structures. The
loader itself performs no schema validation, and these tests do not establish
that every fixture conforms to the backend schema.

## Adding New Responses

1. Create a JSON file in the appropriate operation directory, using a scenario
   name such as `success`, `error`, or `not_found`.
2. Match the response shape consumed by the query and component under test.
3. Add a static import and an operation/scenario entry in
   [`RESPONSE_MAP`](../graphql-response-loader.ts). The scenario key omits `.json`.
4. Configure the relevant handler or test to select that scenario. Follow the
   [ratio handler examples](../graphql/ratio-mocks.ts) when using MSW.
5. Add a test for the new scenario, checking the intended response and UI behavior.
