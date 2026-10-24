# Train 2 contracts: app and data databases

The shared interfaces for release 2 (v4.1.0, ECO-420). Workers build to these names and
signatures. This file changes only through the orchestrator: a worker who needs a change asks
on its ticket, and the owning product owner updates this file in a PR.

Owners: data plane PO (sections 1 to 8 and the section 9 reconciliation notes), platform/infra
PO (section 9), product/UX PO (section 10).

## 1. Environment variables

| Variable | Meaning | Required by |
|----------|---------|-------------|
| `APP_DATABASE_URL` | The app database: users and everything they create. | `econ-graph-backend` (required) |
| `DATA_DATABASE_URL` | The data database: sources, series, observations, crawl state, SEC data. | `crawler-worker`, `crawler`, `seed-fixtures`, `sec-crawler`, `sec-company-crawler` (required); `econ-graph-backend` (optional, see section 8) |
| `DATA_GRAPHQL_URL` | In-cluster URL of the data plane's GraphQL endpoint, for server-side data checks when the backend has no `DATA_DATABASE_URL` (DP-7). | `econ-graph-backend` (optional) |
| `DATA_GRAPHQL_HOST` | `Host` header sent with `DATA_GRAPHQL_URL` requests, when the URL is the gateway's in-cluster Service (section 9 notes). | `econ-graph-backend` (optional) |

- `DATABASE_URL` is no longer read by any binary. A binary that finds `DATABASE_URL` set and
  its own variable unset exits at startup with
  `DATABASE_URL is no longer read; set APP_DATABASE_URL and/or DATA_DATABASE_URL (docs/development/TRAIN2_CONTRACTS.md)`.
- Tests may keep reading `DATABASE_URL` / `TEST_DATABASE_URL`: in tests it names one database
  with both migration sets applied (section 4).
- Database names: data `econ_graph` (the live database, unchanged), app `econ_graph_app`. In
  k8s the URLs come from Secret `econ-graph-secrets`, keys `data-database-url` and
  `app-database-url` (replacing `database-url`); hosts are in section 9.
- An app-only branch backend has no `DATA_DATABASE_URL` and no network path to the primary's
  Postgres; it reads data only through `/data/graphql`.
- Both URLs may name the same database (local development, CI, and `main` until DP-4 gives k8s a
  second Postgres). Nothing may depend on them being different or the same.

## 2. Pool types

In `econ-graph-core`, `src/database.rs`, re-exported from the crate root:

```rust
pub type DatabasePool = Pool<AsyncPgConnection>; // unchanged

#[derive(Clone)]
pub struct AppPool(pub DatabasePool);
#[derive(Clone)]
pub struct DataPool(pub DatabasePool);

impl std::ops::Deref for AppPool  { type Target = DatabasePool; /* &self.0 */ }
impl std::ops::Deref for DataPool { type Target = DatabasePool; /* &self.0 */ }
```

- They are newtypes so that async-graphql's context holds both: resolvers read
  `ctx.data::<AppPool>()` or `ctx.data::<DataPool>()`. No resolver reads
  `ctx.data::<DatabasePool>()` after DP-2.
- The backend's schema holds `AppPool` always and `Option<DataPool>`. A data resolver gets its
  pool through one helper, `econ_graph_graphql::graphql::context::data_pool(ctx) -> Result<&DataPool>`,
  which returns the section 8 error when there is none.
- `create_schema_with_data(app: AppPool, data: Option<DataPool>, additional_data: T)`.
  Dataloaders take `DataPool`. `econ_graph_auth::bearer::authenticate` takes `&AppPool`.
  The MCP server takes `AppPool` and `Option<DataPool>` (it builds the same schema; its tools are all data tools).
- Services and model functions keep taking `&DatabasePool` or `&mut AsyncPgConnection`; the
  newtype stops at the resolver or binary boundary.

## 3. Binaries and URLs

| Binary | Reads | Applies migrations |
|--------|-------|--------------------|
| `econ-graph-backend` | `APP_DATABASE_URL`, optional `DATA_DATABASE_URL`, optional `DATA_GRAPHQL_URL` | app set, at startup |
| `crawler-worker` | `DATA_DATABASE_URL` | none |
| `crawler` (all subcommands) | `DATA_DATABASE_URL` | data set, only in `crawler migrate` |
| `seed-fixtures` | `DATA_DATABASE_URL` | data set (a deliberate tool, not a service) |
| `sec-crawler`, `sec-company-crawler` | `DATA_DATABASE_URL` | none |
| `econ-graph-roles` | none | none |
| Release E2E harness (`scripts/release-e2e.sh`, `docker-compose.release-e2e.yml`) | both | runs `crawler migrate`, then starts the backend |

No service applies data migrations at startup.

## 4. Migration layout

```
backend/migrations/
  app/
    00000000000000_diesel_initial_setup/      # identical copy in both sets
    2026-10-21-000200_v4_0_app_baseline/      # new version; the app half of the v4.0 baseline
  data/
    00000000000000_diesel_initial_setup/
    2026-10-21-000100_v4_0_baseline/          # keeps its version: live v4.0 databases have it recorded
    economic_synonyms.syn                     # unused data file, moved with the data set
```

In `econ-graph-core/src/database.rs`:

```rust
pub const APP_MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/app");
pub const DATA_MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/data");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationSet { App, Data }

pub async fn run_migrations(database_url: &str, set: MigrationSet) -> AppResult<()>;
pub async fn readiness_check(pool: &DatabasePool, set: MigrationSet) -> AppResult<()>;
```

- **Migration table.** Diesel 2.3 hard-codes `__diesel_schema_migrations`, one per database.
  With separate databases each set has its own table. When one database holds both sets they
  share it, so **versions are unique across both directories**; the only shared version is
  `00000000000000`, whose two copies are identical. `check_migration_order.py` enforces both.
- **Lock.** Both sets take the existing advisory lock `MIGRATION_LOCK_KEY`. Advisory locks are
  scoped to a database, so separate databases never contend, and two sets in one database run
  one after the other.
- **Readiness.** A set is current when the newest version embedded for that set is recorded in
  `__diesel_schema_migrations` (not "the table's max version", which mixes sets in a shared
  database).
- **One database with both sets** (tests, local, `main` before DP-4): apply data, then app.
  If the database already has `users` and `20261021000100` recorded but not `20261021000200`
  (a database built from the single v4.0 baseline), the app run records `20261021000200`
  without running it, and logs that at `warn`. Until DP-2 lands, `run_migrations(url)` does
  exactly this for both sets (DP-1).
- The app baseline's guard stops on an existing `users` table; the data baseline's guard is
  unchanged (`data_sources`).
- `diesel.toml` `migrations_directory` is `migrations/app`. The diesel CLI and CI pass
  `--migration-dir migrations/data` for the data set. `schema.rs` stays one file covering both
  sets, printed from a database with both applied.
- `crawler record-reference-seeds` writes into `migrations/data` by default.
- Rules per set: [MIGRATIONS.md](./MIGRATIONS.md) (DP-9).

## 5. Tables

**Data (26):** `data_sources`, `datasets`, `economic_series`, `data_points`, `crawl_queue`,
`crawl_attempts`, `series_metadata`, `reference_file_cache`, `series_fetch_validators`,
`countries`, `global_economic_indicators`, `global_indicator_data`, `country_correlations`,
`trade_relationships`, `global_economic_events`, `event_country_impacts`, `leading_indicators`,
`companies`, `financial_statements`, `financial_line_items`, `financial_ratios`,
`xbrl_taxonomy_schemas`, `xbrl_taxonomy_linkbases`, `xbrl_dts_dependencies`,
`xbrl_instance_dts_references`, `xbrl_taxonomy_concepts`.
Also: views `company_financial_statements`, `financial_line_items_with_context`,
`financial_ratios_with_context`; functions `update_series_metadata_updated_at`,
`dataset_components_valid`, `seed_reference_codes`; the SEC/XBRL enums (`compression_type`
through `taxonomy_source_type`); `pg_trgm`; seed rows for `data_sources`, `series_metadata`,
`countries`, `global_economic_events`.

**App (12):** `users`, `user_sessions`, `chart_annotations`, `annotation_comments`,
`chart_collaborators`, `user_data_source_preferences`, `audit_logs`, `security_events`,
`financial_annotations`, `annotation_assignments`, `annotation_replies`,
`annotation_templates` (with its seed rows). Enums `annotation_type`, `annotation_status`,
`assignment_type`, `assignment_status`, `annotation_visibility`.

**Both:** `pgcrypto`, `update_updated_at_column()`, and `00000000000000_diesel_initial_setup`.

**Cross-plane foreign keys dropped** (the columns stay, as plain UUIDs with their indexes):

1. `user_data_source_preferences.data_source_id → data_sources(id)`
2. `financial_annotations.statement_id → financial_statements(id)`
3. `financial_annotations.line_item_id → financial_line_items(id)`

The matching `joinable!` lines leave `schema.rs`. No query joins an app table to a data table;
`chart_annotations.series_id` was never a foreign key and is checked by DP-7.

## 6. `crawler migrate`

```
crawler migrate [--dry-run]
```

- Applies `DATA_MIGRATIONS` to `DATA_DATABASE_URL` under `MIGRATION_LOCK_KEY`, then prints the
  versions applied, one per line. Exit 0 when the set is current afterwards.
- `--dry-run` prints the pending versions and changes nothing.
- It never touches the app set. Run by `deploy.sh` as a Kubernetes Job (`crawler-migrate`) from
  the `econ-graph-crawler-worker` image (`/app/crawler migrate`) before the crawler-worker rolls
  out, and only in a namespace that owns a data database.

## 7. Readiness and health

- Backend `/readyz`: app set current; and, when it has `DATA_DATABASE_URL`, the data database
  reachable and its set current. The backend never migrates the data database, so a data
  database behind the binary keeps `/readyz` failing until `crawler migrate` runs.
- `systemHealth`: user counts from the app pool; queue counts from the data pool, `null` when
  there is none.

## 8. Backend with no `DATA_DATABASE_URL`

The default in a branch namespace (DP-8):

- App operations (users, sessions, annotations, comments, collaborators, preferences, admin,
  audit) work as before.
- Every data field fails with a GraphQL error whose message is
  `Data is served by the data plane at /data/graphql` and whose `extensions.code` is
  `DATA_SERVED_REMOTELY`. The schema itself is unchanged, so `schema.graphql` is the same in
  every mode.
- `/readyz` checks only the app database.
- MCP data tools fail with the same message.
- `createAnnotation`'s series check (DP-7) asks `DATA_GRAPHQL_URL` (with `DATA_GRAPHQL_HOST`, no
  token); with neither a data pool nor `DATA_GRAPHQL_URL` the check is skipped and the backend
  logs that once at startup. If the data plane is unreachable or times out, the mutation fails
  with `extensions.code = DATA_PLANE_UNAVAILABLE` and creates nothing.
- The admin operations that read crawl state (`GetCrawlerStatus`, `TriggerCrawl`) are app
  operations (section 10) but need data, so on this backend they fail with the same error.
- No backend serves a `/data/graphql` path: the route sends `/data/graphql` to a backend that
  has a data pool, rewritten to `/graphql` (sections 9 and 10).

## 9. Routing (owner: Platform/infra PO)

### Controller, class, gateway
- Envoy Gateway in namespace `envoy-gateway-system` (upstream default); Gateway API v1.5 standard-channel CRDs. Exact versions pinned in `scripts/deploy/install-gateway.sh`.
- EnvoyProxy `econ-graph-proxy` (ns `envoy-gateway-system`): runs the proxy on the kind control-plane node (`ingress-ready=true`) with hostPorts 80/443. `kind-config.yaml` does not change.
- GatewayClass `econ-graph`: controllerName `gateway.envoyproxy.io/gatewayclass-controller`, parametersRef → EnvoyProxy `econ-graph-proxy`.
- Gateway `econ-graph-gateway` in its own namespace `econ-graph-gateway`, so deleting an app or branch namespace never removes it.
  - Listener `http`: port 80, HTTP, no hostname. allowedRoutes: namespaces with label `econgraph.io/gateway-access: "true"`.
  - Listener `https`: port 443, HTTPS, hostname = the production public hostname (pending Joe). TLS terminates with Secret `econ-graph-tls` in `econ-graph-gateway`, issued by cert-manager via a `cert-manager.io/cluster-issuer` annotation. allowedRoutes: only namespaces labelled `econgraph.io/env: production`. Branches never attach here.

### HTTPRoutes (same names in every app namespace; production = `econ-graph`)
Hostnames: production `localhost` (plus the public hostname on `https`); branch `<slug>.localhost`.

| HTTPRoute | Matches | Backend (port) | Filters / policy |
|---|---|---|---|
| `econ-graph-frontend` | PathPrefix `/`, Exact `/auth/callback` | `econ-graph-frontend-service` (3000) | on `https`, response security headers from `ssl-ingress.yaml` |
| `econ-graph-backend` | PathPrefix `/api`, `/auth`, `/health` | `econ-graph-backend-service` (9876) | `/playground` dropped (ECO-170) |
| `econ-graph-graphql` | PathPrefix `/graphql` | `econ-graph-backend-service` (9876) | BackendTrafficPolicy `econ-graph-graphql-ratelimit`: 20 rps per client IP, burst 100, 429 |
| `econ-graph-data-graphql` | PathPrefix `/data/graphql` | production and `--local-data` branches: own-namespace `econ-graph-backend-service` (9876); app-only branches: `econ-graph-backend-service` in namespace `econ-graph` (9876) | URLRewrite ReplacePrefixMatch `/graphql`; RequestHeaderModifier remove `Authorization`, `Cookie`; BackendTrafficPolicy `econ-graph-data-graphql-ratelimit` (same 20 rps / burst 100) |
| `econ-graph-idp` | PathPrefix `/idp/realms/econ-graph`, `/idp/resources` | `keycloak-service` (8080) | never `/idp/admin` or the master realm |
| `econ-graph-mcp` | reserved: `/mcp`, `/.well-known/oauth-protected-resource` | — | not created in train 2 (ECO-292, train 3) |

Admin frontend `/admin`: not routed (ECO-243), as today.

### Branch namespaces
- Namespace `econ-graph-br-<slug>`. Slug: branch name lowercased; non-`[a-z0-9-]` → `-`; collapse runs of `-`; trim; ≤ 40 chars; if truncated or colliding, append `-<first 6 hex of sha1(branch)>`. Hostname `<slug>.localhost`.
- Labels: `econgraph.io/env: branch`, `econgraph.io/branch: <slug>`, `econgraph.io/data-mode: primary|local`, `econgraph.io/gateway-access: "true"`. Annotation `econgraph.io/git-branch: <full branch name>`.
- Production namespace `econ-graph` gets `econgraph.io/env: production`, `econgraph.io/gateway-access: "true"`.
- App-only branch → primary: the branch's `econ-graph-data-graphql` route has a cross-namespace backendRef to `econ-graph/econ-graph-backend-service:9876`, allowed by a ReferenceGrant `data-graphql-from-<slug>` in `econ-graph` (from HTTPRoute in `econ-graph-br-<slug>`; to Service `econ-graph-backend-service`; labelled `econgraph.io/branch: <slug>`). `deploy-branch.sh` creates it on `up` and deletes it on `down`. It is the only object a branch writes into `econ-graph`.
- `--local-data` branch: same route name, backendRef to its own backend, no ReferenceGrant.
- NetworkPolicy: branch pods never reach `econ-graph` pods directly, and never `data-postgres-service`. The primary is reached only through the gateway route, which strips credentials.

### Postgres Services
| Database | Namespace | StatefulSet | Service (5432) | Secret | Storage |
|---|---|---|---|---|---|
| Data (primary) | `econ-graph` | `postgresql` (unchanged) | `data-postgres-service`; `postgres-service` kept as alias until DP-2, then removed | `econ-graph-postgres` | PV `econgraph-data-plane-pg` → `/srv/econgraph/data-plane/pg` |
| App (production) | `econ-graph` | `app-postgres` | `app-postgres-service` | `econ-graph-app-postgres` | PV `econgraph-app-prod-app-pg` → `/srv/econgraph/app-prod/app-pg` |
| Keycloak (production) | `econ-graph` | `keycloak-postgres` | `keycloak-postgres-service` | `econ-graph-keycloak` | PV `econgraph-app-prod-keycloak-pg` → `/srv/econgraph/app-prod/keycloak-pg` |
| App / Keycloak (branch) | `econ-graph-br-<slug>` | same names | same names | same names | dynamic local-path, disposable |
| Data (branch, `--local-data` only) | `econ-graph-br-<slug>` | `postgresql` | `data-postgres-service` | `econ-graph-postgres` | local-path, seeded from the latest verified primary dump |

FQDN form: `<service>.<namespace>.svc.cluster.local:5432`. Env var names and database names: Data plane section.

### Reconciliation notes (data plane PO)

These link section 9 to sections 1 to 8. The items marked **conflict** are open until the infra PO and the orchestrator agree.

- **Database names and URLs** (section 1): data `econ_graph` (unchanged, the live database), app `econ_graph_app`. The URLs are `postgres://…@data-postgres-service.<ns>.svc.cluster.local:5432/econ_graph` and `postgres://…@app-postgres-service.<ns>.svc.cluster.local:5432/econ_graph_app`. They reach binaries as Secret `econ-graph-secrets` keys `data-database-url` and `app-database-url`, written by `scripts/deploy/create-secrets.sh`, replacing today's `database-url`. The per-Postgres Secrets in the table above hold each server's own credentials.
- **App-only branches have no data URL and no route to the primary's Postgres.** No read-only role, no NetworkPolicy exception. Every data read in an app-only branch goes through `/data/graphql`: the frontend directly (section 10), and the backend's series check (DP-7) through `DATA_GRAPHQL_URL`.
- **Conflict: `DATA_GRAPHQL_URL` needs a gateway address reachable from inside the cluster.** Branch pods may not call `econ-graph` pods directly, and `<slug>.localhost` resolves to the pod itself, so the backend must call the Envoy proxy's Service with `Host: <slug>.localhost`. Needed from the infra PO:
  - a stable name for that Service (for example, set through `EnvoyProxy.spec.provider.kubernetes.envoyService.name`);
  - a branch-backend egress rule to it.

  Until then, DP-7 has `DATA_GRAPHQL_URL` (the proxy Service URL) and `DATA_GRAPHQL_HOST` (the Host header). If neither is set, the check is skipped (section 8).
- **Conflict: when `postgres-service` goes away.** The table says "until DP-2", but DP-2 and DP-4 have no fixed order. If DP-2 lands first, it points both URLs at the one database through `postgres-service`. Proposed: remove the alias only once DP-2 and DP-4 have both merged and nothing references it.

## 10. Frontend: data vs app GraphQL (DP-6, ECO-427)

Owner: product/UX PO.

All frontend GraphQL goes through `executeGraphQL` (`frontend/src/utils/graphql.ts`), which routes each operation by name using a classification table exported from that file. A test (`frontend/src/__tests__/graphql-operations.test.ts`) fails if any operation in `src/` is unclassified.

### Data operations: sent to the data URL, never with a token

Shipped: GetSeriesDetail, GetSeriesData, SearchSeries, GetSeriesList, GetDataSources, GetSeriesLatestObservation, GetMapDatasets, GetMapCrossSection, GetMapSeriesMeta.
Defined, not sent by any page: GetSearchSuggestions, SearchSeriesFulltext.
Compiled out (financial_components=false): GetFinancialDashboard, GetFinancialStatement, GetFinancialStatements, GetFinancialLineItems, GetCompanyInfo, GetTrendAnalysis, GetPeerComparison, GetPeerCompanies, GetBenchmarkComparison, GetFinancialAlerts, GetFinancialRatios, GetRatioBenchmarks, GetRatioExplanation, GetFinancialExport.

### App operations: sent to the GraphQL URL with `Authorization: Bearer <token>` when signed in

GetAnnotationsForSeries, GetCommentsForAnnotation, CreateAnnotation, UpdateAnnotation, DeleteAnnotation, AddComment; unused: GetChartCollaborators, GetUser, ShareChart; compiled out: GetFinancialAnnotations, Create/Update/DeleteFinancialAnnotation, FinancialAnnotationSubscription.
Admin (app endpoint, token required): GetCrawlerStatus (AdminSystemRead), TriggerCrawl (AdminCrawlersManage). These read crawl state, so on a backend with no data pool they fail with `DATA_SERVED_REMOTELY` (section 8).
Unnamed or unclassified operations, and every mutation/subscription, are app operations.

### No-token rule

Data operations never carry `Authorization`, signed in or not, whatever the data URL is. The backend answers an unverifiable token with 401 (not anonymous), and the primary data plane does not trust branch Keycloak realms. Data resolvers do not read the caller, so results are identical to today's.

### Endpoint configuration

| Where | `VITE_GRAPHQL_URL` (app) | `VITE_DATA_GRAPHQL_URL` (data) | What serves `/data/graphql` |
|---|---|---|---|
| Unset (default, release builds today) | `/graphql` | unset → same as app URL | not used |
| Vite dev / `vite preview` | `/graphql` → proxy to `BACKEND_URL` (default `http://localhost:8080`) | optional `/data/graphql` | proxy to `DATA_BACKEND_URL` (default `BACKEND_URL`), path rewritten to `/graphql` |
| kind, primary namespace `econ-graph` | `/graphql` | unset until DP-11 lands `/data/graphql`; then `/data/graphql` (open question) | HTTPRoute → `econ-graph-backend` in `econ-graph`, path rewritten to `/graphql` |
| Branch namespace, app-only (DP-8 default) | `/graphql` → branch backend | `/data/graphql` (build arg) | HTTPRoute → primary `econ-graph-backend` in `econ-graph` (cross-namespace; ReferenceGrant), rewritten to `/graphql` |
| Branch namespace, `--local-data` | `/graphql` → branch backend | `/data/graphql` or unset | HTTPRoute → the branch's own backend |

The value is a Vite build-time variable: `frontend/Dockerfile` build arg `VITE_DATA_GRAPHQL_URL`, set by `scripts/deploy/build-images.sh` (empty by default). Same-origin path only; no CORS/CSP change.

### Schema skew

The primary data plane runs a tagged release image. App-only branches validate data operations against `backend/crates/econ-graph-graphql/schema.graphql` at that tag (`DATA_SCHEMA_REF=<tag>`); a branch that changes the data schema must deploy with `--local-data`.
