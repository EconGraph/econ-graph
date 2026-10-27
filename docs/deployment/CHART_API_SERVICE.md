# EconGraph Chart API Service

## Overview

The chart service is a standalone Node.js/Express service that converts economic series into Chart.js configuration and data for the [MCP server](../technical/MCP_SERVER.md). It supports line, bar, and scatter requests, sorts points by date, assigns stable default colors from series IDs, and returns series counts, point counts, and a date range.

The HTTP response is JSON, not a rendered chart or image. A consuming client must configure Chart.js and its date adapter and render the result. JavaScript tooltip and tick callbacks created by the generator are omitted during JSON serialization; the client must supply any callback formatting it needs.

## Access and URLs

The server listens on `0.0.0.0:3001` by default. Deployment controls must provide network isolation: the checked-in Kubernetes Service is ClusterIP, while the package’s Docker run command publishes port 3001 on the host.

The [access helper](../../chart-api-service/src/security.js) accepts a request if its IP matches its loopback/private-address checks **or** both of these caller-supplied marker headers equal the string `true`:

```http
X-MCP-Server-Request: true
X-Internal-Request: true
```

Send both headers in service calls, as MCP does. They are not identity authentication or secret credentials. The helper lists `127.0.0.1`, `::1`, `10.0.0.0/8`, `172.16.0.0/12`, and `192.168.0.0/16`; its range parser is IPv4-oriented, so these checks should not be treated as general IPv6/CIDR enforcement. CORS disables browser cross-origin response access but does not isolate direct HTTP callers.

The [server middleware](../../chart-api-service/src/server.js) limits each IP to 1,000 requests per 15-minute window before endpoint handling, including health checks. The default in-memory quota is local to each process; replicas do not share one quota. Responses include standard rate-limit headers rather than legacy `X-RateLimit-*` headers.

| Use | URL |
| --- | --- |
| Local service origin | `http://localhost:3001` |
| Kubernetes service origin | `http://chart-api-service.econ-graph.svc.cluster.local:3001` |
| Local generation endpoint | `http://localhost:3001/api/chart/generate` |
| Local MCP setting | `CHART_API_SERVICE_URL=http://localhost:3001/api/chart` |
| MCP default | `http://chart-api-service.econ-graph.svc.cluster.local:3001/api/chart` |

`CHART_API_SERVICE_URL` is the `/api/chart` prefix: MCP appends `/generate`. Set it without a trailing slash and without the `/generate` suffix. The localhost commands below assume a local process, a published Docker container, or the port-forward described under Deployment.

## API endpoints

Examples use illustrative timestamps and uptime. All four endpoints pass through the access and rate-limit middleware.

### 1. Health Check
**GET** `/health`

Returns process uptime, service version, and environment. This endpoint does not check backend or database readiness.

**Response:**
```json
{
  "success": true,
  "message": "Chart API Service is healthy",
  "timestamp": "2025-01-15T10:30:00.000Z",
  "version": "1.0.0",
  "environment": "production",
  "uptime": 3600
}
```

### 2. Chart Generation

**POST** `/api/chart/generate`

This complete two-series request includes explicit colors and display controls:

```json
{
  "seriesData": [
    {
      "id": "series-1",
      "name": "GDP",
      "dataPoints": [
        { "date": "2020-01-01", "value": 100 },
        { "date": "2020-02-01", "value": 101.5 }
      ],
      "color": "#1976d2"
    },
    {
      "id": "series-2",
      "name": "Unemployment Rate",
      "dataPoints": [
        { "date": "2020-01-01", "value": 3.5 },
        { "date": "2020-02-01", "value": 3.6 }
      ],
      "color": "#d32f2f"
    }
  ],
  "chartType": "line",
  "title": "Economic Indicators",
  "showLegend": true,
  "showGrid": true,
  "yAxisLabel": "Value",
  "xAxisLabel": "Date"
}
```

The corresponding serialized response contains both datasets in `chartConfig.data` and `chartData`. The two fields contain the same data; metadata counts both series and all four points.

```json
{
  "success": true,
  "chartConfig": {
    "type": "line",
    "data": {
      "datasets": [
        {
          "label": "GDP",
          "data": [
            { "x": "2020-01-01", "y": 100 },
            { "x": "2020-02-01", "y": 101.5 }
          ],
          "borderColor": "#1976d2",
          "backgroundColor": "#1976d220",
          "fill": false,
          "tension": 0.1,
          "pointRadius": 2,
          "pointHoverRadius": 4
        },
        {
          "label": "Unemployment Rate",
          "data": [
            { "x": "2020-01-01", "y": 3.5 },
            { "x": "2020-02-01", "y": 3.6 }
          ],
          "borderColor": "#d32f2f",
          "backgroundColor": "#d32f2f20",
          "fill": false,
          "tension": 0.1,
          "pointRadius": 2,
          "pointHoverRadius": 4
        }
      ]
    },
    "options": {
      "responsive": true,
      "maintainAspectRatio": false,
      "plugins": {
        "title": {
          "display": true,
          "text": "Economic Indicators",
          "font": {
            "size": 16,
            "weight": "bold"
          }
        },
        "legend": {
          "display": true,
          "position": "top"
        },
        "tooltip": {
          "mode": "index",
          "intersect": false,
          "callbacks": {}
        }
      },
      "scales": {
        "x": {
          "display": true,
          "title": {
            "display": true,
            "text": "Date"
          },
          "type": "time",
          "time": {
            "parser": "YYYY-MM-DD",
            "displayFormats": {
              "day": "MMM DD",
              "month": "MMM YYYY",
              "year": "YYYY"
            }
          },
          "grid": {
            "display": true
          }
        },
        "y": {
          "display": true,
          "title": {
            "display": true,
            "text": "Value"
          },
          "grid": {
            "display": true
          },
          "ticks": {}
        }
      },
      "interaction": {
        "mode": "nearest",
        "axis": "x",
        "intersect": false
      }
    }
  },
  "chartData": {
    "datasets": [
      {
        "label": "GDP",
        "data": [
          { "x": "2020-01-01", "y": 100 },
          { "x": "2020-02-01", "y": 101.5 }
        ],
        "borderColor": "#1976d2",
        "backgroundColor": "#1976d220",
        "fill": false,
        "tension": 0.1,
        "pointRadius": 2,
        "pointHoverRadius": 4
      },
      {
        "label": "Unemployment Rate",
        "data": [
          { "x": "2020-01-01", "y": 3.5 },
          { "x": "2020-02-01", "y": 3.6 }
        ],
        "borderColor": "#d32f2f",
        "backgroundColor": "#d32f2f20",
        "fill": false,
        "tension": 0.1,
        "pointRadius": 2,
        "pointHoverRadius": 4
      }
    ]
  },
  "metadata": {
    "seriesCount": 2,
    "dataPointCount": 4,
    "dateRange": {
      "start": "2020-01-01",
      "end": "2020-02-01"
    }
  },
  "message": "Chart configuration generated successfully",
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

### 3. Chart Types
**GET** `/api/chart/types`

Returns information about supported chart types.

**Response:**
```json
{
  "success": true,
  "chartTypes": [
    {
      "type": "line",
      "description": "Line charts for time series data and trends",
      "supportedFeatures": ["multiple series", "time scale", "smooth curves"]
    },
    {
      "type": "bar",
      "description": "Bar charts for categorical comparisons",
      "supportedFeatures": ["grouped bars", "stacked bars", "categorical data"]
    },
    {
      "type": "scatter",
      "description": "Scatter plots for correlation analysis",
      "supportedFeatures": ["point markers", "trend lines", "correlation analysis"]
    }
  ],
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

The `/api/chart/types` feature strings are descriptive labels returned by the endpoint. They do not enable stacking, trend lines, or correlation calculations. The generator uses a time x-axis for all three chart types, including bar and scatter.

### 4. Service Information
**GET** `/api/info`

Returns endpoint and service labels. The `security` labels describe the service’s intended use; the actual access decision is explained above.

**Response:**
```json
{
  "success": true,
  "service": "EconGraph Chart API",
  "version": "1.0.0",
  "description": "Private chart generation service for MCP server integration",
  "endpoints": [
    "GET /health - Health check",
    "POST /api/chart/generate - Generate chart configuration",
    "GET /api/chart/types - Get supported chart types",
    "GET /api/info - Service information"
  ],
  "security": {
    "internalNetworkOnly": true,
    "requiredHeaders": ["X-MCP-Server-Request", "X-Internal-Request"],
    "rateLimit": "1000 requests per 15 minutes"
  },
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

## Request and response contract

The top-level validator checks only that `seriesData` is a nonempty array and `chartType` is `line`, `bar`, or `scatter`. It does not validate nested series or normalize missing values. Callers should supply the following shape so generation and subsequent client rendering can use it:

| Field | Caller input |
| --- | --- |
| `seriesData` | Nonempty array of series objects. |
| `seriesData[].id` | String used to choose a stable default color when `color` is absent. |
| `seriesData[].name` | Display name used as the dataset label. |
| `seriesData[].dataPoints` | Array of points with `date` and `value` fields. |
| `dataPoints[].date` | Date string suitable for the client’s time adapter, such as `2020-01-01`. |
| `dataPoints[].value` | Numeric value suitable for the client chart. |
| `seriesData[].color` | Optional color; use a hex value such as `#1976d2`. The generator appends `20` for background opacity. |
| `chartType` | Required string: `line`, `bar`, or `scatter`. |
| `title` | Optional chart title. |
| `showLegend`, `showGrid` | Optional booleans, both defaulting to `true`. |
| `yAxisLabel`, `xAxisLabel` | Optional axis titles; omitted labels hide their titles. |

The [generator](../../chart-api-service/src/chartApi.js) sorts each series’ points using JavaScript dates, then copies each `date` to `x` and `value` to `y` without coercion or missing-value handling. The metadata date range uses the first and last distinct date strings in lexical order; use consistent ISO dates as in the example. No date-range filtering occurs in this service.

### Error Responses

#### Validation Error (400)
```json
{
  "success": false,
  "error": "Invalid chart request. Required fields: seriesData (array), chartType (line|bar|scatter)",
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

#### Access Denied (403)
```json
{
  "success": false,
  "error": "Access denied: This service is only accessible from internal network",
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

#### Rate Limit Exceeded (429)
```json
{
  "success": false,
  "error": "Too many requests from this IP, please try again later."
}
```

#### Internal Server Error (500)
```json
{
  "success": false,
  "error": "Internal server error while generating chart",
  "timestamp": "2025-01-15T10:30:00.000Z"
}
```

Malformed JSON returns HTTP 400 with `error: "Invalid JSON in request body"`. Nested inputs can pass top-level validation and then fail generation; those failures also return 400 with the generator’s error text and a timestamp. The 500 example above is for unexpected exceptions in the generation handler; other unhandled errors use `"Internal server error"`. Unknown endpoints return 404 with `availableEndpoints`. Unlike the application error envelopes, the configured 429 body has no timestamp.

## Development and testing

Use Node.js 26, as required by [package.json](../../chart-api-service/package.json) and the repository’s [`.nvmrc`](../../.nvmrc). From the repository root:

```bash
cd chart-api-service
npm ci
npm run dev
```

`npm run dev` starts the nodemon development server; use `npm start` to start Node directly. `PORT` defaults to `3001` and `NODE_ENV` defaults to `development`. Set `NODE_ENV=production` for production-style request logs. Package metadata and the lockfile define dependency versions.

Run these commands from `chart-api-service`:

```bash
npm test
npm run test:coverage
npm run test:watch
npm run lint
```

[`tests/chartApi.test.js`](../../chart-api-service/tests/chartApi.test.js) exercises configuration generation, data processing, metadata, validation, and error cases. [`tests/server.test.js`](../../chart-api-service/tests/server.test.js) exercises HTTP endpoints, access checks, and error responses through Supertest. `npm run test:coverage` generates a Jest coverage report for the revision under test.

In [CI Core](../../.github/workflows/ci-core.yml), **Chart API Integration Tests** installs dependencies, lints, runs the package’s build script (which only prints that no build step is required), starts the service, checks health, and runs `npm run test:coverage`. Its subsequent generation curl posts to `/api/chart` rather than `/api/chart/generate` and tolerates failure, so that probe does not establish successful generation. **Backend MCP Integration Tests** starts the chart service and backend, then runs the Rust MCP integration suite with the valid `/api/chart` MCP prefix. MCP’s fallback means successful integration assertions alone need not prove that chart generation succeeded. These jobs do not certify Kubernetes reachability or rendered output.

## Deployment

### Docker

From `chart-api-service`:

```bash
npm run docker:build
npm run docker:run
```

The [checked-in Dockerfile](../../chart-api-service/Dockerfile) uses Node 26 Alpine, installs production dependencies, runs as a non-root user, and configures a `/health` check. The scripts build `econ-graph-chart-api` and publish port 3001. That local image tag differs from the Kubernetes manifest’s tag.

### Kubernetes

Follow the [Kubernetes deployment procedure](../../k8s/README.md). The repository’s [deployment script](../../scripts/deploy/deploy.sh) applies [chart-api-deployment.yaml](../../k8s/manifests/chart-api-deployment.yaml) and [chart-api-service.yaml](../../k8s/manifests/chart-api-service.yaml) in namespace `econ-graph`. Use these manifests for that procedure rather than the separate copies under `chart-api-service/k8s`.

The deployment specifies two replicas of `econ-graph-chart-api:v1.0.0` with `imagePullPolicy: Never`, so that exact image must already be available on the scheduled nodes. The Service selects the chart pods and exposes port 3001 as ClusterIP. Configuration generation keeps no persistent chart state and can run across replicas, while rate-limit state remains per process.

Each replica requests `100m` CPU and `128Mi` memory, with limits of `500m` CPU and `512Mi` memory. Liveness probes call `/health` after 30 seconds and every 10 seconds; readiness probes start after 5 seconds and repeat every 5 seconds. These are configured resource budgets and checks, not measured usage or guarantees of readiness beyond this endpoint.

For local access to an existing cluster deployment, keep this command running in a separate terminal:

```bash
kubectl -n econ-graph port-forward service/chart-api-service 3001:3001
```

The localhost curl commands then use that forwarding connection. In-cluster clients can use the service DNS origin from the URL table.

## MCP integration

The [MCP implementation](../../backend/crates/econ-graph-mcp/src/mcp_server.rs) collects series data, builds the chart request, and sends it with both marker headers to `format!("{}/generate", self.frontend_chart_api_url)`. For a local chart service, configure the MCP process with:

```bash
export CHART_API_SERVICE_URL=http://localhost:3001/api/chart
```

On a successful response containing `chartConfig`, MCP returns the configuration and metadata as text for the caller to consume. If the HTTP call fails, JSON decoding fails, or the response lacks `chartConfig`, it returns series data, chart type, title, and metadata for manual visualization. This fallback is returned with `is_error: false`. Returning fallback data does not suppress later chart API calls.

## Monitoring and troubleshooting

`/health` reports process uptime, version, and environment. Kubernetes probes use that endpoint; they do not check backend or database availability. Morgan emits `combined` logs in production and `dev` logs otherwise, and server error handlers write to the console. The rate limiter runs before Morgan, so requests rejected by the limiter do not reach that request logger. The service does not expose aggregate request counts, error rates, rate-limit event metrics, or a metrics endpoint. Aggregate metrics require external collection.

For cluster logs:

```bash
kubectl -n econ-graph logs -f deployment/chart-api-service
```

For a local service or the forwarding connection above, check health and inspect rate-limit response headers:

```bash
curl -i -H "X-MCP-Server-Request: true" \
  -H "X-Internal-Request: true" http://localhost:3001/health
```

If access fails, check the selected service origin, namespace, forwarding connection, and access headers. A 403 means the application access check rejected the request; a 429 means that process’s per-IP quota was exceeded. Reduce request frequency and inspect the returned rate-limit headers.

For generation failures, check the chart type and every series’ nested fields, not just whether the body is valid JSON. This complete minimal request isolates the generation route:

```bash
curl -i -X POST http://localhost:3001/api/chart/generate \
  -H "Content-Type: application/json" \
  -H "X-MCP-Server-Request: true" \
  -H "X-Internal-Request: true" \
  -d '{"seriesData":[{"id":"gdp","name":"GDP","dataPoints":[{"date":"2020-01-01","value":100}],"color":"#1976d2"}],"chartType":"line"}'
```

A 400 response can identify either top-level validation failure or a generator error; consult its `error` field. Check service logs for unexpected 500 responses. If MCP returns fallback data, inspect the configured `/api/chart` prefix and test the full generation endpoint directly with the payload above.

## License

This project uses the [MIT License](../../LICENSE).
