# EconGraph Admin Interface

## Overview

The EconGraph Admin Interface brings crawler status, queue activity, configuration,
and log controls into one workspace for investigating data acquisition. The Vite
application opens **EconGraph Crawler Administration**, with three tabs:
**Dashboard**, **Configuration**, and **Logs & Monitoring**. These screens expose
crawler requests and controls, but several requests do not match the current
backend schema; a running frontend alone does not provide a working crawler
administration workflow.

## Crawler workspace

- **Dashboard** displays crawler status, queue statistics, and recent logs from
  the requests in [`useCrawlerData.ts`](src/hooks/useCrawlerData.ts).
  It refreshes data periodically and offers manual crawl and stop controls.
- **Configuration** presents global crawler settings and data-source settings,
  including enablement, rate limits, retries, timeouts, and connection testing.
  Its load and save requests are defined in
  [`useCrawlerConfig.ts`](src/hooks/useCrawlerConfig.ts).
- **Logs & Monitoring** requests logs and search results through
  [`useCrawlerLogs.ts`](src/hooks/useCrawlerLogs.ts), with level/source filters,
  search, and automatic refresh. Its performance cards use randomly generated
  demonstration values, not measured CPU, memory, or crawler telemetry. **Export**
  only writes a console message; **Clear** hides the displayed log array until a
  new fetch replaces it, without deleting backend logs.

The backend has `crawlerStatus`, `queueStatistics`, and `triggerCrawl` operations,
but the frontend requests snake_case fields such as `is_running` and
`total_items`, whereas the [backend schema](../backend/crates/econ-graph-graphql/schema.graphql)
exposes `isRunning` and `totalItems`. The current backend also has no
`crawlerConfig`, `crawlerLogs`,
`searchLogs`, or `performanceMetrics` query fields, and no start/stop/pause/resume
crawler, configuration-update, data-source-update, or connection-test mutations.
Consequently, those controls and panels cannot complete their requests against
this backend. Compare the frontend [queries](src/services/graphql/queries.ts),
[mutations](src/services/graphql/mutations.ts), and configuration hook with the
backend [query root](../backend/crates/econ-graph-graphql/src/graphql/query.rs) and
[mutation root](../backend/crates/econ-graph-graphql/src/graphql/mutation.rs) when
diagnosing schema errors.

The service links above the tabs are hardcoded in [`App.tsx`](src/App.tsx): the
main frontend, Grafana, backend API, and GraphQL Playground use
`http://localhost:8080/`, `/grafana`, `/api`, and `/playground`, respectively.
These links do not configure API requests or collect monitoring data. For the
platform's metrics and dashboards, see the [monitoring guide](../docs/monitoring/README.md).

## Source structure

[`index.html`](index.html) loads [`src/main.tsx`](src/main.tsx), which renders
[`App.tsx`](src/App.tsx) under the Material UI theme. App supplies React Query
and Apollo providers and switches between the crawler tabs without a router.

```text
admin-frontend/
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── pages/
│   │   ├── CrawlerDashboard.tsx
│   │   ├── CrawlerConfig.tsx
│   │   └── CrawlerLogs.tsx
│   ├── hooks/
│   │   ├── useCrawlerData.ts
│   │   ├── useCrawlerConfig.ts
│   │   └── useCrawlerLogs.ts
│   └── services/
│       ├── graphqlClient.ts
│       └── graphql/
├── tests/smoke/
├── vite.config.ts
├── Dockerfile
└── nginx.conf
```

### Separate components

The source also contains [`AdminLayout`](src/components/layout/AdminLayout.tsx),
[`MonitoringPage`](src/pages/MonitoringPage.tsx),
[`SystemHealthPage`](src/pages/SystemHealthPage.tsx), and
[`UserManagementPage`](src/pages/UserManagementPage.tsx). They are not mounted by
the current entry point and cannot be reached through its tabs. AdminLayout
contains role-filtered navigation; the separate health and user pages contain
health and user/session administration interfaces.

MonitoringPage offers Grafana links and embedded views. Its
[`useMonitoring.ts`](src/hooks/useMonitoring.ts) dashboard summaries come from
local mock records with randomized values, while the separate `useSystemStatus`
hook fetches `/api/monitoring/system-status`. Those components use Grafana URLs
under `http://localhost:30001`, distinct from App's `/grafana` link on port `8080`.
Neither the mock summaries nor the visible Logs performance cards establish a
live Prometheus or admin-usage telemetry integration.

## Authentication and request boundaries

The mounted crawler app has no login screen or frontend role gate. It does not
mount [`AuthProvider`](src/contexts/AuthContext.tsx),
[`SecurityProvider`](src/contexts/SecurityContext.tsx), or AdminLayout. Backend
authorization is a separate boundary: for example, `crawlerStatus` and
`queueStatistics` require `admin.system:read` in the backend query root. Frontend
role checks and possession of a stored token do not grant backend permissions.
See [admin security](../docs/technical/ADMIN_SECURITY.md) for the backend security
model.

Request behavior differs between the clients:

| Consumer | Endpoint | Token transport |
| --- | --- | --- |
| Crawler data and log hooks | Same-origin `/graphql` | No explicit Authorization header |
| Crawler configuration hook | Same-origin `/graphql` | `Bearer` header from localStorage key `admin_token` |
| Separate Apollo client | `VITE_GRAPHQL_ENDPOINT`, default `/graphql` | `Bearer` header from localStorage key `auth_token`; includes cookies |

Providing Apollo in App does not send the crawler hooks' direct `fetch` requests
through Apollo's authentication, retry, or error links. The configuration hook
constructs its header even when `admin_token` is absent. This is request behavior,
not a supported login procedure for the mounted app.

For developers working on the separate components, AuthContext implements
login/validation/session requests under `/api/admin/auth` and stores
`admin_token` in localStorage. Its UI role labels are `read_only`, `admin`, and
`super_admin`. SecurityContext performs frontend access checks, retains recent
events in React state, and posts high/critical events to
`/api/admin/security/events` when its provider runs. This does not provide a
durable audit trail of all administrative actions in the mounted app.

## Development and testing

Use **Node 26** (`>=26.0.0 <27`), as specified in
[`package.json`](package.json) and the repository [`.nvmrc`](../.nvmrc).
From the repository root:

```bash
cd admin-frontend
npm ci
npm run test:run
npm run build
npm run dev
```

`npm test` starts Vitest in watch mode; `npm run test:run` runs it once.
`npm run lint` checks ESLint rules and `npm run typecheck` checks TypeScript.
Open `http://localhost:3000/admin/` when port `3000` is available; Vite prints
the chosen URL if it uses another port.

Component tests live in [`src/pages/__tests__`](src/pages/__tests__) and
[`src/components/layout/__tests__`](src/components/layout/__tests__); hook tests
live in [`src/hooks/__tests__`](src/hooks/__tests__), and schema-validation tests
in [`src/__tests__`](src/__tests__). Component and hook tests use mocks, including
mocked auth/security contexts and monitoring responses in MonitoringPage tests.
They verify rendering and interactions under those inputs, not live Grafana,
backend compatibility, or protected application startup.

Run `npx playwright install chromium` once, then `npm run e2e:smoke` to build and
load the production app at `/admin/` in Chromium. The
[smoke test](tests/smoke/startup.spec.ts) stubs GraphQL and needs no backend. It checks the crawler heading, Dashboard tab, switching to Configuration,
and absence of browser exceptions; it does not validate live crawler operations
or authentication.

## Configuration and serving

### Development requests

The admin frontend uses Vite. [`vite.config.ts`](vite.config.ts) sets development
port `3000`, base path `/admin/`, production output directory `build/`, and proxies
`/api` and `/graphql` to `http://localhost:8081` during development. The backend's
default port is `9876`; align the proxy target with your backend before making
API requests. See the [backend setup guide](../backend/README.md).

All three crawler hooks call same-origin `/graphql`, so they use that Vite proxy
in development. A production server must route `/graphql` to the backend.
`VITE_GRAPHQL_ENDPOINT` configures only the separate
[Apollo client](src/services/graphqlClient.ts) at build time. For example:

```bash
VITE_GRAPHQL_ENDPOINT=https://api.example.com/graphql npm run build
```

This changes Apollo's endpoint only; it does not redirect crawler-hook requests.
Browser code uses Vite's `import.meta.env`; ESLint rejects `process.env` under
`src/`. Custom public variables are typed in [`src/vite-env.d.ts`](src/vite-env.d.ts).
`REACT_APP_API_URL`, `REACT_APP_GRAFANA_URL`, `REACT_APP_ADMIN_PORT`, and
`REACT_APP_DEBUG` are not read as configuration options.

### Container build and serving requirements

The checked-in [`Dockerfile`](Dockerfile) installs build dependencies, runs
`npm run build`, and copies `build/` into NGINX. From the repository root:

```bash
docker build -t econgraph-admin:local ./admin-frontend
```

The container's [`nginx.conf`](nginx.conf) listens on `3001`, distinct from Vite's
port `3000`. Its `/api` upstream names the Kubernetes service
`econ-graph-backend-service.econ-graph.svc.cluster.local:9876`; serving outside
that cluster requires an upstream reachable from that environment.

The checked-in configuration also needs asset and GraphQL routing changes for a
working `/admin/` deployment. Vite emits asset URLs under `/admin/assets/`, but
the Dockerfile copies assets into `/usr/share/nginx/html/assets/` and NGINX does
not map the prefix to that directory. NGINX proxies `/api` only; `/graphql` falls
through to static/SPA handling instead of reaching the backend. Building the
image does not establish that assets load or crawler requests reach a backend.
See the [Kubernetes deployment guide](../k8s/README.md) for platform deployment
configuration.

## Troubleshooting

Use the browser console and Network panel to distinguish routing, schema, and
authorization failures. For crawler requests, inspect `/graphql` at the frontend
origin and its proxy target; changing Apollo's endpoint does not change these
requests. GraphQL errors about unknown fields or mutations should be compared
with the backend roots linked above. For authorization errors, check the
backend's required permissions and the request's actual token transport.

If the container returns missing assets or HTML for GraphQL requests, check the
`/admin/` asset mapping and `/graphql` routing described above. For Grafana links,
check the hardcoded host/port and dashboard availability separately from the
frontend's API connection. Randomized performance cards do not diagnose service
health.

When contributing, follow the existing ESLint and TypeScript rules, run the tests
appropriate to the changed behavior, and update this README when the mounted
screens or request contracts change.
