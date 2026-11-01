# Restarting EconGraph on MicroK8s

Use this runbook for an existing MicroK8s deployment. A restart recreates pods using
images already available to MicroK8s. To update application code without changing
schemas or deployment configuration, build and import the images first, then restart
the affected deployments. Provisioning, configuration changes, and database migrations
require a separate deployment procedure.

For MicroK8s installation and addon setup, see the
[MicroK8s guide](MICROK8S_DEPLOYMENT.md). For the separate kind workflow,
see the [kind restart guide](DEPLOYMENT_RESTART.md).

## Select the cluster and workloads

Run the commands from the repository root on the MicroK8s host, in the same Bash
session. Use MicroK8s' own kubectl explicitly so an unrelated current context or
`KUBECONFIG` cannot send the restart to kind or another cluster.

```bash
cd /path/to/econ-graph
set -euo pipefail
source config/ports.env
mk() { microk8s kubectl --context=microk8s "$@"; }

microk8s status --wait-ready
mk get nodes -o wide
mk -n econ-graph get deployments -o wide
```

Confirm the node addresses and deployment names belong to the intended installation
before continuing. Select only workloads that are already deployed and should be
restarted. The example covers the backend and frontend; add `crawler-worker` or
`chart-api-service` to the array when updating those components. Leave a crawler
that was deliberately scaled to zero stopped.

```bash
workloads=(econ-graph-backend econ-graph-frontend)
for workload in "${workloads[@]}"; do
  mk -n econ-graph get deployment "$workload" \
    -o jsonpath='{.metadata.name}{"\t"}{.spec.replicas}{"\t"}{.spec.template.spec.containers[*].image}{"\n"}'
done
```

The admin frontend uses retired authentication and cannot perform authenticated
GraphQL operations. It is excluded here, as it is from the current kind builder
and deployment script; see the [builder’s exclusion](../../scripts/deploy/build-images.sh).
The Gateway does not provide an admin route, and the frontend NodePort does not
route `/admin` to the legacy admin Service.

## Restart existing images

Skip the build section when reusing the images already installed on the cluster.
After importing replacement images, use the same commands to recreate pods that
still run the previous image contents.

```bash
for workload in "${workloads[@]}"; do
  mk -n econ-graph rollout restart "deployment/$workload"
  mk -n econ-graph rollout status "deployment/$workload" --timeout=300s
done
mk -n econ-graph get deployments -o wide
mk -n econ-graph get pods -o wide
```

Confirm each selected deployment has its desired replicas available. These commands
leave database, Keycloak, monitoring, Gateway, and tunnel resources as configured. The backend applies app plane migrations at startup. No service
applies data plane migrations at startup; restarting the worker does not replace
the `crawler-migrate` Job. If updating code requires either plane's schema to
change, stop here and follow the [migration requirements](../development/MIGRATIONS.md)
and [database configuration](../../k8s/README.md#databases) before restarting it.

## Build and import replacement images

This path updates application images for an existing installation whose schema,
Secrets, ConfigMaps, routing, and authentication settings already match the code.
Use a Docker daemon on the MicroK8s host that builds for that host's architecture.
The commands explicitly use Docker's `default` context; if your build daemon uses
another context, substitute its name consistently. Review the context endpoint
before building.

The checked-in backend, worker, and frontend manifests use the version from
`backend/Cargo.toml` (currently `v4.0.0`); the chart service uses `v1.0.0`. All four
set `imagePullPolicy: Never`, so a local Docker build alone is insufficient.
MicroK8s must have the exact tags requested by the deployments.

```bash
docker context inspect default
VERSION="v$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/p' backend/Cargo.toml | head -n 1)"
test "$VERSION" != v

# Confirm the checked-in image tags before building.
for spec in \
  "backend-deployment.yaml:econ-graph-backend:$VERSION" \
  "crawler-worker.yaml:econ-graph-crawler-worker:$VERSION" \
  "frontend-deployment.yaml:econ-graph-frontend:$VERSION" \
  "chart-api-deployment.yaml:econ-graph-chart-api:v1.0.0"; do
  grep -F "image: ${spec#*:}" "k8s/manifests/${spec%%:*}"
done
```

Compare these tags with the deployed images printed above. If they differ, this
is a deployment change, not an image replacement under the existing configuration;
do not continue with this procedure. Select the checkout and image contract that
belong to the installed release instead.

The two backend targets share a builder stage. The `backend` target contains the
API; `crawler-worker` contains the worker and the crawler executable used by data
plane migration Jobs. Frontend settings are baked into the Vite build. Preserve
the installation's Vite settings when rebuilding, including `VITE_DATA_GRAPHQL_URL`,
issuer, and client ID. Set any site-specific values in this shell before building;
the examples below supply defaults for the same-origin setup.
The default path issuer below resolves against the browser's origin and requires
Keycloak to be served under `/idp` on that origin. To intentionally hide sign-in,
set `VITE_OIDC_ISSUER` to an empty string and `VITE_OIDC_SIGN_IN_DISABLED=true`
before building. An empty issuer otherwise fails the build; this opt-out does not
change backend authentication requirements.

```bash
docker --context default build --target backend \
  -t "econ-graph-backend:$VERSION" backend
docker --context default build --target crawler-worker \
  -t "econ-graph-crawler-worker:$VERSION" backend

docker --context default build \
  --build-arg VITE_API_URL="${VITE_API_URL:-http://localhost}" \
  --build-arg VITE_GRAPHQL_URL="${VITE_GRAPHQL_URL:-/graphql}" \
  --build-arg VITE_DATA_GRAPHQL_URL="${VITE_DATA_GRAPHQL_URL:-}" \
  --build-arg VITE_WS_URL="${VITE_WS_URL:-ws://localhost/graphql}" \
  --build-arg VITE_OIDC_ISSUER="${VITE_OIDC_ISSUER-/idp/realms/econ-graph}" \
  --build-arg VITE_OIDC_SIGN_IN_DISABLED="${VITE_OIDC_SIGN_IN_DISABLED:-false}" \
  --build-arg VITE_OIDC_CLIENT_ID="${VITE_OIDC_CLIENT_ID:-econ-graph-web}" \
  --build-arg NODE_ENV=production \
  -t "econ-graph-frontend:$VERSION" frontend

docker --context default build \
  -t econ-graph-chart-api:v1.0.0 chart-api-service

images=(
  "econ-graph-backend:$VERSION"
  "econ-graph-crawler-worker:$VERSION"
  "econ-graph-frontend:$VERSION"
  econ-graph-chart-api:v1.0.0
)
for image in "${images[@]}"; do
  docker --context default save "$image" | microk8s ctr images import -
done
microk8s ctr images ls
```

Check that all four imported references are present before restarting. On a
multi-node installation, import the same images on every node where these pods
can run; the local import does not distribute images to other nodes. Then run
[Restart existing images](#restart-existing-images) for the components being updated.
Reusing a tag replaces the image available to new pods; it does not change pods
that are already running.

## Configuration and schema changes

Do not apply `k8s/manifests/*.yaml` as a shortcut for restarting. That directory
also contains database resources, migration Jobs, legacy admin resources, and a
tunnel manifest committed with zero replicas. A complete deployment must order
those operations and preserve the installation's public host and credentials.

For configuration work, use the [Kubernetes configuration reference](../../k8s/README.md)
and the MicroK8s installation guide linked above to prepare a procedure for the
actual installation. Keep these constraints in that procedure:

- Apply data plane migrations successfully before starting the backend and worker
  against a changed data schema. The repository's `deploy.sh` and database precheck
  scripts select kind; they are not MicroK8s commands.
- Create or refresh required [Secrets](../../k8s/README.md#secrets) on the same
  explicitly selected cluster. Do not overwrite existing database credentials or
  expose Secret values in terminal output or logs.
- Preserve `PUBLIC_HOST` when rendering backend, Keycloak, realm, and Gateway
  configuration. Applying the committed localhost values directly can break the
  public site's issuer, redirect URIs, CORS settings, and routes. The
  [public-site configuration](../../k8s/README.md#-public-site-cloudflare-tunnel)
  explains those relationships. The deployment helper
  [`public-host.sh`](../../scripts/deploy/lib/public-host.sh) requires an explicit
  `PUBLIC_HOST` if the cluster already serves a public host; an explicitly empty
  value removes public access.

The HTTP entry point is Envoy Gateway. The Gateway `econ-graph-gateway` lives in
namespace `econ-graph-gateway`; its controller and proxy run in
`envoy-gateway-system`. Application HTTPRoutes live in `econ-graph`, which needs
the label `econgraph.io/gateway-access: "true"` to attach routes to the Gateway.
Configuration work installs prerequisites with
[`install-gateway.sh`](../../scripts/deploy/install-gateway.sh) before applying
routes with [`apply-gateway-routes.sh`](../../scripts/deploy/apply-gateway-routes.sh).
Both helpers require an explicit `KUBECTL` executable to avoid selecting cached
kind tooling and configuration; `--context` alone is insufficient. They do not
use the `mk` function, and `KUBECTL="microk8s kubectl"` is not an executable path.
Create a wrapper outside the checkout, read the context from MicroK8s, and confirm
its nodes match those selected above before invoking either helper:

```bash
export MICROK8S_TOOLS="${MICROK8S_TOOLS:?Set an absolute path for local helper files}"
mkdir -p "$MICROK8S_TOOLS"
cat >"$MICROK8S_TOOLS/kubectl-microk8s" <<'EOF'
#!/bin/sh
exec microk8s kubectl "$@"
EOF
chmod 0755 "$MICROK8S_TOOLS/kubectl-microk8s"
MICROK8S_CONTEXT="$(microk8s kubectl config current-context)"
export MICROK8S_CONTEXT
"$MICROK8S_TOOLS/kubectl-microk8s" --context "$MICROK8S_CONTEXT" get nodes -o wide

KUBECTL="$MICROK8S_TOOLS/kubectl-microk8s" \
  ./scripts/deploy/install-gateway.sh --context "$MICROK8S_CONTEXT"
KUBECTL="$MICROK8S_TOOLS/kubectl-microk8s" \
  ./scripts/deploy/apply-gateway-routes.sh --context "$MICROK8S_CONTEXT"
```

The installer accepts Kubernetes v1.33–v1.36 and needs a node labelled
`ingress-ready=true` for the proxy. Direct host access requires free host ports
80/443 on that node. If the MicroK8s ingress addon holds them, the installer's
default mode runs the proxy without host ports and reports the conflict. For that
addon, prepare a maintenance switch that disables the addon with
`microk8s disable ingress` and reruns the installer to give the Gateway those
ports. The [Gateway setup and cutover reference](../../k8s/README.md#-gateway)
describes the entry point; its `cutover-to-gateway.sh` helper removes kind's
ingress-nginx and refuses the MicroK8s addon, so it cannot perform that addon
switch. The installer discovers pod address ranges from node
`spec.podCIDRs` or MicroK8s Calico IP pools; if neither is available, supply the
installation's ranges through `GATEWAY_POD_CIDRS`.

The routes helper applies HTTPRoutes and rate limits; it does not install the
Gateway prerequisites. It preserves the current public host when `PUBLIC_HOST` is
unset. An explicitly empty value or `none` removes public routes, the HTTPS
listener, and its certificate. This differs from the deployment helper's
requirement for an explicit host above. A public host also requires cert-manager
before applying routes; preserve its configured `GATEWAY_TLS_ISSUER` (the helper
uses the existing certificate's issuer by default).

Manage the tunnel separately. Only the cluster marked production may hold its
token or run a connector. Applying `cloudflared.yaml` directly stops the connector;
the deployment helper starts it only with a production marker, a public host, and
the token Secret. The routes helper does not enforce tunnel ownership or start a
connector. Never start a second cluster's connector to test a deployment.

`restart-k8s-rollout.sh` is not a restart-only shortcut: it replaces
`~/.kube/config`, selects MicroK8s, rebuilds images, and reapplies infrastructure.
Its builder selects kind and omits the admin image that the wrapper subsequently
tries to import. Use the manual path above for image replacements; neither this
wrapper nor kind's `deploy.sh` supplies a complete MicroK8s configuration deployment.

## Verify access and behavior

Use the installation's existing Gateway origin throughout verification: normally
`http://localhost` on the MicroK8s host, or the configured HTTPS public host. This
keeps `/graphql` and `/idp` on the same origin as the frontend and matches Keycloak's
registered redirects. Set the origin to the one you intend to check:

```bash
APP_ORIGIN=http://localhost
curl --fail --show-error "$APP_ORIGIN/health"
curl --fail --show-error -H 'Content-Type: application/json' \
  --data '{"query":"{ __typename }"}' "$APP_ORIGIN/graphql"
mk -n econ-graph get deployments,services,httproutes
mk -n econ-graph-gateway get gateway econ-graph-gateway
```

Expect a successful health response and a GraphQL response with
`data.__typename` equal to `Query`, without GraphQL errors. HTTP success alone is
not sufficient for GraphQL. Open the same origin in a browser, load a series or
chart, and check the browser console and network requests. Verify sign-in and
sign-out through the configured issuer when authentication is enabled. These
checks demonstrate the paths exercised; they do not establish test-suite success
or accessibility compliance.

For direct service diagnostics, the frontend is at
`http://<node-address>:30000` and the backend is at
`http://<node-address>:30080` (the `FRONTEND_NODEPORT` and `BACKEND_NODEPORT` values
in `config/ports.env`). Use an address from `mk get nodes -o wide` that is reachable
from your client. Direct NodePorts bypass Gateway routing; use the Gateway origin
above for the integrated browser and authentication checks. The chart service is
internal to the cluster.

For Grafana, forward the Service in a separate terminal, then open
<http://localhost:3000>. The username is `admin`; retrieve its password through the
[documented Secret procedure](../../k8s/README.md#secrets).

```bash
microk8s kubectl --context=microk8s -n econ-graph \
  port-forward service/grafana-service 3000:3000
```

## Logs and troubleshooting

Inspect rollout failures before retrying. Backend logs describe API activity and
errors; frontend logs are nginx access and error logs, not React build output.

```bash
mk -n econ-graph logs deployment/econ-graph-backend --tail=100
mk -n econ-graph logs deployment/econ-graph-frontend --tail=100

# For optional components, only when deployed and running:
mk -n econ-graph logs deployment/crawler-worker --tail=100
mk -n econ-graph logs deployment/chart-api-service --tail=100

# Stream backend and frontend logs from matching pods, with pod prefixes.
mk -n econ-graph logs -f -l 'app in (econ-graph-backend,econ-graph-frontend)' \
  --all-containers=true --prefix=true --max-log-requests=10

mk -n econ-graph get pods -o wide --watch
mk -n econ-graph top pods
mk -n econ-graph describe deployment econ-graph-backend
mk -n econ-graph describe deployment econ-graph-frontend
mk -n econ-graph get events --sort-by='.metadata.creationTimestamp'
```

Stop streaming logs or the pod watch with Ctrl-C before continuing to the next
command. Resource metrics require the MicroK8s metrics-server addon. For an image
error, describe the affected pod and compare its requested image with `microk8s ctr images ls` on
that pod's node. For a failed rollout, use the pod's events and container logs to
identify the failure before restarting again.
