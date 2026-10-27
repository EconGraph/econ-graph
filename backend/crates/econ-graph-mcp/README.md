# EconGraph MCP

This crate lets AI assistants search economic series, retrieve observations and metadata,
and request visualizations through the Model Context Protocol (MCP). It provides the
backend's MCP request handler and economic-data tools, using GraphQL for data reads.

## Interface

The JSON-RPC handler supports `tools/list`, `tools/call`, `resources/list`, and
`resources/read`. Its tools search series, retrieve series data with optional date and
result limits, retrieve series metadata, and request chart configurations. Resources expose
the data-source list and series catalog. Visualization requests use the chart API service;
if it is unavailable or returns no chart configuration, the tool returns the series data
for manual visualization.

The backend exposes `POST /mcp` when the `mcp` build flag is enabled. Its
[route](../econ-graph-backend/src/mcp_routes.rs) requires a valid bearer token for an active
account with the `api:mcp` role before calling this crate's handler. Authentication belongs
to that backend route; callers embedding the handler must supply their own access checks.
See the [MCP server guide](../../../docs/technical/MCP_SERVER.md) for tool arguments and
backend configuration.

## Testing

Library tests exercise tool and resource definitions, argument validation, JSON-RPC
handling, errors, concurrent requests, and visualization fallback data. Most use a real
PostgreSQL connection, including tests that invoke the handler directly. They do not
exercise the backend route's authentication or conduct conversations with AI models.

The `mcp_server_integration_tests` target starts PostgreSQL containers, applies data plane
and app plane migrations to each database, and seeds series and observations for data
retrieval and visualization checks. It calls server methods directly to check stored data,
date filtering, errors, and concurrent requests. Visualization checks allow the data
fallback when the chart service is unavailable. Even the chart API success-named test
accepts a failed service call, so a passing suite does not establish successful live chart
rendering.

### Running tests

Run these commands from `backend/` with the repository's Rust toolchain and PostgreSQL
client libraries available. For the library suite, set `DATABASE_URL` to a reachable,
disposable PostgreSQL database; the default is `postgres://localhost/econ_graph_test`.
The core test helper is compiled as a dependency here, so it connects to that database
without starting a container or applying migrations.

For a newly created empty test database, install the Diesel CLI with PostgreSQL support
and apply both planes' migrations before running the library tests:

```bash
diesel migration run --migration-dir migrations/data
diesel migration run --migration-dir migrations/app
cargo test -p econ-graph-mcp --lib

# Focus on JSON-RPC handler tests using the same database setup.
cargo test -p econ-graph-mcp --lib test_mcp_handler
```

The library tests do not seed economic series. Tests using the fixed series ID
`550e8400-e29b-41d4-a716-446655440000` expect it to be absent, so use a fresh database
rather than a copy of populated application data. See
[database testing](../../../docs/testing/DATABASE_TESTING.md) for helper behavior and
[database migrations](../../../docs/development/MIGRATIONS.md) for migration details.

The integration target manages its own PostgreSQL 18 containers and database setup. It
requires a running Docker daemon and access to the PostgreSQL image:

```bash
cargo test -p econ-graph-mcp --test mcp_server_integration_tests
```

No AI provider is required by either suite. To exercise chart-service responses, run the
chart API service and set `CHART_API_SERVICE_URL` to its base endpoint, including
`/api/chart`; the server appends `/generate`. The integration suite still permits service
failure, so inspect the chart response separately when verifying that service.

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the
[LICENSE](../../../LICENSE) file for complete terms and conditions.
