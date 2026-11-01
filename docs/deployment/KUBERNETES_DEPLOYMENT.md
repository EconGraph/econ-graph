# Kubernetes Deployment Guide

This guide takes the repository's kind deployment from preparation through access,
diagnosis, restart and removal. Use the [Kubernetes deployment reference](../../k8s/README.md)
for detailed configuration and the [Host Storage guide](HOST_STORAGE.md) before creating
or replacing a cluster. The [deployment entry guide](K8S_DEPLOYMENT_GUIDE.md) explains the
platform choices; the [MicroK8s guide](MICROK8S_DEPLOYMENT.md) provides a separate procedure
that requires operator adaptations for storage, image import and Gateway setup. The kind
helpers below do not substitute for that procedure.

## Deployment components

The maintained kind deployment runs in namespace `econ-graph`:

- The React frontend calls the Rust backend's GraphQL API. Keycloak supplies OIDC sign-in.
- The backend handles the app plane: users and application state. The crawler worker
  handles the data plane: crawled and reference data.
- PostgreSQL StatefulSets hold data plane, app plane and Keycloak data. App and data
  connection URLs can also name one shared database; deploying a separate app database
  does not move existing app rows. See [database settings](../../k8s/README.md#databases).
- The chart API has a private ClusterIP Service for chart generation.
- Prometheus collects metrics, Promtail ships logs to Loki, and Grafana displays them.

The retired admin frontend is excluded from the build. Deployment removes its old
Deployment and Service and provides no admin HTTPRoute. Do not use `/admin` as an application
health check or expect an admin UI from this procedure.

## Prepare the kind host and cluster

Run repository commands from the checkout root. This deployment needs more than a generic
Docker Desktop cluster: its databases use static PVs on host btrfs subvolumes.

1. Prepare the host using [Host Storage](HOST_STORAGE.md), including ownership and mount
   ordering. The mounted subvolumes at `/srv/econgraph/data-plane` and
   `/srv/econgraph/app-prod` must exist before Docker starts the storage worker. Preserve
   the database Secrets when reusing populated directories; new generated passwords do
   not recover access to an existing database.
2. Install the repository-pinned kind v0.33.0 as the operator, without sudo:

   ```bash
   ./scripts/deploy/install-kind.sh
   ```

   The deployment helpers use this installed binary and the rootful Docker daemon at
   `unix:///var/run/docker.sock`, rather than a rootless daemon selected by the shell.
   See [Docker selection](HOST_STORAGE.md#which-docker-daemon) and
   [kind selection](HOST_STORAGE.md#which-kind) for the override and installation details.
   Install the other tools required by the [deployment reference](../../k8s/README.md),
   including kubectl and Terraform for the Terraform setup route. The deployment also
   invokes `curl` and the jq-based `yq` (with `jq`); install those before starting.
   Gateway and cert-manager installation downloads upstream release manifests, and
   external workload images must be available to the nodes.
3. Create the cluster with the maintained configuration, or keep a reachable existing one:

   ```bash
   ./scripts/deploy/recreate-kind-cluster.sh --if-missing
   ```

   This installs Envoy Gateway and uses
   [the repository kind configuration](../../terraform/k8s/kind-config.yaml): a control
   plane with host port mappings and a worker labelled `econgraph.io/host-storage=true`
   with both host subvolumes bind-mounted. A cluster created without those mounts needs
   the [host-storage migration procedure](HOST_STORAGE.md#moving-the-live-deployment),
   not just a new Service or port mapping. The
   [manual Terraform setup](../../k8s/README.md#-manual-setup) is another creation entry
   point; use the same storage prerequisites.

For a throwaway cluster, follow [scratch storage setup](HOST_STORAGE.md#testing-on-a-scratch-cluster)
with a distinct cluster name and owned btrfs subvolumes. Keep its worker mounts and PV
paths consistent, and use unused host ports. Never mount the production database
subvolumes into a test cluster. Keep `KIND_CLUSTER`, `ECONGRAPH_SCRATCH_STORAGE`,
`XDG_CACHE_HOME` and `ECONGRAPH_BACKUP_ROOT` from that procedure in the operator
session for every helper below. Create it with `--if-missing --config <scratch-config>`;
the default configuration still names the production mounts and ports.

## Build and deploy

### Build and import the images

For a scratch cluster, retain all variables from the linked scratch setup and explicitly
align the image loader's independent target before building:

```bash
export KIND_CLUSTER_NAME="${KIND_CLUSTER:?Set the scratch cluster name first}"
```

The recreate/deploy helpers use `KIND_CLUSTER`, but the builder's loading step uses
`KIND_CLUSTER_NAME` and otherwise defaults to `econ-graph`. For the default installation,
keep both targets at `econ-graph`. Then build and import:

```bash
./scripts/deploy/build-images.sh
```

The helper builds and loads images into kind; a second `kind load` step is unnecessary.
The workloads use `imagePullPolicy: Never`, so locally built images must be imported into
the cluster under the tags the manifests reference:

| Image | Tag | Consumer |
| --- | --- | --- |
| `econ-graph-backend` | `v4.0.0` | Backend |
| `econ-graph-crawler-worker` | `v4.0.0` | Crawler worker and `crawler-migrate` Job |
| `econ-graph-frontend` | `v4.0.0` | Frontend |
| `econ-graph-chart-api` | `v1.0.0` | Chart API |

The crawler image uses the backend Dockerfile's `crawler-worker` stage. The helper checks
that the application version matches `backend/Cargo.toml`; changing only a build tag
would leave the manifests requesting an unavailable image.

Frontend settings are Vite build arguments (`VITE_*`). The helper sets the GraphQL path
and defaults `VITE_OIDC_ISSUER` to `/idp/realms/econ-graph` and
`VITE_OIDC_CLIENT_ID` to `econ-graph-web`. The relative issuer resolves against the page's
origin, so the image works with local and public routing. Set a full issuer URL only for
Keycloak on another origin. An explicitly empty issuer hides frontend sign-in; it does
not disable backend authentication. See the [build helper](../../scripts/deploy/build-images.sh)
for the full argument list.

When updating an existing installation under the same image tags, building/importing and
applying manifests does not replace existing pods if their Deployment templates are
unchanged. After deployment, complete the [image-update rollouts](#restart-and-resize)
for each affected workload, including frontend-only OIDC build-setting changes.

### Select local or public routing

`deploy.sh` targets the kind cluster `econ-graph` by default and pins its own kubectl to
`kind-econ-graph`. `PUBLIC_HOST` changes routing on that target; it does not select a
different cluster or follow whichever context is active in the operator's shell.

Before deployment, follow the [production marker procedure](../../k8s/README.md#production-marker).
Mark only the cluster serving the public site, and mark it again after recreation. On
nonproduction clusters, run the procedure's `--unmark` mode and remove any tunnel token
when prompted. A test connector with the production token joins the same tunnel and can
silently receive part of the public site's traffic.

Run **one** of these commands for the intended routing mode:

```bash
# Local routing on an unmarked cluster:
PUBLIC_HOST= ./scripts/deploy/deploy.sh

# Public routing, only on the marked production cluster:
PUBLIC_HOST=econgraph.com ./scripts/deploy/deploy.sh
```

`PUBLIC_HOST` is the base domain; `MAIN_HOST` selects the canonical HTTPS name and
must be either that domain or `www.<PUBLIC_HOST>`. An explicit `MAIN_HOST` overrides
the value in the deploy configuration for the same base domain; otherwise the default
is `www.<PUBLIC_HOST>`. With no override or configured main host, the public example
above serves `https://www.econgraph.com`. Set `MAIN_HOST=econgraph.com` to select the
apex instead. For an existing installation, retain its selected main host; follow
[Main public address](MAIN_HOST.md) when changing it so routing, Keycloak and sign-in
configuration move together.

A nonempty `PUBLIC_HOST` on an unmarked cluster stops deployment before applying
resources. An explicitly empty value serves local routes, removes the public routes,
HTTPS listener and certificate, and stops the tunnel connector. If `PUBLIC_HOST` is
unset while the cluster already serves a public host, deployment stops and asks for
an explicit choice.

Do not apply `k8s/manifests/` as a directory: `cloudflared.yaml` is committed with `replicas: 0`,
so a blanket apply stops an existing production connector. The deployment helper starts
it only with the production marker, public host and tunnel token in place. A failed
marker or token lookup stops the helper without changing the connector; a missing token
keeps it at zero replicas. Configure DNS, the tunnel origin and the token through the
[public site reference](../../k8s/README.md#-public-site-cloudflare-tunnel).

### Follow the deployment stages

The script checks host storage before PostgreSQL and preserves existing Secret values.
For populated host directories whose database Secrets are missing, it stops so the
operator can restore the matching Secrets from backup. Follow
[Secrets Management](../technical/SECRETS_MANAGEMENT.md) for operator handling.

Deployment starts the databases and Keycloak and starts the realm-import Job. It then
runs the `crawler-migrate` Job before applying the backend and crawler worker, and later
waits for realm import to complete. The migration Job applies data plane migrations
when the namespace has a `data-database-url`; failure stops that
deployment stage. The backend applies app plane migrations at startup. See
[database migrations](../development/MIGRATIONS.md) before changing either plane.

The script installs the Gateway infrastructure, then cert-manager before applying the
HTTPRoutes and, for a public host, the HTTPS listener and certificate. It installs
monitoring, assembles Grafana's combined dashboard configuration and waits for monitoring
readiness. Do not follow it with `kubectl apply -f k8s/monitoring/`: that would reapply alternative ConfigMaps and
replace the assembled dashboard configuration. For a separate monitoring installation,
select components and prerequisites through the [monitoring guide](../monitoring/README.md).

## Access and inspect the deployment

For manual inspection, use a kubectl binary that honors kubeconfig; on hosts where
`kubectl` is the MicroK8s wrapper, use the cached kind binary. In a Bash operator session,
the following selects the same cluster as the deployment helpers without replacing
`~/.kube/config`:

```bash
source scripts/deploy/lib/kind-env.sh
ensure_kind_kubectl
kubectl config use-context "kind-${KIND_CLUSTER}"
kubectl get pods,services -n econ-graph
kubectl get deployments -n econ-graph
```

The commands below assume that session and the default cluster name. For a scratch
cluster, use its cluster name and context throughout.

### Gateway access

Envoy Gateway provides the shared entry point. The installer uses pinned Gateway API
CRDs and Envoy Gateway releases; see the [Gateway reference](../../k8s/README.md#-gateway)
for the version matrix and upgrade/cutover procedure. The controller and proxy run in
`envoy-gateway-system`; Gateway `econ-graph-gateway` lives in namespace
`econ-graph-gateway`, separate from the application's namespace. The proxy runs on the
control-plane node labelled `ingress-ready=true`, where kind maps host ports 80/443.
If another pod already holds those ports, the installer leaves the proxy without host
ports; use the referenced cutover procedure or a proxy Service forward for diagnosis.

With the proxy ready and host mappings available, local access uses `http://localhost`.
HTTPRoutes in `econ-graph` match host `localhost` on listener `http`:

| Paths | Destination |
| --- | --- |
| `/`, Exact `/auth/callback`, Exact `/silent-callback.html` | Frontend |
| `/api`, `/auth`, `/health` | Backend |
| `/graphql` | Backend GraphQL |
| `/data/graphql` | Backend, rewriting the prefix to `/graphql` and removing `Authorization` and `Cookie` so requests arrive anonymous |
| `/idp/realms/econ-graph`, `/idp/resources` | Keycloak realm and login assets |

The exact callback matches take precedence over the backend's `/auth` prefix. Longer
prefixes take precedence over `/`; path prefixes match whole segments. Grafana, the
chart API, the admin UI and Keycloak's admin/master-realm endpoints have no dedicated
Gateway routes. The frontend catch-all does not make those applications available.

Public access uses `https://<MAIN_HOST>` through Cloudflare, cloudflared and the Gateway
HTTPS listener. Public route copies attach to listener `https`, and their hostnames
match `MAIN_HOST`; the certificate lives with the Gateway and covers both public names.
Keycloak's `KC_HOSTNAME` is `https://<MAIN_HOST>/idp`, and the backend's `OIDC_ISSUER`
is `https://<MAIN_HOST>/idp/realms/econ-graph`.

HTTP on both public names and HTTPS on the alternate name redirect to the canonical
HTTPS origin, preserving the path and query. Redirects to the apex use 301. Redirects
to `www.<PUBLIC_HOST>` use 302 until `MAIN_HOST_PERMANENT` names it, then use 301;
follow the [main-host switch procedure](MAIN_HOST.md#redirect-status) to finalize a switch.

The HTTP listener admits namespaces labelled `econgraph.io/gateway-access: "true"`;
HTTPS admits those labelled `econgraph.io/env: production`. The checked-in application
namespace carries both labels; the production marker remains a separate deployment guard. See
[public site configuration](../../k8s/README.md#-public-site-cloudflare-tunnel) for DNS,
TLS and tunnel requirements. Accepted routes alone do not establish public reachability.

Each GraphQL route has a separate per-client limit of 20 requests per second, with
bursts of 100 when the Gateway's EnvoyPatchPolicy is programmed. Without the burst
patch, the bucket size is 20. The limit is local to each proxy replica; the configured
proxy has one replica. Host-local requests share the Docker bridge client address.
The ClientTrafficPolicy trusts forwarded client addresses only from pod CIDRs, and each
route sets `X-Forwarded-For` to the address the proxy selected. In-cluster callers can
still claim an address through that trusted path; these addresses are not audit evidence.

### Service addresses and ports

| Service | In-cluster address and port |
| --- | --- |
| Backend | `econ-graph-backend-service.econ-graph.svc.cluster.local:9876` |
| Frontend | `econ-graph-frontend-service.econ-graph.svc.cluster.local:3000` |
| Keycloak | `keycloak-service.econ-graph.svc.cluster.local:8080` |
| Chart API | `chart-api-service.econ-graph.svc.cluster.local:3001` |
| Data PostgreSQL | `data-postgres-service.econ-graph.svc.cluster.local:5432` |
| Data PostgreSQL alias | `postgres-service.econ-graph.svc.cluster.local:5432` |
| App PostgreSQL | `app-postgres-service.econ-graph.svc.cluster.local:5432` |
| Keycloak PostgreSQL | `keycloak-postgres-service.econ-graph.svc.cluster.local:5432` |

The nonsecret [port settings](../../config/ports.env) can be sourced with
`source config/ports.env` for shell-based tooling. Sourcing them does not render
Kubernetes manifests or change cluster networking. Service ports and NodePorts are
literal values in the manifests.

The frontend and backend NodePorts are `30000` and `30080`. Kind runs nodes inside Docker
containers, so NodePorts reachable on a node's Docker network are not automatically
reachable at host localhost. The checked-in kind configuration maps host ports
`80`, `443`, `3000` and `8080`, not those NodePorts. Prefer the Gateway or port forwarding.
If customizing mappings, preserve the storage worker, mounts and Gateway mappings in the
repository configuration; do not replace it with a one-node cluster just to expose ports.

For direct Service diagnosis, use unused local ports and run each forward in its own
terminal. The frontend example avoids the kind host mapping on port 3000:

```bash
kubectl -n econ-graph port-forward service/econ-graph-frontend-service 3003:3000
kubectl -n econ-graph port-forward service/econ-graph-backend-service 9876:9876
kubectl -n econ-graph port-forward service/grafana-service 3002:3000
```

The frontend is then at `http://localhost:3003`, backend health at
`http://localhost:9876/health`, and Grafana at `http://localhost:3002`. A frontend-only
forward does not supply same-origin GraphQL and Keycloak routes for a complete session.
Grafana remains off the Gateway; its login uses the `grafana-admin` Secret as
described in [Secret handling](../../k8s/README.md#secrets). Stop forwards with Ctrl-C
when diagnosis is complete.

### Monitoring

Grafana provisions **EconGraph Platform Overview** and **EconGraph Logs & Debugging**.
The overview includes database connection panels; these are not separate database or
crawler dashboards. Use the [monitoring guide](../monitoring/README.md) to check metric
names, log labels and alert coverage. Provisioned dashboards do not prove that their
queries currently return data.

## Troubleshooting

### Pods, storage and database dependencies

```bash
kubectl describe pod POD_NAME -n econ-graph
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
kubectl get pvc -n econ-graph
kubectl get pv
kubectl get nodes -l econgraph.io/host-storage=true -o wide
kubectl logs deployment/econ-graph-backend -n econ-graph
kubectl logs statefulset/postgresql -n econ-graph
```

For Pending database pods, inspect PVC binding, the host-storage node label and worker
mounts before changing storage configuration. For connection failures, identify whether
the failed connection uses the app, data or Keycloak Service and check that database's
StatefulSet. Follow [Host Storage](HOST_STORAGE.md) before replacing volumes or credentials.
For migration failures, inspect `job/crawler-migrate` and its logs before retrying:

```bash
kubectl -n econ-graph describe job crawler-migrate
kubectl -n econ-graph logs job/crawler-migrate --all-containers
```

### Service or Gateway failures

```bash
kubectl get endpoints -n econ-graph
kubectl describe service econ-graph-backend-service -n econ-graph
kubectl get gatewayclass econ-graph -o yaml
kubectl -n econ-graph-gateway describe gateway econ-graph-gateway
kubectl -n econ-graph get httproute
kubectl -n econ-graph describe httproute econ-graph-frontend
kubectl -n envoy-gateway-system get pods -o wide
kubectl -n envoy-gateway-system logs deployment/envoy-gateway --tail=50
kubectl -n econ-graph get backendtrafficpolicy
kubectl -n econ-graph-gateway get envoypatchpolicy
```

Check GatewayClass `Accepted`, Gateway `Programmed` and each HTTPRoute's `Accepted` and
`ResolvedRefs` conditions. Compare the request's Host and path with the route's
listener, namespace labels, backend Service, port and ready endpoints. An unresolved
Service reference differs from a 503 caused by a Service without ready pods. Check the
proxy's placement on the control-plane node and host-port conflicts for connection
refusals. A root-route failure alone does not establish a controller defect.

Check the routes with the repository helper:

```bash
./scripts/deploy/test-gateway-routes.sh
```

This checks HTTP responses through the local Gateway; it does not establish every
application workflow or public reachability. On scratch clusters without host mappings,
forward the proxy Service in a separate terminal and pass its URL instead:

```bash
kubectl -n envoy-gateway-system port-forward service/econ-graph-gateway-proxy 18080:80
```

```bash
./scripts/deploy/test-gateway-routes.sh http://127.0.0.1:18080
```

For a failing backend health request, compare status and response content through the
Gateway and the direct backend forward from the access section:

```bash
curl --max-time 10 -sS -i http://localhost/health
curl --max-time 10 -sS -i http://localhost:9876/health
```

For the proxy forward, substitute `http://127.0.0.1:18080/health` for the first URL
and send `-H 'Host: localhost'`. A direct Service success narrows the fault to layers
that request bypasses; it does not verify integrated routing or browser sign-in. The
backend health handler returns JSON; the frontend's plain-text health handler is a
different endpoint. Neither response alone verifies database readiness. Follow the
[Gateway routing troubleshooting guide](GATEWAY_ROUTING_TROUBLESHOOTING.md) for route
tests, policy conditions, proxy access logs and public-host checks before changing routes.

## Restart and resize

A rollout restart replaces pods using their configured images. It does not build or
import images or run data plane migrations. For image changes, first complete
[build/import and deployment](#build-and-deploy). If you rebuilt an existing tag, you must
then restart each affected Deployment and wait for its rollout: applying an unchanged
image reference can leave old pods running. Use the commands for each workload whose
image changed; a full rebuild requires all four rollouts:

```bash
kubectl rollout restart deployment/econ-graph-backend -n econ-graph
kubectl rollout status deployment/econ-graph-backend -n econ-graph
kubectl rollout restart deployment/econ-graph-frontend -n econ-graph
kubectl rollout status deployment/econ-graph-frontend -n econ-graph
kubectl rollout restart deployment/crawler-worker -n econ-graph
kubectl rollout status deployment/crawler-worker -n econ-graph
kubectl rollout restart deployment/chart-api-service -n econ-graph
kubectl rollout status deployment/chart-api-service -n econ-graph
```

Monitoring uses StatefulSet `grafana`, Deployment `prometheus` and DaemonSet `promtail`;
use the matching workload kind when restarting and checking status. Treat database and
Keycloak version changes as upgrades with backup requirements.

Do not use `restart-k8s-rollout.sh` as a kind restart or bootstrap. It starts MicroK8s,
replaces `~/.kube/config` with MicroK8s configuration and pins the `microk8s` context.
It also calls the kind-only builder and attempts to tag the excluded admin image, so it
is not a self-contained MicroK8s deployment route. Use the
[MicroK8s procedure](MICROK8S_DEPLOYMENT.md) for that platform.

Review workload requests and limits against node capacity and observed usage before
resizing. `kubectl top pods -n econ-graph` requires a working metrics-server. The backend
and frontend can be scaled with `kubectl scale deployment NAME --replicas=COUNT -n econ-graph`;
this does not resize database storage or change the single-replica PostgreSQL layout.

## Exposure and security boundaries

Browser sign-in uses Keycloak OIDC; backend authentication and authorization are separate
from the chart API's request checks. The chart API has no public Gateway route and uses a
ClusterIP Service. Its middleware accepts internal network addresses or requests with both
`X-MCP-Server-Request: true` and `X-Internal-Request: true`; these identifying headers
are not user credentials or a replacement for network isolation. See the
[chart API security implementation](../../chart-api-service/src/security.js).

Public HTTPS follows the configured Cloudflare tunnel path; local Gateway and direct
NodePort or port-forward access have different exposure. Restrict those paths according
to the installation. The maintained kind `deploy.sh` does not apply `network-policy.yaml`.
Review its selectors and required connections before applying policies, and establish
that the selected CNI enforces them. A checked-in policy does not establish active
isolation. See [Kubernetes security guidance](../../k8s/README.md#-security) and
[Secrets Management](../technical/SECRETS_MANAGEMENT.md).

## Remove the application or cluster

Use the guarded teardown helper rather than deleting manifest directories. It targets
the kind cluster's own context, backs up the databases and Secrets outside the cluster,
and protects database and backup PVs before deletion. Review the
[cleanup reference](../../k8s/README.md#-cleanup) and [Host Storage](HOST_STORAGE.md) for
recovery requirements. A dry run runs checks and prints steps; it is not an offline plan.

| Intended scope | Command | What remains |
| --- | --- | --- |
| Remove the app plane | `./scripts/deploy/teardown.sh` | Data PostgreSQL, crawler and backup CronJob keep running; app/Keycloak Secrets and PVCs remain for redeployment |
| Remove the data plane too | `./scripts/deploy/teardown.sh --include-data-plane` | Host database directories and protected PVs remain; namespace, PVCs and Secrets are removed |
| Remove the cluster | `./scripts/deploy/teardown.sh --include-data-plane --delete-cluster` | Host database directories and the off-cluster backup remain; node-resident backups and Grafana data are lost |

App removal includes this namespace's HTTPRoutes (including public copies), rate-limit
policies, the tunnel connector, frontend, backend, chart API, Keycloak and the
app/Keycloak database StatefulSets and Services. The shared Gateway and controller
remain in their separate namespaces until the cluster is deleted. The helper asks for
the namespace name; data-plane removal additionally requires typing `econ_graph`, and cluster deletion
requires the cluster name. `--yes` does not answer the latter two confirmations.

The next deployment after data-plane removal needs matching Secrets restored first; use
the backup/restore procedure in the linked cleanup reference. Retained host directories
do not by themselves restore credentials or node-resident data. The teardown never deletes
the host-storage PVs or database directories; `terraform destroy` does not delete the
kind cluster. Do not bypass these boundaries with direct cluster or namespace deletion.
