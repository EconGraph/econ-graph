# MicroK8s Deployment Guide

Use this guide to adapt EconGraph to a dedicated local MicroK8s installation. The application consists of the Rust backend, React frontend, private chart API, crawler worker, PostgreSQL databases and Keycloak. Monitoring is optional. The legacy admin frontend is excluded because it uses retired authentication and cannot make authenticated GraphQL calls.

The repository's automated deployment targets kind. `build-images.sh` requires kind and loads images into kind; `deploy.sh` pins that cluster. The older `restart-k8s-rollout.sh` mixes MicroK8s operations with the kind builder and expects an admin image the builder excludes. Use the explicit MicroK8s procedure below for installation and restart. The Gateway installer and route helper are different: they accept an explicit kubectl executable and context and can be used on MicroK8s.

## Prepare the host and cluster

Use a Linux host with snap support, a Docker daemon for image builds, and enough memory and disk for the workloads you select. Size the host from the resource requests in the manifests and allow additional disk space for image builds, database growth and backups. The examples assume one MicroK8s node, a repository checkout, Bash, `curl`, and a SHA-256 utility (`sha256sum` or `shasum`). The Gateway installer downloads and verifies its pinned release manifests; its full prerequisite list is in the script.

Choose a MicroK8s snap channel whose Kubernetes version passes the current [Gateway installer](../../scripts/deploy/install-gateway.sh) guard: Kubernetes 1.33–1.36. This is the installer's supported range, not a record of this application being tested on those MicroK8s versions. Set `MICROK8S_CHANNEL` to the chosen channel before installing. Review [Canonical's installation guidance](https://canonical.com/microk8s/docs/getting-started) for host requirements.

```bash
sudo snap install microk8s --classic --channel="${MICROK8S_CHANNEL:?Choose a MicroK8s channel}"
sudo usermod -aG microk8s "$USER"
newgrp microk8s
microk8s status --wait-ready
microk8s enable dns
microk8s enable hostpath-storage
microk8s enable metrics-server
microk8s status --wait-ready
microk8s kubectl get nodes
```

All cluster commands below use `microk8s kubectl`, so they do not require overwriting an existing kubeconfig or switching another cluster's context. Run repository commands from the checkout root:

```bash
cd /path/to/econ-graph
set -euo pipefail
```

### Prepare and install the Gateway

The shared [Gateway installer](../../scripts/deploy/install-gateway.sh) installs the standard Gateway API CRDs, Envoy Gateway, and this cluster's GatewayClass, EnvoyProxy and Gateway. Its proxy has one replica and selects nodes labelled `ingress-ready=true`. Select the intended node explicitly; on a single-node installation it is the node listed above:

```bash
export MICROK8S_GATEWAY_NODE="${MICROK8S_GATEWAY_NODE:?Set the selected MicroK8s node name}"
microk8s kubectl label node "$MICROK8S_GATEWAY_NODE" ingress-ready=true --overwrite
microk8s kubectl get nodes -l ingress-ready=true
microk8s kubectl get deployments,daemonsets,services -A
```

Ensure that only the intended gateway node is eligible and that host ports 80 and 443 are free there. An existing MicroK8s ingress addon or another workload may hold them. On this dedicated local cluster, retire a conflicting addon with `microk8s disable ingress` only after deciding that its routes may stop. Check host listeners as well as pod port allocations.

By default, the installer detects other pods holding either host port on eligible nodes and installs a proxy **without host ports** when it finds a conflict. That proxy is reachable through its ClusterIP Service, but successful installation then does not make `http://localhost` reachable. For the local recipe, free both ports and rerun the installer. `GATEWAY_HOST_PORTS=true` below makes detected pod conflicts fail instead of silently choosing Service-only access; it cannot detect every host process using a port.

Both Gateway helpers invoke `KUBECTL` as one executable, so `KUBECTL="microk8s kubectl"` does not work. Create an executable wrapper outside the checkout and obtain the context name from MicroK8s's bundled kubectl. The context is then passed explicitly on each helper invocation:

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
KUBECTL="$MICROK8S_TOOLS/kubectl-microk8s" GATEWAY_HOST_PORTS=true \
  ./scripts/deploy/install-gateway.sh --context "$MICROK8S_CONTEXT"
```

The installer derives the pod CIDRs trusted for forwarded client addresses from node `spec.podCIDRs`, falling back to Calico IP pools when MicroK8s leaves that field empty. If neither is available, it stops: set `GATEWAY_POD_CIDRS` to the actual space-separated pod networks and rerun. Do not substitute an arbitrary broad network. Routes are applied after the application below; installing the Gateway alone does not deploy EconGraph.

## Prepare manifests and persistent dependencies

Keep MicroK8s adaptations in a separate local directory; the examples call it `MICROK8S_MANIFESTS`. Copy only the files selected below, preserving the `keycloak/` subdirectory. Do not copy or apply the whole manifests directory as a deployment unit. The Gateway helpers use the committed Gateway manifests directly, separately from these local application copies. Preserve `econgraph.io/gateway-access: "true"` in the namespace copy: that label permits its HTTPRoutes to attach to the HTTP listener.

```bash
export MICROK8S_MANIFESTS="${MICROK8S_MANIFESTS:?Set an absolute path for local manifest copies}"
mkdir -p "$MICROK8S_MANIFESTS/keycloak"
for name in namespace configmap host-storage postgres-init postgres-deployment postgres \
  app-postgres-init app-postgres crawler-migrate-job backend-deployment backend-service \
  crawler-worker frontend-deployment frontend-service chart-api-deployment chart-api-service; do
  cp "k8s/manifests/$name.yaml" "$MICROK8S_MANIFESTS/"
done
cp k8s/manifests/keycloak/{postgres,deployment,realm-import-job}.yaml \
  "$MICROK8S_MANIFESTS/keycloak/"
```

### Storage and database settings

The [hostpath-storage addon](https://canonical.com/microk8s/docs/addon-hostpath-storage) supplies a default provisioner, but the database manifests explicitly request `econgraph-host`. They also select a node labelled `econgraph.io/host-storage=true`. Enabling the addon alone does not satisfy those requirements.

Prepare storage before starting any database. The copied `host-storage.yaml` defines three retained, prebound PVs:

| Database | Host directory | PVC in `econ-graph` |
| --- | --- | --- |
| Data plane | `/srv/econgraph/data-plane/pg` | `postgresql-data-postgresql-0` |
| App plane | `/srv/econgraph/app-prod/app-pg` | `app-postgres-data-app-postgres-0` |
| Keycloak | `/srv/econgraph/app-prod/keycloak-pg` | `keycloak-postgres-data-keycloak-postgres-0` |

On MicroK8s these paths refer to the node's filesystem, without kind's container bind mounts. Select a storage node, prepare dedicated directories with appropriate database ownership, and label that node with `microk8s kubectl label node STORAGE_NODE econgraph.io/host-storage=true --overwrite`, replacing `STORAGE_NODE` with its name. Keep the StatefulSet selectors, PV node affinity, paths, storage class and PVC prebindings consistent. See [Host Storage](HOST_STORAGE.md) for directory ownership, mount checks, backup and retention requirements; its kind node-container commands are not MicroK8s commands. Never point a new test installation at another installation's database directories.

Alternatively, use dynamic provisioning for all three databases. In all three StatefulSets (`postgres-deployment.yaml`, `app-postgres.yaml` and `keycloak/postgres.yaml`), under `volumeClaimTemplates`, remove `spec.volumeName` so the claim no longer requests a named static PV, and set `spec.storageClassName` to the chosen provisioner's storage class. Remove the StatefulSets' host-storage node selectors and omit `host-storage.yaml`. Choose the reclaim policy and backup location explicitly; dynamically provisioned storage does not inherit the static PVs' retention guarantees.

The app plane holds users and application state; the data plane holds crawled and reference data. Their connection URLs may name separate databases or one shared database. Match the database names, services and Secret references in your copies to the selected layout; see [database settings](../../k8s/README.md#databases).

### Secrets and identity configuration

A human operator must provision the required Secrets in `econ-graph` through the approved process before applying their consumers. Use the names and keys in the [Secret contract](../../k8s/README.md#secrets), including `econ-graph-postgres`, `econ-graph-app-postgres`, `econ-graph-secrets` and `econ-graph-keycloak`. Preserve credentials for existing database volumes. Do not generate replacement passwords merely because a Secret is missing. Follow [Secrets Management](../technical/SECRETS_MANAGEMENT.md) for handling values; do not put them in the copied manifests or command history.

Confirm that any Secret tooling targets MicroK8s explicitly; the interactive crawler-key wrapper selects kind and is not a MicroK8s setup command. Agents must not run Secret setup or retrieve values.

This recipe uses the committed local origin `http://localhost`. Keep these settings aligned:

- The frontend image's `VITE_OIDC_ISSUER` is `/idp/realms/econ-graph`.
- Backend `OIDC_ISSUER` is `http://localhost/idp/realms/econ-graph`; its JWKS URL uses the internal Keycloak Service.
- Keycloak `KC_HOSTNAME` is `http://localhost/idp`; `KC_WEB_BASE_URL` and the realm import configuration use `http://localhost`.
- `econ-graph-config` lists the browser origin in `CORS_ORIGIN`, which the backend reads as `CORS_ALLOWED_ORIGINS`.

Keycloak and its realm import are required for sign-in. An empty frontend issuer fails the build unless `VITE_OIDC_SIGN_IN_DISABLED=true` explicitly opts into hiding sign-in. That opt-out does not disable the backend's `OIDC_REQUIRED` requirement or configure an alternative authentication service.

## Build and import images

The selected manifests reference backend, crawler and frontend tag `v4.0.0`, and chart API tag `v1.0.0`. Keep the tags in your manifest copies and images identical. Build from the same checkout, using the frontend arguments from the kind builder without invoking its kind-loading step:

```bash
docker build -t econ-graph-backend:v4.0.0 backend
docker build --target crawler-worker -t econ-graph-crawler-worker:v4.0.0 backend
docker build \
  --build-arg VITE_API_URL=http://localhost \
  --build-arg VITE_GRAPHQL_URL=/graphql \
  --build-arg VITE_DATA_GRAPHQL_URL= \
  --build-arg VITE_WS_URL=ws://localhost/graphql \
  --build-arg VITE_OIDC_ISSUER=/idp/realms/econ-graph \
  --build-arg VITE_OIDC_CLIENT_ID=econ-graph-web \
  --build-arg NODE_ENV=production \
  -t econ-graph-frontend:v4.0.0 frontend
docker build -t econ-graph-chart-api:v1.0.0 chart-api-service
```

Import into MicroK8s's container runtime, which does not share Docker's image store:

```bash
set -o pipefail
docker save econ-graph-backend:v4.0.0 econ-graph-crawler-worker:v4.0.0 \
  econ-graph-frontend:v4.0.0 econ-graph-chart-api:v1.0.0 \
  | microk8s ctr image import -
microk8s ctr images ls
```

The application manifests use `imagePullPolicy: Never`, so missing images fail rather than being pulled. The import above is node-local; for multiple nodes use [Canonical's image side-loading procedure](https://canonical.com/microk8s/docs/sideload) to populate every eligible node. Upstream database, Keycloak and init-container images still need registry access or separate imports.

## Deploy in dependency order

These commands assume the manifest copies, storage, Secrets and Gateway have been prepared as described above. Inspect each stage before proceeding; do not continue after a failed rollout or Job.

1. Create the namespace, then provision the operator-managed Secrets. Apply the adapted configuration and storage. Omit the last command if you chose dynamic provisioning.

   ```bash
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/namespace.yaml"
   # Provision the required Secrets here through the approved operator process.
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/configmap.yaml"
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/host-storage.yaml"
   ```

2. Start the data and app databases, then Keycloak's database. Wait for all three and check that their PVCs are Bound.

   ```bash
   for name in postgres-init postgres-deployment postgres app-postgres-init app-postgres; do
     microk8s kubectl apply -f "$MICROK8S_MANIFESTS/$name.yaml"
   done
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/keycloak/postgres.yaml"
   for name in postgresql app-postgres keycloak-postgres; do
     microk8s kubectl -n econ-graph rollout status "statefulset/$name" --timeout=300s
   done
   microk8s kubectl -n econ-graph get pvc
   ```

3. Create the realm ConfigMap, start Keycloak and apply the realm import Job. The import configures the clients and origins needed for sign-in.

   ```bash
   microk8s kubectl -n econ-graph create configmap keycloak-realm \
     --from-file=econ-graph-realm.json=config/keycloak/econ-graph-realm.json \
     --from-file=render-realm.sh=scripts/keycloak/render-realm.sh \
     --dry-run=client -o yaml | microk8s kubectl apply -f -
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/keycloak/deployment.yaml"
   microk8s kubectl -n econ-graph rollout status deployment/keycloak --timeout=900s
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/keycloak/realm-import-job.yaml"
   microk8s kubectl -n econ-graph wait --for=condition=complete job/keycloak-realm-import --timeout=600s
   ```

4. Run the data plane migrations before starting the backend or crawler worker. The backend applies app plane migrations at startup; no service applies data plane migrations at startup. See [Database migrations](../development/MIGRATIONS.md).

   ```bash
   microk8s kubectl apply -f "$MICROK8S_MANIFESTS/crawler-migrate-job.yaml"
   microk8s kubectl -n econ-graph wait --for=condition=complete job/crawler-migrate --timeout=1860s
   ```

   If either Job fails, inspect its status and logs before retrying. These apply commands are for the initial installation: an existing completed Job does not run again on apply. On subsequent deployments, confirm no instance is running before replacing a Job to rerun the realm import or migrations.

5. Apply the application workloads and Services, excluding the admin frontend. Wait for their rollouts.

   ```bash
   for name in backend-deployment backend-service crawler-worker frontend-deployment \
     frontend-service chart-api-deployment chart-api-service; do
     microk8s kubectl apply -f "$MICROK8S_MANIFESTS/$name.yaml"
   done
   for name in econ-graph-backend crawler-worker econ-graph-frontend chart-api-service; do
     microk8s kubectl -n econ-graph rollout status "deployment/$name" --timeout=300s
   done
   ```

6. Apply local HTTPRoutes and their rate-limit policies with the same MicroK8s wrapper and context. This recipe is for a **dedicated local cluster**: `PUBLIC_HOST=none` removes any existing public HTTPRoutes, HTTPS listener, burst policy, certificate and TLS Secret owned by the helper. Do not use that setting on a cluster whose public site must remain available. Unset `PUBLIC_HOST` would instead preserve the detected public host.

   ```bash
   KUBECTL="$MICROK8S_TOOLS/kubectl-microk8s" PUBLIC_HOST=none \
     ./scripts/deploy/apply-gateway-routes.sh --context "$MICROK8S_CONTEXT"
   microk8s kubectl -n econ-graph-gateway wait --for=condition=Programmed \
     gateway/econ-graph-gateway --timeout=300s
   microk8s kubectl get gatewayclass econ-graph -o yaml
   microk8s kubectl -n econ-graph-gateway get gateway econ-graph-gateway -o yaml
   microk8s kubectl -n econ-graph get httproute -o yaml
   ```

   Require `Accepted=True` on GatewayClass `econ-graph`, and `Accepted=True` and `Programmed=True` on the Gateway with current observed generations. HTTPRoute conditions live under `status.parents[].conditions`; inspect each route's entry for `econ-graph-gateway` and require both `Accepted=True` and `ResolvedRefs=True`, with the current generation observed, for all five routes: `econ-graph-frontend`, `econ-graph-backend`, `econ-graph-graphql`, `econ-graph-data-graphql` and `econ-graph-idp`. Stop and diagnose any missing or false condition before testing access. Pod readiness alone does not establish routing readiness. Also inspect the two BackendTrafficPolicies and the Gateway burst policy after routes exist:

   ```bash
   microk8s kubectl -n econ-graph get backendtrafficpolicy -o yaml
   microk8s kubectl -n econ-graph-gateway get envoypatchpolicy econ-graph-ratelimit-burst -o yaml
   ```

Do not apply `cloudflared.yaml` during local work. Its committed zero replicas would stop an existing connector, and starting a connector belongs only on the production-marked cluster. Public hostname rendering, tunnel credentials and production marking are a separate operation described in [Public site](../../k8s/README.md#-public-site-cloudflare-tunnel); that guide's `deploy.sh` command selects kind and must not be used to deploy this MicroK8s cluster.

For public access, visitor TLS ends at Cloudflare; the tunnel connector reaches the Gateway's HTTPS listener. The route helper creates that listener and its certificate only for a public host and defaults to the existing issuer or `econ-graph-selfsigned`. `PUBLIC_HOST` retains the base domain, while `MAIN_HOST` selects the served address and defaults to `www.${PUBLIC_HOST}`; public application and Keycloak origins must follow that main address. See [Main public address](MAIN_HOST.md). A public MicroK8s adaptation must also align application and identity origins and install cert-manager with Gateway API support (`--enable-gateway-api`) before public route application. Installing cert-manager or an issuer alone does not configure a TLS route. This local recipe uses HTTP and does not issue certificates.

## Access and verify this installation

With Envoy listening on the local host's port 80, open `http://localhost`. The committed host rules require `localhost`; mapping the production domain in `/etc/hosts` does not change those rules. Run these checks on the MicroK8s host:

```bash
curl --fail http://localhost/health
curl --fail http://localhost/idp/realms/econ-graph/.well-known/openid-configuration
microk8s kubectl -n econ-graph get pods,services,httproute
```

Check the frontend, sign-in callback and a GraphQL request in the browser. A request to the real public site does not test this local cluster.

For service-level diagnosis, run each forward in its own terminal:

```bash
microk8s kubectl -n econ-graph port-forward service/econ-graph-frontend-service 3000:3000
microk8s kubectl -n econ-graph port-forward service/econ-graph-backend-service 9876:9876
# If Grafana has been installed:
microk8s kubectl -n econ-graph port-forward service/grafana-service 3002:3000
```

The backend health check is then `http://localhost:9876/health`. The frontend forward alone does not supply the same-origin GraphQL and Keycloak routes needed for a full application session. Grafana remains private and is available at `http://localhost:3002` through its forward.

## Security and monitoring

Backend CORS enforcement comes from `CORS_ALLOWED_ORIGINS`; Keycloak enforces its clients' web origins. The [HTTPRoutes](../../k8s/manifests/gateway/routes/httproutes.yaml) configure response headers at the Gateway. Frontend responses receive CSP and the listed security headers; `/silent-callback.html` specifically allows same-origin framing (`SAMEORIGIN` and `frame-ancestors 'self'`) for silent sign-in. Other frontend responses deny framing. Backend and GraphQL routes set `nosniff`, `DENY` and HSTS. Browsers ignore HSTS over local HTTP. A separate Keycloak origin requires corresponding frontend CSP changes. Check the headers on the edge you actually use, including the silent callback exception.

The two GraphQL BackendTrafficPolicies each allow 20 requests per second per client address, with separate buckets for `/graphql` and `/data/graphql`. The installer's EnvoyPatchPolicy raises burst capacity to 100 when successfully programmed. Limits are local to each proxy and rely on the committed single replica; adding replicas changes the aggregate limit. NAT and proxying can make clients share a bucket, so verify client-address handling on the actual MicroK8s network and access path. The client-traffic policy trusts forwarded addresses only from the selected pod CIDRs; those pods must be trusted. These limits do not cover every backend route or direct Service access. `/data/graphql` rewrites to backend `/graphql` and strips cookies and authorization, so those requests are anonymous.

The file named `pod-security-policy.yaml` sets namespace labels for Pod Security Admission; it is not a PodSecurityPolicy. Do not apply restricted enforcement indiscriminately: database initialization and monitoring workloads need a compatibility review. The current [NetworkPolicies](../../k8s/manifests/network-policy.yaml) select Envoy pods in `envoy-gateway-system` and the application ports. Review those labels, ports and all required connections for your selected workloads before applying the file; a policy applied successfully is not evidence that the selected networking implementation enforces it. See [Kubernetes security guidance](../../k8s/README.md#-security).

Choose monitoring components through the [monitoring guide](../monitoring/README.md). Apply their configuration, service accounts and RBAC before workloads, and provision Grafana's required Secret first. The monitoring directory contains alternative configurations and certificate resources; do not apply the whole directory. Measure this host's capacity and workload usage rather than assuming a fixed CPU, memory or pod limit:

```bash
microk8s kubectl describe nodes
microk8s kubectl top nodes
microk8s kubectl top pods -n econ-graph
microk8s kubectl get pvc -n econ-graph
```

`top` requires metrics-server to be ready. PVC capacity does not show free space inside a hostPath filesystem; inspect the selected host filesystem as well. Schedule database backups and verify restoration using the [storage guidance](HOST_STORAGE.md) before depending on persistent data.

## Troubleshooting

Check cluster readiness and group membership if MicroK8s cannot be reached:

```bash
microk8s status
groups "$USER"
```

If stopped, use `microk8s start`. If membership is missing, repeat the installation's group step and start a new group session. [Canonical's command reference](https://canonical.com/microk8s/docs/command-reference) documents `microk8s inspect`, which collects a diagnostic archive; review it for sensitive information before sharing it.

For Pending or restarting pods, inspect the named pod and namespace events:

```bash
microk8s kubectl describe pod POD_NAME -n econ-graph
microk8s kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
```

For a PostgreSQL crash, including SIGBUS, establish the failure before changing configuration. Check pod events, container exit state, PVC binding, the selected node's mounts and filesystem free space. Inspect database logs through the approved operator process. A bus error alone does not establish a hugepage problem or justify changing database image, initialization arguments or resource limits. Follow [MicroK8s troubleshooting](MICROK8S_TROUBLESHOOTING.md) and [Host Storage](HOST_STORAGE.md) before changing persistent state.

For routing failures, check the Gateway and route conditions first, then namespace attachment labels, host matches, backend Service names/endpoints, the Envoy node selection and host-port allocation. The proxy Service is `econ-graph-gateway-proxy` in `envoy-gateway-system`; its pod ports are 10080/10443, while Service ports are 80/443. NetworkPolicy must allow the actual pod ports wherever it selects the proxy. Test the backend through its port forward to distinguish service health from Gateway behavior. For public TLS failures, inspect the HTTPS listener, Certificate and cert-manager events separately:

```bash
microk8s kubectl -n econ-graph-gateway describe gateway econ-graph-gateway
microk8s kubectl -n econ-graph-gateway get certificate econ-graph-tls
microk8s kubectl -n econ-graph get httproute -l econgraph.io/public-host=true
```

For memory pressure, use pod events and metrics to identify this installation's workload before resizing it.

## Restart and remove

Restart individual application deployments without rebuilding images:

```bash
microk8s kubectl rollout restart deployment/econ-graph-backend -n econ-graph
microk8s kubectl rollout restart deployment/econ-graph-frontend -n econ-graph
microk8s kubectl rollout restart deployment/chart-api-service -n econ-graph
```

Wait for each changed deployment with `microk8s kubectl rollout status`. A restart does not import new images or apply data plane migrations. For an image update, rebuild/import matching tags and follow the migration and rollout order above. Treat database and Keycloak version changes as upgrades with backup requirements, rather than routine application restarts.

Stopping MicroK8s interrupts all workloads on this installation:

```bash
microk8s stop
microk8s start
microk8s status --wait-ready
```

To remove only the application workloads and local routes, the following list leaves the namespace, Secrets, database StatefulSets, PVCs, PVs and monitoring resources in place:

```bash
microk8s kubectl -n econ-graph delete backendtrafficpolicy \
  econ-graph-graphql-ratelimit econ-graph-data-graphql-ratelimit --ignore-not-found
microk8s kubectl -n econ-graph delete httproute \
  econ-graph-frontend econ-graph-backend econ-graph-graphql \
  econ-graph-data-graphql econ-graph-idp --ignore-not-found
microk8s kubectl -n econ-graph delete deployment \
  econ-graph-backend econ-graph-frontend crawler-worker chart-api-service keycloak --ignore-not-found
microk8s kubectl -n econ-graph delete service \
  econ-graph-backend-service econ-graph-frontend-service chart-api-service keycloak-service --ignore-not-found
```

Check for running migration or realm-import Jobs before running these removal commands. This list removes only the local routes and their application policies. It leaves the cluster Gateway, GatewayClass, EnvoyProxy, controller, CRDs and Gateway-level policies installed for separate lifecycle management. A public-host installation also needs deliberate removal of its named public routes and policies; the local list is not public-site teardown. Remove optional monitoring using the explicit files selected for that installation, retaining any desired persistent volumes. Do not use `kubectl delete -f k8s/manifests/`: that includes the namespace and protected storage resources.

Full MicroK8s removal also removes cluster configuration and can affect addon-managed storage. First take and verify backups outside MicroK8s, record the database storage paths and retention policy, and arrange recovery of the operator-managed Secrets. Retained host directories still require deliberate recovery and must not be deleted as routine cleanup. Then remove the snap if that is the intended scope:

```bash
microk8s stop
sudo snap remove microk8s
```

## MicroK8s and kind in this repository

| Operation | MicroK8s | kind |
| --- | --- | --- |
| Installation | Snap channel selected by the operator | Repository-pinned kind binary and cluster configuration |
| Local application images | Explicit import into MicroK8s containerd | `build-images.sh` builds and loads into kind |
| Database storage | Operator adapts node paths, placement and provisioning | Host directories bind-mounted into the configured worker |
| Gateway | Shared installer with explicit MicroK8s context, node label and free host ports | Shared installer with kind's node labels and host-port mappings |
| Deployment | Ordered adaptation in this guide | Repository deployment automation |

For related procedures, see the [Kubernetes deployment guide](KUBERNETES_DEPLOYMENT.md), [MicroK8s troubleshooting](MICROK8S_TROUBLESHOOTING.md) and [documentation index](../README.md).
