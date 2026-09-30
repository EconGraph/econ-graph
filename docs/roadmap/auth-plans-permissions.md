# Roadmap: Authentication, Plans and Permissions

> **Status**: Draft roadmap, 2026-09-26
> **Scope**: Backend (`backend/crates`), Keycloak configuration, and the admin UI surfaces that depend on them
> **Supersedes**: the auth/authorization parts of open PRs #141 (Keycloak + mTLS) and #149 (JWT permissions)

## Goal

Move EconGraph from a single hardcoded `role` column to an authorization model that supports:

- **Commercial plans**: several paid tiers (individual and team), each unlocking features and usage limits.
- **Team plans**: an organization owns a subscription, has seats, and manages its own members.
- **Tiered staff access**: EconGraph employees at several levels, from front-line support up to senior administrators.
- **Fine-grained RBAC**: the backend checks only fine-grained roles. The identity provider (Keycloak) composes them into plans, organization roles and staff levels, so new combinations need no backend change.

Billing is deferred: plans are modeled now and assigned by staff, and payment integration comes later.

## Where we are today

Checked against `main` at `fd2d1db` (2026-09-27). Items marked fixed landed after the first draft of this document.

### Authentication

- In-house auth in `econ-graph-auth`: email/password plus Google and Facebook OAuth, issuing HS256 JWTs valid for 24 hours (`econ-graph-auth/src/auth/services.rs`).
- **Fixed (#180).** The signing secret used to fall back to a hardcoded default when `JWT_SECRET` was unset; startup now fails without a non-empty secret (`services.rs:20-31`).
- The GraphQL handler re-reads the user from Postgres on every request (`econ-graph-backend/src/main.rs:500`), so a role change takes effect immediately. **Fixed (#181):** a `sub` that is not a UUID is now rejected instead of being mapped to the nil UUID.
- **Fixed (#185).** `/mcp` now requires a signed-in user's token (`main.rs:306`). OAuth for MCP clients arrives in Phase 6.
- **Fixed (#182).** CORS allows only the configured frontend origins (`main.rs:195`). Earlier security docs had recorded the CORS and JWT-secret fixes as done, but those fixes landed only in the pre-workspace `backend/src`, which has since been deleted.
- The GraphQL `user(userId)` query (`econ-graph-graphql/src/graphql/query.rs:327`) returns any user's record to any caller, authenticated or not (fix open in #183).
- A `user_sessions` table and model exist (`econ-graph-core/src/models/user.rs`), but login never writes a row and token verification never reads one, so logging out or revoking a session does not invalidate an issued token.

### Authorization

There are three role vocabularies that do not agree:

| Where | Roles |
|---|---|
| `users.role` column (`VARCHAR(50)`, initial schema) and `econ-graph-core::auth_models::UserRole` | `admin`, `analyst`, `viewer` |
| `econ-graph-graphql::graphql::context::UserRole` | `super_admin`, `admin`, `analyst`, `viewer`, `guest` |
| `admin-frontend` | `super_admin`, `admin`, `read_only`, `support` |

- `context.rs` defines a 25-variant `Permission` enum with a hardcoded role-to-permission map. Resolvers only ever call `require_admin`, which passes if the user holds *any* of five system permissions. No resolver asks for a specific permission.
- `users.organization` is free text. There is no organization, team, membership, plan or subscription table.
- Chart sharing has its own model: `chart_collaborators.role` (`VARCHAR`) plus a `permissions` JSONB blob, and a separate `PermissionLevel` enum in `collaboration_service.rs`.
- `audit_logs` and `security_events` tables exist and are the right foundation for admin auditing.

### The two stale PRs

**#141 Keycloak IAM and mTLS** is stacked on all of #125 and carries 405 changed files (Keycloak, HashiCorp Vault as an internal CA, cert-manager, Istio). It cannot be merged or rebased in a useful way. What is worth keeping is the design in its whitepaper (`docs/security/KEYCLOAK_INTEGRATION_WHITEPAPER.md` on that branch): OIDC with PKCE for the frontend, client-credentials tokens for internal services, and "run in parallel with legacy auth, migrate gradually."

**#149 JWT permissions** merges cleanly but adds a 640-line `permissions.rs` with no callers. It defines `EconGraphPermission` (about 50 variants, string form like `export:watermark-free`, parameterized ones like `stock:intraday:30`) and four subscription tiers (Free, Basic, Professional, Enterprise) hardcoded in Rust. Worth keeping: the permission naming scheme and the feature list for graphs, exports, collaboration, API and analytics. Not worth keeping: news and stock-market permissions for data we do not ingest, plan definitions compiled into code, and a second permission enum alongside the one in `context.rs`. Its Keycloak Kubernetes manifests are the cleaner copy if Keycloak is adopted later.

**Recommendation**: close both, and link this roadmap from each as the replacement plan. Phase 2 starts from #149's Keycloak manifests, and #141's OIDC and client-credentials design carries over.

## Target model

### Principles

1. **The backend knows only fine-grained roles.** Each fine-grained role is one capability, named `resource:action` (for example `series:read`, `chart:share`, `export:watermark_free`, `admin.users:suspend`). The backend checks these and nothing else. It never sees plan names, team tiers or staff levels.
2. **The identity provider composes them.** Keycloak holds the larger roles (a plan, an organization role like owner or member, a staff level like support or admin) as composite roles made of fine-grained ones. It flattens them into the access token. Adding a plan, repricing a tier or creating a new support level is a Keycloak change, not a backend deploy.
3. **The backend stays stateless for authorization.** A request's roles come from the verified token, not from a database lookup. The backend keeps only data about its own objects: who owns a chart, who it is shared with.
4. **One organization per token.** A person can belong to several organizations with different roles in each. Every access token is issued for one active organization, and its roles are the roles for that organization. Switching organization means getting a new token.
5. **Short-lived access tokens.** Because roles live in the token, a revoked role or a downgraded plan takes effect when the token refreshes. Access tokens last five minutes; refresh tokens and sessions are revocable in Keycloak.

This is the same pattern as Auth0's RBAC with permissions in the access token, and Keycloak composite roles. The alternative, used by GitHub, Slack and Stripe, keeps authorization in the application database and uses the identity provider only for login. We chose the token model to keep the backend free of product and pricing logic.

### What lives where

| Concern | Lives in | Notes |
|---|---|---|
| Login, passwords, social login (Google, Facebook), MFA, sessions | Keycloak | Replaces the in-house auth in `econ-graph-auth` |
| Fine-grained role catalog | Backend code (Rust enum) and Keycloak (client roles on the API client) | The Rust enum is the source of truth; a CI check verifies Keycloak's realm export defines exactly the same set |
| Plans, organization roles, staff levels | Keycloak composite roles | Built from fine-grained roles; editable in Keycloak |
| Organizations and membership | Keycloak Organizations plus a custom protocol mapper | Keycloak's built-in organization mapper emits an `organization` claim keyed by alias, so a custom mapper emits one scalar `org` id for the organization the client selected. Token issuance rejects a missing or ambiguous selection, and the mapper emits only that organization's roles |
| Plan assignment | Keycloak (organization's group or attributes) | Set by staff by hand until billing exists |
| Quotas (seats, API requests per hour, export rows) | Keycloak, emitted as a `limits` claim | Numbers, not roles |
| Object ownership and sharing | Backend Postgres | `chart_collaborators` and similar; cannot live in a token |
| Audit of admin actions | Keycloak admin events, plus backend `audit_logs` for backend actions | |

### Token shape

```json
{
  "iss": "https://auth.econ-graph.com/realms/econ-graph",
  "aud": "econ-graph-api",
  "sub": "user uuid",
  "sid": "keycloak session id",
  "org": "organization uuid",
  "roles": ["series:read", "chart:create", "chart:share", "export:image", "api:graphql"],
  "limits": { "api_requests_per_hour": 1000, "export_rows": 100000 },
  "exp": 1790000000
}
```

- `roles` is produced by a Keycloak protocol mapper that flattens the user's composite roles for the active organization into fine-grained roles for the `econ-graph-api` client only. Scoping to one client keeps the token small; if it still grows past a few hundred roles, the fallback is a cached UserInfo lookup keyed by `sid`.
- Staff tokens carry the staff fine-grained roles (`admin.users:read`, `admin.crawlers:manage`, and so on) and no `org`, or the organization being supported.

### Backend check

```text
allowed(token, role, object) =
      role ∈ token.roles
  AND (object has no organization  OR  object.org == token.org  OR  token has a staff role for it)
  AND (object is not shared-only   OR  object's sharing grants the caller access)
  AND (object is not private       OR  caller is its author  OR  object's sharing grants the caller access)
```

Sharing grants are organization-local: a grant does not let a caller reach an object owned by a different organization, so `object.org == token.org` still has to hold even when a sharing grant matches. `chart_collaborators` has no `organization_id` today, and `share_chart` (`mutation.rs`) does not check one; Phase 3 (Organizations) adds the column and the check when it adds `organization_id` to owned objects.

In code: a `Principal` built once per request from the verified token, and resolvers calling `ctx.authorize(Role::ChartShare, &chart)?` instead of `require_admin`. Quotas are checked where they are consumed (the rate limiter, save and export paths) against `token.limits`.

### Composite roles in Keycloak

Starting set. All of these are Keycloak configuration, kept as code in a realm export in the repo, and none is known to the backend.

**Staff levels** (realm roles): only three for now, with room for more. Train 1 seeds them from the 15-role catalog (see Release trains): `support` is the `user` roles plus `admin.users:read`, `admin.sessions:read` and `admin.system:read`; `admin` and `super_admin` hold every role. The table is the target once organizations, plans and impersonation exist.

| Composite | Intended for | Fine-grained roles it contains once later phases add them (examples) |
|---|---|---|
| `support` | Support staff | `admin.users:read`, `admin.orgs:read`, `admin.activity:read`, `admin.impersonate:read_only` |
| `admin` | Senior staff | everything in `support`, plus `admin.users:suspend`, `admin.plans:assign`, `admin.crawlers:manage`, `admin.audit:read` |
| `super_admin` | Owners, break-glass | everything, including managing staff roles in Keycloak; alert on every use |

Later levels (a second support tier, billing, data operations, security) are new composites built from the same catalog. In train 1, `admin` and `super_admin` hold the same roles, because the catalog has no role yet that only owners should hold.

**Preview access** (train 2, with the first runtime flag; train 1 has only build-time flags): the fine-grained role `preview:access`, granted by the staff composites and a `beta-tester` composite, is what feature flags target for previews (see the [feature flags roadmap](https://linear.app/econgraph/document/feature-flags-roadmap-design-record-from-pr-195-82a6458acab2), #195). A flag never grants access by itself, and anything that differs by plan is a role, never a flag.

**Organization roles**: `org-owner`, `org-admin`, `org-member`, `org-viewer`.

**Plans**: `plan-free`, `plan-pro`, `plan-team`, `plan-enterprise` as placeholders. A plan composite is the ceiling on what an organization's members can do; the mapper emits the intersection of the member's organization role and the organization's plan.

### Keycloak risks to prototype first

- **Per-organization roles.** Keycloak 26.6 added organization groups and 26.7 added role assignments on them with per-organization role claims (Keycloak blog, April 2026); train 1 pins 26.7.4. Prototype that first, since it may replace a group per organization (`/orgs/<id>/admins`) as the way to scope a composite to one organization. A custom protocol mapper is still needed to flatten the selected organization's roles into `roles` and to intersect them with the plan. If the prototype is painful, Auth0 Organizations supports org-scoped roles natively and is the fallback.
- **The intersection of organization role and plan** needs a custom mapper; Keycloak does not intersect composites out of the box.
- **Catalog drift** between the Rust enum and Keycloak. Mitigated by the CI check above.

### Services and API access

- Keycloak publishes JWKS; every service (GraphQL backend, chart-api, MCP, crawler-worker) verifies tokens locally.
- Machine access uses Keycloak clients with the client-credentials grant, each assigned the fine-grained roles it needs. This replaces a separate API-key system.
- **mTLS between services**: defer. If needed, get it from a service mesh rather than running Vault as an internal CA as #141 proposed.

### Multiple services and split development

The [federation roadmap](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) (#178) splits the backend into an app side (users, organizations, charts, annotations, admin) and a data side (crawler, sources, series metadata, Parquet catalog and files), so a feature build can run a fresh app database against the shared, already-crawled data. The token design has to work across both:

- **One catalog, two enforcers.** The fine-grained role catalog lives in a small shared crate used by both services. Each service checks only the roles for its own resources: the data side checks `series:read`, `data:export` and similar; the app side checks `chart:share` and similar.
- **Audience covers both services.** Train 1 has one backend and one audience, `econ-graph-api`. When the backend splits, Keycloak issues user tokens with `aud: ["econ-graph-api", "econ-graph-app", "econ-graph-data"]` (one audience mapper per service) and each new service accepts a token only if its own name is in `aud`; the `econ-graph-api` mapper is removed once no deployed service checks it, so a token issued before the switch stays valid through its five-minute life. The `roles` claim carries the fine-grained roles for both services, so one sign-in works everywhere.
- **The app side calls the data side with the user's token.** It forwards the caller's token rather than using a service identity, so the data side enforces the user's plan roles itself. Where the app side needs data access with no user (background jobs), it uses its own client-credentials client with only the data roles it needs.
- **Local and feature builds get read-only data credentials.** A `dev-readonly` client-credentials client in the shared realm holds only read roles on the data side (`series:read`, `data:read`). A local build with a fresh app database uses it to read shared data and cannot write to the data side or touch shared users. Its local app side can run its own Keycloak realm (or dev tokens) for test users, because the data side trusts only the shared realm's issuer and the `dev-readonly` client.

### MCP authorization with OAuth

The MCP specification defines authorization as OAuth 2.1. The MCP server is an OAuth resource server; Keycloak is the authorization server. The 2026-07-28 revision requires:

- Protected Resource Metadata (RFC 9728) at `/.well-known/oauth-protected-resource`, plus a `WWW-Authenticate` header on 401, so clients can discover the authorization server.
- Authorization code flow with PKCE, and Resource Indicators (RFC 8707) so each token is bound to our MCP server.
- Client ID Metadata Documents (CIMD) as the preferred way for clients to identify themselves. Dynamic Client Registration (RFC 7591) is deprecated but still used by many clients.
- Streamable HTTP transport, the transport the current spec defines.

Client support, checked 2026-09-26:

| Client | Custom remote MCP servers | Auth it can use against us |
|---|---|---|
| Claude (web, desktop; Free, Pro, Max, Team, Enterprise) | Yes. Team and Enterprise owners add the connector once for the organization; members each sign in | OAuth via CIMD, DCR or a pre-registered client (a fixed request-header key is a beta open to a limited set of organizations, so not planned for) |
| ChatGPT | Business, Enterprise and Edu on web, including write actions. Pro only in developer mode, read-only. Admins publish and can disable individual tools on Enterprise and Edu | OAuth (DCR, CIMD preferred when offered) |
| Gemini Enterprise (Business edition) | Pre-GA. Admin registers the server | OAuth with a pre-registered client ID and secret, or no auth |
| Gemini consumer app | No custom MCP servers | n/a |
| Gemini CLI, Claude Code and other developer tools | Yes | OAuth, or API key headers |

What this means for us:

- Users sign in exactly as they do on the website, for example with Google. When Claude or ChatGPT sends a user to Keycloak's authorization page, Keycloak offers "Sign in with Google" through identity brokering, then issues its own token for our MCP server. Google is where the user proves who they are; Keycloak issues the token, because a Google token cannot carry our fine-grained roles or be bound to our server as the audience.
- One OAuth setup in Keycloak (PKCE, DCR, pre-registered clients, and CIMD once Keycloak supports it; check its status before Phase 6) covers Claude, ChatGPT and Gemini Enterprise.
- The consent step is where a user picks which organization the MCP token acts for. The token then carries that user's fine-grained roles in that organization, and MCP tools call `authorize()` like any resolver.
- Team plans map directly onto how Claude Team/Enterprise and ChatGPT Business roll connectors out: an organization admin enables EconGraph once, and each member signs in with their own account.

Sources: [MCP 2026-07-28 authorization changes (WorkOS)](https://workos.com/blog/mcp-2026-spec-agent-authentication), [Claude remote MCP connectors](https://claude.com/docs/connectors/custom/remote-mcp), [ChatGPT developer mode and MCP apps](https://help.openai.com/en/articles/12584461-developer-mode-and-mcp-apps-in-chatgpt), [ChatGPT MCP compatibility (Zuplo)](https://zuplo.com/learn/mcp/compatibility/clients/chatgpt-connectors), [Gemini Enterprise custom MCP servers](https://support.google.com/g/answer/17106276?hl=en).

## Phased plan

Each phase is a small PR or a short stack, in order. Later phases depend on earlier ones.

### Phase 0: Hygiene (mostly done)

- Done: fail startup when `JWT_SECRET` is unset (#180); reject tokens whose `sub` is not a UUID (#181); require a token on `/mcp` (#185; OAuth arrives in Phase 6); allow CORS only from configured frontend origins (#182).
- Open: restrict the `user(userId)` query to the user themselves or an admin (#183); close #141 and #149 with a link to this roadmap.

### Phase 1: Fine-grained role catalog

- One Rust enum of fine-grained roles in `econ-graph-auth`, named `resource:action`, replacing both the `Permission` enum in `econ-graph-graphql` and #149's `EconGraphPermission`, keeping the useful parts of #149's list.
- Merge the two structs named `GraphQLContext` (the per-request caller in `graphql/context.rs` and the pool, dataloader and security holder in `graphql/schema.rs`) so the principal lives in one request context.
- `Principal` and `authorize()`; replace every `require_admin` call with the specific fine-grained role the resolver needs.
- Until Keycloak is live, derive a principal's fine-grained roles from the existing `users.role` column through a temporary mapping, so behavior does not change. Unit tests assert each resolver's required role.

### Phase 2: Keycloak as the identity provider

- Deploy Keycloak, starting from #149's Kubernetes manifests; realm configuration kept as code in the repo.
- Fine-grained roles defined as client roles on `econ-graph-api`, with a CI check against the Rust enum (`scripts/check-keycloak-roles`). Staff composites seeded as realm roles: `support` (the `user` roles plus read-only `admin.users`, `admin.sessions` and `admin.system`), `admin` and `super_admin` (every role). `user` is a realm default role. A client-role mapper puts the flattened roles in the access token's `roles` claim, and the backend checks that `aud` contains `econ-graph-api` (Keycloak also adds `account`). `preview:access` and the `beta-tester` composite are added with the first runtime flag (train 2), since train 1 has only build-time flags.
- Backend verifies Keycloak tokens via JWKS and reads `roles` from the token; the temporary mapping and `users.role` are removed.
- Audience mappers for each service, and the `dev-readonly` client for split development (see Multiple services and split development).
- Google becomes a Keycloak identity provider (Facebook and password login are not configured; see Release trains). Nothing is imported: `users.id` is Keycloak's `sub`. In-house login and JWT issuance are retired.
- Align `admin-frontend` with the staff composites.

### Phase 3: Organizations and teams

- Keycloak Organizations for membership and invitations; organization composites; the per-organization role approach chosen from the prototype above.
- Backend: organization id on owned objects (charts, workspaces), `org` claim checked in `authorize()`; `users.organization` free text removed.
- Organization switcher in the frontend that re-requests a token for the chosen organization.

### Phase 4: Plans and limits

- Plan composites and the mapper that intersects organization role with plan; `limits` claim from plan attributes.
- Every organization starts on `plan-free`; staff assign other plans by hand in Keycloak or the admin UI.
- Backend enforces `limits` in the rate limiter and at save and export. No plan names appear in backend code.

### Phase 5: Billing provider (deferred)

Not scheduled. When it is: Stripe as the default choice, with a small webhook service that sets the organization's plan in Keycloak through its Admin API. The backend is unaffected.

### Phase 6: Machine access and MCP OAuth

- Client-credentials clients in Keycloak for machine access, each with its own fine-grained roles.
- MCP OAuth per the section above: Protected Resource Metadata on `/mcp`, Keycloak as authorization server with PKCE, DCR, pre-registered clients and CIMD when available, and an organization picker on consent.

### Phase 7: Staff tooling (with the admin UI work)

- Day one, staff use the Keycloak admin console for role and plan changes.
- The admin UI then wraps the Keycloak Admin API for support workflows: user and organization lookup, plan assignment, staff role assignment, read-only impersonation with a visible banner and audit entry.
- Audit views combining Keycloak admin events with backend `audit_logs` and `security_events`.

### Phase 8: Enterprise identity (when a customer needs it)

- Per-organization SSO through Keycloak identity brokering (OIDC or SAML), SSO enforcement per organization, SCIM provisioning.

## Release trains

Train 1 ([releases.md](./releases.md)) takes phases 1 and 2 plus signed-in annotations. Organizations, plans, MCP OAuth and staff tooling come in later trains. The train 1 work is split into small PRs, and each PR updates this section when it lands or changes the plan.

Decisions taken for train 1:

- **Google only.** Keycloak brokers Google sign-in. Facebook and email with password are not configured.
- **In-house login deleted.** Once Keycloak issues tokens, the in-house `/auth/*` routes, password hashing, JWT issuance and `JWT_SECRET` are deleted rather than compiled out, because a second token issuer is wrong, not unfinished.
- **Keycloak's `sub` is `users.id`.** There is no production data, so nothing is imported. A row is created on the first authenticated request and holds only profile fields. `users.role` and the password columns are dropped.
- **Backend configuration.** `OIDC_ISSUER` (the JWKS is discovered from the issuer, and fetched lazily so a Keycloak outage never blocks anonymous browsing) and `OIDC_AUDIENCE` (`econ-graph-api`). In a dev build an unset issuer means authentication is off and every caller is anonymous; a set but unreachable or invalid issuer fails closed; the release build makes the issuer mandatory. The backend ships a test helper (a local signing key served as JWKS, and a function that mints tokens) so tests never depend on HS256 issuance.
- **Frontend sign-in with `oidc-client-ts`.** It uses the authorization code flow with PKCE, keeps tokens in memory and renews them silently. The frontend stays neutral about the identity provider. Sign-in is hidden when `VITE_OIDC_ISSUER` is unset.
- **Two realm clients.** `econ-graph-web` is a public client. `econ-graph-api` is bearer-only and owns the fine-grained client roles. A client-role mapper on `econ-graph-web` puts the flattened `econ-graph-api` roles in a top-level `roles` claim; it moves to a shared client scope before Phase 6 adds MCP clients.
- **The first role catalog.** Every account gets `annotation:create`, `annotation:comment`, `chart:share` and `api:mcp`. The staff roles are `admin.users:{read,create,update,delete,suspend}`, `admin.sessions:{read,revoke}`, `admin.crawlers:manage`, `admin.system:read`, `admin.security:read` and `admin.audit:read`. These are grouped into the `support`, `admin` and `super_admin` composites.
- **Annotation visibility.** It is a native Postgres enum with two values, `private` (the default) and `public`, replacing `chart_annotations.is_visible`. Comments are visible exactly when their annotation is, and only the author can edit or delete either.
- **The realm is applied on every deploy.** `--import-realm` only creates a realm, so a `keycloak-config-cli` job applies the realm file through the admin API after each deploy, changing only what differs and leaving users and role mappings alone. Dev and QA users live in separate import files; a CI check keeps them out of the Kubernetes manifests, and the QA file (two password users for the release checks) is behind a switch that is off by default.
- **Sign-in and annotation controls are hidden** when `VITE_OIDC_ISSUER` is unset, which is the read-only fallback if Keycloak is not deployed. The auth release spec is its own file so the release suite can skip it.
- **No `preview:access` yet.** Train 1 has only build-time flags.

## Open decisions

These change the shape of the work and are Joe's call. The roadmap assumes the first option in each.

1. **Per-organization roles in Keycloak**: groups per organization (assumed) or a custom mapper, decided after a prototype; or switch to Auth0 if neither is workable.
2. **Launch plans**: which paid tiers exist at launch and which fine-grained roles each plan composite includes. The `free` / `pro` / `team` / `enterprise` set is a placeholder.
3. **Access token lifetime**: five minutes (assumed), trading Keycloak load against how quickly revocations take effect.
