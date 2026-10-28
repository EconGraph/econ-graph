# GraphQL Security Implementation Summary

The backend applies controls at three boundaries: the HTTP route limits and parses requests, the schema limits query depth and complexity, and resolvers check the caller's capabilities and access to particular resources. This summary identifies those enforcement points and the audit records they produce. See the [GraphQL Security Guide](GRAPHQL_SECURITY_GUIDE.md) for related guidance.

## Request flow

```text
GET /graphql (query in URL) or POST /graphql (body)
    │
    ├─ POST: count received bytes, stop above 1 MiB, parse a single request
    │
    ├─ Authenticate bearer token and check account status, or use anonymous context
    │
    ├─ Build schema with this request's GraphQLContext
    │
    ├─ Validate query: depth ≤ 15, complexity ≤ 1000
    │
    ├─ Execute resolvers: capability and resource checks, operation-specific validation
    │
    └─ Finish audit writes, sanitize server errors, return GraphQL response
```

The [backend request route](../../backend/crates/econ-graph-backend/src/main.rs) implements this flow in `graphql_request`, `graphql_caller`, and `build_routes`. Its POST body reader counts bytes as they arrive rather than trusting `Content-Length`; a body above `GRAPHQL_BODY_LIMIT` (1,048,576 bytes) receives HTTP 413 with a GraphQL-shaped error. Malformed request bodies receive a GraphQL-shaped bad-request response. The POST body limit does not impose the same byte limit on GET query URLs.

## Authentication boundary

[Bearer authentication](../../backend/crates/econ-graph-auth/src/bearer.rs) returns a caller containing the user's app database row and a `Principal`. The [OIDC verifier](../../backend/crates/econ-graph-auth/src/oidc.rs) checks the token's RS256 signature against the provider's keys and validates issuer, audience, expiry, and not-before claims. Authentication loads or creates the user's row and checks `is_active` before the request reaches resolvers.

Requests without a bearer token run anonymously. When `OIDC_ISSUER` is unset, sign-in is disabled and no supplied token is trusted. With verification enabled, the backend refuses invalid tokens with HTTP 401, suspended accounts with 403, and account database failures with 500. If the provider's keys cannot be fetched and the token cannot be checked, the request runs anonymously; it gains no token capabilities, so protected operations still require sign-in.

## Query limits and schema tooling

The [schema constructors](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs) apply async-graphql's depth and complexity limits before query execution:

| Control | Limit | Enforcement |
| --- | --- | --- |
| Query depth | 15 | `.limit_depth(MAX_QUERY_DEPTH)` |
| Query complexity | 1000 | `.limit_complexity(MAX_QUERY_COMPLEXITY)`; one per selected field, with `__typename` excluded |
| POST body size | 1 MiB | Streaming HTTP body reader, before parsing |

Depth and complexity are source constants, not environment-specific configuration settings. They bound query shape and selection cost; they do not specify a wall-clock execution deadline or a request rate policy.

Schema introspection remains available. The depth limit leaves room for the standard introspection query used by GraphiQL, the playground, and code generation tools, and the schema tests check that query. The browser playground is controlled separately: `/playground` is served only when `ENABLE_GRAPHQL_PLAYGROUND` is `1` or `true` (case-insensitive, with surrounding whitespace ignored). Leaving that setting unset disables the playground route without disabling introspection on `/graphql`.

## Capability and resource authorization

Each request's [GraphQL context](../../backend/crates/econ-graph-graphql/src/graphql/context.rs) carries an optional `Principal`. Its fine-grained `Role` values come from the access token's top-level `roles` claim, not a `users.role` staff tier. The [role catalog](../../backend/crates/econ-graph-auth/src/roles.rs) defines individual capabilities such as `annotation:create`, `chart:share`, and `admin.users:read`. A signed-in principal with no catalog roles receives no capabilities.

Resolvers use the free `require_role` helper to require sign-in and the relevant capability. For example, this is the caller check used when creating or updating an annotation:

```rust
use econ_graph_auth::Role;
use econ_graph_graphql::graphql::context::require_role;

// Inside a resolver with ctx: &async_graphql::Context<'_>:
let user_id = require_role(ctx, Role::AnnotationCreate)?.user_id;
```

The caller's identity comes from the authenticated context rather than an input user ID. A capability check is only one part of authorization: [annotation mutations](../../backend/crates/econ-graph-graphql/src/graphql/mutation.rs) pass that identity to the [collaboration service](../../backend/crates/econ-graph-services/src/services/collaboration_service.rs), which permits annotation updates only by the author. Chart sharing requires both `chart:share` and an existing chart collaborator grant with admin permission. Holding a catalog role alone does not establish ownership or access to every resource.

## Input and error handling

Request parsing and GraphQL argument types establish the request's structure; resolver and service code apply operation-specific checks. For example, annotation mutations parse string IDs as UUIDs before passing them to the collaboration service. These checks are part of the individual operation, rather than a universal input validator that automatically encodes HTML or sanitizes all strings.

Every schema installs the [error sanitizing extension](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs). It logs typed server-side failures and replaces their public message and extensions with `Internal server error` and `INTERNAL_SERVER_ERROR`. Known client-facing errors retain their messages. Resolver authors must propagate typed errors, for example with `?`, instead of formatting internal failures into hand-written error strings: those strings are treated as client-facing and would bypass this classification.

## Audit records and security events

The [GraphQL audit writer](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs) records admin mutation attempts after the caller passes the mutation's role check. Each audit row includes the outcome in `details.outcome`; update details name the affected fields without storing their values. Outcomes distinguish success from validation failures, missing targets, conflicts, authorization refusals, self-lockout protection, database errors, and unavailable data.

The free `require_role` helper starts a `permission_denied` security-event write when a signed-in caller lacks a required role. It deduplicates these events by caller and permission within a request, so aliases of the same denied field do not each create a row. Anonymous authentication failures do not produce that event. The backend's `graphql_caller` also records a `suspended_account` event when it refuses a suspended account before execution.

The [admin query resolvers](../../backend/crates/econ-graph-graphql/src/graphql/query.rs) expose these records through `auditLogs` (requiring `admin.audit:read`) and `securityEvents` or `securityEventsPage` (requiring `admin.security:read`). The rows live in the app database's `audit_logs` and `security_events` tables. These producers and readers provide an inspection path; they do not define automatic threat detection, notification channels, or alert thresholds.

Writes are best-effort. The schema waits for already-started permission-denial writes before returning a normal response, and admin attempts await their outcome write. A failed insert is logged and counted by the [admin record model](../../backend/crates/econ-graph-core/src/models/admin.rs), without changing the request's reply. Cancellation leaves detached writes running. An admin attempt interrupted before its result is known is recorded as `cancelled`, because its database change may already have committed. Process crashes or runtime shutdown can still interrupt writes, so this mechanism is not a durable audit queue.
