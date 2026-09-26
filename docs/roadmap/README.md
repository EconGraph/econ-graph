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

## Topic roadmaps

| Topic | Doc | State |
|---|---|---|
| Auth, commercial plans, teams and fine-grained permissions | [auth-plans-permissions.md](./auth-plans-permissions.md) | Draft ([#176](https://github.com/EconGraph/econ-graph/pull/176)) |
| Admin UI | [admin-ui.md](./admin-ui.md) | Draft ([#177](https://github.com/EconGraph/econ-graph/pull/177)) |
| Postgres metadata federated with Arrow Flight / Parquet time series | [federation.md](./federation.md) | Draft ([#178](https://github.com/EconGraph/econ-graph/pull/178)) |
| Global analysis (world map, cross-country data) | [GLOBAL_ANALYSIS_ROADMAP.md](../development/GLOBAL_ANALYSIS_ROADMAP.md) | Needs a rewrite (see [Global analysis](#global-analysis)) |
| SEC EDGAR / XBRL financial data | [SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md](../development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md) | Needs a rewrite (see [SEC financial data](#sec-financial-data)) |
| Security hardening | [SECURITY_IMPLEMENTATION_PLAN.md](../projects/SECURITY_IMPLEMENTATION_PLAN.md) | Needs a rewrite (see [Security](#security)) |
| Product (user-facing features, business phases) | [ROADMAP.md](../business/ROADMAP.md) | Needs a rewrite (see [Product features](#product-features)) |

## What is built today

Checked against `main` at `1f17cc8` (2026-09-26).

**Backend.** The backend is a Rust workspace in `backend/crates` built on Axum,
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
and Logs. Logs runs on mock data.

**Auth.** Auth uses a custom HS256 JWT with no refresh token. Login works with
Google, Facebook, or email and password (bcrypt). The code defines three role
vocabularies that disagree with each other (details in
[auth-plans-permissions.md](./auth-plans-permissions.md)).

**Search.** Series search is `ILIKE` with a constant rank
(`econ-graph-services/src/services/search_service.rs`).
`docs/technical/FULLTEXT_SEARCH.md` describes a design that was never built.

**Time series storage.** Time series are stored in the Postgres `data_points` table.
The code has no Arrow, Parquet or Iceberg.

## Open work by area

These items come from the older docs and are still open. "Evidence" shows where the
code stands today.

### Security

The canonical findings list is
[FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md](../security/FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md).
`SECURITY_FIXES_SUMMARY.md` marks the JWT and CORS fixes as done. Those fixes were
made in the deleted `backend/src` and never reached the crates, so they are still
open.

| Item | Evidence |
|---|---|
| JWT secret falls back to a hardcoded default | `econ-graph-auth/src/auth/services.rs:17` |
| CORS allows any origin, and `cors.allowed_origins` is only logged | `econ-graph-backend/src/main.rs:269` and ingress `cors-allow-origin: "*"` |
| `/mcp` and `/playground` are unauthenticated | `econ-graph-backend/src/main.rs` |
| GraphQL depth, complexity and rate limits exist but are not enforced | `econ-graph-graphql/src/security/*` is never attached to the schema |
| Plaintext DB and monitoring credentials in k8s manifests | `k8s/manifests/postgres-deployment.yaml`, `configmap.yaml`, `ingress-cloudflare-dns01.yaml` |
| Sealed Secrets / secrets submodule not set up | `k8s/secrets` is uninitialized. `SECRETS_MANAGEMENT.md` describes a target state, not the current one |
| Tokens are stored in `localStorage` | `frontend/src/contexts/AuthContext.tsx` and `admin-frontend/src/contexts/AuthContext.tsx` |
| Terraform state is local, and provider binaries are committed | `terraform/k8s/.terraform/` |
| Security scans upload results but never fail the build | `.github/workflows/security.yml` |

Items about roles, permissions, MFA and session length belong to
[auth-plans-permissions.md](./auth-plans-permissions.md).

### Admin UI

[admin-ui.md](./admin-ui.md) owns this area and supersedes the older docs below. Inputs it draws on:

- [CRAWLER_ADMIN_PLAN.md](../../admin-frontend/docs/features/CRAWLER_ADMIN_PLAN.md).
  About 20 of the mutations it calls, such as start/stop crawler, data source
  CRUD and queue control, have no backend resolver.
- [GITHUB_ISSUE_CRAWLER_LOGS.md](../../GITHUB_ISSUE_CRAWLER_LOGS.md). No
  `crawler_logs` query exists. `crawl_attempts` could back a logs view.
- [ADMIN_SECURITY.md](../technical/ADMIN_SECURITY.md).
- `UserManagementPage`, `MonitoringPage`, `SystemHealthPage` and `LoginPage` exist in
  `admin-frontend/src/pages` but are not mounted in `App.tsx`.
- `AuthContext` calls `/api/admin/auth/*` endpoints that do not exist.

### Data federation and time series storage

[federation.md](./federation.md) owns this area. No older doc covers it. The stale federation PRs #125 and #129 are reviewed there.

### Global analysis

- **The backend exists but nothing can reach it.** `GlobalAnalysisQuery`
  (`econ-graph-graphql/src/graphql/global_analysis.rs`) is never merged into the root
  `Query`. Registering it is step one.
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

- **The SEC plan's "complete" GraphQL claim is false.** There is no GraphQL API for
  companies, statements or ratios, and no subscriptions (`EmptySubscription`).
- **The ratio calculator has no API.** `financial_ratio_calculator.rs` exists, but
  nothing exposes its results.
- **The financial UI is orphaned.** `frontend/src/components/financial/*` has no
  route. Its components query mock GraphQL documents the backend does not serve.
  [PERFORMANCE_TODO.md](../../PERFORMANCE_TODO.md) covers these components.
- **Still open:** MCP financial tools, financial E2E specs, and the
  "educational" features.

### Data sources

- Implement fetch for BEA, IMF and World Bank.
- Replace the static catalogs (ECB, OECD, BoE and others) with real adapters, or
  drop them.
- Add a per-source data freshness dashboard (`economic-data-crawlers.json` was
  planned and never built).
- Verify the per-source rate limits in `policy.rs` against provider terms.

### Search

- Build real full-text search (tsvector, pg_trgm, synonyms). Draft
  [#165](https://github.com/EconGraph/econ-graph/pull/165) covers the indexing. After
  it lands, rewrite `FULLTEXT_SEARCH.md` to match.

### Product features

These come from `business/ROADMAP.md`, `NEAR_TERM_FEATURES.md` and
`INVESTOR_PITCH.md`. Items owned by the auth, admin UI and federation roadmaps are
left out.

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

## Doc inventory

The verdict column says what should happen to each older doc. **Archive** means move
it to `docs/archive/` with a note that it is historical. **Delete** means it is empty
or duplicates another doc. **Update** means the doc is still useful but out of date.
The moves and deletions are a separate follow-up change, so the links above keep
working until then.

### Roadmaps and plans

| Doc | Verdict | Why |
|---|---|---|
| `docs/business/ROADMAP.md` | Update | The product roadmap. Its 2025 dates have passed, and its statuses are superseded by this index |
| `docs/development/NEAR_TERM_FEATURES.md` | Archive | Duplicates ROADMAP phases 1 to 3. The file paths it proposes don't exist |
| `docs/development/GLOBAL_ANALYSIS_ROADMAP.md` | Update | Keep as the one global-analysis roadmap and add the wiring steps above |
| `docs/development/GLOBAL_ANALYSIS_UI_ROADMAP.md` | Delete | Duplicates phases 2 to 5 of the doc above |
| `docs/projects/frontend-developer-global-analysis-plan.md` | Archive | Week 1 is done. Its test-suite claim is false |
| `docs/projects/global-analysis-ui-phase1.md` | Delete | The same plan as the doc above with every box unchecked |
| `docs/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md` | Update | 2,835 lines with false completion claims. Rewrite it as a short SEC roadmap |
| `docs/development/vitest-migration-plan.md` | Archive | The migration is complete |
| `docs/technical/CRATE_SPLIT_PLAN.md` | Archive | The split is complete |
| `docs/technical/CRAWLER_MONITORING_IMPLEMENTATION_PLAN.md` | Archive | Mostly done. Its paths are stale |
| `admin-frontend/docs/features/CRAWLER_ADMIN_PLAN.md` | Archive | Superseded by [admin-ui.md](./admin-ui.md) |
| `docs/projects/SECURITY_IMPLEMENTATION_PLAN.md` | Update | Phase 1 is still open. Repoint its paths at `backend/crates` |
| `docs/business/FINANCIAL_DATA_USER_RESEARCH_PLAN.md` | Keep | An unstarted research plan. Its pricing questions feed plan design |
| `PERFORMANCE_TODO.md` (root) | Update | Move it under `docs/` with the SEC UI work |
| `GITHUB_ISSUE_CRAWLER_LOGS.md` (root) | Delete | Superseded by [admin-ui.md](./admin-ui.md) |
| `docs/development/CI_OPTIMIZATION_NOTES.md` | Update | Its open items are listed under CI above |
| `super-secret-projects/NIGHTLY_RESEARCH_PLAN.md` | Archive | A one-off research plan |
| `super-secret-projects/EARNINGS_TRANSCRIPT_ANALYSIS.md`, `STT_TESTING_RESULTS.md` | Keep | An idea backlog. No code exists |

### Status reports and assessments

| Doc | Verdict | Why |
|---|---|---|
| `docs/business/PRODUCT_SUMMARY_2025.md` | Update | Claims OECD data and real-time features that don't exist. Its bad metrics ("0 lines", "99.9%") are rewritten daily by `scripts/update-cost-analysis.sh`, so fix or drop that script step before archiving it |
| `docs/business/INVESTOR_PITCH.md` | Update | Its roadmap and pricing belong in ROADMAP and the plans doc. Its customer quotes have no source and should be removed before it is shared |
| `docs/technical/GLOBAL_ANALYSIS_SUMMARY.md` | Archive | A third copy of the global roadmap. Its Jest references are stale |
| `docs/technical/GLOBAL_ANALYSIS_FEATURES.md` | Update | Cut it down to what is actually built |
| `docs/technical/FRONTEND_SUMMARY.md` | Update | Its stack section still lists Jest, Cypress and React Router 6 |
| `docs/technical/FULLTEXT_SEARCH.md` | Update | Describes search that was never built. Revisit after #165 |
| `docs/technical/MCP_SERVER_ANALYSIS.md` | Archive | The bug it describes is fixed |
| `docs/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md` | Archive | Its open follow-ups are listed under Data sources above |
| `docs/technical/RATE_LIMIT_SOURCES.md` | Update | Repoint it at `econ-graph-crawler/src/policy.rs` |
| `docs/technical/SECRETS_MANAGEMENT.md` | Update | Label it as a plan. None of it exists yet |
| `docs/technical/ADMIN_SECURITY.md` | Archive | Superseded by [admin-ui.md](./admin-ui.md) and [auth-plans-permissions.md](./auth-plans-permissions.md). MFA and 30-minute sessions are still open |
| `docs/projects/SECURITY_FINDINGS_REPORT.md` | Archive | Superseded by the FINAL report |
| `docs/security/FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md` | Update | The canonical findings list. Add a status column |
| `docs/security/SECURITY_ASSESSMENT_REPORT.md` | Delete | An earlier draft of the FINAL report |
| `docs/security/COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md` | Delete | Identical to the FINAL report, minus its frontend findings |
| `docs/security/SECURITY_FIXES_SUMMARY.md` | Archive | Its JWT and CORS "fixed" claims no longer hold |
| `docs/security/SECURITY_IMPLEMENTATION_SUMMARY.md` | Update | Describes GraphQL security as live, but it isn't wired in |
| `docs/security/ARCHITECTURE_DESIGN.md`, `IMPLEMENTATION_DETAILS.md`, `API_REFERENCE.md`, `DEPLOYMENT_GUIDE.md` | Archive | Four overlapping docs for one module. Fold what is useful into `GRAPHQL_SECURITY_GUIDE.md` |
| `docs/security/GRAPHQL_SECURITY_GUIDE.md` | Update | Keep as the one GraphQL-security doc. Add a note that it is not enforced yet |
| `CLOUDFLARE_INTEGRATION_STATUS.md` (root) | Archive | Its open items are listed under CI above |
| `LETSENCRYPT_CLOUDFLARE_INTEGRATION_SUMMARY.md` (root) | Delete | Duplicates `docs/deployment/LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md` |
| `docs/deployment/PRIVATE_CHART_API.md` | Delete | Superseded by `CHART_API_SERVICE.md` |
| `docs/monitoring/README.md` | Update | Its metric names don't match the code |
| `frontend/docs/storybook-testing.md` | Update | Claims Storybook tests run in CI. Its related-doc links are broken |
| `ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md`, `E2E_TEST_FAILURE_ANALYSIS.md` | Archive | Point-in-time incident notes from September 2025 |
| `ci/docs/workflow-status-report.md` | Keep | Generated by `scripts/fix-github-workflow-cache.sh` |
| `progress-reports/2025-09-13/*` | Archive | Retrospectives. The crawler work they describe was superseded by #157 |
| `super-secret-projects/EDGAR_INGESTION.md` | Delete | Empty |
| `super-secret-projects/EDGAR_INTEGRATION_ANALYSIS.md`, `EDGAR_XBRL_RESEARCH_FINDINGS.md`, `STEALTH_CRAWLING_VS_POLITE_CRAWLING.md` | Archive | Research superseded by `econ-graph-sec-crawler` |

### Other docs with stale roadmap sections

- **`README.md` (root).** Its project structure still shows `backend/src`. The "Development Cost Transparency" block appears twice, and the file ends with trailing "CI trigger" lines.
- **`personas/frontend-developer.md`.** Its "Current Project Focus" section repeats the global roadmap, and its testing guidance still uses Jest.
- **`personas/backend-engineer.md`.** Its crate list is out of date, and "Future Considerations" appears twice.
- **`personas/security-engineer.md`.** It cites `backend/src/auth/services.rs`.

Guides not listed here, such as deployment, testing and API reference docs, are not
roadmaps and were left alone.
