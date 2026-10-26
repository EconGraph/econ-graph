# Admin audit and security events

Admin mutation attempts record their outcome in `audit_logs`. Permission denials
record a `security_events` row as soon as authorization refuses the request.
Normal requests wait for these writes. A client disconnect detaches pending
writes instead of cancelling them. An admin attempt cancelled before its result
is known records `details.outcome = "cancelled"`: the database statement may have
committed. Do not treat this outcome as proof of rollback. Changes and audit
writes are separate; process crashes, runtime shutdown and database failures can
still lose rows. Failed inserts are logged and counted in
`audit_write_failures_total{table}`.

Permission events are deduplicated per caller and permission within a request.
For user-management denials the permission key is shared across targets, so the
first target supplies the description. There is deliberately no additional
in-process denial rate limit: suppressing these rows would hide repeated denied
requests, and a per-process limit would not be consistent across replicas. Monitor
volume and database capacity; HTTP request limits remain the appropriate outer
control. This is an accepted observability trade-off, not a complete record of
every aliased target.

`auditLogs(filter, pagination)` and `securityEventsPage(filter, pagination)` expose
`nodes`, `totalCount` and `pageInfo`. Both accept a nonnegative signed-64-bit offset
as `pagination.after` and clamp `first` to 1–100 (default 50). Follow
`pageInfo.endCursor` while `hasNextPage` is true. Security filters support
`eventType`, `severity`, `resolved`, `createdAfter` and `createdBefore`; date bounds
are exclusive. Offset pages can shift when new events arrive. Security events
use `(created_at DESC, id DESC)` to order equal timestamps consistently.
The original `securityEvents(limit)` response remains a list for existing clients;
it also accepts `after` and `filter`.

## Trusted proxies in Kubernetes

`TRUSTED_PROXIES` is empty by default, including in the committed manifests. This
fails closed: audit records show the backend's TCP peer, the gateway's Envoy
proxy pod. Deployment operators must supply verified proxy addresses/ranges to
record the forwarded client IP. A universal pod CIDR is unsafe: it would trust
headers forged by every pod in that range. Proxy pod addresses differ per
cluster and change when the pod is recreated, so there is no static range here
that is correct for all deployments.

The gateway replaces `X-Forwarded-For` with the one address it settled on: the
connecting peer, or, for a request from a pod (the Cloudflare Tunnel connector
included), the last address that pod sent in `X-Forwarded-For`. Any pod can
claim an address that way (`k8s/README.md`, "Public site").

1. Identify the actual Gateway proxy pod IPs reaching the backend, and the
   forwarding chain (for production, Cloudflare Tunnel and the Gateway).
2. Configure the edge to sanitize or append client addresses from its verified
   immediate peer. Trust only the specific proxy IPs or a dedicated proxy subnet
   whose membership is restricted by network policy. A Cloudflare address range
   alone is insufficient when the backend's TCP peer is an in-cluster proxy.
3. Set the backend Deployment's `TRUSTED_PROXIES` environment variable to those
   comma-separated IPs/CIDRs. Reconcile this setting when proxy pod IPs change.
4. Send a request through the real route with a deliberately forged leading
   `X-Forwarded-For`, then verify that an authorized audit reader sees the real
   client, not the forged address. Also check a direct untrusted peer: its header
   must be ignored.

The backend walks `X-Forwarded-For` right to left and stops at the first untrusted
hop or malformed address. IPv4-mapped IPv6 ranges with prefixes /96–/128 are
canonicalized to IPv4 (/0–/32). Shorter mapped prefixes also include native IPv6
addresses, so they are rejected with a startup warning rather than silently
changing their meaning. Keep the default empty value until this chain is verified.
