# EconGraph Roadmap

This is the single entry point for EconGraph's roadmap. The project sat idle from
October 2025 to September 2026. The roadmap and plan docs written before that pause
were scattered across `docs/`, the repo root, `admin-frontend/`, `personas/` and
`super-secret-projects/`. Several of them duplicate each other, and some claim work
is finished when the code shows otherwise.

This index:

1. Links the current topic roadmaps.
2. Records what is actually built, checked against the code on 2026-09-26.
3. Collects the open work that is still worth doing from the older docs.
4. Lists every older plan or status doc with a verdict: keep, update, archive or delete.

When a topic roadmap changes, update its row here. New roadmaps go in `docs/roadmap/<topic>.md`.

Each topic doc lands in its own pull request, linked in the State column. Until that pull
request merges, its Doc link does not resolve; read the pull request instead.

## Topic roadmaps

| Topic | Doc | State |
|---|---|---|
| Auth, commercial plans, teams and fine-grained permissions | [auth-plans-permissions.md](./auth-plans-permissions.md) | Draft ([#176](https://github.com/EconGraph/econ-graph/pull/176)) |
| Admin UI | [admin-ui.md](./admin-ui.md) | Draft ([#177](https://github.com/EconGraph/econ-graph/pull/177)) |
| Postgres metadata federated with Arrow Flight / Parquet time series | [federation.md](./federation.md) | Draft ([#178](https://github.com/EconGraph/econ-graph/pull/178)) |
| Global analysis (world map, cross-country data) | [global-analysis.md](./global-analysis.md) | Draft ([#188](https://github.com/EconGraph/econ-graph/pull/188)) |
| Release trains: what ships in which release | [releases.md](./releases.md) | Draft ([#190](https://github.com/EconGraph/econ-graph/pull/190)) |
| Data sources: which sources, in what order | [data-sources.md](./data-sources.md) | Proposed ([#192](https://github.com/EconGraph/econ-graph/pull/192)) |
| Analysis workspace: multi-series charts, saved charts, export | [analysis-workspace.md](./analysis-workspace.md) | Accepted ([#191](https://github.com/EconGraph/econ-graph/pull/191)) |
| SEC EDGAR / XBRL financial data | [sec-financial-data.md](./sec-financial-data.md) | Draft ([#193](https://github.com/EconGraph/econ-graph/pull/193)) |
| Feature flags and experiments | [feature-flags.md](./feature-flags.md) | Proposed ([#195](https://github.com/EconGraph/econ-graph/pull/195)) |
| Security hardening | [SECURITY_IMPLEMENTATION_PLAN.md](../projects/SECURITY_IMPLEMENTATION_PLAN.md) | Needs a rewrite (see [Security](#security)) |
| Product (user-facing features, business phases) | [ROADMAP.md](../business/ROADMAP.md) | Needs a rewrite (see [Product features](#product-features)) |

### Direction set by the topic roadmaps

- **Auth and permissions ([#176](https://github.com/EconGraph/econ-graph/pull/176)).**
  The plan is fine-grained RBAC: the backend will check only `resource:action` roles
  carried in the token. Keycloak is planned for Phase 2 and is not in place yet. Once
  adopted, it will compose those roles into plans, organization roles and staff
  levels. The phases are:
  - 0 Hygiene, including CORS (backend CORS done in #182)
  - 1 Fine-grained role catalog
  - 2 Keycloak as the identity provider
  - 3 Organizations and teams
  - 4 Plans and limits
  - 5 Billing (deferred)
  - 6 Machine access and MCP OAuth
  - 7 Staff tooling
  - 8 Enterprise identity
- **Federation ([#178](https://github.com/EconGraph/econ-graph/pull/178)).** Time
  series data will not live in Postgres. Series metadata and the SEC/XBRL financial
  data stay in Postgres. Observations are served from Arrow Flight / Parquet.
- **Admin UI ([#177](https://github.com/EconGraph/econ-graph/pull/177)).** Covers the
  crawler admin pages, user and role management, and the unmounted admin pages
  listed [below](#admin-ui).
- **Feature flags ([#195](https://github.com/EconGraph/econ-graph/pull/195)).** OpenFeature
  with flagd, flags checked into the repo as JSON. Flags are for release flags and kill
  switches only. Entitlements stay fine-grained roles and source switches stay in
  `data_sources`. Code a topic roadmap replaces is deleted, not flagged. Experiments
  wait for analytics and traffic.

## What is built today

Checked against `main` at `eb7634d` (2026-09-26), which includes the JWT secret (#180), token subject (#181), backend CORS (#182) and `/mcp` token (#185) fixes.

**Backend.** The backend is a Rust workspace in `backend/crates` built on warp,
async-graphql and Diesel on Postgres. Its crates are core, services, graphql, auth,
crawler, crawler-worker, sec-crawler, mcp, metrics and backend. The crate split in
`CRATE_SPLIT_PLAN.md` is complete. The pre-workspace trees `backend/src` and
`backend/backend` were deleted in #174. Any older doc that points at those paths is
stale.

**Data ingestion.** The queue-based crawler (#157) has these sources in
`econ-graph-crawler/src/sources/`:

| Source | What works |
|---|---|
| FRED, BLS, Census BDS (national and per state, #175), FHFA | Discovery and observation fetch |
| BEA, IMF, World Bank | Discovery only. `fetch_series` is not implemented |
| BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats, WTO | Hardcoded catalogs in `static_catalogs.rs`. Fetch always fails |
| SEC EDGAR | Runs through `econ-graph-sec-crawler`, which `crawler-worker` calls |

Per-source rate limits live in `econ-graph-crawler/src/policy.rs`.

**Crawler monitoring.** The following are in place:

- Metrics: `econ-graph-metrics` and `crawler-worker/src/metrics.rs`
- Grafana dashboards: `grafana-dashboards/`
- Alert rules: `k8s/monitoring/prometheus-rules-crawler.yaml`

**Frontend.** The main frontend is React and Vite. The Jest-to-Vitest migration is
finished, and Storybook and Playwright are set up. The global-analysis world map
and the chart collaboration features (annotations, comments, sharing) are built.

**Admin frontend.** The admin frontend mounts three crawler pages: Dashboard, Config
and Logs. Logs queries a `crawlerLogs` field the backend does not have, and its
performance metrics are random mock values.

**Auth.** Auth uses a custom HS256 JWT with no refresh token. Login works with
Google, Facebook, or email and password (bcrypt). The code defines three role
vocabularies that disagree with each other (details in
[auth-plans-permissions.md](./auth-plans-permissions.md)).

**Search.** Series search is `ILIKE` with a two-value rank (1.0 for a title match,
0.5 for a description-only match) (`econ-graph-services/src/services/search_service.rs`).
[#165](https://github.com/EconGraph/econ-graph/pull/165) adds a weighted tsvector and
pg_trgm index and rewrites `docs/technical/FULLTEXT_SEARCH.md` to match.

**Time series storage.** Time series are stored in the Postgres `data_points` table.
The code has no Arrow, Parquet or Iceberg. The database target is PostgreSQL 18
([#159](https://github.com/EconGraph/econ-graph/pull/159)).

## Open work by area

These items come from the older docs and are still open. "Evidence" shows where the
code stands today.

### Security

The canonical findings list is
[FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md](../security/FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md).
`SECURITY_FIXES_SUMMARY.md` marks the JWT and CORS fixes as done. Those fixes were
made in the deleted `backend/src` and never reached the crates. They were redone in
the crates by [#180](https://github.com/EconGraph/econ-graph/pull/180) (JWT secret)
and [#182](https://github.com/EconGraph/econ-graph/pull/182) (backend CORS).
[#185](https://github.com/EconGraph/econ-graph/pull/185) added a token check on `/mcp`.
[#227](https://github.com/EconGraph/econ-graph/pull/227) removed every credential
committed under `k8s/`; each component reads Secrets created by
`scripts/deploy/create-secrets.sh` (see the Secrets section of `k8s/README.md`).

| Item | Evidence |
|---|---|
| Two ingress variants still allow any CORS origin (the backend no longer does, since #182) | `cors-allow-origin: "*"` in `k8s/manifests/ingress.yaml` and `ingress-cloudflare-dns01.yaml` |
| `/playground` is unauthenticated. `/mcp` requires a signed-in user's token since #185, a stopgap until MCP OAuth (Phase 6) | `econ-graph-backend/src/main.rs` (`graphql_playground`, `mcp_route`) |
| GraphQL depth, complexity and rate limits exist but are not enforced | `/graphql` calls `schema.execute` directly in the `graphql_filter` closure (`econ-graph-backend/src/main.rs`; the `graphql_handler` function there is unused). Nothing calls `SecureGraphQLServer::execute_secure_request` in `econ-graph-graphql/src/security/server.rs` |
| The Google and Facebook credentials committed before #227 are still in git history and must be rotated | git history of `k8s/manifests/configmap.yaml` |
| Sealed Secrets / secrets submodule not set up | `k8s/secrets` is uninitialized. `SECRETS_MANAGEMENT.md` describes a target state, not the current one |
| Tokens are stored in `localStorage` | `frontend/src/contexts/AuthContext.tsx` and `admin-frontend/src/contexts/AuthContext.tsx` |
| Terraform state files and provider binaries are committed to git | `terraform/k8s/terraform.tfstate`, `terraform.tfstate.backup`, `terraform/k8s/.terraform/` |
| Security scans upload results but never fail the build | `.github/workflows/security.yml` |

The ingress CORS item is scheduled in Phase 0 (Hygiene), and OAuth for `/mcp` in
Phase 6, of [auth-plans-permissions.md](./auth-plans-permissions.md). Items about
roles, permissions, MFA and session length belong there too.

### Admin UI

[admin-ui.md](./admin-ui.md) owns this area and supersedes the older docs below. Inputs it draws on:

- [CRAWLER_ADMIN_PLAN.md](../../admin-frontend/docs/features/CRAWLER_ADMIN_PLAN.md).
  About 20 of the mutations it calls, such as start/stop crawler, data source
  CRUD and queue control, have no backend resolver.
- [GITHUB_ISSUE_CRAWLER_LOGS.md](../../GITHUB_ISSUE_CRAWLER_LOGS.md). No
  `crawler_logs` query exists. `crawl_attempts` could back a logs view.
- [ADMIN_SECURITY.md](../technical/ADMIN_SECURITY.md).
- `UserManagementPage`, `MonitoringPage`, `SystemHealthPage`, `DashboardPage` and
  `auth/LoginPage` exist in `admin-frontend/src/pages` but are not mounted in `App.tsx`.
- `AuthContext` calls `/api/admin/auth/*` endpoints that do not exist.

### Data federation and time series storage

[federation.md](./federation.md) owns this area. No older doc covers it. The stale federation PRs #125 and #129 are reviewed there.

### Global analysis

[global-analysis.md](./global-analysis.md) owns this area and supersedes the older
global-analysis docs. The findings below are its inputs.

- **The backend exists but nothing can reach it.** `GlobalAnalysisQuery`
  (`econ-graph-graphql/src/graphql/global_analysis.rs`) is never merged into the root
  `Query`. Registering it as-is would expose `calculateCountryCorrelations`, whose results
  come from `calculate_pairwise_correlation` in `global_analysis_service.rs`, which
  returns hard-coded values (0.75, p = 0.01). [#188](https://github.com/EconGraph/econ-graph/pull/188)
  recommends a generic `crossSection` query instead and deleting
  `GlobalAnalysisQuery`, and [releases.md](./releases.md) agrees. That choice is
  Joe's to confirm.
- **The frontend runs on sample data.** Every global component uses hardcoded data
  (`MultiCountryDashboard`, `GlobalEventsExplorer`, `data/sampleCountryData.ts`). The
  queries in `frontend/src/utils/graphql.ts` are defined but unused.
- **Nothing loads the global tables.** `global_indicator_data`,
  `trade_relationships` and the events tables have no loader. Only 20 countries are
  seeded. `calculate_country_correlations` is never scheduled.
- **No unit tests exist for `components/global/*`.** An older plan claimed 100 or
  more, but none were ever committed.
- **Still open:** map animation, map export, trade-flow arrows, event overlays,
  statistical tools and forecasting.

### SEC financial data

[sec-financial-data.md](./sec-financial-data.md) owns this area and supersedes
`SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md`. The findings below are its inputs.

- **The XBRL parser is not on the live path.** The crawler stores the raw filings as
  zstd blobs (`econ-graph-sec-crawler/src/storage.rs`). The handler never runs
  `xbrl_parser.rs`, so no statements or line items are loaded from live crawls.
- **The SEC plan's "complete" GraphQL claim is false.** There is no GraphQL API for
  companies, statements or ratios, and no subscriptions (`EmptySubscription`).
- **The ratio calculator has no API.** `financial_ratio_calculator.rs` exists, but
  nothing exposes its results.
- **The financial UI is orphaned.** `frontend/src/components/financial/*` has no
  route. Its components query mock GraphQL documents the backend does not serve.
  [PERFORMANCE_TODO.md](../../PERFORMANCE_TODO.md) covers these components.
  Joe decided to compile them out of the release build rather than delete them
  ([#193](https://github.com/EconGraph/econ-graph/pull/193), decision 7).
- **Still open:** MCP financial tools, financial E2E specs, and the
  "educational" features.

### Data sources

[data-sources.md](./data-sources.md) owns this area. It proposes a source order, a
generic SDMX adapter, BIS for central-bank data, and deleting the static catalogs.
The findings below are its inputs.

- Implement fetch for BEA, IMF and World Bank.
- Replace the static catalogs (ECB, OECD, BoE and others) with real adapters, or
  drop them.
- Add a per-source data freshness dashboard (`economic-data-crawlers.json` was
  planned and never built).
- Verify the per-source rate limits in `policy.rs` against provider terms.

### Search

- Land [#165](https://github.com/EconGraph/econ-graph/pull/165) (weighted tsvector
  and pg_trgm search). Its rewrite of `docs/technical/FULLTEXT_SEARCH.md` lists the
  remaining gaps, such as synonyms.

### Product features

These come from `business/ROADMAP.md`, `NEAR_TERM_FEATURES.md` and
`INVESTOR_PITCH.md`. Items owned by the auth, admin UI and federation roadmaps are
left out. Chart export and saved charts are covered by
[analysis-workspace.md](./analysis-workspace.md), and [releases.md](./releases.md)
says which release each item lands in.

| Item | Status |
|---|---|
| User profile page | Partial: a dialog in `UserProfile.tsx`, no route |
| Real-time collaboration updates | Open: no GraphQL subscriptions |
| Data quality dashboard | Open |
| Chart export (PNG/SVG/PDF) | Open: `ProfessionalChart.tsx` `exportChart` is empty |
| Saved searches, history, favorites | Open |
| Statistical analysis, regression, forecasting | Open |
| REST API and webhooks | Open: only auth routes are REST |
| Audit logging coverage | Partial: `audit_logs` table exists, write coverage unknown |
| Mobile and WCAG accessibility | Partial |
| ML, NL queries, alerts (roadmap phases 4 to 6) | Open, long term |

### CI and tooling

- CI speed items from [CI_OPTIMIZATION_NOTES.md](../development/CI_OPTIMIZATION_NOTES.md):
  - A composite setup action.
  - Using the prebuilt `ci/docker/Dockerfile.test-runner*` images, which exist but
    are unused.
- Storybook tests are not run in CI, although
  [storybook-testing.md](../../frontend/docs/storybook-testing.md) says they are.
- `ci-core.yml` compares `github.event.inputs.run_e2e_tests == true`, which compares
  a string to a boolean. Check whether manual E2E runs actually trigger.
- The monitoring docs expect metric names (`econgraph_queue_items*`) that the code
  doesn't emit, and kind deployments load no alert rules.
- Certificates: several ingress variants compete, and renewal testing and cert
  alerting are open (from `CLOUDFLARE_INTEGRATION_STATUS.md`).
- `frontend/package.json` runs `privateChartServer.js`, but the file is `.cjs`, and
  the server is superseded by `chart-api-service/`.

## Stale pull requests from 2025

Each topic roadmap reviews the open 2025 PRs in its area and says what to keep:

| PR | Recommendation | Where |
|---|---|---|
| #141 Keycloak / mTLS, #149 JWT permissions | Close | [auth-plans-permissions.md](./auth-plans-permissions.md) |
| #151 SEC admin UI | Close once its admin-frontend half is ported | [admin-ui.md](./admin-ui.md) |
| #125 financial-data federation, #129 Iceberg | Close | [federation.md](./federation.md) |
| #154 global analysis UI improvements | Close and port its tests | [global-analysis.md](./global-analysis.md) |

## Doc inventory

The verdict column says what happened, or should happen, to each older doc.
**Archived** docs were moved to [`docs/archive/`](../archive/README.md) and are kept
for history only. **Deleted** docs were empty or duplicated another doc; git history
still has them. **Archive** and **Delete** mark docs that wait on a topic roadmap
merging or on content being folded elsewhere first. **Update** means the doc is still
useful but out of date.

### Roadmaps and plans

| Doc | Verdict | Why |
|---|---|---|
| `docs/business/ROADMAP.md` | Update | The product roadmap. Its 2025 dates have passed, and its statuses are superseded by this index |
| [`docs/development/NEAR_TERM_FEATURES.md`](../archive/development/NEAR_TERM_FEATURES.md) | Archived | Duplicates ROADMAP phases 1 to 3. The file paths it proposes don't exist |
| `docs/development/GLOBAL_ANALYSIS_ROADMAP.md` | Archive | Superseded by [global-analysis.md](./global-analysis.md) |
| `docs/development/GLOBAL_ANALYSIS_UI_ROADMAP.md` | Deleted | Duplicates phases 2 to 5 of the doc above |
| [`docs/projects/frontend-developer-global-analysis-plan.md`](../archive/projects/frontend-developer-global-analysis-plan.md) | Archived | Week 1 is done. Its test-suite claim is false |
| `docs/projects/global-analysis-ui-phase1.md` | Deleted | The same plan as the doc above with every box unchecked |
| [`docs/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md`](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md) | Archived | Superseded by [sec-financial-data.md](./sec-financial-data.md). Its 2,835 lines include false completion claims |
| [`docs/development/vitest-migration-plan.md`](../archive/development/vitest-migration-plan.md) | Archived | The migration is complete |
| [`docs/technical/CRATE_SPLIT_PLAN.md`](../archive/technical/CRATE_SPLIT_PLAN.md) | Archived | The split is complete |
| [`docs/technical/CRAWLER_MONITORING_IMPLEMENTATION_PLAN.md`](../archive/technical/CRAWLER_MONITORING_IMPLEMENTATION_PLAN.md) | Archived | Mostly done. Its paths are stale |
| `admin-frontend/docs/features/CRAWLER_ADMIN_PLAN.md` | Archive | Superseded by [admin-ui.md](./admin-ui.md) |
| `docs/projects/SECURITY_IMPLEMENTATION_PLAN.md` | Update | Phase 1 is still open. Repoint its paths at `backend/crates` |
| `docs/business/FINANCIAL_DATA_USER_RESEARCH_PLAN.md` | Keep | An unstarted research plan. Its pricing questions feed plan design |
| `PERFORMANCE_TODO.md` (root) | Keep | Tunes only `components/financial`. It stays with those components, even if they are compiled out rather than shipped |
| `GITHUB_ISSUE_CRAWLER_LOGS.md` (root) | Delete | Superseded by [admin-ui.md](./admin-ui.md) |
| `docs/development/CI_OPTIMIZATION_NOTES.md` | Update | Its open items are listed under CI above |
| `super-secret-projects/NIGHTLY_RESEARCH_PLAN.md` | Archive | A one-off research plan |
| `super-secret-projects/EARNINGS_TRANSCRIPT_ANALYSIS.md`, `STT_TESTING_RESULTS.md` | Keep | An idea backlog. No code exists |

### Status reports and assessments

| Doc | Verdict | Why |
|---|---|---|
| `docs/business/PRODUCT_SUMMARY_2025.md` | Update | Claims OECD data and real-time features that don't exist. Its bad metrics ("0 lines", "99.9%") are rewritten daily by `scripts/update-cost-analysis.sh`, so fix or drop that script step before archiving it |
| `docs/business/INVESTOR_PITCH.md` | Update | Its roadmap and pricing belong in ROADMAP and the plans doc. Its customer quotes have no source and should be removed before it is shared |
| [`docs/technical/GLOBAL_ANALYSIS_SUMMARY.md`](../archive/technical/GLOBAL_ANALYSIS_SUMMARY.md) | Archived | A third copy of the global roadmap. Its Jest references are stale |
| `docs/technical/GLOBAL_ANALYSIS_FEATURES.md` | Update | Cut it down to what is actually built |
| `docs/technical/FRONTEND_SUMMARY.md` | Update | Its stack section still lists Jest, Cypress and React Router 6 |
| `docs/technical/FULLTEXT_SEARCH.md` | Keep | Will be updated by [#165](https://github.com/EconGraph/econ-graph/pull/165) to describe search as built and list the gaps |
| [`docs/technical/MCP_SERVER_ANALYSIS.md`](../archive/technical/MCP_SERVER_ANALYSIS.md) | Archived | The bug it describes is fixed |
| [`docs/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md`](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md) | Archived | Its open follow-ups are listed under Data sources above |
| `docs/technical/RATE_LIMIT_SOURCES.md` | Update | Repoint it at `econ-graph-crawler/src/policy.rs` |
| `docs/technical/SECRETS_MANAGEMENT.md` | Update | Label it as a plan. None of it exists yet |
| `docs/technical/ADMIN_SECURITY.md` | Archive | Superseded by [admin-ui.md](./admin-ui.md) and [auth-plans-permissions.md](./auth-plans-permissions.md). MFA and 30-minute sessions are still open |
| [`docs/projects/SECURITY_FINDINGS_REPORT.md`](../archive/projects/SECURITY_FINDINGS_REPORT.md) | Archived | Superseded by the FINAL report |
| `docs/security/FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md` | Update | The canonical findings list. Add a status column |
| `docs/security/SECURITY_ASSESSMENT_REPORT.md` | Deleted | An earlier draft of the FINAL report |
| `docs/security/COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md` | Deleted | Identical to the FINAL report, minus its frontend findings |
| [`docs/security/SECURITY_FIXES_SUMMARY.md`](../archive/security/SECURITY_FIXES_SUMMARY.md) | Archived | Its JWT and CORS fixes were in the deleted `backend/src`; #180 and #182 redid them in the crates |
| `docs/security/SECURITY_IMPLEMENTATION_SUMMARY.md` | Update | Describes GraphQL security as live, but it isn't wired in |
| `docs/security/ARCHITECTURE_DESIGN.md`, `IMPLEMENTATION_DETAILS.md`, `API_REFERENCE.md`, `DEPLOYMENT_GUIDE.md` | Archive | Four overlapping docs for one module. Fold what is useful into `GRAPHQL_SECURITY_GUIDE.md` |
| `docs/security/GRAPHQL_SECURITY_GUIDE.md` | Update | Keep as the one GraphQL-security doc. Add a note that it is not enforced yet |
| [`CLOUDFLARE_INTEGRATION_STATUS.md`](../archive/CLOUDFLARE_INTEGRATION_STATUS.md) (root) | Archived | Its open items are listed under CI above |
| `LETSENCRYPT_CLOUDFLARE_INTEGRATION_SUMMARY.md` (root) | Deleted | Duplicates `docs/deployment/LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md` |
| `docs/deployment/PRIVATE_CHART_API.md` | Deleted | Superseded by `CHART_API_SERVICE.md` |
| `docs/monitoring/README.md` | Update | Its metric names don't match the code |
| `frontend/docs/storybook-testing.md` | Update | Claims Storybook tests run in CI. Its related-doc links are broken |
| [`ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md`](../archive/ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md), [`E2E_TEST_FAILURE_ANALYSIS.md`](../archive/ci/docs/E2E_TEST_FAILURE_ANALYSIS.md) | Archived | Point-in-time incident notes from September 2025 |
| `ci/docs/workflow-status-report.md` | Keep | Generated by `scripts/fix-github-workflow-cache.sh` |
| [`progress-reports/2025-09-13/*`](../archive/progress-reports/2025-09-13/) | Archived | Retrospectives. The crawler work they describe was superseded by #157 |
| `super-secret-projects/EDGAR_INGESTION.md` | Deleted | Empty |
| `super-secret-projects/EDGAR_INTEGRATION_ANALYSIS.md`, `EDGAR_XBRL_RESEARCH_FINDINGS.md`, `STEALTH_CRAWLING_VS_POLITE_CRAWLING.md` | Archive | Research superseded by `econ-graph-sec-crawler` |

### Other docs with stale roadmap sections

- **`README.md` (root).** Its project structure still shows `backend/src` and calls the backend Axum (it is warp). The "Development Cost Transparency" block appears twice, and the file ends with trailing "CI trigger" lines.
- **`personas/frontend-developer.md`.** Its "Current Project Focus" section repeats the global roadmap, and its testing guidance still uses Jest.
- **`personas/backend-engineer.md`.** Its crate list is out of date, and "Future Considerations" appears twice.
- **`personas/security-engineer.md`.** It cites `backend/src/auth/services.rs`.

Guides not listed here, such as deployment, testing and API reference docs, are not
roadmaps and were left alone.
