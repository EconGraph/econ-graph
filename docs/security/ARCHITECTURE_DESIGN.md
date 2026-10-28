# GraphQL Security Architecture

EconGraph distributes GraphQL request protection across Envoy Gateway, the backend HTTP
route, the schema, and individual resolvers and services. These boundaries address
different threats: the Gateway limits request frequency, the backend bounds POST body
buffering and establishes identity, and the schema and resolvers control query
shape and access to application data. No single layer substitutes for the others.

This document explains those mechanisms and their trade-offs for maintainers.
See the [GraphQL API reference](../api/GRAPHQL_API.md) for the caller contract and
[admin audit guidance](ADMIN_AUDIT.md) for audit records and operational checks.

## Request and trust boundaries

```text
Client
  |
  +---------------------------------------------+
  |                                             |
  v                                             v
Envoy Gateway                                   Direct backend / admin proxy
  /graphql: retains bearer credentials           bypasses GraphQL route limits
  /data/graphql: strips credentials,
    rewrites to /graphql
  per-client rate limits; request timeout
  |                                             |
  +------+--------------------------------------+
         |
         v
Backend /graphql HTTP route
  GET query parsing / POST streaming body cap and request parsing
         |
         v
Bearer authentication + active app-plane account lookup
  -> refusal, anonymous context, or signed-in Principal
         |
         v
Schema execution boundary
  type validation + depth/complexity limits
         |
         v
Resolvers and services
  required capabilities + resource visibility/ownership + operation validation
         |
         v
App/data resources through typed pools and query construction
         |
         v
Response boundary
  audit completion on normal responses + internal-error sanitization
```

The diagram follows the [backend route](../../backend/crates/econ-graph-backend/src/main.rs)
and [schema builder](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs).
Audit writes also begin during execution; cancellation can leave them running
after the request ends. Resolver checks run where the operation needs them,
rather than in a universal input-sanitization pass before execution.

## Gateway: request frequency and proxy trust

The [GraphQL HTTPRoutes](../../k8s/manifests/gateway/routes/httproutes.yaml)
forward `/graphql` to the backend. The separate `/data/graphql` route rewrites
that prefix to `/graphql` and removes `Authorization` and `Cookie`, so data
requests reach the backend anonymously regardless of browser credentials.
The [BackendTrafficPolicies](../../k8s/manifests/gateway/routes/ratelimit-policies.yaml)
give each route independent per-client-address token buckets, refilled at
20 requests per second. Exhausting a bucket produces HTTP 429. These inbound
limits are separate from the crawler's outbound source-fetching limits.

The ordinary bucket holds 20 tokens. A burst of 100 requires the matching
`EnvoyPatchPolicy` to raise its capacity while retaining the 20-per-second refill:
[the HTTP patch](../../k8s/manifests/gateway/ratelimit-burst.yaml) and
[the HTTPS patch](../../k8s/manifests/gateway/gateway-https.yaml) target their
respective listeners. The larger burst applies only when the patch is effective
in the generated Envoy configuration; the base rate-limit policy alone allows
bursts of 20. Counting is local to each proxy replica. The
[proxy manifest](../../k8s/manifests/gateway/envoy-proxy.yaml) sets one replica;
adding replicas gives each its own buckets rather than sharing a global quota.

The [route application script](../../scripts/deploy/apply-gateway-routes.sh)
applies the localhost routes and policies. For a public host, its
[renderer](../../scripts/deploy/render-public-host.py) also produces HTTPS route
copies and extends the rate-limit policy targets to cover them. The backend
NodePort and the [admin frontend proxy](../../k8s/manifests/admin-frontend-nginx-configmap.yaml)
at `/admin/graphql` bypass the GraphQL Gateway route limits. Backend body,
identity and schema checks still apply through those paths. See the
[Kubernetes deployment guide](../../k8s/README.md) for Gateway setup and routing.

Per-client fairness depends on the address the proxy selects. The
[`ClientTrafficPolicy`](../../k8s/manifests/gateway/client-traffic-policy.yaml)
uses the TCP peer unless that peer belongs to a trusted pod CIDR. For a pod peer,
it reads `X-Forwarded-For` from right to left and selects the first non-pod
address. The [Gateway installer](../../scripts/deploy/install-gateway.sh)
substitutes the trusted pod ranges into the policy. This accommodates the
Cloudflare Tunnel connector, but any pod can claim a client address through
this trust boundary. In-cluster client addresses therefore are not reliable
evidence of origin.

Every route replaces `X-Forwarded-For` with the address the Gateway selected.
The client policy removes supplied `X-Forwarded-Host`, `X-Forwarded-Port`,
`X-Forwarded-Proto`, `Forwarded` and `X-Real-IP` before Envoy adds its own headers;
Envoy derives `X-Forwarded-Proto` from the listener. These rules prevent an
external caller from passing an arbitrary forwarding chain to services.
Source NAT can still collapse clients into one bucket: clients behind a shared
NAT share a budget even with correct attribution. Frequency limits trade
protection from repeated expensive requests against legitimate shared-network
traffic, and their scope is each route on each proxy replica.

Both GraphQL route rules set a 30-second request timeout. This bounds the proxy's
wait for a response, not total resolver or database execution time. A client can
lose a legitimate long-running response while work continues; the setting does
not establish backend cancellation or an escalating per-query timeout policy.

The backend independently determines addresses for audit records. Its
`client_ip` function ignores `X-Forwarded-For` unless the immediate peer is in
`TRUSTED_PROXIES`. It then walks the chain from right to left through trusted
proxies, stopping at the first untrusted address or malformed entry. The default
empty trust list uses the peer address. This audit-address boundary must match
the actual proxy chain; trusting a peer also trusts the address claims that peer
can forward. Gateway client detection and backend audit attribution are separate
configurations.

## HTTP parsing and browser access

The backend's `graphql_request` filter accepts a query in a GET URL or a POST
body. POST uses a streaming cap of 1 MiB (`GRAPHQL_BODY_LIMIT`) before parsing:
chunks that would exceed the cap are refused instead of extending the buffer.
An oversized body receives HTTP 413 with a GraphQL-shaped error. The POST parser
converts the parsed batch representation into a single request, rejecting batch
requests. This cap bounds POST body buffering; it is not a row limit or a GET URL
size limit.

The backend validates `CORS_ALLOWED_ORIGINS` at startup and applies its CORS
filter to routes. CORS governs browser cross-origin access; non-browser callers
still need the same authentication and authorization checks. The Gateway routes
leave CORS enforcement to the backend so that this policy has one owner.

## Authentication establishes identity; resolvers authorize actions

[`OidcVerifier`](../../backend/crates/econ-graph-auth/src/oidc.rs) checks RS256
access-token signatures using the identity provider's JWKS, together with issuer,
audience, expiry and not-before claims. Keys are fetched lazily and cached.
An unknown signing-key ID triggers a rate-limited refresh; aged keys also trigger
refresh, with bounded stale-key use during provider outages. This reduces
provider calls while allowing key rotation, at the cost of a period during
which cached keys remain usable.

[`authenticate`](../../backend/crates/econ-graph-auth/src/bearer.rs) loads or
creates the token subject's user row in the app plane and checks `is_active`.
The result is a `Caller` containing that row and a `Principal` with the user ID
and recognized fine-grained roles from the top-level token `roles` claim.
Unknown role names grant no capability. The backend does not authorize using a
`users.role` hierarchy: the identity provider composes broader user or staff
roles from the capabilities in the [role catalog](../../backend/crates/econ-graph-auth/src/roles.rs).

The HTTP route deliberately distinguishes authentication failures:

| Condition | GraphQL route behavior |
| --- | --- |
| No bearer token, or sign-in disabled by an absent verifier | Run anonymously; trust no token |
| Verification unavailable because provider keys cannot be obtained | Run anonymously, granting no token roles |
| Invalid or expired token | Refuse with HTTP 401 and a bearer challenge |
| Suspended account | Refuse with HTTP 403 and record a suspended-account event |
| Account lookup or creation fails | Refuse with HTTP 500 |

Anonymous fallback preserves public browsing through a provider outage. It does
not permit protected actions: [`GraphQLContext`](../../backend/crates/econ-graph-graphql/src/graphql/context.rs)
requires a principal before checking a role. A signed-in caller lacking the
required role receives an insufficient-permissions error.

Roles and resource checks solve different access problems. For example,
[`create_annotation`](../../backend/crates/econ-graph-graphql/src/graphql/mutation.rs)
requires `annotation:create` and derives the author ID from the principal,
rather than trusting a submitted user ID. Commenting requires
`annotation:comment`, while the collaboration service also checks whether the
caller can see the target annotation. Public annotations are visible to everyone;
private ones are visible only to their author. Chart sharing requires both
`chart:share` and an existing chart collaborator grant that permits
administration. A capability alone does not establish ownership or visibility.

## Schema limits and introspection

The executable schema applies these compiled settings in
[`create_schema_with_data_plane`](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs):

```rust
.limit_depth(MAX_QUERY_DEPTH)           // 15
.limit_complexity(MAX_QUERY_COMPLEXITY) // 1000
```

Complexity counts one per selected field, with `__typename` excluded by
async-graphql; it does not assign custom field weights or multiply cost by the
size of returned lists. Depth limits nested selection shape. The depth allowance
leaves room for the standard introspection query, which nests 13 levels, while
refusing more deeply nested queries before
resolver execution. The tests exercise the complexity boundary with aliases,
so repeating selections still consumes the budget.

These limits constrain query shape, not the number of rows a field can return
or the cost of each database operation. A shallow series-data request can still
return thousands of points. Gateway frequency limits and operation-specific
pagination address different parts of that resource cost. Increasing the shape
budget can admit legitimate larger selections and also more work per request;
it requires a code change and rebuild.

Introspection is available, including the standard tooling query tested in the
schema module. Schema visibility is therefore not an authorization barrier.
`/playground` is a separate browser interface, disabled unless
`ENABLE_GRAPHQL_PLAYGROUND` is `true` or `1`; hiding that interface does not
remove introspection from `/graphql`.

## Validation, SQL construction and error disclosure

GraphQL validates operation types and argument shapes. Resolvers and services
then validate domain rules. The
[collaboration service](../../backend/crates/econ-graph-services/src/services/collaboration_service.rs),
for example, rejects blank titles, bounds annotation titles to 200 characters,
notes to 5,000 and comments to 2,000, and validates annotation dates and numeric
values before storage. The annotation resolver separately parses the series ID
as a UUID. These checks reject malformed application data; they do not replace
permissions or safely construct SQL by themselves.

The annotation insert passes a `NewChartAnnotation` value to Diesel's
`insert_into(...).values(...)`, and its idempotent lookup uses typed predicates
such as `.filter(chart_annotations::user_id.eq(user_id))`. User values are query
parameters through that construction, rather than strings interpolated into
SQL syntax. This is the relevant injection boundary for these operations.
Character filtering is not the mechanism that makes those queries safe, and
these examples do not establish the construction of every query in the system.

[`SanitizeErrors`](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs)
is a response-disclosure control. It keeps known client-facing errors and hides
other typed server errors behind `Internal server error` with
`extensions.code = INTERNAL_SERVER_ERROR`, logging the detail on the server.
For hidden errors it also replaces extensions, which may contain internal
information. A resolver must pass the typed error through, using `?` or
`Error::new_with_source`; formatting a driver error into a hand-written message
marks it as client-facing and defeats this distinction.

Error sanitization does not sanitize stored HTML or make content safe in every
browser rendering context. Maintaining these separate responsibilities avoids
mistaking a friendly validation error or hidden database message for a general
SQL-injection or XSS guarantee.

## Audit and observability boundaries

The [`audit` module](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs)
records admin mutation attempts by callers holding the mutation's role, with
an outcome describing success, refusal, validation failure or other termination.
Audit rows include actor, action, resource, client information and request ID;
update details name changed fields without storing their values. A signed-in
caller denied a required role produces a `permission_denied` security event,
deduplicated per caller and permission within a request. This prevents aliases
of one denied field from producing hundreds of identical records. Anonymous
requests do not produce these signed-in role-denial events.

Writes are best-effort: insertion failure is logged and counted without changing
the request result. Normal responses wait for pending denial writes. Cancellation
leaves detached writes running; an interrupted admin attempt is recorded as
`cancelled` because cancellation cannot establish whether its statement committed.
Process crashes or runtime shutdown can still interrupt writes. This preserves
request availability when audit storage fails, with a corresponding limit on
audit durability; the mechanism is not a durable queue or an audit of every
request and authorization decision.

The backend's [metrics exporter](../../backend/crates/econ-graph-backend/src/metrics.rs)
exposes `audit_write_failures_total` with a `table` label for `audit_logs` and
`security_events`, synchronized from the model's failure counters at scrape time.
Registered HTTP and GraphQL metric families do not by themselves prove coverage
of the active request route. The active `/graphql` closure executes the schema
directly, without calling the separate GraphQL metric-recording helper.
Audit records and exported counters also do not themselves send notifications.
See [monitoring guidance](../monitoring/README.md) for metric consumers and alert
configuration, and [admin audit guidance](ADMIN_AUDIT.md) for event coverage and
failure handling.

## Configuration, cost and verification

The security controls have distinct configuration lifecycles. Gateway routes and
traffic policies take effect through applied controller configuration. Body, depth
and complexity limits are compiled constants. The backend loads CORS and playground
settings at startup; trusted proxy ranges are initialized once from the
environment. There is no shared runtime `SecurityConfig` API that validates,
hot-reloads and rolls back all of these policies together. Updating one boundary
must account for the others, particularly Gateway bypasses and proxy identity.

Request costs include parsing and selection validation, token verification and
key-cache refresh, app-plane account lookup, resolver work and audit writes.
Adding backend replicas does not remove the shared database cost or make audit
writes durable. Capacity decisions need measurements for the workload and
routing arrangement being deployed; this architecture supplies no fixed latency
or memory allowance for a separate security subsystem. Use the
[Kubernetes deployment guide](../../k8s/README.md) for deployment settings and
[host storage guidance](../deployment/HOST_STORAGE.md) for backup and recovery
procedures.

Existing test sources protect specific boundaries:

- [Backend route tests](../../backend/crates/econ-graph-backend/src/main.rs) exercise
  body-size refusal and authentication response behavior.
- [Schema tests](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs)
  exercise depth and complexity rejection and standard introspection acceptance.
- [Context tests](../../backend/crates/econ-graph-graphql/src/graphql/context.rs)
  check anonymous callers, token-derived roles and empty role sets.
- [Authorization tests](../../backend/crates/econ-graph-graphql/src/graphql/authorization_tests.rs)
  and [annotation visibility tests](../../backend/crates/econ-graph-graphql/src/graphql/annotation_visibility_tests.rs)
  cover capability and resource-access decisions.
- [Error tests](../../backend/crates/econ-graph-graphql/src/graphql/errors.rs) and
  [audit tests](../../backend/crates/econ-graph-graphql/src/graphql/audit.rs) cover
  response disclosure, failed-mutation audit outcomes and cancellation behavior.

These are test contracts, not evidence of a live deployment's configuration or a
penetration-test result. The [security workflow](../../.github/workflows/security.yml)
runs daily or by manual dispatch. It collects Rust and npm dependency audits,
Trivy filesystem and container-image scans, JavaScript CodeQL analysis, and
license reports. Its Rust and npm audit steps allow failures, so their artifacts
require inspection even when the workflow passes. These checks complement the
request-boundary tests; they do not establish live routing behavior, application
penetration-test results or successful delivery of security alerts.
