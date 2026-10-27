# GraphQL Security API Reference

This reference describes authentication, permissions, request limits, and failures
for callers of the application’s `/graphql` endpoint. Use the
[GraphQL API reference](../api/GRAPHQL_API.md) for operation semantics and the
[committed schema](../../backend/crates/econ-graph-graphql/schema.graphql) for
complete arguments and result fields. Server development is covered in the
[backend architecture overview](../../backend/README.md#architecture-overview).

## Authentication

Send an access token in the `Authorization` header for operations that require
sign-in. This example reads the caller’s own account; replace the token and user
ID placeholders before using it:

```http
POST /graphql HTTP/1.1
Content-Type: application/json
Authorization: Bearer <access-token>

{"query":"query OwnAccount($id: ID!) { user(userId: $id) { id name } }","variables":{"id":"<signed-in-user-id>"}}
```

A request without an authorization header, or with a scheme other than `Bearer`,
runs anonymously. The scheme name is case-insensitive. When sign-in is disabled
on the server, every request runs anonymously, regardless of the header; no token
is trusted.

When sign-in is enabled, an empty bearer token or an invalid or expired token
refuses the request before GraphQL execution. A valid token must also belong to
an active account. These failures are distinct:

| Condition | HTTP status | `errors[].extensions.code` | `errors[].message` |
| --- | --- | --- | --- |
| Empty, invalid, or expired bearer token | 401 | `UNAUTHENTICATED` | `Invalid or expired access token` |
| Suspended account | 403 | `ACCOUNT_SUSPENDED` | `This account is suspended` |
| Signed-in account cannot be loaded or created | 500 | `INTERNAL_SERVER_ERROR` | `Could not load the signed-in account` |

The 401 response includes `WWW-Authenticate: Bearer error="invalid_token"`.
The 403 and 500 responses above do not include that challenge. For example, the
401 response body is:

```json
{
  "data": null,
  "errors": [
    {
      "message": "Invalid or expired access token",
      "extensions": { "code": "UNAUTHENTICATED" }
    }
  ]
}
```

If the identity provider’s signing keys are unavailable and the token cannot be
checked, the request instead runs anonymously. This fallback grants no token
roles: public reads can succeed, while restricted fields fail their sign-in
checks. An identity-provider outage does not automatically invalidate tokens
that the server can still verify with available keys.

## Permissions and resource access

Permissions come from fine-grained role strings in the access token’s top-level
`roles` claim. A role grants a particular capability, such as
`annotation:create` or `admin.users:read`; a stored account role or a general
“admin” label does not substitute for these strings. Resource visibility and
ownership still apply after a role check succeeds.

The following operations illustrate the access boundaries:

| Operation | Required access |
| --- | --- |
| Economic data reads, such as `series`, `seriesList`, and `dataSources` | Available anonymously. See the general API reference for data endpoint selection. |
| `annotationsForSeries` | Anonymous callers see public annotations; signed-in callers also see their own private annotations. The deprecated `userId` argument is ignored. |
| `commentsForAnnotation` | The parent annotation must be public or belong to the caller. A hidden annotation is reported as not found. |
| `createAnnotation` | `annotation:create`. The author is the signed-in caller; `isPublic` defaults to `false`. |
| `updateAnnotation` | `annotation:create`, and only the annotation’s author may update it. |
| `addComment` | `annotation:comment`, and the annotation must be visible to the caller. A chart annotation additionally requires an explicit admin grant on that chart, even for its author. |
| `deleteAnnotation` | Sign-in. The author may delete it; another caller may delete a public chart annotation only with an admin grant on that chart. Private annotations remain accessible only to their author. |
| `shareChart` | `chart:share` and an existing admin grant on the chart. |
| `chartCollaborators` | Sign-in. Chart collaborators see the list; other callers receive an empty list. |
| `user(userId: ...)` | Sign-in to read your own account; `admin.users:read` to read another account. |
| `users` | `admin.users:read`. |
| `crawlerStatus`, `queueStatistics`, `systemHealth` | `admin.system:read`. |
| `triggerCrawl` | `admin.crawlers:manage`. |
| `securityEvents`, `securityEventsPage` | `admin.security:read`. |
| `auditLogs` | `admin.audit:read`. |

A chart admin grant is a permission on a particular chart, separate from the
staff roles whose names start with `admin.`. A staff role does not by itself
reveal another user’s private annotations. Hidden annotations are reported as not
found when reading their comments, commenting, updating, or deleting them.

A restricted field called anonymously returns `Authentication required`. A
signed-in caller who lacks its required token role receives
`Insufficient permissions`. These are GraphQL errors during execution, distinct
from the HTTP authentication refusals above. Additional resource checks can also
refuse an operation even when the required token role is present.

## Request limits and schema discovery

| Boundary | Application limit | Scope |
| --- | --- | --- |
| POST body | 1 MiB (1,048,576 bytes) | The entire received body, including variables and request metadata. Bodies larger than this receive HTTP 413. |
| Query depth | 15 | Maximum selection nesting depth. Deeper operations fail GraphQL validation before execution. |
| Query complexity | 1000 | One unit per selected field. Operations above the limit fail GraphQL validation before execution. This is not a row or result-size limit. |

`/graphql` accepts a single GraphQL request per POST; it also accepts GET queries
in the URL. The POST body limit is not a separate query-text limit and does not
apply to the GET URL. Clients must also respect the limits of their gateway and
HTTP stack.

Schema introspection is available under the same depth and complexity limits.
The `/playground` browser page is a separate feature: the backend serves it only
when `ENABLE_GRAPHQL_PLAYGROUND=true`, and otherwise returns 404. Disabling that
page does not disable introspection at `/graphql`.

Deployments can impose additional HTTP quotas. The committed Kubernetes Gateway
policies apply separate rate-limit buckets to the application and data GraphQL
routes and can return HTTP 429 before GraphQL execution; see
[Gateway routing](../deployment/GATEWAY_ROUTING_TROUBLESHOOTING.md). These are
separate from the application’s field limits.

## Handling errors

Check both the HTTP status and the GraphQL response’s `errors` array. HTTP 200
alone does not establish that the requested fields succeeded. GraphQL execution
errors can include `path` and `locations`, and nullability determines whether
other requested fields remain in `data`.

| Failure | Response contract | Caller action |
| --- | --- | --- |
| Malformed JSON or an invalid GraphQL request body | HTTP 400 with `errors` containing a `message`; the text depends on the parse failure. | Correct the request encoding and body. |
| Body larger than 1,048,576 bytes | HTTP 413 with the body shown below. | Reduce the complete request body. |
| Authentication refusal | HTTP 401, 403, or 500 as specified above, with `data: null`. | Renew an invalid token; resolve account suspension or retry account-loading failures as appropriate. |
| GraphQL syntax, schema, depth, or complexity failure | GraphQL `errors` before field execution. | Correct or reduce the operation. |
| Missing sign-in, missing role, or resource denial | GraphQL `errors` during field execution. | Check caller identity, token roles, and resource access. |
| Invalid argument | A client-facing GraphQL error message; some errors include `extensions.code: BAD_USER_INPUT`. | Correct the named argument. |
| Internal field failure | `Internal server error` with `extensions.code: INTERNAL_SERVER_ERROR`. | Treat the field as failed; server details are not exposed. |

The oversize-body response is:

```json
{
  "errors": [
    { "message": "Request body is larger than 1048576 bytes" }
  ]
}
```

Codes are present only where defined by the operation or failure path. For
example, an invalid security-event offset has `BAD_USER_INPUT`, but not every
argument, authorization, or GraphQL validation error has a code. Do not require
`UNAUTHENTICATED` for an execution-time `Authentication required` error, or
assume that every error carries a bearer challenge. Gateway refusals also need
not use the application’s GraphQL error shape.

## Security events

Both security-event queries require `admin.security:read`.
`securityEventsPage(filter, pagination)` returns `nodes`, `totalCount`, and
`pageInfo`. This operation selects all exposed event fields and the pagination
fields needed to continue:

```graphql
query SecurityEventPage(
  $filter: SecurityEventFilterInput
  $pagination: PaginationInput
) {
  securityEventsPage(filter: $filter, pagination: $pagination) {
    nodes {
      id
      eventType
      userId
      userEmail
      ipAddress
      userAgent
      description
      severity
      createdAt
    }
    totalCount
    pageInfo {
      hasNextPage
      hasPreviousPage
      startCursor
      endCursor
    }
  }
}
```

For example, these variables request the first 50 unresolved events strictly
within the supplied UTC interval:

```json
{
  "filter": {
    "resolved": false,
    "createdAfter": "2026-10-01T00:00:00Z",
    "createdBefore": "2026-10-02T00:00:00Z"
  },
  "pagination": { "first": 50 }
}
```

`id`, `eventType`, `description`, `severity`, and `createdAt` are non-null.
`userId`, `userEmail`, `ipAddress`, and `userAgent` may be null. `eventType` and
`severity` are stored strings, not GraphQL enums. The result does not expose a
`resolved` field, although that value can be filtered.

All supplied filters apply together. `eventType` and `severity` match exact
string values; they do not perform substring or case-insensitive searches.
`resolved` matches the supplied Boolean. `createdAfter` and `createdBefore` are
exclusive: an event exactly on either boundary is excluded. Omitted filters
leave that property unrestricted. Events appear newest first by `createdAt`,
with descending `id` breaking equal-timestamp ties.

For both queries, the default page size is 50; a supplied size is clamped to
1–100. The page query uses `pagination.first` and `pagination.after`; it ignores
`last` and `before`. `after` is a nonnegative signed-64-bit decimal offset,
starting at zero when omitted. An invalid offset returns `BAD_USER_INPUT` with
message `` `after` must be a nonnegative 64-bit offset ``.

While `hasNextPage` is true, pass `endCursor` as the next `pagination.after`,
keeping the filters the same. `endCursor` is the next offset and is null on the
last page. `hasPreviousPage` means the current offset is greater than zero;
`startCursor` is that offset when positive and otherwise null. `totalCount`
counts matching events, capped at the GraphQL `Int` maximum (2,147,483,647).
These are offset pages, not snapshots: concurrent inserts can shift pages,
causing repeated or missed events during traversal.

The compatible list query, `securityEvents(limit, after, filter)`, returns a
`[SecurityEventType!]!` directly. It uses the same filters and ordering, with
`limit` as the page size and `after` as the offset, but supplies no count or page
information.

An event query reports recorded events; it does not guarantee an exhaustive
record of every rejected request or every denied target in an operation. See
[admin audit and security events](ADMIN_AUDIT.md) for recording behavior,
permission-denial deduplication, write-failure limitations, and audit-log access.
