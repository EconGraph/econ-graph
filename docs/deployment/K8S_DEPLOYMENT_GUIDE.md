# Kubernetes Image and Operations Guide

Use this reference to build EconGraph images for MicroK8s, select the cluster explicitly,
and diagnose or restart an existing installation. For a fresh installation, follow the
[MicroK8s deployment procedure](MICROK8S_DEPLOYMENT.md), which covers manifest adaptation,
storage, Secrets, Keycloak, migration gates and resource selection. The
[Kind deployment guide](KUBERNETES_DEPLOYMENT.md) covers the repository's automated path.
Run repository commands below from the checkout root in Bash.

## Select the cluster and deployment path

The deployment scripts are not interchangeable between platforms. `build-images.sh`
loads images into Kind, and `deploy.sh` targets Kind. The older
`scripts/deploy/restart-k8s-rollout.sh` mixes MicroK8s commands with that Kind builder
and expects an admin image the builder excludes. Use the explicit build/import and
restart commands below for MicroK8s.

On an installed MicroK8s host, keep any existing kubeconfig intact. Create a dedicated
file and select it only for this shell; the commands in this guide then use `kubectl`:

```bash
mkdir -p "$HOME/.kube"
(umask 077; microk8s kubectl config view --raw > "$HOME/.kube/econgraph-microk8s.config")
export KUBECONFIG="$HOME/.kube/econgraph-microk8s.config"
kubectl config current-context
kubectl get nodes -o wide
microk8s status --wait-ready
```

Confirm the selected nodes belong to the intended installation before changing resources.
Alternatively, use `microk8s kubectl` throughout to avoid relying on a shell's kubeconfig.

## Installation prerequisites and resource selection

Use the [MicroK8s preparation and deployment steps](MICROK8S_DEPLOYMENT.md#prepare-the-host-and-cluster)
for installation. These repository-specific requirements also matter when diagnosing failures:

- The static database resources require existing directories on the selected node:
  `/srv/econgraph/data-plane/pg`, `/srv/econgraph/app-prod/app-pg` and
  `/srv/econgraph/app-prod/keycloak-pg`. Their prebound PVs use `econgraph-host`, and
  the database StatefulSets select `econgraph.io/host-storage=true`. Enabling a dynamic
  storage addon alone does not satisfy those constraints. Adapt storage and placement
  together as described in [storage preparation](MICROK8S_DEPLOYMENT.md#storage-and-database-settings).
  The app plane holds users and application state; the data plane holds crawled and
  reference data. HostPath capacity declarations do not limit filesystem usage.
- Provision the [required Secrets](../../k8s/README.md#secrets) before their consumers,
  preserving credentials for existing database volumes. Sign-in requires Keycloak,
  its database and a successful realm import. Align the frontend origin, backend issuer,
  Keycloak hostname and client redirect origins through the
  [identity configuration](MICROK8S_DEPLOYMENT.md#secrets-and-identity-configuration).
- Wait for the databases, Keycloak and realm import, then complete the `crawler-migrate`
  Job before starting the backend or worker. The backend applies app plane migrations
  at startup; data plane migrations require the Job. Follow the explicit
  [dependency order](MICROK8S_DEPLOYMENT.md#deploy-in-dependency-order), including its
  failure and subsequent-deployment handling. Reapplying a completed Job does not rerun it.
- Select optional resources individually. The admin frontend is excluded from the current
  build path and uses retired authentication. Monitoring needs its own configuration,
  RBAC and Secrets; certificate resources need cert-manager CRDs before issuers or
  Certificates. Neither the application nor monitoring directory is an apply unit.
  Applying `cloudflared.yaml` locally can stop an existing connector because its
  committed replica count is zero.

Local routing uses Envoy Gateway and HTTPRoutes. The shared
[`install-gateway.sh`](../../scripts/deploy/install-gateway.sh) and
[`apply-gateway-routes.sh`](../../scripts/deploy/apply-gateway-routes.sh) helpers accept
an explicit `--context` and a `KUBECTL` executable; they can target MicroK8s without
using the Kind deployment script. `KUBECTL="microk8s kubectl"` is not an executable
name: use the wrapper and explicit context in
[Gateway preparation](MICROK8S_DEPLOYMENT.md#prepare-and-install-the-gateway).

That procedure covers the installer's Kubernetes 1.33–1.36 guard, pinned downloads
and command prerequisites, trusted pod networks, the `ingress-ready=true` node label,
and available host ports 80/443. In automatic mode, detected pod port conflicts make
the installer omit host ports; a healthy proxy then does not establish localhost
access. The local installation procedure requires host ports explicitly and explains
how to retire a conflicting controller deliberately. It also preserves the namespace's
`econgraph.io/gateway-access: "true"` attachment label. Install the Gateway before
applying routes, and require the GatewayClass, Gateway and HTTPRoute conditions in the
[dependency-order checks](MICROK8S_DEPLOYMENT.md#deploy-in-dependency-order) before
testing access. Pod readiness alone does not establish routing readiness.

## Build and import application images

These tags match the committed application manifests. Keep any adapted manifest tags
in sync with the images, and build from the checkout being deployed. Docker's image
store is separate from MicroK8s containerd.

```bash
docker build -t econ-graph-backend:v4.0.0 backend
docker build --target crawler-worker -t econ-graph-crawler-worker:v4.0.0 backend
docker build \
  --build-arg VITE_API_URL="http://localhost" \
  --build-arg VITE_GRAPHQL_URL="/graphql" \
  --build-arg VITE_DATA_GRAPHQL_URL= \
  --build-arg VITE_WS_URL="ws://localhost/graphql" \
  --build-arg VITE_OIDC_ISSUER="${VITE_OIDC_ISSUER-/idp/realms/econ-graph}" \
  --build-arg VITE_OIDC_CLIENT_ID="${VITE_OIDC_CLIENT_ID:-econ-graph-web}" \
  --build-arg NODE_ENV=production \
  -t econ-graph-frontend:v4.0.0 frontend
docker build -t econ-graph-chart-api:v1.0.0 chart-api-service

set -o pipefail
for image in econ-graph-backend:v4.0.0 econ-graph-crawler-worker:v4.0.0 \
             econ-graph-frontend:v4.0.0 econ-graph-chart-api:v1.0.0; do
  docker save "$image" | microk8s ctr image import - || exit 1
done
microk8s ctr images ls
```

The backend Dockerfile's final/default target is `backend`; the separate
`crawler-worker` target supplies the worker image. The application manifests use
`imagePullPolicy: Never`, so an image absent from a scheduled node fails instead of
being pulled. The import above populates only the local node. For multiple nodes,
use [MicroK8s image side-loading](https://canonical.com/microk8s/docs/sideload) on every
eligible node. Upstream database, Keycloak and init-container images still need registry
access or separate imports.

The frontend resolves `/idp/realms/econ-graph` against its own origin at runtime. This
lets the same issuer path work on `http://localhost` and on a separately configured
[public host](../../k8s/README.md#-public-site-cloudflare-tunnel). `PUBLIC_HOST`
keeps the base domain; `MAIN_HOST` selects the served address and defaults to
`www.${PUBLIC_HOST}`. See [Main public address](MAIN_HOST.md). A full issuer URL
selects an identity provider on another origin. An explicitly empty `VITE_OIDC_ISSUER`
fails the frontend build unless `--build-arg VITE_OIDC_SIGN_IN_DISABLED=true` is also
passed, which builds the image with sign-in hidden; it does not disable backend `OIDC_REQUIRED` enforcement or
provision an alternative identity provider. The other URL arguments above target local
HTTP; review them when changing the browser origin or transport. The empty
`VITE_DATA_GRAPHQL_URL` preserves the standalone frontend's fallback to
`VITE_GRAPHQL_URL`. The Gateway also offers `/data/graphql`, but exposing that route
does not require this image to select it.

For a fresh installation, continue with the linked dependency order. For an image update,
complete any required data plane migrations before restarting consumers. Rebuilding and
importing an existing tag does not replace running pods; use the rollout procedure below.

## Access and health checks

With the local HTTPRoutes installed and Envoy listening on the MicroK8s host's
port 80, open `http://localhost` on that host. `/etc/hosts` entries for the production
domain do not change the Gateway route's `localhost` host match. This local HTTP path
does not issue certificates. A request to the public site does not test a local cluster.

The separate [Cloudflare Tunnel procedure](../../k8s/README.md#-public-site-cloudflare-tunnel)
configures public host routes and visitor TLS at Cloudflare; the connector reaches the
Gateway's HTTPS listener. There is no dedicated admin destination for `/admin` or
`/api/admin`. That procedure's `deploy.sh` targets Kind. For public MicroK8s access,
follow the [public adaptation requirements](MICROK8S_DEPLOYMENT.md#deploy-in-dependency-order):
align application and identity origins, install cert-manager with Gateway API support,
and configure the HTTPS listener and certificate. The route helper uses an existing
issuer or defaults to `econ-graph-selfsigned`; it does not universally obtain a
Let's Encrypt certificate. Applying cert-manager or an issuer alone does not create
a working TLS route.

For diagnosis, run each forward in its own terminal:

```bash
kubectl port-forward service/econ-graph-frontend-service 3000:3000 -n econ-graph
kubectl port-forward service/econ-graph-backend-service 9876:9876 -n econ-graph
# Only if Grafana is installed:
kubectl port-forward service/grafana-service 3002:3000 -n econ-graph
```

The frontend forward serves `http://localhost:3000`, but does not supply the same-origin
GraphQL and Keycloak Gateway routes for a full sign-in session. Grafana stays private
at `http://localhost:3002`. The optional admin frontend, if separately adapted and
installed, uses `service/econ-graph-admin-frontend-service` with `3001:3001`.

In another terminal, check the backend through its forward:

```bash
curl --fail http://localhost:9876/health
curl --fail http://localhost:9876/livez
curl --fail http://localhost:9876/readyz
# Separately check the local Gateway route:
curl --fail http://localhost/health
```

`/health` reports process status and version; `/livez` reports process liveness.
`/readyz` requires the app database to be reachable with current app plane migrations.
Its response also reports data database status, but a data outage does not make the
backend unready. Verify the `data` result and migration Job separately before relying
on data queries. The committed backend HTTPRoute exposes `/health`; use the direct
backend forward for `/livez` and `/readyz`. `/api/health` is not the health route. Keep
certificate verification enabled when checking an operator-configured HTTPS endpoint.

## Diagnose a failed rollout

Start with the failing workload's events and container state:

```bash
kubectl get pods,deployments,statefulsets -n econ-graph
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
kubectl describe pod POD_NAME -n econ-graph
kubectl logs POD_NAME -c CONTAINER_NAME -n econ-graph
```

Replace `POD_NAME` and `CONTAINER_NAME` with the affected pod and container. For a
restarting container, use `kubectl logs ... --previous`. Review logs before sharing
them because application and identity diagnostics can include sensitive information.

### Admission and storage failures

For admission rejections, inspect the namespace labels and the event's named security
constraint. The file `pod-security-policy.yaml` sets Pod Security Admission labels;
it does not create a PodSecurityPolicy. Its `restricted` enforcement conflicts with
PostgreSQL initialization running as UID 0 with added capabilities, and Promtail's
hostPath mounts. Review those workload requirements against
[Kubernetes Pod Security Standards](https://kubernetes.io/docs/concepts/security/pod-security-standards/)
before choosing enforcement; do not apply the file as a universal hardening step.

```bash
kubectl get namespace econ-graph -o jsonpath='{.metadata.labels}'
kubectl get pvc -n econ-graph
kubectl get pv
kubectl get nodes --show-labels
kubectl describe pod postgresql-0 -n econ-graph
kubectl describe pod app-postgres-0 -n econ-graph
```

For Pending database pods, compare claim binding, node placement, PV affinity and the
selected node's actual directory mounts. For database crashes, inspect container exit
state, logs and host filesystem free space before changing the image or initialization
arguments. See [Host Storage](HOST_STORAGE.md) for ownership, backup and recovery requirements.
A PVC's declared capacity does not report free space on its host filesystem. For
cluster startup and host diagnosis, see [MicroK8s troubleshooting](MICROK8S_TROUBLESHOOTING.md).

### Database connectivity and network policies

A backend stuck in `wait-for-postgres` has not completed its database connection check.
Inspect both database Services, their selectors and selected endpoints before attributing
the failure to policy:

```bash
kubectl logs POD_NAME -c wait-for-postgres -n econ-graph
kubectl get service postgres-service app-postgres-service -n econ-graph -o wide
kubectl get endpoints postgres-service app-postgres-service -n econ-graph
kubectl get pods -n econ-graph --show-labels
kubectl get networkpolicies -n econ-graph
kubectl describe networkpolicy econ-graph-network-policy -n econ-graph
kubectl diff -f k8s/manifests/network-policy.yaml
```

Compare Service selectors and target ports with ready database pods and port 5432.
Then inspect every policy selecting the source or destination: allowed traffic is
additive across policies. Adapt the specific ingress or egress rule for the actual
pod labels and proxy namespace. Preserve independently managed policies; do not
delete every policy in the namespace to test connectivity. Establish that the installed
networking implementation enforces policies before relying on them.

The checked-in namespace-wide policy permits same-namespace traffic and outbound
DNS, HTTP and HTTPS. Its broader allowances remain effective even where a second
policy lists narrower connections; it does not restrict all traffic to named Services.

### Gateway, identity and image failures

```bash
kubectl get gatewayclass econ-graph -o yaml
kubectl get gateway econ-graph-gateway -n econ-graph-gateway -o yaml
kubectl get httproute -n econ-graph -o yaml
kubectl get backendtrafficpolicy -n econ-graph -o yaml
kubectl get envoypatchpolicy econ-graph-ratelimit-burst -n econ-graph-gateway -o yaml
kubectl get namespace econ-graph --show-labels
kubectl get deployments,pods,services -n envoy-gateway-system -o wide
kubectl get service econ-graph-gateway-proxy -n envoy-gateway-system -o yaml
microk8s ctr images ls
```

Require GatewayClass `Accepted=True`, Gateway `Accepted=True` and `Programmed=True`,
and HTTPRoute `Accepted=True` and `ResolvedRefs=True` for the intended parent, all
observing the current resource generation. Compare listener names, namespace attachment
labels, request host/path and backend Service references. Inspect the actual Envoy
workload's events and logs, node selection and host-port allocations. The proxy Service
is `econ-graph-gateway-proxy` in `envoy-gateway-system`: its Service ports 80/443 map
to pod ports 10080/10443. Policies selecting the proxy must allow the pod ports.
Compare destination Service endpoints with pod labels and readiness; a successful
backend port forward separates backend health from Gateway behavior. Avoid copying
Kind control-plane placement or Docker bridge assumptions into MicroK8s diagnosis.

For sign-in failures, inspect Keycloak and the realm-import Job status, then compare
the browser origin, issuer and redirect settings with the identity configuration.
A successful frontend rollout alone does not establish authentication readiness.
For `ErrImageNeverPull`, compare the pod's exact image tag and scheduled node with that
node's containerd image list; repeat the build/import steps for the affected image.

Certificate diagnosis belongs to the TLS boundary actually in use. Cloudflare visitor
certificate failures are separate from Gateway origin TLS. For configured Gateway HTTPS,
inspect its listener, Certificate and issuer, then cert-manager events and logs:

```bash
kubectl describe gateway econ-graph-gateway -n econ-graph-gateway
kubectl describe certificate econ-graph-tls -n econ-graph-gateway
kubectl get httproute -n econ-graph -l econgraph.io/public-host=true
```

Use the Certificate's actual issuer reference when investigating issuance. Only an
ACME HTTP-01 solver uses a challenge HTTP route; DNS-01 uses DNS records, and a
self-signed issuer uses neither. Derive any solver resources from the installation's
Certificate and challenge rather than choosing an arbitrary token or historical pod IP.
The [Gateway route test script](../../scripts/deploy/test-gateway-routes.sh) can check
routing behavior, but skips TLS certificate verification; it is not a certificate-validity
check. Neither public TLS nor ACME is part of the local HTTP recipe.

## Check security at the component that enforces it

Backend CORS reads `CORS_ALLOWED_ORIGINS` from the configured browser origins. The
[backend CORS filter](../../backend/crates/econ-graph-backend/src/main.rs) allows
`content-type` and `authorization` headers and GET, POST, PUT, PATCH, DELETE and OPTIONS;
it does not configure credentialed CORS or a max-age here. Match the configured origin
to the browser rather than assuming public domains are already allowed.

The [HTTPRoutes](../../k8s/manifests/gateway/routes/httproutes.yaml) set security
headers at the Gateway. Frontend responses receive CSP, `nosniff`, XSS, referrer,
permissions and HSTS headers, and deny framing. `/silent-callback.html` instead sets
`SAMEORIGIN` and `frame-ancestors 'self'` so silent sign-in can use a same-origin iframe.
Backend and GraphQL routes set `nosniff`, `DENY` and HSTS. Browsers ignore HSTS over
local HTTP. A separate Keycloak origin requires corresponding frontend CSP changes.
The [frontend NGINX configuration](../../frontend/nginx.conf) also declares headers;
check responses on the edge and path actually used, including the silent callback.
Direct Service access bypasses the Gateway's header modifications.

The two [GraphQL BackendTrafficPolicies](../../k8s/manifests/gateway/routes/ratelimit-policies.yaml)
configure separate per-client buckets for `/graphql` and `/data/graphql`, each refilling
at 20 requests per second. The [burst patch](../../k8s/manifests/gateway/ratelimit-burst.yaml)
raises capacity from 20 to 100 when successfully programmed; inspect its status after
the route policies exist. Each proxy counts locally. The committed single replica
makes that one counter per route and client; adding replicas changes the aggregate
limit. These are not limits on every backend route, admin access or direct backend
Service/NodePort requests. The legacy admin GraphQL proxy also bypasses them.

The [client-traffic policy](../../k8s/manifests/gateway/client-traffic-policy.yaml)
uses the TCP peer's address unless the peer belongs to a trusted pod CIDR, when it
accepts forwarded client addresses. Routes replace `X-Forwarded-For` with the selected
address. The client-traffic policy discards other client-supplied forwarding headers
before Envoy supplies its own protocol header. This prevents an outside client from
choosing an arbitrary forwarded address, but trusted pods can claim one; pod networks
must be trusted and these addresses are not audit-grade identities. NAT and proxying can make clients
share a bucket. Verify actual address handling on the MicroK8s network and access path
before relying on per-client limits.

`/data/graphql` rewrites to backend `/graphql` and strips `Authorization` and `Cookie`,
so requests through that route are anonymous. `/graphql` preserves credentials for
backend authentication. Direct Service requests bypass this Gateway stripping as well
as its rate limits; these controls do not replace backend authorization.

## Restart, capacity and removal

After importing updated images and completing required migrations, restart each changed
workload and wait for its rollout before proceeding. Use all four for a full image rebuild:

```bash
for name in econ-graph-backend econ-graph-frontend crawler-worker chart-api-service; do
  kubectl rollout restart "deployment/$name" -n econ-graph || exit 1
  kubectl rollout status "deployment/$name" -n econ-graph --timeout=300s || exit 1
done
```

A restart does not import images or apply data plane migrations. Treat database and
Keycloak version changes as upgrades with verified backups. The legacy admin deployment
is outside this build path; restart it only if separately installed and adapted.
Stopping MicroK8s interrupts every workload in the installation:

```bash
microk8s stop
microk8s start
microk8s status --wait-ready
```

Compare workload `resources.requests` and `resources.limits` in the selected manifests
with node capacity and actual usage. Use events to distinguish scheduling failures,
OOM kills and failed probes before resizing:

```bash
kubectl describe nodes
kubectl top nodes
kubectl top pods -n econ-graph
kubectl get pvc -n econ-graph
# Example application scaling; choose counts from measured capacity:
kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph
kubectl scale deployment econ-graph-frontend --replicas=2 -n econ-graph
```

`top` requires metrics-server. Application scaling does not resize the database or
host filesystem. Select monitoring components and dashboards through the
[monitoring guide](../monitoring/README.md), and verify database backup restoration
through [Host Storage](HOST_STORAGE.md) before depending on persistent data.

For removal, use the [MicroK8s removal procedure](MICROK8S_DEPLOYMENT.md#restart-and-remove),
which explicitly selects application workloads and routes while retaining database
StatefulSets, PVCs, PVs, Secrets and the namespace. Check for running migration and
realm-import Jobs first. The procedure removes local HTTPRoutes and their application
policies while leaving the shared Gateway, controller, CRDs and Gateway-level policies
for separate lifecycle management. Public routes and policies require deliberate
additional removal scope. Do not delete a manifest directory:
it includes the namespace and protected cluster-scoped storage. Namespace deletion
also removes credentials and PVCs; dynamically provisioned storage follows its own
reclaim policy. Retained host directories alone do not restore credentials or a usable
installation. Full MicroK8s removal requires verified off-cluster backups and a recovery
plan for operator-managed Secrets. The repository's guarded `teardown.sh` targets Kind;
it is not a MicroK8s removal command.
