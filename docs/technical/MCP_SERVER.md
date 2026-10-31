# EconGraph MCP Server

## Overview

The EconGraph server exposes Model Context Protocol (MCP) tools and resources that let AI assistants search for economic series, retrieve their data, and request visualizations. An assistant can find indicators for economic research, compare metrics for a financial report, help students explore economic concepts, or supply economic data to a business analytics workflow.

The interface is an authenticated HTTP JSON-RPC endpoint with four supported methods. It does not implement the MCP initialization lifecycle: `initialize` receives Method not found. A client must support the direct HTTP requests described here; this guide does not establish compatibility with desktop MCP clients that require initialization or a persistent transport.

| Operation | Tool or resource | What it provides |
| --- | --- | --- |
| Find indicators | `search_economic_series` | Matching series and descriptive metadata |
| Retrieve observations | `get_series_data` | A bounded date-ordered set of values for one series |
| Understand a series | `get_series_metadata` | Frequency, units, source, and timestamps |
| Prepare a chart | `create_data_visualization` | Chart configuration or fallback data for caller-side rendering |
| Browse sources and series | `resources/read` | Source information or a bounded series catalog |

## Endpoint and access

Send `POST http://localhost:9876/mcp` with `Content-Type: application/json` and `Authorization: Bearer <access-token>`. The account must be active, and its access token must grant the `api:mcp` role. The backend needs `OIDC_ISSUER` and `OIDC_AUDIENCE` configured to verify tokens.

The `/mcp` route is compiled only when the `mcp` build flag is enabled. The `dev` flag profile enables it; `release` disables it. A build without the route answers 404. See [local startup](#running-the-server) and [build flags](../../config/flags/README.md).

Requests may contain at most 1 MiB of body data. Authentication is checked before the body is read. Use a separate request for each operation and inspect both the HTTP status and the JSON-RPC `error` field: HTTP 200 alone does not establish success.

For a direct HTTP caller with an access token already available in `ACCESS_TOKEN`:

```bash
curl -X POST http://localhost:9876/mcp \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

### Supported methods

`tools/list` returns `result.tools`, including each tool's `name`, `description`, and `input_schema`. `resources/list` returns `result.resources`, including `uri`, `name`, `description`, and `mime_type`. These field names reflect the endpoint's actual response spelling.

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}
```

Use `tools/call` to run a named tool:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "search_economic_series",
    "arguments": {
      "query": "GDP",
      "limit": 5
    }
  }
}
```

Discover resource URIs with `resources/list`:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resources/list"
}
```

Then read a resource with `resources/read`:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "resources/read",
  "params": {
    "uri": "econ-graph://data-sources"
  }
}
```

### Reading results

Successful tool calls return `result.content`, an array of text blocks, and `result.is_error: false`. For search, data, and metadata tools, parse the JSON string in `result.content[0].text` to obtain the payload described below. For example, a search with no matches has this response:

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"data\":{\"searchSeries\":{\"series\":[],\"totalCount\":0}}}"
      }
    ],
    "is_error": false
  }
}
```

Resource reads instead return `result.contents`, also containing text blocks whose `text` holds JSON. Visualization text includes explanatory prose and embedded JSON sections; the whole text is not a JSON document.

## Tool reference

The examples in this section are the `params` objects for `tools/call`. Replace illustrative UUIDs with IDs from search or the catalog. In returned series objects, `name` means the series title.

### search_economic_series

Find series by name, description, or keywords, with spelling correction and synonym support. Results exclude inactive series and series without observations, and are ordered by relevance.

- `query` (string, required): Search text, from 1 to 500 characters.
- `limit` (integer, optional): Maximum results, from 1 to 1,000; default 10.

```json
{
  "name": "search_economic_series",
  "arguments": {
    "query": "unemployment rate",
    "limit": 10
  }
}
```

The parsed payload contains `data.searchSeries.series` and `data.searchSeries.totalCount`. Each series contains `id`, `name`, `description`, `frequency`, `units`, `source` (`name`, `description`), and `lastUpdated`. `totalCount` counts all matches, so it can exceed the returned array length. This tool exposes no continuation cursor or offset; a capped result is not a complete search export.

### get_series_data

Retrieve observations and basic metadata for one economic series.

- `series_id` (string, required): Series UUID.
- `start_date` and `end_date` (strings, optional): Inclusive bounds in `YYYY-MM-DD` format. An omitted bound leaves that side unrestricted.
- `limit` (integer, optional): Maximum observations, the latest ones; default 100, capped at 10,000. Zero returns no observations; negative values are rejected.

```json
{
  "name": "get_series_data",
  "arguments": {
    "series_id": "123e4567-e89b-12d3-a456-426614174000",
    "start_date": "2020-01-01",
    "end_date": "2023-12-31",
    "limit": 50
  }
}
```

The parsed payload contains `data.series` with `id`, `name`, `description`, `dataPoints`, `totalCount`, and `truncated`. Each point contains `date` and `value`; a missing value is `null`. Each date appears once, at its latest revision, as on the site's charts; pre-vintage synthetic rows are left out. Points are in date order.

When more dates match than `limit`, the tool returns the **latest** `limit` of them, sets `truncated` to `true`, and gives the number that matched in `totalCount`. Narrow the dates or raise `limit` (at most 10,000) for the rest. There is no `asOf`, original-release selector, transformation argument, or continuation cursor, and the revision date of each value is not included. Use the [GraphQL API](../api/GRAPHQL_API.md) when analysis requires those controls.

An unknown series, or a series without observations, produces a tool error. An existing series with no observations within the requested date range returns an empty `dataPoints` array.

### get_series_metadata

Read the descriptive metadata needed to interpret a series.

- `series_id` (string, required): Series UUID.

```json
{
  "name": "get_series_metadata",
  "arguments": {
    "series_id": "123e4567-e89b-12d3-a456-426614174000"
  }
}
```

The parsed payload contains `data.series` with `id`, `name`, `description`, `frequency`, `units`, `source` (`name`, `description`, `url`), `lastUpdated`, and `createdAt`. The source `url` is its base URL. Unlike the data tool, metadata returns `data.series: null` for an unknown series or one without observations.

### create_data_visualization

Prepare a chart comparing one or more series. The caller renders the returned chart configuration or builds a visualization from fallback data; this tool does not return an image.

- `series_ids` (array, required): Series UUID strings.
- `chart_type` (string, optional): `line`, `bar`, or `scatter`; default `line`.
- `title` (string, optional): Default `Economic Data Visualization`.
- `start_date` and `end_date` (strings, optional): Inclusive observation bounds in `YYYY-MM-DD` format.

```json
{
  "name": "create_data_visualization",
  "arguments": {
    "series_ids": [
      "123e4567-e89b-12d3-a456-426614174000",
      "987fcdeb-51a2-43d1-b789-123456789abc"
    ],
    "chart_type": "line",
    "title": "GDP vs Unemployment Rate",
    "start_date": "2020-01-01",
    "end_date": "2023-12-31"
  }
}
```

Each series contributes at most its latest 1,000 matching dates, with the same date selection and revision behavior as `get_series_data`, including `totalCount` and `truncated`. A missing series or a data-query failure prevents visualization; chart-service fallback does not cover those errors.

When the chart service returns a `chartConfig`, the tool's text contains a **Chart Configuration** JSON section, a **Metadata** JSON section, and a **Coverage** JSON section listing each series' `dataPoints` count, `totalCount`, and `truncated`. If the service request fails or its response lacks `chartConfig`, the tool instead returns text containing **Visualization Data**: `chart_type`, `title`, `series_data`, and `metadata` with `series_count`, `total_data_points`, and a fallback `note`. Each entry in `series_data` has the same fields as the data tool's `data.series`.

Both outcomes set `is_error: false`. Inspect the text to distinguish a generated configuration from fallback data; a successful tool call does not prove that the chart service is available.

Omitted tool dates do not restrict the fetched observations. Separately, the chart request defaults its `startDate` to `2020-01-01` and `endDate` to `2024-01-01` when those arguments are omitted. Supply explicit dates when chart request bounds must match the observation range you intend to analyze.

## Resource reference

### econ-graph://data-sources

The parsed JSON contains `data.dataSources`, with `id`, `name`, `description`, `url`, and `lastUpdated` for each source. `url` is the source's base URL; `lastUpdated` is the source record's update timestamp. Sources are listed by name, and only sources with at least one series that has observations are included.

### econ-graph://series-catalog

The parsed JSON contains `data.series.nodes`. Each node contains `id`, `name`, `description`, `frequency`, `units`, and `source.name`. This is the first 100 series in title order, with IDs breaking title ties. Inactive series and series without observations are excluded. The resource exposes no pagination argument, total count, or continuation cursor, so it is a bounded browsing aid rather than the complete catalog.

## Errors

Access and body failures use non-200 HTTP statuses. Their JSON-RPC response has `id: null`. Method and tool/resource errors use HTTP 200 and echo the request ID. For example:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32601,
    "message": "Method not found"
  }
}
```

| Condition | HTTP status | JSON-RPC error code |
| --- | --- | --- |
| Missing token, sign-in disabled, or invalid/expired token | 401 | `-32001` |
| Missing `api:mcp` role or suspended account | 403 | `-32003` |
| Sign-in temporarily unavailable | 503 | `-32002` |
| Signed-in account cannot be loaded | 500 | `-32603` |
| Body exceeds 1 MiB | 413 | `-32600` |
| Body cannot be read, or malformed JSON | 400 | `-32700` |
| Unsupported or missing method | 200 | `-32601` |
| Tool/resource failure, unknown tool/URI, or invalid tool arguments | 200 | `-32603` |

The handler does not emit `-32602` for invalid tool arguments. Data-query errors are tool/resource failures; a backend without a local data plane reports `DATA_SERVED_REMOTELY` through that error path.

Missing-token and disabled-sign-in responses include `WWW-Authenticate: Bearer`. Invalid-token responses use `Bearer error="invalid_token"`; missing-role responses use `Bearer error="insufficient_scope"`. Suspended-account, unavailable-sign-in, and account-loading failures do not include that header.

## Development

### Running the server

From the repository root, follow the [backend development setup](../../backend/README.md#development-workflow) to configure `APP_DATABASE_URL` and `DATA_DATABASE_URL` and apply data plane migrations. Set `OIDC_ISSUER` and `OIDC_AUDIENCE` for sign-in before starting the backend:

```bash
cd backend
FLAGS_PROFILE=dev cargo run -p econ-graph-backend --bin econ-graph-backend
```

At the default backend port, the endpoint is `http://localhost:9876/mcp`. Calling it requires an active account's access token granting `api:mcp`, including for tool and resource discovery.

For chart configuration generation, run the [Chart API service](../deployment/CHART_API_SERVICE.md) and set `CHART_API_SERVICE_URL` before backend startup. For a local service the base is `http://localhost:3001/api/chart`; the MCP caller appends `/generate`. The default base is `http://chart-api-service.econ-graph.svc.cluster.local:3001/api/chart`. If the service cannot supply a configuration, visualization calls with valid series data return the fallback described above.

### Architecture

The backend's Warp route enforces authentication and body limits before passing requests to the MCP crate. That crate manually dispatches the four supported JSON-RPC methods and uses the GraphQL schema for economic data reads. It uses an HTTP client to request chart configurations.

`EconGraphMcpServer::new(app: AppPool, data: Option<DataPool>)` accepts typed pools for the two logical planes. The app plane holds users and application state; the data plane holds economic series and observations. Both pools may refer to the same physical database. Without a local data pool, discovery still works, but the economic-data tools and resource reads fail with `DATA_SERVED_REMOTELY`.

The implementation lives in [the MCP crate](../../backend/crates/econ-graph-mcp/src/mcp_server.rs), with the authenticated HTTP route in [mcp_routes.rs](../../backend/crates/econ-graph-backend/src/mcp_routes.rs).

### Testing and CI

From `backend/`, run only the pure discovery tests without a database:

```bash
cargo test -p econ-graph-mcp --lib test_get_available_tools
cargo test -p econ-graph-mcp --lib test_get_available_resources
```

The rest of the crate's library tests include database-backed cases that use disposable PostgreSQL containers and require Docker. The [integration tests](../../backend/crates/econ-graph-mcp/tests/mcp_server_integration_tests.rs) create their own PostgreSQL containers, apply both migration sets, and store observations to check retrieval and date filtering. These tests call the server object directly; they do not establish authenticated HTTP or desktop-client interoperability. With Docker available:

```bash
cargo test -p econ-graph-mcp
```

To exercise chart-service calls, make the service reachable at `CHART_API_SERVICE_URL` as described above. Visualization assertions allow fallback, and the direct chart-call success test tolerates an unavailable service, so a passing MCP suite alone does not establish chart-service health. Backend route tests separately cover access rejection and body limits.

In [Core CI](../../.github/workflows/ci-core.yml), `backend-smoke-tests` follows `backend-build-cache` and selects the pure MCP tool/resource discovery tests. `chart-api-integration-tests` follows smoke tests; `backend-mcp-integration-tests` follows both and provisions PostgreSQL, the backend, and the chart service. See the [MCP CI guide](MCP_CI_PIPELINE.md) for workflow details and validation limits.

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the [project license](../../LICENSE) for the terms and conditions.
