# GraphQL Security Guide

The GraphQL backend combines request bounds, optional OIDC authentication, resolver authorization, service validation, and error handling. This guide explains where those controls apply and how developers and operators use them. See the [GraphQL API reference](../api/GRAPHQL_API.md) for operations and types.

## Contents

1. [Request controls](#request-controls)
2. [Authentication and authorization](#authentication-and-authorization)
3. [Input validation and error handling](#input-validation-and-error-handling)
4. [Configuration and traffic controls](#configuration-and-traffic-controls)
5. [Security evidence and alerting](#security-evidence-and-alerting)
6. [Incident response](#incident-response)
7. [Validation before deployment](#validation-before-deployment)

## Request controls

The HTTP route parses the request, resolves its caller, and passes a per-request `GraphQLContext` to the schema. The schema validates the operation and its depth, field and cost limits before executing resolvers, and cancels it if it runs past the request timeout. Each protected resolver checks its required role; services then enforce resource access and business rules. Schema extensions sanitize server errors and wait for pending permission-denial writes before returning a normal response.

### Per-request limits

One request can ask for a lot of data: a page of series, each with its observations, aliased many times. Four limits bound what one request may make the backend do. They are set in [`graphql/limits.rs`](../../backend/crates/econ-graph-graphql/src/graphql/limits.rs) and applied by the [schema builder](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs) to every schema the backend serves:

| Limit | Bound | When it applies | Error |
| --- | --- | --- | --- |
| Depth | 15 levels | Before the request runs | `QUERY_LIMIT_EXCEEDED`, "nested too deep" |
| Fields | 500 field selections, counting every alias and every expanded fragment spread | Before the request runs | `QUERY_LIMIT_EXCEEDED`, "too many fields" |
| Cost | 20,000 (see below) | Before the request runs, from the arguments sent | `QUERY_LIMIT_EXCEEDED`, "too complex" |
| Time | 25 seconds | While the request runs; it is then cancelled | `REQUEST_TIMEOUT` |

The API's database connections also carry a PostgreSQL `statement_timeout` of 20 seconds (`API_STATEMENT_TIMEOUT` in [`database.rs`](../../backend/crates/econ-graph-core/src/database.rs)), so a statement a cancelled request was waiting on stops on the server too. It is set per connection by the API's pools, not on the database role, because the crawler and migrations share the role and run longer statements. The edge's own route timeout is 30 seconds, so the API answers first with an error the client can read.

These are source settings, not environment profiles. Changing one requires a source change and a rebuilt backend.

#### How cost is counted

A field costs 1 plus the cost of its selections, except that a list costs what it can return:

- **Lists of series, companies and users** cost their page size times their selections. `seriesList`, `DataSource.series` and `searchSeries` count `first` (after its default and cap); `dataSources` counts as 25 sources, `datasets` as 100, `crossSection` as 250 entries.
- **Lists of observations** (`seriesData`, `EconomicSeries.dataPoints`, `EconomicSeries.recentDataPoints`, and `companyFacts`) cost one unit per 25 rows, times their selections plus 4. The 4 stands for reading the row, which costs the same however few of its fields are selected.
- **Unpaged lists cost their worst case.** `EconomicSeries.dataPoints` returns at most 10,000 points, so it costs as 10,000 points wherever it is selected.
- **Expensive roots** add 500 whatever they return: `searchSeries`, `searchSuggestions`, `crossSection`, `financialStatement`, `companyRatios`, `companyPeers` (which adds 200 more per peer compared).
- **Fields that query the database per object** (`EconomicSeries.source`, `EconomicSeries.dataPointCount`, `DataSource.seriesCount`) add 5 each time they are selected.

A document holding several operations is costed by the one the request names: the others, and fragments only they use, are dropped before validation.

What this allows, at 20,000 (the site's figures are measured by `the_sites_own_queries_pass_the_limits`):

| Request | Cost |
| --- | --- |
| One 10,000-point page of `seriesData` as the series page and CSV export request it | 5,601 |
| `seriesList`, 100 series, as the explorer requests them | 2,801 |
| `searchSeries`, 100 series, as the explorer requests them | 3,001 |
| Company comparisons (`companyRatios` and 10 `companyPeers`) | 3,277 |
| `Train3SeriesSubject` in the [data access contract](../api/operations/train3-data.graphql): one series read five ways through `dataPoints` | 17,090 |
| One series with its full `dataPoints { date value }` | 2,402 |
| Seven 10,000-point `seriesData` pages of `{ date value }` in one request | 19,607 |
| Eight such pages | 22,408: refused |
| `seriesList(pagination: {first: 10}) { nodes { dataPoints { value } } }` | 20,021: refused |
| `seriesList(pagination: {first: 100}) { nodes { dataPoints { value } } }` | 200,201: refused |
| `dataSources { series(first: 100) { nodes { dataPoints { value } } } }` | over 5,000,000: refused |

So one request holds at most about 100,000 observations, whatever its shape. A longer history, or several series, is read a page at a time: the site's charts and CSV exports already read `seriesData` in 10,000-point pages until `pageInfo.hasNextPage` is false, one request per page, so a daily series with decades of history (15,000 to 20,000 points) takes two requests.

#### Page arguments

| Field | Page argument | Out of range |
| --- | --- | --- |
| `seriesData` | `first` 0 to 10,000 (default 10,000); `after` a cursor | Negative `first` or a malformed `after`: `BAD_USER_INPUT`. Over 10,000: one page of 10,000, with `pageInfo.hasNextPage`. |
| `EconomicSeries.dataPoints` | none; returns every matching point, at most 10,000 | More than 10,000 matching points: `QUERY_LIMIT_EXCEEDED`, naming `seriesData`. It never returns part of the history. Narrow `startDate`/`endDate` or page with `seriesData`. |
| `EconomicSeries.recentDataPoints` | `limit` 0 to 10,000 (default 100) | `BAD_USER_INPUT` |
| `DataSource.series` | `first` 0 to 100 (default 50); `after` a previous `endCursor` | `BAD_USER_INPUT`. An `after` past the end is an empty page. |
| `searchSeries` | `first` 0 to 1,000 (default 50); `after` an offset | `BAD_USER_INPUT` |
| `seriesList` | `pagination.first` 1 to 100 (default 50) | Clamped into range, with `pageInfo` showing the rest. |

The MCP `search_economic_series` tool takes `limit` 1 to 100 and refuses anything else, so a search never returns fewer results than were asked for without saying so.

#### Monitoring the limits

The backend exports `econgraph_graphql_rejected_requests_total{reason}`, with `reason` one of `complexity`, `depth`, `fields`, `timeout` and `statement_timeout`, and `econgraph_graphql_request_duration_seconds{outcome}` (`ok` or `error`). The Grafana dashboard **EconGraph API: GraphQL limits** ([`graphql-api.json`](../../platform/observability/dashboards/graphql-api.json)) shows both, with backend memory against its limit. [`rules-backend.yaml`](../../platform/observability/rules-backend.yaml) alerts when a backend pod holds more than 80% of its memory limit for 10 minutes (`EconGraphBackendMemoryHigh`) and when one is OOM-killed (`EconGraphBackendOOMKilled`).

The limits bound one request, not how many a client sends. The edge's per-client request rate limit covers that.

### POST body size

The backend [request filter](../../backend/crates/econ-graph-backend/src/main.rs) caps a `/graphql` POST body at **1 MiB (1,048,576 bytes)**. It counts bytes as they arrive, including chunked bodies, and stops collecting when the bound is exceeded. The caller receives HTTP **413** with a GraphQL-shaped `errors` response.

This bounds the whole body, including variables, rather than just query text. GET operations carry their query in the URL and do not pass through this POST body cap. URL limits must be handled by the deployment's HTTP infrastructure. Malformed JSON and unsupported request content types are handled separately as bad requests.

### Introspection and Playground

Schema introspection is supported. The standard introspection query nests 13 levels and holds about 160 fields, so the depth bound of 15 and the field bound of 500 leave room for schema tooling; a test checks that query.

The browser interface at `/playground` is separately disabled unless `ENABLE_GRAPHQL_PLAYGROUND=true`. Disabling that page does not disable `__schema`, `__type`, or `__typename` on `/graphql`. Protect sensitive data through resolver authorization rather than assuming the schema is hidden.

## Authentication and authorization

### How a request gets a principal

The backend accepts an OIDC access token in `Authorization: Bearer <token>`. The [OIDC verifier](../../backend/crates/econ-graph-auth/src/oidc.rs) checks its RS256 signature against provider keys, the configured issuer and audience, expiration, and the not-before claim when present. The token subject identifies the app-plane user row, which is created on first use. An active account and a verified token produce a principal containing that user's ID and the token's recognized fine-grained roles.

Authentication is optional at the route boundary. Public reads can run anonymously; protected fields must explicitly require authentication or a role. The [bearer helper](../../backend/crates/econ-graph-auth/src/bearer.rs) and the backend's `graphql_caller` distinguish these outcomes:

| Condition | `/graphql` outcome |
| --- | --- |
| No authorization header, or a scheme other than Bearer | Executes anonymously. |
| Sign-in disabled because no issuer is configured | Executes anonymously; tokens confer no privileges. |
| Invalid, expired, or empty bearer token with sign-in enabled | HTTP **401**, `UNAUTHENTICATED`, with a `WWW-Authenticate` bearer challenge. |
| Provider keys unavailable and the verifier cannot check the token | Logs a warning and executes anonymously; the token grants no privileges. |
| Verified token for a suspended account | HTTP **403**, `ACCOUNT_SUSPENDED`, before execution. |
| Failure reading or creating the signed-in account | HTTP **500**, `INTERNAL_SERVER_ERROR`, before execution. |

An identity-provider outage does not necessarily prevent verification: cached keys can remain usable under the verifier's cache policy. The anonymous fallback applies when verification returns an unavailable-keys error. It preserves public access, but callers must handle protected fields failing because no principal was established.

The main frontend uses authorization code with PKCE, keeps tokens in memory, and coordinates renewal in its [authentication context](../../frontend/src/contexts/AuthContext.tsx). That is a browser-client behavior, not a server guarantee about other clients' token storage.

### Authorizing a resolver

The backend uses capabilities named `resource:action`, such as `annotation:create` and `admin.security:read`. The [role catalog](../../backend/crates/econ-graph-auth/src/roles.rs) is the source of truth. Identity-provider composites may group these capabilities, but the backend requires the flattened top-level `roles` claim and checks its recognized capabilities; it does not infer authority from a staff label or `users.role`.

Use the shared `require_role` helper so that a signed-in caller's missing role is also recorded as a security event. This excerpt from `createAnnotation` obtains the author from the principal:

```rust
use econ_graph_auth::Role;
use econ_graph_graphql::graphql::context::require_role;

// Inside a resolver with ctx: &async_graphql::Context<'_>:
let user_id = require_role(ctx, Role::AnnotationCreate)?.user_id;
```

An anonymous caller receives the GraphQL error `Authentication required`; a signed-in caller lacking the role receives `Insufficient permissions`. These are resolver errors in the GraphQL response, distinct from the HTTP authentication refusals above. Callers must inspect the response's `errors` even when HTTP succeeds.

A capability check does not establish access to every resource. Keep the [collaboration service's checks](../../backend/crates/econ-graph-services/src/services/collaboration_service.rs) in the execution path: private annotations are visible only to their author, comments require access to the annotation, and sharing a chart requires an explicit chart-admin collaborator role. Derive the acting user from the principal rather than accepting an author or owner ID from request input.

User administration also applies target checks. Acting on another user requires every staff role in the catalog, beyond the mutation's individual capability. Suspension and deletion refuse the caller's own account and recheck the acting account's liveness within the transaction. See [admin security](../technical/ADMIN_SECURITY.md) for administration guidance and the [mutation implementation](../../backend/crates/econ-graph-graphql/src/graphql/mutation.rs) for these guards.

## Input validation and error handling

### Validate at the boundary that owns the rule

GraphQL checks argument types and required fields. Resolvers parse values such as UUIDs, while services validate business inputs before writing them. New write paths must retain those service checks; accepting a GraphQL string does not establish that its content is valid.

The collaboration service provides concrete examples:

| Field | Validation |
| --- | --- |
| Annotation title | Required, nonblank, at most 200 characters. |
| Annotation note | At most 5,000 characters; may be empty. |
| Comment | Required, nonblank, at most 2,000 characters. |
| Annotation type | Required, nonblank, at most 20 characters. |
| Color, when supplied | Nonblank, at most 7 characters. |

`validate_annotation_fields` and `validate_comment` count characters, not bytes. `create_annotation` also calls `validate_annotation_point` for date and numeric bounds before storing the value. These functions reject invalid inputs; they do not promise to strip HTML, scripts, or SQL fragments. Use parameterized Diesel operations for database access and render user text safely in clients. A length check does not make arbitrary HTML safe to render.

### Preserve typed errors

The schema's [SanitizeErrors extension](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs) logs typed server-error detail with the operation and field path, then returns `Internal server error` and `extensions.code = INTERNAL_SERVER_ERROR`. It replaces the error's extensions as well, preventing server detail in an extension from reaching the caller.

Known client-facing errors, including validation, authentication, permission, and malformed-argument errors, keep their useful messages. Hand-written strings are also treated as client-facing. Pass server errors with `?` or `Error::new_with_source`; formatting a database or internal error into `Error::new(format!(...))` bypasses the typed-error protection. This extension handles response errors, not input sanitization.

## Configuration and traffic controls

Configure sign-in through these settings in [OidcConfig](../../backend/crates/econ-graph-auth/src/oidc.rs):

| Setting | Effect |
| --- | --- |
| `OIDC_ISSUER` | Exact issuer expected in access tokens. Unset or empty disables sign-in. |
| `OIDC_AUDIENCE` | Expected audience; defaults to `econ-graph-api`. |
| `OIDC_JWKS_URL` | Optional key-fetch URL override when the public issuer is not reachable from the backend. The expected token issuer remains unchanged. |
| `OIDC_REQUIRED=true` | Refuses startup without an issuer. It does not require a bearer token for every GraphQL operation. |
| `ENABLE_GRAPHQL_PLAYGROUND=true` | Enables the browser page at `/playground`. |

Malformed OIDC configuration fails at startup. For deployment settings and the public issuer/in-cluster key-fetch relationship, see [Kubernetes deployment](../../k8s/README.md).

The `/graphql` backend route does not provide per-client incoming request quotas, limiter status, or a limiter reset API. The core `RateLimitConfig` concerns outbound FRED/BLS calls and does not throttle GraphQL callers. Operators who need incoming traffic limits must configure and verify them at their deployment's edge. Record the client-identity key, quotas, rejection behavior, and proxy trust assumptions in the deployment's operational configuration; the per-request limits cannot replace those controls.

Client attribution also needs deployment configuration. `TRUSTED_PROXIES` defaults to empty, so records use the backend's TCP peer rather than trusting forwarded addresses. Follow [trusted proxy configuration](ADMIN_AUDIT.md#trusted-proxies-in-kubernetes) before using recorded IPs to identify or block a client. Trust only verified proxy addresses or restricted proxy subnets.

## Security evidence and alerting

### Read events and audit records

Security events and admin audit records serve different purposes:

- `security_events` records `permission_denied` for signed-in callers refused a role or full staff-role requirement, and `suspended_account` when a valid token belongs to a suspended account. Permission events are deduplicated per caller and permission within a request, so repeated aliases need not produce separate records.
- `audit_logs` records admin mutation attempts after the caller passes the mutation's capability check. The record's `details.outcome` distinguishes success, invalid input, denial, failure, and other outcomes. A cancelled attempt can have committed; `cancelled` is not proof of rollback.

Read events through `securityEventsPage(filter, pagination)` with `admin.security:read`, and audit records through `auditLogs(filter, pagination)` with `admin.audit:read`. The legacy `securityEvents` list is also available. Both connections default to 50 records and clamp page size to 1–100. Follow `pageInfo.endCursor` while `hasNextPage` is true; offset pages can shift as new records arrive. See [admin audit and security events](ADMIN_AUDIT.md) for filters and pagination semantics.

These writes are best-effort and separate from the change being audited. Normal responses wait for writes; disconnects detach pending writes rather than cancel them. Database failures, process crashes, and runtime shutdown can still lose records. A failed insert is logged and counted without changing the original response. Absence of an event is therefore not proof that no request or change occurred.

### Monitor evidence gaps and arrange notification

The backend exports `audit_write_failures_total{table="audit_logs"}` and `audit_write_failures_total{table="security_events"}` on its metrics endpoint. Configure monitoring to alert on increases, then investigate the corresponding write-failure logs. These counters measure lost writes, not all attacks or all authentication failures. A process restart resets its in-process counts.

Use the [monitoring guide](../monitoring/README.md) to connect collection and alert rules to the deployed backend. Event persistence and metric export do not establish delivery to a security team. The deployment operator must assign an incident owner, choose the notification route, and verify receipt. Keep escalation contacts in the operational runbook so responders can find a maintained route.

## Incident response

1. **Detect and preserve evidence.** Capture the time range, affected operation, HTTP status and GraphQL errors. Review security events, audit records, and backend logs under the roles above. Preserve request IDs when available, without copying bearer tokens or sensitive request content into incident notes.
2. **Assess the boundary that failed.** A 401 points to token rejection; a 403 with `ACCOUNT_SUSPENDED` identifies an account refusal. A role denial can appear as a GraphQL error on HTTP success. Key-unavailable warnings explain anonymous fallback, while account-load 500s and audit-write counters indicate failures in account access or evidence collection. Check proxy trust before attributing traffic to an event's IP.
3. **Contain through supported controls.** An authorized administrator can call `suspendUser(id)` to set the account inactive. It requires `admin.users:suspend`, all staff roles when acting on another user, and refuses self-suspension. Subsequent verifiable bearer requests for that account receive 403; suspension does not block anonymous public reads. Apply IP blocks or request quotas through the operator-owned edge configuration, using verified client attribution. Notify the assigned incident owner through the operational runbook's route.
4. **Repair the cause.** If a resolver missed a capability or resource check, correct that execution path and its relevant authorization coverage. If a server detail escaped in a hand-written error, preserve its typed source. Change schema bounds in the schema builder when justified by the workload; there is no runtime security-profile reload or stored query to remove as a containment action.
5. **Recover and verify.** Use `activateUser(id)` only after the account is safe to reactivate; it has the same capability and target-management checks. Verify public access, protected-operation denials, and the corrected path in an authorized test environment. Check audit outcomes against actual application state, especially after cancellation or write failures, and verify that temporary edge restrictions have the intended scope.
6. **Retain the findings.** Record affected accounts/resources, evidence gaps, containment decisions, and the observed recovery. Update the operational runbook's escalation and notification procedures where the incident exposed a gap.

## Validation before deployment

Use an isolated, authorized environment for behavior checks. The existing source tests provide a starting point; a passing source check alone does not establish deployed routing, proxy trust, or notification delivery.

- Check per-request limit coverage in `graphql/request_limits_tests.rs`: standard introspection, depth, the field limit at and above 500, cost at and above 20,000, the data access contract, the worst-case observation requests, the site's own queries (read from the frontend sources) staying under 60% of the cost limit, and the request timeout. Exercise POST body rejection separately from GET URL handling.
- Check authentication outcomes with sign-in enabled and disabled, invalid tokens, unavailable keys, suspended accounts, and account-load failures. Verify that anonymous fallback cannot acquire signed-in capabilities.
- For each changed protected resolver, verify both the capability check and its resource-specific access rules. Verify validation at the owning service and that typed server errors reach clients only in sanitized form.
- Use authorized audit readers to verify event and mutation outcomes. Observe the write-failure metric and logs when persistence fails; do not infer a complete audit trail from a successful response.
- Verify deployed edge quotas, forwarded-client attribution, metrics collection, and receipt by the assigned incident owner against the operational configuration. These controls need their own deployment evidence.
