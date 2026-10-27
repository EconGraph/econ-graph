# GraphQL Security Deployment Guide

The GraphQL API runs in `econ-graph-backend`; `econ-graph-graphql` is the
library it links. This guide covers configuring that server, deploying it behind
the maintained Kubernetes Gateway, and checking identity, exposure, monitoring
and recovery. Commands below are operator procedures, not evidence that a
particular deployment has passed them.

## Choose the deployment boundary

Use a local source build for development and the [Kubernetes deployment
procedure](../../k8s/README.md#-quick-start) for the maintained cluster topology.
The backend serves HTTP, normally on port 9876. Envoy Gateway supplies external
routing, response headers and request rate limits; TLS terminates outside the
backend. A direct HTTP connection to the backend retains its schema and identity
checks but bypasses Gateway policies.

Development may deliberately run without sign-in, with every caller anonymous,
and may enable the GraphQL playground. Staging should exercise the same identity
and Gateway boundaries as production using separate databases, credentials and
host configuration. Production uses the [public-site
procedure](../../k8s/README.md#-public-site-cloudflare-tunnel), including its
production marker and tunnel credential safeguards. There is no environment
profile that turns the protections below into stricter production values.

## Configure a local backend

Clone [EconGraph/econ-graph](https://github.com/EconGraph/econ-graph) and work from
its `backend/` workspace. Use the repository's pinned
[toolchain](../../rust-toolchain.toml), currently Rust 1.99.1; the workspace uses
edition 2024. Native builds need PostgreSQL client libraries, OpenSSL development
libraries and `pkg-config`, as shown in the maintained
[Dockerfile](../../backend/Dockerfile). The cluster uses PostgreSQL 18. Capacity
must be sized for the workload; the container resource settings below are not
minimum hardware requirements.

### Databases and migrations

The app plane stores users and application state, including audit records. The
data plane stores economic data, reference data and crawl state. Set
`APP_DATABASE_URL` to the app database and, when serving data locally,
`DATA_DATABASE_URL` to the data database. Both URLs may name the same physical
database, which then contains both planes' tables. The backend requires the app
URL and does not fall back to `DATABASE_URL`.

Provision the chosen databases and supply their URLs through your local secret
handling before starting a process. Do not commit credentials or paste them into
shared logs. With no data URL, the backend can serve the app plane; data fields
report `DATA_SERVED_REMOTELY`. See [database configuration](../../k8s/README.md#databases)
for the shared and separate layouts and [migrations](../development/MIGRATIONS.md)
for migration ownership and CLI details.

The backend applies `backend/migrations/app` on startup. Data plane migrations
in `backend/migrations/data` run through `crawler migrate`; service startup does
not apply them. For a local database owned by this development environment, run
these from `backend/` after setting the URLs:

```bash
cargo run --locked --bin crawler -- migrate
cargo run --locked --bin econ-graph-backend
```

The first command needs `DATA_DATABASE_URL`; omit it for an app-only backend.
Do not substitute a bare `diesel migration run` for both operations: the
repository's Diesel default selects app migrations only. Diesel's CLI URL
handling is distinct from the server's environment settings; use the migration
guide when applying migrations manually.

Because the server runs app migrations, its app database role needs the object
ownership and DDL privileges required by those migrations, as well as runtime
access. Granting only table DML and sequence usage is insufficient. The cluster's
init scripts create an `econgraph` role in each app/data database. Use roles
scoped to the chosen databases and do not grant PostgreSQL superuser access to
the API. Changing to separate migration and runtime roles requires a compatible
startup and ownership procedure; revoking creation rights alone does not supply
one.

### Runtime settings and identity

The executable binds HTTP/1.1 on `0.0.0.0` at `BACKEND_PORT` (default 9876).
Although the configuration object reads `SERVER_HOST`, the listener does not use
it. Restrict development access with the host/network boundary or container port
publishing; do not rely on that setting for loopback isolation. Keep container
mappings, Services and probe ports consistent when changing the port.

`CORS_ALLOWED_ORIGINS` is a comma-separated list of browser origins such as
`http://localhost:3000`. Startup rejects an empty list, wildcards, and values
that are not HTTP(S) origins. CORS controls browser access; it does not replace
resolver authorization or restrict non-browser clients.

Configure sign-in using these settings:

| Setting | Meaning |
| --- | --- |
| `OIDC_ISSUER` | Issuer URL matching the token's `iss`, including the realm path. Unset or empty disables sign-in and makes every caller anonymous. |
| `OIDC_AUDIENCE` | Required token audience; defaults to `econ-graph-api`. An explicitly empty value is rejected. |
| `OIDC_JWKS_URL` | Optional signing-key URL override, useful when the public issuer cannot be reached from the API. Without it, keys are found through issuer discovery. |
| `OIDC_REQUIRED` | Set to `true` in deployments requiring sign-in configuration. It stops startup without an issuer; it does not require authentication for every public query. |

Malformed identity settings stop startup. The verifier checks RS256 signatures,
issuer, audience, expiry and not-before claims. It fetches signing keys lazily,
so startup and anonymous requests can succeed while the provider is unavailable.
Cached keys refresh after 15 minutes and may remain usable for up to 24 hours
while the provider cannot be reached. Failure to fetch a needed key prevents
verification; it does not grant an authenticated principal. Diagnose issuer and
audience mismatches separately from key-fetch failures.

For the cluster, the committed issuer is
`http://localhost/idp/realms/econ-graph`; the audience is `econ-graph-api`,
`OIDC_REQUIRED` is `true`, and JWKS is fetched from the in-cluster Keycloak
service. `PUBLIC_HOST` changes the issuer and Keycloak hostname together to
`https://<host>/idp/realms/econ-graph`. Follow the [Keycloak
procedure](../../k8s/README.md#-keycloak) for realm and identity-provider setup.
Anonymous callers may use public data operations; app and admin operations keep
their own permission checks. Disabling sign-in does not grant admin access.

### Controls applied by the server

The [schema](../../backend/crates/econ-graph-graphql/src/graphql/schema.rs)
compiles in maximum depth 15 and complexity 1000. These values count query
structure rather than returned rows and are not environment settings. The HTTP
handler bounds POST `/graphql` bodies at 1 MiB. Validation and authorization also
occur at individual inputs and resolvers; there is no global environment switch
that enables HTML sanitization, SQL-injection prevention or XSS prevention.

The playground is off unless `ENABLE_GRAPHQL_PLAYGROUND` is `true` or `1`.
Enabling it is useful locally; disabling it removes the interactive UI but does
not disable schema introspection. Gateway rate limits apply only to traffic
through the routes described below.

For container builds, use `backend/Dockerfile` with `backend/` as its context,
rather than copying a second Dockerfile into the repository root. It builds the
actual server and crawler binaries with `--locked`, embeds both migration sets,
and copies runtime reference files. It runs as UID 1000 and checks `/health` at
`BACKEND_PORT`. Its default build flag profile is `release`; local Cargo builds
use the `dev` profile from `backend/.cargo/config.toml`. See [build-time
flags](../../config/flags/README.md) before changing that profile. Supply URLs and
OIDC settings explicitly for a standalone container; building an image does not
install a Gateway, TLS or request limits.

## Deploy the maintained cluster

Start with the [cluster prerequisites and image build
procedure](../../k8s/README.md#-quick-start), including [host
storage](../deployment/HOST_STORAGE.md). Run the repository scripts from the
repository root against the intended context. Do not apply a replacement
Deployment or a top-level `kubectl apply -f k8s/` as a substitute for orchestration.

1. Select the database layout and provision credentials using
   `scripts/deploy/create-secrets.sh`. The deploy script also runs it. Follow
   [Secrets](../../k8s/README.md#secrets) and [secret
   handling](../technical/SECRETS_MANAGEMENT.md); do not commit Secret manifests.
   Existing shared layouts keep their app URL until explicitly changed. Creating
   a separate app database does not move existing app rows into it.
2. Review `scripts/deploy/deploy.sh --dry-run`, then run
   `scripts/deploy/deploy.sh` when ready to change the target cluster. It installs
   the Gateway and applies the maintained resources. After PostgreSQL is ready,
   it runs `crawler-migrate` before deploying the backend and worker. A failed
   migration Job stops deployment. An app-only namespace without
   `data-database-url` skips that Job.
3. Inspect rollout readiness and logs. A running pod alone does not establish
   database readiness, working sign-in or route policy enforcement. Use the
   acceptance checks below before treating the deployment as ready for traffic.

### Workload, credentials and probes

The maintained [Deployment](../../k8s/manifests/backend-deployment.yaml) is
`econ-graph-backend` in namespace `econ-graph`, initially with two replicas.
Its `backend` container listens on 9876. The
[Service](../../k8s/manifests/backend-service.yaml),
`econ-graph-backend-service`, selects those pods on port 9876; it also exposes
NodePort 30080. Review reachability of that direct path as part of the host's
network policy, since it bypasses the Gateway.

The Deployment maps `econ-graph-secrets` key `app-database-url` to
`APP_DATABASE_URL`, and optional key `data-database-url` to
`DATA_DATABASE_URL`. `BACKEND_PORT` comes from `econ-graph-config`;
`CORS_ALLOWED_ORIGINS` comes from its `CORS_ORIGIN` key. Identity settings are
specified on the Deployment and public-host rendering updates the issuer.

The backend container requests 100m CPU and 256Mi memory, with limits of 500m
and 512Mi. It runs as non-root UID 1000, disallows privilege escalation, drops
all Linux capabilities, uses a read-only root filesystem and `RuntimeDefault`
seccomp. The pod uses filesystem group 1000. The PostgreSQL wait init container
also has an explicit security context and resource limits. Preserve these
settings when changing images or adding writable mounts.

Startup and readiness probes call `/readyz`: the app database must be reachable
and its app migrations at least as new as this binary requires. Data database
state appears in the readiness body but does not fail readiness, so a data
outage does not remove sign-in from service. Liveness calls `/livez`, which
checks the process without depending on PostgreSQL. `/health` is the general
health endpoint used by the image, not the Deployment's readiness probe.

### Gateway, TLS and exposure

The [Gateway reference](../../k8s/README.md#-gateway) explains installation and
route application. The namespace label `econgraph.io/gateway-access: "true"`
admits its routes to the Gateway. The localhost HTTP routes send `/graphql` to
the backend and `/data/graphql` to backend `/graphql` after removing
`Authorization` and `Cookie`. The backend route handles `/api`, `/auth` and
`/health`; frontend and Keycloak have separate routes. API routes set
`X-Content-Type-Options`, `X-Frame-Options` and HSTS. Frontend routes also set CSP.

Each GraphQL route limits each client address to 20 requests per second, with
bursts of 100 and HTTP 429 above the limit. The
[route policies](../../k8s/manifests/gateway/routes/ratelimit-policies.yaml)
provide the rate; the [burst patch](../../k8s/manifests/gateway/ratelimit-burst.yaml)
raises the bucket size to 100. Check both policy application and patch status:
a rate policy alone has a burst size of 20. The two routes use separate buckets.
Limits are local to each Envoy proxy replica; the maintained proxy runs one
replica. Clients behind one NAT, and requests from the host via the Docker
bridge, can share a bucket. Scaling backend replicas does not change these
limits; scaling proxy replicas changes their aggregate behavior.

The committed topology serves `http://localhost`. For the public site, visitors
use HTTPS to Cloudflare, a tunnel reaches the in-cluster connector, and the
connector uses HTTPS to the Gateway. The Gateway forwards HTTP to the app and
Keycloak. The default Gateway certificate is self-signed and the tunnel's
origin connection uses **No TLS Verify**; this is not a publicly trusted origin
certificate. Use the [public-site TLS and tunnel
procedure](../../k8s/README.md#-public-site-cloudflare-tunnel) to configure the
host, certificate, connector and redirects together. The backend does not
consume `SSL_CERT_PATH` or `SSL_KEY_PATH`.

The Gateway drops client-supplied scheme/host forwarding headers, derives the
scheme from the connection, and replaces `X-Forwarded-For` with its selected
client address. It trusts forwarded addresses from pod peers for the tunnel
path, so in-cluster callers can claim addresses. Backend audit attribution has
a separate boundary: `TRUSTED_PROXIES` is empty by default, recording the TCP
peer rather than trusting a forwarded address. Follow [trusted proxy
configuration](ADMIN_AUDIT.md#trusted-proxies-in-kubernetes) before changing it;
do not trust a universal pod CIDR.

Keep `/metrics`, `/livez` and `/readyz` internal; the public routes do not expose
them. Review the actual [NetworkPolicies](../../k8s/manifests/network-policy.yaml)
and their enforcement by the cluster's networking implementation. They admit
Gateway traffic to the application ports and namespace traffic to the proxy's
pod ports 10080/10443. Host firewall rules must account for Docker/kind
networking, NodePorts and operator access. A blanket host INPUT drop is not a
replacement for these policies or a safe deployment recipe.

### Operator acceptance and troubleshooting

Use the correct cluster context for these observations:

```bash
kubectl -n econ-graph rollout status deployment/econ-graph-backend
kubectl -n econ-graph get pods
kubectl -n econ-graph logs deployment/econ-graph-backend -c backend
kubectl -n econ-graph get job crawler-migrate
```

For a deployment with a data database, inspect `crawler-migrate` logs if the Job
fails. For unready backends, inspect pod events and the app database/migration
state before changing probe thresholds. Liveness succeeding while readiness
fails points to a different problem than a process that cannot start.

Run `scripts/deploy/test-gateway-routes.sh` through the intended entry point;
its `--rate-limit` option exercises rate rejection and generates traffic. Check
Gateway/HTTPRoute acceptance and policy status, then verify sign-in with the
configured issuer and a permission-protected operation. Also check anonymous
public access and rejection of unauthorized app/admin access. Test forged
forwarding headers through the actual chain when configuring trusted proxies.
These checks serve different purposes: a successful health response proves
neither authentication nor Gateway enforcement.

## Observe and maintain the deployment

Use container stdout/stderr for backend logs; the server initializes formatted
tracing at INFO. Although the Deployment supplies `RUST_LOG`, this initializer
does not install an environment filter, so setting it to `debug` or `trace` does
not enable those levels. There is no configured `/var/log/econ-graph/security.log`
writer in this deployment.

Prometheus scrapes `/metrics` internally on port 9876. Use the maintained
[monitoring guidance](../monitoring/README.md) and inspect metric recording sites,
not only registrations, before building alerts. The active `/graphql` route
executes its authenticated schema directly and does not call the legacy
`graphql_handler` metrics helper. Consequently, the registered
`graphql_queries_total` and `graphql_query_duration_seconds` do not observe
traffic through that route. The helper supplies constant complexity 1.0 to
`graphql_query_complexity`, so it would not measure actual query cost either.
Do not use these series to infer query volume, latency or security rejection
rates for the deployed route.

`http_requests_total` is recorded by `/`, `/health`, `/livez` and `/readyz`
handlers, rather than all HTTP traffic; their duration observations are 0.0.
For those recorded requests, `sum(rate(http_requests_total[5m]))` is requests per
second averaged over five minutes; `sum(increase(http_requests_total[5m]))`
estimates a count over that window. Neither expression measures GraphQL load.
Gateway 429s never reach the backend; use Gateway observations for them.

Admin attempts and permission denials write app-plane `audit_logs` and
`security_events`; consult [admin audit guidance](ADMIN_AUDIT.md) for authorized
pagination and interpretation. These writes are best-effort and separate from
the operation being audited. Failed writes log errors and increment
`audit_write_failures_total{table}`, synchronized when metrics are scraped.
Investigate any increase: an empty event page does not prove no attempts occurred.

For upgrades, follow the [release procedure](../development/RELEASE_PROCESS.md)
and take the appropriate pre-change backup. Update pinned toolchains,
dependencies and images through reviewed changes rather than running unbounded
updates on a serving host. Preserve credential rotation and log retention in
the operator's maintenance process; Kubernetes logs do not create a separate
security archive.

Backend scaling uses `kubectl -n econ-graph scale deployment/econ-graph-backend
--replicas=3`. The Service selects the new ready pods, so scaling alone needs no
route reapplication. Account for database connections and startup app migrations
when sizing replicas. Change resource requests/limits in the maintained manifest
for persistent tuning; a live patch alone will be overwritten by deployment.

## Back up and recover the whole service

Recovery needs economic data, app state and identity state. With separate app
storage, that means `econ_graph`, `econ_graph_app` and Keycloak's `keycloak`
database. With `APP_DATABASE=shared`, app tables are in `econ_graph`. A backup of
only the data database in a separate layout cannot restore users, audit history
or sign-in.

The [host backup runbook](../deployment/BACKUPS.md) covers snapshots of the data
and app subvolumes, logical database dumps, dump verification, restore drills
and freshness checks. The independent in-cluster nightly `postgres-backup`
CronJob writes database dumps and a `MANIFEST` to its PVC. It selects app coverage
from the backend's app URL: shared app tables are covered by the data dump;
separate app state needs its own dump. Consult [backup scope and
failures](../deployment/POSTGRES_BACKUP_RESTORE.md) for required versus skipped
databases and partial-run handling. A successful data dump alone does not prove
the whole run succeeded. The CronJob's kind-local PVC can be lost with the
cluster; keep recoverable copies outside that failure boundary.

Treat backups, exported Secrets, database passwords, OIDC/provider credentials,
tunnel tokens and any off-site encryption keys as sensitive recovery material.
The operator must retain the necessary configuration and credentials outside
version control using the [Secrets procedure](../technical/SECRETS_MANAGEMENT.md)
and [cluster Secret reference](../../k8s/README.md#secrets). Database dumps are not
a substitute for that material. Do not collect it into an unprotected `.env`
archive or publish it as review evidence.

Before recovery, choose a backup covering the intended databases, check its
manifest and verification results, identify which writes will be lost, and
stop the relevant writers. Use the human-run [dump restore
procedures](../deployment/POSTGRES_BACKUP_RESTORE.md#restore) or [host
recovery runbook](../deployment/BACKUPS.md), including their destination,
ownership, confirmation and rollback steps. Snapshot recovery also depends on
the kind node seeing the restored mount. After recovery, check migration history,
app readiness, economic data and Keycloak sign-in before resuming writers.
Archive verification and a disposable restore drill provide different evidence;
neither alone proves production authentication works.

Backups and snapshots are not deleted automatically. Monitor space and freshness,
and let the operator explicitly remove unneeded recovery points according to
the runbooks. Do not infer encryption, off-site availability or restore readiness
from the presence of a backup Job.
