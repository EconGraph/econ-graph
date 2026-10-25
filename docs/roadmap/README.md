# EconGraph Roadmap

> Roadmaps, release plans and their status live in [Linear](https://linear.app/econgraph).
> Releases are issues ECO-5 (train 1, `v4.0.0`) through ECO-11 (train 7), and each topic is
> a Linear project. The docs in this directory record the reasoning behind each plan; where
> they differ from Linear, Linear is current.

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

Each topic doc below was proposed in its own pull request. Four of those pull requests
(#178, #192, #193, #195) closed without merging, and their Doc links open the Linear design
records instead.

## Topic roadmaps

| Topic | Doc | Proposed in |
|---|---|---|
| Auth, commercial plans, teams and fine-grained permissions | [auth-plans-permissions.md](./auth-plans-permissions.md) | [#176](https://github.com/EconGraph/econ-graph/pull/176) |
| Admin UI | [admin-ui.md](./admin-ui.md) | [#177](https://github.com/EconGraph/econ-graph/pull/177) |
| Postgres metadata federated with Arrow Flight / Parquet time series | [Linear design record](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) | [#178](https://github.com/EconGraph/econ-graph/pull/178) |
| Global analysis (world map, cross-country data) | [global-analysis.md](./global-analysis.md) | [#188](https://github.com/EconGraph/econ-graph/pull/188) |
| Release trains: what ships in which release | [releases.md](./releases.md) | [#190](https://github.com/EconGraph/econ-graph/pull/190) |
| Data sources: which sources, in what order | [Linear design record](https://linear.app/econgraph/document/data-sources-roadmap-design-record-from-pr-192-2db0c59b5869) | [#192](https://github.com/EconGraph/econ-graph/pull/192) |
| Analysis workspace: multi-series charts, saved charts, export | [analysis-workspace.md](./analysis-workspace.md) | [#191](https://github.com/EconGraph/econ-graph/pull/191) |
| SEC EDGAR / XBRL financial data | [Linear design record](https://linear.app/econgraph/document/sec-financial-data-roadmap-design-record-from-pr-193-3a5b22c09d18) | [#193](https://github.com/EconGraph/econ-graph/pull/193) |
| Feature flags and experiments | [Linear design record](https://linear.app/econgraph/document/feature-flags-roadmap-design-record-from-pr-195-82a6458acab2) | [#195](https://github.com/EconGraph/econ-graph/pull/195) |
| Security hardening | [SECURITY_IMPLEMENTATION_PLAN.md](../projects/SECURITY_IMPLEMENTATION_PLAN.md) | |
| Product (user-facing features, business phases) | [ROADMAP.md](../business/ROADMAP.md) | |

### Direction set by the topic roadmaps

- **Auth and permissions ([#176](https://github.com/EconGraph/econ-graph/pull/176)).**
  Fine-grained RBAC: the backend checks only `resource:action` roles carried in the
  token (`econ-graph-auth/src/roles.rs`). Keycloak is the identity provider (phase 2), and
  the later phases use it to compose those roles into plans, organization roles and staff
  levels. The phases are:
  - 0 Hygiene, including CORS (backend CORS in #182)
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
  in `admin-frontend/src/pages` (see [admin-ui.md](./admin-ui.md)).
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

## Archived and deleted docs

Older plan and status docs were reviewed when the topic roadmaps were written. **Archived**
docs were moved to [`docs/archive/`](../archive/README.md) and are kept for history only.
**Deleted** docs were empty or duplicated another doc; they remain in the repository history.

| Doc | Verdict | Why |
|---|---|---|
| [`docs/development/NEAR_TERM_FEATURES.md`](../archive/development/NEAR_TERM_FEATURES.md) | Archived | Duplicates ROADMAP phases 1 to 3. The file paths it proposes don't exist |
| `docs/development/GLOBAL_ANALYSIS_UI_ROADMAP.md` | Deleted | Duplicates phases 2 to 5 of `docs/development/GLOBAL_ANALYSIS_ROADMAP.md` |
| [`docs/projects/frontend-developer-global-analysis-plan.md`](../archive/projects/frontend-developer-global-analysis-plan.md) | Archived | Its test-suite claim is false |
| `docs/projects/global-analysis-ui-phase1.md` | Deleted | The same plan as the doc above with every box unchecked |
| [`docs/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md`](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md) | Archived | Superseded by the [SEC financial data roadmap](https://linear.app/econgraph/document/sec-financial-data-roadmap-design-record-from-pr-193-3a5b22c09d18). Its 2,835 lines include false completion claims |
| [`docs/development/vitest-migration-plan.md`](../archive/development/vitest-migration-plan.md) | Archived | The migration is complete |
| [`docs/technical/CRATE_SPLIT_PLAN.md`](../archive/technical/CRATE_SPLIT_PLAN.md) | Archived | The split is complete |
| [`docs/technical/CRAWLER_MONITORING_IMPLEMENTATION_PLAN.md`](../archive/technical/CRAWLER_MONITORING_IMPLEMENTATION_PLAN.md) | Archived | Mostly done. Its paths are stale |
| [`docs/technical/GLOBAL_ANALYSIS_SUMMARY.md`](../archive/technical/GLOBAL_ANALYSIS_SUMMARY.md) | Archived | A third copy of the global roadmap. Its Jest references are stale |
| [`docs/technical/MCP_SERVER_ANALYSIS.md`](../archive/technical/MCP_SERVER_ANALYSIS.md) | Archived | The bug it describes is fixed |
| [`docs/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md`](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md) | Archived | A point-in-time post-mortem |
| [`docs/projects/SECURITY_FINDINGS_REPORT.md`](../archive/projects/SECURITY_FINDINGS_REPORT.md) | Archived | Superseded by the FINAL report |
| `docs/security/SECURITY_ASSESSMENT_REPORT.md` | Deleted | An earlier draft of the FINAL report |
| `docs/security/COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md` | Deleted | Identical to the FINAL report, minus its frontend findings |
| [`docs/security/SECURITY_FIXES_SUMMARY.md`](../archive/security/SECURITY_FIXES_SUMMARY.md) | Archived | Its JWT and CORS fixes were in the deleted `backend/src`; #180 and #182 redid them in the crates |
| [`CLOUDFLARE_INTEGRATION_STATUS.md`](../archive/CLOUDFLARE_INTEGRATION_STATUS.md) (root) | Archived | A point-in-time status report |
| `LETSENCRYPT_CLOUDFLARE_INTEGRATION_SUMMARY.md` (root) | Deleted | Duplicates `docs/deployment/LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md` |
| `docs/deployment/PRIVATE_CHART_API.md` | Deleted | Superseded by `CHART_API_SERVICE.md` |
| [`ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md`](../archive/ci/docs/CI_FAILURE_ANALYSIS_AND_FIXES.md), [`E2E_TEST_FAILURE_ANALYSIS.md`](../archive/ci/docs/E2E_TEST_FAILURE_ANALYSIS.md) | Archived | Point-in-time incident notes from September 2025 |
| [`progress-reports/2025-09-13/*`](../archive/progress-reports/2025-09-13/) | Archived | Retrospectives. The crawler work they describe was superseded by #157 |
| `super-secret-projects/EDGAR_INGESTION.md` | Deleted | Empty |
