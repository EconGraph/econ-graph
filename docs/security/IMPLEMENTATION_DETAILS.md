# GraphQL Security Implementation Details

## Overview

This reference explains where GraphQL request admission, authorization, validation,
error handling, and auditing attach to the backend. The controls span the HTTP
server, the schema, and individual resolvers and services; there is no single
security middleware that applies every check to every field.

## Code Architecture

### Module boundaries

Paths below are relative to the repository root.

| Owner | Implementation |
| --- | --- |
| HTTP admission, caller authentication, routes, and client address | [Backend `main.rs`](../../backend/crates/econ-graph-backend/src/main.rs) |
| Token verification and shared bearer authentication | [Auth `oidc.rs`](../../backend/crates/econ-graph-auth/src/oidc.rs), [auth `bearer.rs`](../../backend/crates/econ-graph-auth/src/bearer.rs) |
| Fine-grained capability catalog and principal | [Auth `roles.rs`](../../backend/crates/econ-graph-auth/src/roles.rs) |
| Schema limits and extensions | [GraphQL `schema.rs`](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs) |
| Per-request caller and resolver guards | [GraphQL `context.rs`](../../backend/crates/econ-graph-graphql/src/graphql/context.rs) |
| Field implementations and argument checks | [Query resolvers](../../backend/crates/econ-graph-graphql/src/graphql/query.rs), [mutation resolvers](../../backend/crates/econ-graph-graphql/src/graphql/mutation.rs) |
| Client-safe error boundary | [GraphQL `errors.rs`](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs) |
| Admin audit attempts and permission-denial writes | [GraphQL `audit.rs`](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs), [core admin models](../../backend/crates/econ-graph-core/src/models/admin.rs) |
| Prometheus registration and export | [Backend `metrics.rs`](../../backend/crates/econ-graph-backend/src/metrics.rs) |

### Request execution

`build_routes` in the backend accepts a GET query through the GraphQL HTTP adapter
or reads a POST body through `read_graphql_body`. POST bytes are counted before
GraphQL body parsing. The adapter converts the parsed batch representation to a
single request, so this endpoint does not execute a JSON batch of operations.

After parsing the HTTP request, `graphql_caller` authenticates its bearer token.
A refusal returns immediately with an HTTP error. Otherwise the backend creates
an `Arc<GraphQLContext>` with the caller and client information, builds the
request's schema with the app pool and optional data pool/client, and calls
`auth_schema.execute(request)` directly. Schema validation checks depth and
complexity before resolver execution. Resolvers then enforce their roles and
argument rules, and services enforce their domain and object-access rules.
`SanitizeErrors` and `AuditWrites` participate in schema execution to sanitize
returned errors and wait for pending permission-denial writes.

The app pool holds users and application state. The optional data pool holds
sources, series, observations, and crawl state. A backend without a data pool
returns `extensions.code = DATA_SERVED_REMOTELY` for data fields; the context's
`data_pool` and `data_loaders` helpers enforce that distinction.

The backend serves HTTP/1.1 only via `hyper::Server::http1_only(true)`; TLS
terminates at the gateway. This keeps cleartext HTTP/2 unavailable even on the
backend NodePort. The peer address is inserted into each request as `PeerAddr`
for client attribution.

## Request and Query Limits

### HTTP body admission

`GRAPHQL_BODY_LIMIT` in backend `main.rs` is `1024 * 1024` bytes (1 MiB). The POST
reader accumulates chunks only while the received total fits; it does not trust
`Content-Length`. An oversized body returns HTTP 413 with a GraphQL-shaped error:

```json
{"errors":[{"message":"Request body is larger than 1048576 bytes"}]}
```

This bounds the accepted POST body, including variables and its JSON envelope.
It is not a 1 MiB limit on the query string alone, and it does not set a GET URL
length limit. Malformed GraphQL request bodies use the adapter's status (400 for
malformed JSON) and a JSON `errors` response through `graphql_bad_request`.
These HTTP refusals happen before schema execution and its error extension.

### Schema depth and complexity

The executable schema builders in `schema.rs` apply:

```rust
.limit_depth(MAX_QUERY_DEPTH)
.limit_complexity(MAX_QUERY_COMPLEXITY)
```

`MAX_QUERY_DEPTH` is 15. The standard introspection query nests 13 levels, so this
allows schema tooling while rejecting deeper operations. `MAX_QUERY_COMPLEXITY`
is 1000, using async-graphql's default field cost. There are no configured custom
field weights or list-result multipliers in this builder. `__typename` does not
count toward the cost used in the boundary tests.

For example, each aliased `dataSources { id }` selection costs 2. The existing
tests construct 500 such selections to reach 1000 and 501 to exceed it. The
first passes complexity validation, although its resolvers can still fail; the
second is rejected with an error containing `too complex`. The depth test
similarly asserts an error containing `too deep`.

Selected-field cost does not measure returned rows, database work, elapsed time,
or bytes in the response. A list field can return many rows without increasing
its selected-field cost. Resolver pagination and service validation therefore
remain necessary. These schema limits do not establish a hard CPU or memory
budget, and they do not impose a backend execution deadline.

Introspection is available within the schema limits. `/playground` is a separate
HTTP route, disabled unless `ENABLE_GRAPHQL_PLAYGROUND` is `true` or `1`.
Disabling that route does not disable introspection.

### Gateway request throttling and forwarding

The [Gateway HTTPRoutes](../../k8s/manifests/gateway/routes/httproutes.yaml)
route `/graphql` and `/data/graphql` to the backend. The latter rewrites its
prefix to `/graphql` and removes `Authorization` and `Cookie`, so data-route
requests reach the backend anonymously regardless of browser credentials. Both
routes set a 30-second gateway request timeout; this is separate from a backend
execution deadline and does not guarantee that cancelled work never commits.

[BackendTrafficPolicy rate limits](../../k8s/manifests/gateway/routes/ratelimit-policies.yaml)
apply a local per-client token bucket to each GraphQL route: 20 requests per
second, with separate buckets for the two routes and HTTP 429 refusals above
the allowance. The [HTTP burst patch](../../k8s/manifests/gateway/ratelimit-burst.yaml)
and [HTTPS gateway configuration](../../k8s/manifests/gateway/gateway-https.yaml)
raise bucket capacity to 100 while retaining the 20-per-second refill. Without
the matching burst patch, the policy's ordinary capacity is 20. Check policy
status and generated configuration before relying on the larger burst.

Each proxy replica counts locally. The configured single proxy replica keeps
one set of counters; scaling replicas does not create a shared global quota.
Direct backend access, including the backend NodePort and the admin frontend's
`/admin/graphql` proxy, bypasses these gateway policies. There is no per-IP
sliding-window limiter or 20-connection limit in the Rust GraphQL handler.

The [client traffic policy](../../k8s/manifests/gateway/client-traffic-policy.yaml)
uses the TCP peer except for trusted pod CIDRs, where it selects the rightmost
forwarded address outside those ranges. This lets the Cloudflare tunnel convey
the visitor address. Any pod in the trusted ranges can claim an address, so
in-cluster client attribution is not audit-grade. Host-local requests, shared
NATs, and some in-cluster paths can share an address and therefore a bucket.
Every HTTPRoute replaces `X-Forwarded-For` with the gateway's selected address.

Backend `TRUSTED_PROXIES` is a separate trust decision: it determines which
forwarded addresses the backend records for auditing. Backend `client_ip` starts
at the peer and walks `X-Forwarded-For` from right to left only through trusted
proxies; an untrusted peer cannot supply an arbitrary audit address by writing
that header. Invalid hops stop the walk.

## Input Validation

### Resolver and service sequence

GraphQL type validation checks the operation against the schema. Application
validation then happens at the field and service boundaries. The
`create_annotation` resolver in [mutation.rs](../../backend/crates/econ-graph-graphql/src/graphql/mutation.rs)
is a concrete example:

```rust
let user_id = require_role(ctx, Role::AnnotationCreate)?.user_id;
let series_id = uuid::Uuid::parse_str(&input.series_id)?;
let is_public = input.is_public.unwrap_or(false);
```

The author comes from the principal, never a user ID supplied in the input. The
resolver constructs `CollaborationService` with the app pool and a `SeriesCheck`,
then calls `create_annotation` with the parsed ID and input fields.

[CollaborationService](../../backend/crates/econ-graph-services/src/services/collaboration_service.rs)
validates the idempotency key when supplied, annotation text, and the annotation's
date and numeric value before writing. Text limits count Unicode characters:
titles must be nonblank and at most 200 characters, notes at most 5,000,
annotation types nonblank and at most 20, and colors at most 7. Comments have a
separate nonblank, 2,000-character limit. These length checks do not establish a
color syntax or strip HTML.

Series existence is checked in the data database when available, otherwise
through the configured data-plane client. With neither source configured,
`SeriesCheck::Skip` omits that check and startup logs say so. An unavailable
configured data plane refuses the annotation rather than silently skipping the
check. Object visibility and editing rights remain separate from the capability
guard: the collaboration service restricts private annotation visibility to its
author and applies ownership checks to updates and deletes.

### SQL and rendering boundaries

There is no query-wide HTML/JavaScript sanitizer or SQL-character escaping stage
in this request path. Validation is specific to the operation. For example,
[SearchService](../../backend/crates/econ-graph-services/src/services/search_service.rs)
validates search parameters and passes user search terms through Diesel `.bind`
parameters; annotation insertion uses Diesel's typed insert builder. Preserve
parameter binding when changing these queries rather than treating text
escaping as the SQL boundary. Output rendering must handle its own text/HTML
context; successful input validation does not make arbitrary HTML safe to render.

## Authentication and Authorization

### From token to request context

`authenticate` verifies a bearer token, loads or creates the app-plane user row
for its subject, and rejects an inactive account. [OidcVerifier](../../backend/crates/econ-graph-auth/src/oidc.rs)
checks RS256 signatures, issuer, audience, token times, and access-token type.
The verified subject supplies the principal's user ID. Known fine-grained roles
come from the token's top-level `roles` claim; names outside the role catalog
are ignored and grant no capability.

`Principal` stores the roles in a `BTreeSet<Role>`. `authorize` checks membership
in that set. Higher-level staff, plan, or organization roles are composed by the
identity provider into these capabilities; the backend does not infer runtime
inheritance from a user-row role or grant every capability to an `Admin` label.

The backend's `graphql_caller` makes authentication failures explicit:

| Condition | GraphQL HTTP route behavior |
| --- | --- |
| No bearer token, or sign-in disabled because there is no verifier | Anonymous context; no token grants |
| Provider keys unavailable so a supplied token cannot be verified | Warning log and anonymous context; no token grants |
| Invalid or expired token | HTTP 401, `UNAUTHENTICATED`, and a bearer `WWW-Authenticate` challenge |
| Suspended account | HTTP 403, `ACCOUNT_SUSPENDED`, and a best-effort security event |
| Account load/create fails | HTTP 500, `INTERNAL_SERVER_ERROR`, with a generic account-loading message |

The provider-unavailable path preserves anonymous access to public data. It does
not grant authenticated access: protected resolvers still require a principal.

### Resolver guards and object access

Resolvers use the free `context::require_role` helper, for example:

```rust
let caller = require_role(ctx, Role::AdminUsersUpdate)?;
```

`GraphQLContext::require_role` returns `Authentication required` without a
principal and `Insufficient permissions` when the principal lacks the role.
The free helper also records a signed-in caller's missing-role denial for the
schema's audit extension. Calling only the context method performs the role
check without that helper's audit recording. A resolver that needs the user row
also calls `current_user`; role membership alone is not object ownership.

When adding a protected field, use the required capability from the
[role catalog](../../backend/crates/econ-graph-auth/src/roles.rs) and retain the
service's object-specific checks. For example, `annotation:create` permits an
attempt to create an annotation; it does not authorize editing another author's
private annotation.

## Error Handling and Auditing

### Typed errors and safe client messages

`SanitizeErrors` inspects typed error sources after execution. Allowlisted
client-facing errors, such as validation, permission, not-found, and malformed
UUID/date errors, keep their messages. Unrecognized typed server errors,
including database and pool failures, are logged with the operation and field
path, then replaced with `Internal server error` and
`extensions.code = INTERNAL_SERVER_ERROR`. Existing extensions on a hidden
server error are replaced too, so they cannot leak its detail.

Resolvers should propagate a service error with `?` or `Error::new_with_source`.
Do not turn driver errors into hand-written strings: source-free errors and
string sources are treated as deliberate client messages. For invalid arguments
that need a stable code, `errors::bad_user_input` supplies `BAD_USER_INPUT`.
This is an error-output boundary, separate from input validation and HTTP token
refusals.

### Audit records and failure behavior

The [audit extension](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs)
and [admin models](../../backend/crates/econ-graph-core/src/models/admin.rs) write
`audit_logs` and `security_events` in the app database. Admin mutation attempts
by callers holding the mutation's role record an outcome such as `success`,
`invalid`, `not_found`, `conflict`, `denied`, `self_lockout`, `error`, or
`unavailable`. Update details name affected fields without recording their
values. A signed-in caller missing a role generates a `permission_denied`
security event, deduplicated by caller and permission within the request.

Denial writes start when the denial occurs, and `AuditWrites` awaits their
handles before a normal response returns. Writes detach when requests are
cancelled. An interrupted admin attempt is recorded as `cancelled` because the
mutation may already have committed. A process crash or runtime shutdown can
still interrupt these writes; they are not a durable queue.

Audit insert failures are logged and counted but do not change the response or
undo the action. The audit insert is separate from the action's transaction, so
a successful action can have no persisted audit row. This best-effort behavior
must be considered when interpreting missing records.

## State, Cost, and Monitoring

The backend shares an OIDC verifier and database pools. The verifier caches
signing keys, refreshes after 15 minutes, and may use cached keys for up to 24
hours when refresh fails. Unknown key IDs trigger refresh attempts limited to
one per 10 seconds. These constants bound key refresh behavior, not GraphQL
request rate or operation duration. See `MAX_KEY_AGE`, `MAX_STALE_KEY_AGE`, and
`MIN_REFRESH_INTERVAL` in the verifier for the exact refresh paths.

The active GraphQL route builds a schema and its optional data-database
DataLoaders for each authenticated or anonymous request. `GraphQLContext`
holds that request's principal, client metadata, ID, and pending denial writes.
It is not a cache of grants across requests. Cost depends on selected fields,
resolver work, database access, and audit writes; schema admission limits alone
do not establish a latency guarantee.

[Backend metrics](../../backend/crates/econ-graph-backend/src/metrics.rs) exports
`audit_write_failures_total{table="audit_logs"}` and
`audit_write_failures_total{table="security_events"}` at `/metrics`. Scraping
synchronizes these counters from process-local failed-write totals. An increase
means audit information was lost, even if the associated request succeeded.

The module also defines `graphql_queries_total`,
`graphql_query_duration_seconds`, and `graphql_query_complexity`. The separate
`graphql_handler` function records them with a fixed complexity of `1.0`, but
`build_routes` executes `auth_schema` directly without calling that function.
Those instruments therefore do not establish active-route GraphQL traffic,
measured complexity, or security overhead. There are no rejection counters in
this path for each depth, complexity, or gateway-rate refusal. Use responses
and the relevant backend or gateway logs to diagnose those cases.

## Testing Implementation

The repository contains concrete tests for the boundaries above:

| Source and test group | What it checks |
| --- | --- |
| [`schema.rs`, `query_limit_tests`](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs) | Shallow operations and standard introspection; excessive depth; complexity at and above 1000 |
| [`context.rs`, `tests`](../../backend/crates/econ-graph-graphql/src/graphql/context.rs) | Anonymous refusal, token-role membership, and no grants from an empty role set |
| [`errors.rs`, `tests`](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs) | Generic server messages, removal of hidden extensions, preserved client errors, and logging of hidden detail |
| [Backend HTTP and authentication tests](../../backend/crates/econ-graph-backend/src/main.rs) | Malformed JSON 400, oversized POST 413, streamed-byte counting, token refusals, CORS, and trusted proxy handling |
| [`collaboration_service.rs`, `validation_tests`](../../backend/crates/econ-graph-services/src/services/collaboration_service.rs) | Text boundaries, dates, numeric values, and idempotent ID scoping |
| [`audit.rs`, database-backed tests](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs) | Admin outcome rows, deduplicated denials, client metadata, and request-cancellation behavior |

These tests have different infrastructure needs. Schema limit tests use an
unreachable pool instead of a live database; the at-limit complexity test can
reach resolvers and incur its one-second connection timeout. Audit tests need a
disposable database, while OIDC verification tests use a local test issuer.
Consult [database testing](../testing/DATABASE_TESTING.md) before running
integration tests. None of these correctness tests establishes a per-request
latency or production load capacity.

## Configuration and Deployment

### Configuration lifecycle

Query depth, query complexity, and POST body size are Rust constants. Changing
them requires changing source and rebuilding the backend; setting
`SECURITY_MAX_DEPTH` or similar environment variables does not change these
controls. There is no security TOML loader for this request path.

The backend reads [core configuration](../../backend/crates/econ-graph-core/src/config.rs)
and [OIDC configuration](../../backend/crates/econ-graph-auth/src/oidc.rs) at
startup. Core configuration also attempts to load a local `.env` file. Relevant
settings are:

| Setting | Effect |
| --- | --- |
| `BACKEND_PORT` | Listening port; defaults to `9876`. The server binds `0.0.0.0`; the parsed `SERVER_HOST` setting does not control this bind |
| `CORS_ALLOWED_ORIGINS` | Comma-separated browser origins, validated as bare HTTP(S) origins; defaults to the local frontend origin |
| `ENABLE_GRAPHQL_PLAYGROUND` | Enables the playground route for `true` or `1`; unset leaves it off |
| `OIDC_ISSUER` | Enables token verification; unset/empty disables sign-in |
| `OIDC_REQUIRED` | `true` stops startup if the issuer is absent; it does not require authentication on public fields |
| `OIDC_AUDIENCE` | Required token audience; defaults to `econ-graph-api` |
| `OIDC_JWKS_URL` | Optional key-fetch URL override; token issuer checks still use `OIDC_ISSUER` |
| `TRUSTED_PROXIES` | Trusted proxy IP addresses/CIDRs for audit client attribution; parsed on first use and retained for the process |

Malformed OIDC settings and invalid CORS origins fail startup. Provider discovery
and key fetching are lazy, so provider availability is not a startup check.
Configuration changes require a process restart; key refresh follows the
verifier's own cache lifecycle. The backend tracing subscriber fixes its maximum
level to INFO, so setting `RUST_LOG=debug` alone does not enable debug output.

### Executable and deployment ownership

The executable is `econ-graph-backend`; `econ-graph-graphql` is a library linked
into it. Use the repository's [backend Dockerfile](../../backend/Dockerfile) and
[backend deployment](../../k8s/manifests/backend-deployment.yaml), which run the
backend on port 9876.
Use [Kubernetes deployment guidance](../../k8s/README.md) for routing and
[database settings](../../k8s/README.md#databases) for the app/data layout.
Gateway request limits are changed in the applicable traffic policies and burst
patches, independently of compiled schema limits.

## Troubleshooting

### Identify the rejecting layer

| Symptom | Diagnosis |
| --- | --- |
| HTTP 413 and `Request body is larger than 1048576 bytes` | Backend POST admission rejected actual received bytes; inspect variables and the JSON envelope as well as the document |
| GraphQL error containing `too deep` or `too complex` | Schema admission rejected the selection; reduce nesting or selected fields/aliases |
| HTTP 429 at the gateway | Check the HTTPRoute, rate-limit policy, and proxy logs; verify the selected client address before changing a per-IP limit |
| HTTP 401 with `UNAUTHENTICATED` | Token verification rejected the token; check issuer, audience, expiry, access-token type, and signing-key configuration |
| `Authentication required` despite sending a token | Check startup sign-in configuration and warnings about unverifiable tokens; the route can deliberately run anonymously |
| `Insufficient permissions` | Inspect the required catalog role and the token's flattened role grants; account labels do not supply capabilities |
| `Internal server error` | Find `GraphQL request failed` in backend output using operation and field path; preserve typed sources when fixing the resolver |
| Missing audit row or security event | Check `audit_write_failures_total` and write-failure logs; a successful response does not prove persistence |

Backend tracing goes to process output through the formatting subscriber; there
is no configured `/var/log/econ-graph-graphql/security.log` writer. For an already
running local backend on its default port, these read-only requests inspect
metrics and exercise a shallow public operation:

```bash
curl --fail-with-body http://127.0.0.1:9876/metrics
curl --fail-with-body http://127.0.0.1:9876/graphql \
  -H 'Content-Type: application/json' \
  --data '{"query":"{ __typename }"}'
```

To investigate missing audit data, search process logs for `could not write the
audit log entry` or `could not write the security event`, and examine the
counter for the corresponding table. Counters reset with the process; a zero
value does not prove historical completeness. For broader collection and alert
configuration, see [monitoring guidance](../monitoring/README.md).
