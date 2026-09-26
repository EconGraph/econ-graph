# Admin UI roadmap

Status: draft, surveyed 2026-09-26 against `main` at `1f17cc8`.

This roadmap covers the unfinished work in `admin-frontend/`, the separate React admin app. It does not design the role, identity or plan model. That belongs to the auth, plans and permissions roadmap (`docs/roadmap/auth-plans-permissions.md`, PR #176). This plan depends on that model and marks where it plugs in, as "auth phase N".

## Where things stand

The admin app builds, lints and passes its unit tests in CI (`admin-frontend-tests` in `.github/workflows/ci-core.yml`). But it cannot run against the real backend. The tests pass only because they check hand-written mocks against a hand-written copy of the schema, never the backend's real one.

### What a user actually sees

`index.html` loads `src/main.tsx`, which renders `App.tsx`. `App.tsx` is a three-tab shell: Crawler Dashboard, Configuration, and Logs. It has no router, no login, and no `AuthProvider` or `SecurityProvider`.

Several pages exist in the source tree but nothing outside the tests imports them:

- `DashboardPage`
- `MonitoringPage`
- `SystemHealthPage`
- `UserManagementPage`
- `auth/LoginPage`
- `components/layout/AdminLayout`, which already has role-based navigation

Because those pages are unreachable, `AuthContext`, `SecurityContext`, `useUsers`, `useMonitoring` and `useSystemHealth` are dead code in the running app. `src/index.tsx` is a dead duplicate of `main.tsx`.

### Why it can't talk to the backend

**Field casing.** The backend is async-graphql with its default camelCase names (`econ-graph-graphql/src/graphql/schema.rs`). Every admin query asks for snake_case names instead (`is_running`, `base_url`, `created_at`), so no typed operation can succeed as written. See `admin-frontend/src/services/graphql/queries.ts`, `mutations.ts` and the inline queries in `hooks/useCrawlerConfig.ts`.

**Operations that exist in the backend but have the wrong shape:**

| Operation                                | Mismatch                                                                                                                                                                        |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crawlerStatus`, `queueStatistics`       | Casing only.                                                                                                                                                                    |
| `triggerCrawl`                           | The frontend sends `force`, which `TriggerCrawlInput` does not have.                                                                                                            |
| `dataSources`, `dataSource`              | The backend type has 8 fields. The frontend expects `enabled`, `priority`, `health_status`, `last_success`, `configuration` and more.                                           |
| `systemHealth`                           | The backend returns `{status, metrics, lastUpdated}`. The frontend expects `components[]`, `uptime_seconds` and `version` (issue #126).                                         |
| `users`                                  | The backend takes `filter` and `pagination` and returns a `UserConnection`. The frontend passes flat arguments, expects a list, and reads `status`, `isOnline` and `sessionId`. |
| `user`                                   | The backend argument is `userId`. The frontend sends `id`.                                                                                                                      |
| `createUser`, `updateUser`, `deleteUser` | The mutations exist, but their selection sets ask for user fields that don't exist.                                                                                             |

**Operations the backend lacks entirely:**

- Crawler control: `start/stop/pause/resumeCrawler`, `crawlerConfig`, `updateCrawlerConfig`, `setMaintenanceMode`.
- Queue management: `queueItems`, `clearQueue`, `retryFailedItems`, `prioritizeQueueItem`, `cancelQueueItem`.
- Data source CRUD: `create/update/delete/toggleDataSource`, `testDataSourceConnection`.
- Logs: `crawlerLogs`, `searchLogs`, `logStatistics`, `logsCount`, `clearLogs`, `exportLogs`.
- System actions: `performanceMetrics`, `onlineUsers`, `restartSystem`, `backupSystem`.
- All three subscriptions, because the schema uses `EmptySubscription`.

**Backend admin operations the UI never calls:** `suspendUser`, `activateUser`, `forceLogoutUser`, `userSessions`, `activeSessions`, `securityEvents` and `auditLogs` exist and are admin-guarded.

**Login doesn't exist.** `AuthContext` POSTs to `/api/admin/auth/{login,validate,refresh,extend,logout}`. The backend has no `/api/admin` routes; its auth routes are `/auth/*` in `econ-graph-auth/src/auth/routes.rs`. Only `useCrawlerConfig` sends a bearer token, and that token is the literal `admin_token`. The other hooks send none, so admin-guarded resolvers reject them.

**Monitoring is mocked.** `useMonitoring` returns hard-coded dashboards with fake delays and `Math.random` jitter. It fetches `/api/monitoring/system-status`, which doesn't exist. `CrawlerLogs` generates random performance metrics, and its export is a TODO.

**Two clients, and neither is shared.** `App.tsx` sets up Apollo, but the hooks use react-query with a separate copy of a raw `fetch` wrapper in each hook.

### Roles today

The roles don't agree anywhere:

| Where                                       | Roles                                                                                             |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Admin frontend                              | `read_only`, `admin`, `super_admin`, with a client-side permission table in `SecurityContext.tsx` |
| `econ-graph-core/src/auth_models.rs`        | `Admin`, `Analyst`, `Viewer`                                                                      |
| `econ-graph-graphql/src/graphql/context.rs` | `SuperAdmin`, `Admin`, `Analyst`, `Viewer`, `Guest`, plus a 26-value `Permission` enum            |

`read_only` maps to no backend role, so it gets no permissions. The backend's `require_admin` means "holds any of five system permissions". Its fine-grained `require_permission` and `can_*` helpers are never called outside `context.rs`. These resolvers are unguarded: `crawlerStatus`, `queueStatistics`, `dataSources` and `user(userId)`. `user(userId)` returns any user's record to any caller.

### Crawler model drift

The admin crawler pages were built before the crawler consolidation (#157). That change replaced the old crawler with a queue-based crawler, source adapters and `crawler-worker`. Pause and resume, per-source priority and the config model all need re-deriving from the current `econ-graph-crawler` crates, not from `admin-frontend/docs/features/CRAWLER_ADMIN_PLAN.md`, which predates #157.

## Open issues and PRs this plan absorbs

| Item                                                        | Verdict                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| #151 SEC crawler admin UI                                   | Salvage the admin half only: `components/sec/{CompanySearch,SecCrawlerManager}.tsx`, `hooks/{useCompanySearch,useSecCrawler}.ts`, `types/index.ts` and their tests. The backend half was written into `backend/src`, which is dead and was deleted in #174. None of its GraphQL fields (`searchCompanies`, `company`, `triggerSecCrawl`, `importSecRss`) exist in the live backend. Close #151 once phase 4 lands a replacement. |
| #126 SystemHealth schema mismatch                           | Phase 2.                                                                                                                                                                                                                                                                                                                                                                                                                         |
| #116, #127 User management UI and tests                     | Phase 3. The UI was built later (#127). What's left is rebuilding its data source on the Keycloak Admin API and un-skipping its tests.                                                                                                                                                                                                                                                                                           |
| #88, #108, #103, #128, #93 Test mocking, hangs and slowness | Most go away in phase 1, when the tests run against the real schema through MSW instead of per-hook module mocks. Re-check each one after phase 1, then close or rescope it.                                                                                                                                                                                                                                                     |
| #122 testing-library lint                                   | Phase 1 cleanup.                                                                                                                                                                                                                                                                                                                                                                                                                 |
| #99, #100 Performance audits                                | Fold into phase 3 as review criteria, then close. They are guidelines, not tracked work.                                                                                                                                                                                                                                                                                                                                         |
| `origin/feature/admin-crawler-ui` branch                    | Landed as #82 (squash). The branch can be deleted. This was inferred from the #82 merge; the branch's own history was not diffed.                                                                                                                                                                                                                                                                                                |

## Phases

Each phase is its own small PR or short PR stack.

### Phase 0: Cleanup (no behavior change)

- Delete `src/index.tsx`, `fix-crawlerconfig-tests.js`, `fix-remaining-issues.js`, `package.json.backup`, the committed `tsconfig*.tsbuildinfo`, `vite.config.js` and `vite.config.d.ts`, `playwright-report/` and `test-results/`, and gitignore the generated ones.
- Remove the duplicate `SEARCH_LOGS`, which is defined in both `queries.ts` and `mutations.ts`.
- Rewrite `admin-frontend/README.md` to describe what actually works, and mark `CRAWLER_ADMIN_PLAN.md` as superseded by this doc.

### Phase 1: Static schema check and one client

The existing check, `src/__tests__/graphql-schema-validation.test.ts`, is static but validates the wrong things. It compares the mock JSON files against a hand-typed table of snake_case types, some of which (`CrawlerConfig`) don't exist in the backend. It never reads the backend's schema, and it never parses the operations in `queries.ts` or `mutations.ts`. The main frontend has the same gap: `frontend/src/test-utils/mocks/schema.ts` is a hand-written SDL. The replacement is also static, with no running server:

- Commit the backend schema as `schema.graphql`, generated from `econ-graph-graphql` with async-graphql's `Schema::sdl()`. A backend unit test fails when the committed file differs from the generated one, so any backend change that alters the API also updates the file in the same PR.
- In `admin-frontend`, validate every operation against that file with `graphql-js` `validate()`, and generate TypeScript types for the operations with GraphQL codegen. A misspelled field, a snake_case name or a missing operation then fails `npm run type-check`.
- Because the schema file changes with every backend API change, CI runs the admin-frontend check whenever `schema.graphql` changes, not only when `admin-frontend/**` changes. A backend PR that breaks the admin app then fails before it merges.
- Delete `graphql-schema-validation.test.ts`, and type the MSW handlers from codegen so the mocks cannot drift either.
- Apply the same check to the main frontend, replacing its hand-written SDL (a separate PR).
- Pick one client and delete the other. The recommendation is react-query plus a single typed `graphql-request` or `fetch` wrapper, because that is what every hook already uses. Apollo is only a provider in `App.tsx`.
- Optionally, add a CI job that runs one smoke query per page against a real backend with seeded data. The static check catches schema drift; this job catches resolver and auth behavior.

### Phase 2: Login, routing and shell

Login depends on auth phase 2 (Keycloak as the identity provider). Routing and the shell don't, so they can land first behind a temporary login.

- Log in through Keycloak with the OIDC authorization code flow and PKCE, as its own public client. Drop the `/api/admin/auth/*` contract and the username, password and MFA form in `LoginPage`; Keycloak owns the login screen, MFA and session timeouts.
- Read the caller's fine-grained roles from the access token's `roles` claim. The backend reads the same claim and enforces it (auth phase 1), so the admin app needs no separate permissions query.
- Send the token from the shared client (phase 1), not per hook, and refresh it with the Keycloak adapter.
- Add a router. Mount `AdminLayout` with `DashboardPage`, `SystemHealthPage`, `MonitoringPage`, `UserManagementPage` and the crawler pages as routes.
- Drive navigation and button visibility from the fine-grained roles in the token, not the client-side `ROLE_HIERARCHY` and `ROLE_PERMISSIONS` tables, which are deleted. The client check is for UX only; the backend enforces. The admin app never checks a composite or staff-level name, only fine-grained roles, so staff composites can change in Keycloak without a frontend release.
- `SystemHealthPage`: extend `SystemHealthType` with per-service health (#126), or cut the page down to what the backend reports.
- `MonitoringPage`: replace the mocked dashboards with links or embeds of the real Grafana dashboards in `grafana-dashboards/`. Drop `/api/monitoring/system-status`, or serve it from the backend.

### Phase 3: User management and staff tooling

Keycloak owns identities, sessions, roles and plan assignment after auth phase 2. On day one, staff use the Keycloak admin console for all of it (auth phase 7). The admin UI then wraps the Keycloak Admin API for the workflows support staff do often, so they don't need console access.

- Don't wire the backend's user lifecycle operations (`createUser`, `updateUser`, `deleteUser`, `suspendUser`, `activateUser`, `forceLogoutUser`, `userSessions`, `activeSessions`). Auth phase 2 retires in-house login, so these move to Keycloak. `UserManagementPage` and its "Online users" tab are rebuilt on the Admin API instead: user lookup, enable and disable, and session list and logout.
- Organization lookup and member lists, after auth phase 3.
- Plan assignment for an organization, after auth phase 4. Billing is deferred (auth phase 5), so until it lands this is how an organization moves off `plan-free`.
- Staff role assignment: adding and removing staff composites.
- Read-only impersonation that shows a visible banner and writes an audit entry.
- Audit views that combine Keycloak admin events with the backend's `auditLogs` and `securityEvents`, which are already in `AdminLayout`'s nav but have no page.
- Decide where Admin API calls run. The recommendation is a thin backend proxy using a Keycloak service account, guarded by fine-grained roles, so the browser never holds Keycloak admin credentials and every change is audited in one place.
- Un-skip or replace the `UserManagementPage` tests (#116, #127) against the new data source.

### Phase 4: Crawler and data source admin on the new crawler

- Re-derive the crawler GraphQL surface from the post-#157 crawler: its queue, source adapters and worker. Decide which of the missing operations are real needs and which were speculative (`restartSystem`, `backupSystem`, `setMaintenanceMode`). Add only the backend resolvers that are needed, each guarded by a specific fine-grained role through `authorize()` (auth phase 1) rather than `require_admin`.
- Add the missing data source fields (`enabled`, health) to the backend, or drop them from the UI.
- Logs: serve crawler logs from wherever the worker writes them. Remove the random metrics in `CrawlerLogs`.
- Real-time: poll first. Add subscriptions only if polling proves insufficient; the schema has none today.
- Port #151's admin half here. Rebuild `searchCompanies`, `company`, `triggerSecCrawl` and `importSecRss` in `econ-graph-graphql` on top of `econ-graph-sec-crawler`, then land its components and tests.

### Backend fixes to make early

These are security fixes in resolvers the admin UI calls. They are owned by the auth roadmap, and are listed here so the admin work doesn't assume they're done.

- Restricting `user(userId)` to the user themselves or an admin is auth phase 0.
- Whether `crawlerStatus`, `queueStatistics` and `dataSources` should be public belongs in auth phase 1, where every resolver gets a specific fine-grained role. Guard them if not.
- The two `UserRole` enums and the `Permission` enum are replaced by the fine-grained role catalog in auth phase 1. The two `GraphQLContext` structs should be merged at the same time.

## Dependencies

| This roadmap needs                                                                                                                                                                | From                                                                               |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Fine-grained role catalog and `authorize()` (all phases), Keycloak login (phase 2 here), Keycloak Organizations, plan composites and the staff Admin API workflows (phase 3 here) | Auth, plans and permissions roadmap, #176: auth phases 1, 2, 3, 4 and 7            |
| The current crawler model                                                                                                                                                         | `econ-graph-crawler`, `econ-graph-crawler-worker` (#157)                           |
| Time series storage for any admin data views                                                                                                                                      | Postgres and Arrow Flight federation roadmap, only if admin pages show series data |
