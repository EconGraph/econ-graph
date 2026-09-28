# EconGraph Roadmap

> **Moved to Linear on 2026-09-27.** Roadmaps and release plans now live in
> [Linear](https://linear.app/econgraph). Releases are issues ECO-5 (train 1, `v4.0.0`)
> through ECO-11 (train 7), and each topic is a Linear project. This index is frozen as
> of that date: its State column, the "What is built today" snapshot and the open-work
> lists are no longer updated, and where they differ from Linear, Linear is current. The
> stale-PR recommendations and the doc inventory below are kept only here.

Linear projects:

- [Auth, plans and permissions](https://linear.app/econgraph/project/auth-plans-and-permissions-852b7217015b)
- [Admin UI](https://linear.app/econgraph/project/admin-ui-ad2b8b56fe3f)
- [Federation and datasets](https://linear.app/econgraph/project/federation-and-datasets-2d6e3c5159f8)
- [Global analysis](https://linear.app/econgraph/project/global-analysis-4d1fbf9d5143)
- [Releases and release engineering](https://linear.app/econgraph/project/releases-and-release-engineering-ee56b60d9c7a)
- [Data sources](https://linear.app/econgraph/project/data-sources-7b8a25b3313e)
- [Analysis workspace](https://linear.app/econgraph/project/analysis-workspace-e2c63872061b), which also holds the product features
- [SEC financial data](https://linear.app/econgraph/project/sec-financial-data-63d758410e88)
- [Feature flags](https://linear.app/econgraph/project/feature-flags-bc727011a0f7)
- [Security hardening](https://linear.app/econgraph/project/security-hardening-7a081ff9105c)
- [CI and tooling](https://linear.app/econgraph/project/ci-and-tooling-cd91fbb406c6)

This was the single entry point for EconGraph's roadmap until the move to Linear. The project sat idle from
October 2025 to September 2026. The roadmap and plan docs written before that pause
were scattered across `docs/`, the repo root, `admin-frontend/`, `personas/` and
`super-secret-projects/`. Several of them duplicate each other, and some claim work
is finished when the code shows otherwise.

This index:

1. Links the topic roadmaps as they stood on 2026-09-27.
2. Records what is actually built, checked against the code on 2026-09-26.
3. Collects the open work that is still worth doing from the older docs.
4. Lists every older plan or status doc with a verdict: keep, update, archive or delete.

Each topic doc was proposed in its own pull request, linked in the State column. Four of
those pull requests (#178, #192, #193, #195) closed without merging, and their Doc links
open the Linear design records instead.

## Topic roadmaps

| Topic | Doc | State |
|---|---|---|
| Auth, commercial plans, teams and fine-grained permissions | [auth-plans-permissions.md](./auth-plans-permissions.md) | Accepted ([#176](https://github.com/EconGraph/econ-graph/pull/176)) |
| Admin UI | [admin-ui.md](./admin-ui.md) | Accepted ([#177](https://github.com/EconGraph/econ-graph/pull/177)) |
| Postgres metadata federated with Arrow Flight / Parquet time series | [Linear design record](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) | Moved to Linear; [#178](https://github.com/EconGraph/econ-graph/pull/178) closed without merging. Tracked under ECO-5 |
| Global analysis (world map, cross-country data) | [global-analysis.md](./global-analysis.md) | Accepted ([#188](https://github.com/EconGraph/econ-graph/pull/188)) |
| Release trains: what ships in which release | [releases.md](./releases.md) | Accepted for train 1 ([#190](https://github.com/EconGraph/econ-graph/pull/190)); fork 4 still open |
| Data sources: which sources, in what order | [Linear design record](https://linear.app/econgraph/document/data-sources-roadmap-design-record-from-pr-192-2db0c59b5869) | Moved to Linear; [#192](https://github.com/EconGraph/econ-graph/pull/192) closed without merging. Tracked under ECO-5, plus ECO-202, ECO-203, ECO-204 |
| Analysis workspace: multi-series charts, saved charts, export | [analysis-workspace.md](./analysis-workspace.md) | Accepted ([#191](https://github.com/EconGraph/econ-graph/pull/191)) |
| SEC EDGAR / XBRL financial data | [Linear design record](https://linear.app/econgraph/document/sec-financial-data-roadmap-design-record-from-pr-193-3a5b22c09d18) | Moved to Linear; [#193](https://github.com/EconGraph/econ-graph/pull/193) closed without merging. Tracked as ECO-39, ECO-82, ECO-86, ECO-116, ECO-117, ECO-118, ECO-126, ECO-144 |
| Feature flags and experiments | [Linear design record](https://linear.app/econgraph/document/feature-flags-roadmap-design-record-from-pr-195-82a6458acab2) | Moved to Linear; [#195](https://github.com/EconGraph/econ-graph/pull/195) closed without merging. Tracked in the [Feature flags](https://linear.app/econgraph/project/feature-flags-bc727011a0f7) project, issues ECO-27, ECO-28, ECO-74, ECO-77, ECO-114, ECO-115, ECO-120, ECO-123, ECO-142 |
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
  `data_sources`. Unfinished screens are compiled out of release builds behind
  build-time flags; code is deleted only when it is known bad (the fake correlations,
  `imf.rs`, mock data), per [releases.md](./releases.md). Experiments wait for
  analytics and traffic.
- **Release trains ([releases.md](./releases.md)) and analysis workspace
  ([analysis-workspace.md](./analysis-workspace.md)).** Train 1 (`v4.0.0`) is broad
  real data behind pages that work: FRED, BLS, Census BDS, FHFA, BEA and World Bank
  WDI; the series page on `series`/`seriesData` with CSV download; the world map on
  `crossSection`; Keycloak sign-in with private and public annotations. `/analysis`,
  `ProfessionalChart` and `ChartCollaboration` are deleted as known bad. MCP, the admin
  app, saved charts and sharing wait for train 2. One chart component serves
  `/series/:id` now and `/chart` later.

## What is built today

Checked against `main` at `9dc14ea` (2026-09-27), which includes the JWT secret (#180), token subject (#181), backend CORS (#182) and `/mcp` token (#185) fixes.

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
| FRED, BLS, Census BDS (national and per state, #175) | Discovery and observation fetch |
| FHFA | Discovery. Its fetch targets `api.fhfa.gov`, which probably never existed; train 1 rebuilds it on the published master CSV ([releases.md](./releases.md)) |
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
renders, but on sample data. Collaboration UI (annotations, comments, sharing) exists
in the frontend and backend, but the series-page panel cannot reach the backend and
the API takes the acting user from the request; see
[analysis-workspace.md](./analysis-workspace.md).

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

| Item | Evidence |
|---|---|
| Fixed: `/mcp` requires a signed-in user's token ([#185](https://github.com/EconGraph/econ-graph/pull/185)), a stopgap until MCP OAuth (Phase 6), and `/playground` is served only when `ENABLE_GRAPHQL_PLAYGROUND` is set, which only local development does ([#221](https://github.com/EconGraph/econ-graph/pull/221)) | `econ-graph-backend/src/main.rs` |
| GraphQL depth (15) and complexity (1000) limits are enforced by both schema builders ([#221](https://github.com/EconGraph/econ-graph/pull/221)). Per-client rate limits still exist only in the unused `SecureGraphQLServer` | `econ-graph-graphql/src/graphql/schema.rs`; `econ-graph-graphql/src/security/server.rs` |
| Plaintext DB, monitoring and Google OAuth client-secret credentials in k8s manifests. The OAuth secret must be rotated | `k8s/manifests/postgres-deployment.yaml`, `configmap.yaml`, `ingress-cloudflare-dns01.yaml` |
| Sealed Secrets / secrets submodule not set up | `k8s/secrets` is uninitialized. `SECRETS_MANAGEMENT.md` describes a target state, not the current one |
| Tokens are stored in `localStorage` | `frontend/src/contexts/AuthContext.tsx` and `admin-frontend/src/contexts/AuthContext.tsx` |
| Terraform state files and provider binaries are committed to git | `terraform/k8s/terraform.tfstate`, `terraform.tfstate.backup`, `terraform/k8s/.terraform/` |
| Security scans upload results but never fail the build | `.github/workflows/security.yml` |

Fixed: no ingress sets CORS headers any more, so the backend's `CORS_ALLOWED_ORIGINS`
(#182) is the only CORS policy ([#196](https://github.com/EconGraph/econ-graph/pull/196)).

OAuth for `/mcp` is scheduled in Phase 6 of
[auth-plans-permissions.md](./auth-plans-permissions.md). Items about
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

The [federation roadmap](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) owns this area. No older doc covers it. The stale federation PRs #125 and #129 are reviewed there.

### Global analysis

[global-analysis.md](./global-analysis.md) owns this area and supersedes the older
global-analysis docs. The findings below are its inputs.

- **The backend exists but nothing can reach it.** `GlobalAnalysisQuery`
  (`econ-graph-graphql/src/graphql/global_analysis.rs`) is never merged into the root
  `Query`. Registering it as-is would expose `calculateCountryCorrelations`, whose results
  come from `calculate_pairwise_correlation` in `global_analysis_service.rs`, which
  returns hard-coded values (0.75, p = 0.01). [#188](https://github.com/EconGraph/econ-graph/pull/188)
  recommends a generic `crossSection` query instead and deleting
  `GlobalAnalysisQuery`, and [releases.md](./releases.md) adopts it: train 1 deletes
  the fake correlations and builds the map on `crossSection`.
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

The [SEC financial data roadmap](https://linear.app/econgraph/document/sec-financial-data-roadmap-design-record-from-pr-193-3a5b22c09d18) owns this area and supersedes
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

The [data sources roadmap](https://linear.app/econgraph/document/data-sources-roadmap-design-record-from-pr-192-2db0c59b5869) owns this area. It proposes a source order, a
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
| REST API and webhooks | Open: only auth, `/health` and `/metrics` are non-GraphQL routes |
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
| #125 financial-data federation, #129 Iceberg | Close | [federation roadmap](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) |
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
| [`docs/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md`](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md) | Archived | Superseded by the [SEC financial data roadmap](https://linear.app/econgraph/document/sec-financial-data-roadmap-design-record-from-pr-193-3a5b22c09d18). Its 2,835 lines include false completion claims |
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
