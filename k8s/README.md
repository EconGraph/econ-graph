# EconGraph Kubernetes Deployment

This directory contains all the necessary files to deploy EconGraph to a local Kubernetes cluster using Terraform and kind (Kubernetes in Docker).

## 🚀 Quick Start

### Prerequisites

- Docker
- Terraform
- kubectl
- kind v0.33.0, the version the deploy scripts run. Install it once, as the user who runs
  them (no sudo):

  ```bash
  ./scripts/deploy/install-kind.sh
  ```

  It downloads the release binary, checks it against the SHA-256 pinned in the script and
  installs it as `${XDG_DATA_HOME:-~/.local/share}/econgraph/bin/kind-v0.33.0`, with a
  `kind` symlink next to it. The deploy scripts run that copy, not the `kind` on `PATH`; a
  packaged kind (`/usr/bin/kind`) is left alone. Without it they stop and say to run the
  installer; only read-only `kind get` commands fall back to the `kind` on `PATH`, with a
  warning. Re-running the installer does nothing when the right version is in place.

### Setup

```bash
sudo scripts/host/setup-subvolumes.sh                    # host storage under /srv/econgraph, once per host
./scripts/deploy/recreate-kind-cluster.sh --if-missing   # the kind cluster, if there is none
./scripts/deploy/build-images.sh                         # the images
./scripts/deploy/deploy.sh                               # databases, Secrets, application, routes
```

## 📁 Directory Structure

```text
k8s/
├── manifests/           # Kubernetes deployment manifests
│   ├── namespace.yaml   # Namespace definition
│   ├── configmap.yaml   # Configuration
│   ├── host-storage.yaml        # StorageClass and static PVs on host subvolumes
│   ├── postgres-deployment.yaml # Data database (StatefulSet postgresql)
│   ├── postgres.yaml            # data-postgres-service, alias postgres-service
│   ├── postgres-init.yaml       # Data database roles
│   ├── app-postgres.yaml        # App database (StatefulSet app-postgres, app-postgres-service)
│   ├── app-postgres-init.yaml   # App database roles
│   ├── backend-deployment.yaml  # Backend deployment
│   ├── frontend-deployment.yaml # Frontend deployment
│   ├── admin-frontend-deployment.yaml # Admin UI deployment
│   ├── admin-frontend-service.yaml    # Admin UI service
│   └── gateway/         # The gateway (Envoy Gateway): see "Gateway" below
│       ├── envoy-proxy.yaml, gatewayclass.yaml, gateway.yaml
│       ├── gateway-https.yaml   # optional listener https
│       ├── client-traffic-policy.yaml # drops client-sent forwarding headers
│       ├── ratelimit-burst.yaml # bursts of 100 for the GraphQL rate limits
│       └── routes/          # this namespace's HTTPRoutes and rate limits
└── README.md           # This file

terraform/k8s/          # Terraform configuration
├── main.tf            # Main Terraform configuration
├── variables.tf       # Variables
└── outputs.tf         # Outputs

scripts/deploy/         # Deployment scripts
├── build-images.sh    # Build Docker images
├── load-release-images.sh     # Put a release candidate's GHCR images on the nodes
├── verify-release-images.sh   # Restart and check the pods run them
├── rollback-release-images.sh # Go back to what ran before the load
├── create-secrets.sh  # Create the Secrets (run by deploy.sh)
├── install-gateway.sh # Install the gateway (run by deploy.sh)
├── apply-gateway-routes.sh # Apply the routes, with the https host name (run by deploy.sh)
├── test-gateway-routes.sh # Check every route on the gateway
├── cutover-to-gateway.sh  # Move a cluster from ingress-nginx to the gateway
├── deploy.sh          # Deploy to K8s
├── mark-production-cluster.sh # Mark the cluster that serves the public site
├── teardown.sh        # Remove the app plane (the data plane only on request)
└── released-pvs.sh    # List Released PVs; delete one on typed confirmation
```

## 🛠️ Manual Setup

If you prefer to run each step manually:

### 1. Create Kubernetes Cluster

```bash
cd terraform/k8s
terraform init
terraform apply
```

Only on the production cluster (the one that serves <https://econgraph.com>), mark it as
production once, after creating it and again after every recreate. See
[Production marker](#production-marker). On every other cluster, run it with `--unmark`
once instead.

```bash
./scripts/deploy/mark-production-cluster.sh
```

### 2. Build Docker Images

```bash
./scripts/deploy/build-images.sh
```

### 3. Deploy Application

```bash
./scripts/deploy/deploy.sh
```

### Release images instead of local builds

To run the images that `.github/workflows/release.yml` built for a release candidate tag,
and not images built here, load them onto the nodes under the manifests' names before
`deploy.sh`, then restart and check:

```bash
./scripts/deploy/load-release-images.sh --tag vX.Y.Z-rc.N
./scripts/deploy/deploy.sh
./scripts/deploy/verify-release-images.sh --tag vX.Y.Z-rc.N
```

`./scripts/deploy/rollback-release-images.sh --to pre-vX.Y.Z-rc.N` goes back to what ran
before the load. The load covers every workload that runs the three images, including the
`crawler-migrate` Job. See [Release images on the kind cluster](../docs/deployment/RELEASE_IMAGES.md).

## 🔐 Keycloak

`scripts/deploy/deploy.sh` deploys Keycloak from `k8s/manifests/keycloak/` (its own
Postgres, then Keycloak with the `econ-graph` realm from `config/keycloak/`). Its
credentials are never committed: they live in the Secret `econ-graph-keycloak`,
written by `scripts/deploy/create-secrets.sh`, which `deploy.sh` runs. That script generates the
admin and database passwords on its first run and keeps them afterwards. `deploy.sh` skips Keycloak while the Secret
does not exist.

```bash
# Optional, for Google sign-in (redirect URI https://<PUBLIC_HOST>/idp/realms/econ-graph/broker/google/endpoint,
# or http://localhost/idp/... without a PUBLIC_HOST):
export KEYCLOAK_GOOGLE_CLIENT_ID=... KEYCLOAK_GOOGLE_CLIENT_SECRET=...
./scripts/deploy/create-secrets.sh
./scripts/deploy/deploy.sh
```

- Issuer: `http://localhost/idp/realms/econ-graph`, or `https://<PUBLIC_HOST>/idp/realms/econ-graph`
  with a public host (see [Public site](#-public-site-cloudflare-tunnel)). Only `/idp/realms/econ-graph`
  and `/idp/resources` are routed by the gateway; the master realm and the admin console are not
  reachable from outside the cluster.
- Admin console: `kubectl -n econ-graph port-forward svc/keycloak-service 18080:8080`,
  then <http://localhost:18080/idp/admin>. The password is in the Secret:
  `kubectl -n econ-graph get secret econ-graph-keycloak -o jsonpath='{.data.admin-password}' | base64 -d`
- Every deploy re-applies `config/keycloak/econ-graph-realm.json` through the Job
  `keycloak-realm-import` (keycloak-config-cli), so the realm, its roles and the
  Google client follow the repository. Users and their role assignments are kept.
  Logs: `kubectl -n econ-graph logs job/keycloak-realm-import -c keycloak-config-cli`.
- Seeded test users exist only in the local docker-compose stack, never in the cluster.
- A new Keycloak version migrates its database on first start, and an older one can't
  run on the result. Before a deploy that changes the Keycloak image, run
  `scripts/deploy/backup-keycloak-db.sh && scripts/deploy/deploy.sh`: the script takes a
  dump of the `keycloak` database, verified by a restore into a scratch database, into
  `~/econgraph-backups/keycloak/`. `scripts/deploy/restore-keycloak-db.sh` puts one
  back, and is the only way to downgrade Keycloak (never `kubectl rollout undo` or
  `set image`); see the [v4.0.0 QA plan](../docs/release/v4.0.0-qa.md). The Deployment
  uses `Recreate`, and `deploy.sh` waits for the new Keycloak before applying the realm.

## 🌐 Accessing the Application

After deployment, the application will be available at:

- **Frontend**: <http://localhost>
- **Backend API**: <http://localhost/api>
- **GraphQL**: <http://localhost/graphql>
- **Health Check**: <http://localhost/health>

**Note**: the routes match host name `localhost`, so no `/etc/hosts` entry is needed. The admin
frontend isn't deployed by `deploy.sh`, and `/admin` is not routed.

## 🚪 Gateway

The cluster's HTTP entry point is a Gateway API gateway run by Envoy Gateway.
`scripts/deploy/install-gateway.sh` (run by `deploy.sh`; `--dry-run` prints what it
would apply) installs pinned versions from the upstream release manifests, with no Helm:

| Component | Version | Notes |
| --- | --- | --- |
| Gateway API CRDs | v1.6.3, standard channel | |
| Envoy Gateway | v1.9.2 (Envoy Proxy 1.39) | supports Kubernetes v1.33 to v1.36 |

New kind clusters run Kubernetes 1.36: `terraform/k8s/kind-config.yaml` pins
`kindest/node:v1.36.4` by digest, which needs kind v0.33.0 or newer
(`recreate-kind-cluster.sh` checks). A cluster created earlier keeps its Kubernetes version
until it is recreated; `install-gateway.sh` accepts v1.33 to v1.36.

| Object | Name | Where |
| --- | --- | --- |
| EnvoyProxy | `econ-graph-proxy` | `envoy-gateway-system`: one proxy replica on the node labelled `ingress-ready=true` (kind's control-plane node; the MicroK8s setup scripts label their node), holding host ports 80/443 as hostPorts; Service `econ-graph-gateway-proxy` (ClusterIP) |
| GatewayClass | `econ-graph` | cluster |
| Gateway | `econ-graph-gateway` | namespace `econ-graph-gateway`. Listener `http` (port 80) admits routes from namespaces labelled `econgraph.io/gateway-access: "true"`; listener `https` (port 443) exists only for a public host (see [Public site](#-public-site-cloudflare-tunnel)) |
| ClientTrafficPolicy | `econ-graph-client-headers` | the client address: the TCP peer, or for a peer in the pod CIDRs (the tunnel connector) the rightmost `X-Forwarded-For` entry outside them; drops client-sent `X-Forwarded-Proto/-Host/-Port`, `Forwarded` and `X-Real-IP` |

Routes in `econ-graph` (`k8s/manifests/gateway/routes/`), on host `localhost`:

| HTTPRoute | Paths | Backend |
| --- | --- | --- |
| `econ-graph-frontend` | `/`, `/auth/callback`, `/silent-callback.html` | frontend; sets the security headers (CSP, HSTS, X-Frame-Options; only `/silent-callback.html` may be framed, by this origin) |
| `econ-graph-backend` | `/api`, `/auth`, `/health` | backend; sets `X-Content-Type-Options`, `X-Frame-Options` and HSTS, as do the two GraphQL routes |
| `econ-graph-graphql` | `/graphql` | backend; 20 requests a second per client address (IPv4 and IPv6), bursts of 100, then 429 |
| `econ-graph-data-graphql` | `/data/graphql` | backend `/graphql`, with `Authorization` and `Cookie` removed; same rate limit |
| `econ-graph-idp` | `/idp/realms/econ-graph`, `/idp/resources` | Keycloak |

Every route sets `X-Forwarded-For` to the client address the gateway settled on, so Keycloak
and the backend never see an entry a client wrote.

The rate limit counts per client address: through the hostPorts that is the client's own
address, except that requests from the host itself all arrive from the Docker bridge gateway;
through the tunnel it is the visitor's. In-cluster callers reach the gateway at
`http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/` with a `Host` header
that one of the routes matches. NetworkPolicies match the proxy pod's ports (10080 for listener
`http`, 10443 for `https`), not the Service port 80; `k8s/manifests/network-policy.yaml` lets
every pod in `econ-graph` reach them, and admits the proxy only to the frontend, backend and
Keycloak ports.

Check every route with `./scripts/deploy/test-gateway-routes.sh [--rate-limit] [BASE_URL]`.
Apply the routes with `./scripts/deploy/apply-gateway-routes.sh` (as `deploy.sh` does), not
with `kubectl apply` on the directory: it also keeps the public host's routes, listener and
certificate, or removes them (`PUBLIC_HOST`, below).

While another pod holds host port 80 or 443 (kind's ingress-nginx, or MicroK8s's ingress
add-on), `install-gateway.sh` runs the proxy without host ports and says how to free them.

**Moving a cluster from ingress-nginx.** `./scripts/deploy/cutover-to-gateway.sh --dry-run`
prints the steps; without `--dry-run` it installs the gateway next to ingress-nginx, checks every
route through the proxy Service (the public host's too, over https as the tunnel sends it) and
that a pod in `econ-graph` reaches the proxy, asks for confirmation, removes the Ingresses and
ingress-nginx, moves the proxy onto ports 80/443 and checks the routes again. First it saves
everything a rollback needs, including ingress-nginx's manifest, ConfigMaps, Deployment and
Secrets and the gateway's release manifests, so neither direction needs the network once
ingress-nginx is gone; `--rollback <dir>` restores ingress-nginx, the Ingresses and the saved
NetworkPolicies. When `econ-graph` has any NetworkPolicy, the rollback also adds
`econ-graph-ingress-nginx-rollback`, which admits ingress-nginx on ports 3000, 8080 and 9876
(the next cut-over deletes it). It then checks that the frontend answers and that a GraphQL
query gets data from the database. On a
cluster with the tunnel it keeps the dashboard's old origin name answering (an ExternalName
Service `ingress-nginx/ingress-nginx-controller` pointing at the gateway) and prints the
dashboard change that retires it. Its header lists every step.

## 🌍 Public site (Cloudflare Tunnel)

Production is served at <https://econgraph.com> (no hyphen) through a Cloudflare Tunnel:

```text
browser --https--> Cloudflare edge --tunnel--> cloudflared (pod) --https--> gateway (listener https) --> app, Keycloak
```

TLS for visitors ends at Cloudflare. The `cloudflared` Deployment
(`k8s/manifests/cloudflared.yaml`) dials out to Cloudflare, so no port is opened for it on
the host, and it runs in the cluster, so it comes back with the cluster after a reboot or a
recreate. It reaches the gateway's https listener, so the scheme the gateway passes on is the
connection's own.

**The host name is one setting, `PUBLIC_HOST`.** `deploy.sh` and `restart-k8s-rollout.sh`
apply it through `scripts/deploy/lib/public-host.sh` and
`scripts/deploy/render-public-host.py`. The committed manifests stay localhost-only, and
`http://localhost` keeps working on the host:

| What | Value for `PUBLIC_HOST=econgraph.com` |
| --- | --- |
| Gateway | listener `https` (port 443, no host name, so any TLS server name gets the certificate), certificate `econ-graph-tls` for `econgraph.com` and `www.econgraph.com` from ClusterIssuer `GATEWAY_TLS_ISSUER` (default `econ-graph-selfsigned`: only the tunnel connector sees it, unverified) |
| HTTPRoutes | a separate `<name>-public` route per localhost one, for `econgraph.com` on listener `https` only, labelled `econgraph.io/public-host=true`, without `/admin`, `/api/admin` and `/playground`; the rate limits cover them |
| `www.econgraph.com` and plain http | `econ-graph-www-redirect` and `econ-graph-public-redirect`: permanent redirects to `https://econgraph.com`, same path |
| Keycloak `KC_HOSTNAME` | `https://econgraph.com/idp`; the admin console stays on the port-forward (`KC_HOSTNAME_ADMIN`) |
| Realm client `econ-graph-web` | redirect URIs and web origins for `https://econgraph.com` and `http://localhost` |
| Backend `OIDC_ISSUER` | `https://econgraph.com/idp/realms/econ-graph`; the signing keys are still fetched in-cluster (`OIDC_JWKS_URL`) |
| Backend `CORS_ALLOWED_ORIGINS` | `https://econgraph.com` added to `http://localhost,http://localhost:3000` |

`PUBLIC_HOST=` (empty) removes every piece again: the public routes, the https listener and its
certificate, and cloudflared's replicas. With `PUBLIC_HOST` unset, a cluster that serves no
public host is deployed localhost-only, and on one that does, the scripts stop rather than
take the site down. A non-empty `PUBLIC_HOST` is accepted only on a cluster marked
production (see [Production marker](#production-marker)); anywhere else the deploy stops
before applying anything.

**What comes from client headers.** Only the visitor's address, from `X-Forwarded-For`
(cloudflared sends it, as the Cloudflare edge saw the visitor), and only when the request
comes from a pod address. The gateway drops `X-Forwarded-Proto`, `-Host` and `-Port` that a
client sends and sets the scheme from the connection
(`k8s/manifests/gateway/client-traffic-policy.yaml`). A plain-http request
for `econgraph.com` (say from the LAN to the host's port 80) is redirected to https and
never reaches the app or Keycloak in clear text. Keycloak builds its URLs from the full
`KC_HOSTNAME`. `KC_PROXY_HEADERS=xforwarded` is kept only so that its events and
brute-force detection see the visitor's address (`X-Forwarded-For`, which every route sets to
the address the gateway settled on). Any pod can claim an address that way, so addresses
logged for in-cluster callers are not audit-grade.

The frontend image needs no rebuild per host: `build-images.sh` builds it with the issuer
`/idp/realms/econ-graph`, which the app resolves against the page's origin at runtime.
Images built before ECO-457 have an absolute issuer baked in, so rebuild them once.

www is redirected rather than served because sign-in uses one Keycloak issuer that the
backend checks exactly. A second origin would need its own redirect URIs, web origins and
CORS entry, and would split sessions between hosts.

**Tunnel token.** The token is the Secret `cloudflare-tunnel` (key `token`). Enter it
yourself with hidden input, on the production cluster only, after marking it. It is never
committed, printed, or given to an agent:

```bash
scripts/deploy/set-crawler-keys.sh --tunnel
```

It works on the kind cluster through its own kubectl (`scripts/deploy/lib/kind-env.sh`),
reads from the terminal only, refuses piped input, refuses a cluster that is not marked
production before it asks, and then starts cloudflared through `apply_cloudflared`, for
the public host the cluster serves. `scripts/deploy/create-secrets.sh --prompt-tunnel-token`
writes the token only, with `${KUBECTL:-kubectl}` on its current context. Run by hand
with neither `KUBECTL` nor `KUBECONFIG` set, it uses the kind cluster's own kubectl and
kubeconfig when copies are kept at `~/.cache/econgraph/bin/kubectl` and
`~/.cache/econgraph/kubeconfig-econ-graph`, and says so; plain `kubectl` may be another
cluster's (the microk8s snap wrapper ignores `~/.kube/config`). An exported `KUBECONFIG`
(some runbooks export one) turns that off, which it says, and with only one of the two
files it warns which is missing and uses plain `kubectl`. Don't put the token on a command line
(`CLOUDFLARE_TUNNEL_TOKEN=...`), where shell history keeps it. Without the Secret, `deploy.sh` applies `cloudflared` with 0
replicas and says so. If a cluster that is not marked already holds the Secret,
`create-secrets.sh` keeps it and warns; delete it there with
`scripts/deploy/mark-production-cluster.sh --unmark`.

**Cloudflare dashboard** (Zero Trust > Networks > Tunnels > the tunnel > Public hostnames),
one entry per host name:

| Subdomain | Domain | Path | Service type | URL |
| --- | --- | --- | --- | --- |
| (empty) | `econgraph.com` | (empty) | `HTTPS` | `econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443` |
| `www` | `econgraph.com` | (empty) | `HTTPS` | `econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local:443` |

Under *Additional application settings*, turn *TLS > No TLS Verify* on: the gateway answers
with a self-signed certificate, and this hop stays inside the cluster. Leave *HTTP Settings >
HTTP Host Header* empty, so that cloudflared sends the visitor's host (`econgraph.com` or
`www.econgraph.com`), which is what the public routes match on.

**Checks.** `scripts/deploy/check-public-site.sh --host econgraph.com` runs against the local
gateway without DNS or the tunnel, connecting over https as cloudflared does. It checks:

- the frontend and a `/graphql` search;
- the Keycloak discovery document (issuer `https://econgraph.com/idp/realms/econ-graph`) and the sign-in page;
- CORS and the www redirect;
- that spoofed `X-Forwarded-Proto`, `-Host` and `-Port` headers change nothing, and that plain http is redirected;
- that `/idp/admin`, `/idp/realms/master`, `/metrics`, `/playground`, `/api/admin` and encoded traversals of them are not served.

Add `--via-cloudflare` to go through Cloudflare the way a visitor does. The Prometheus alerts
`CloudflareTunnelDown` and `CloudflareTunnelNoConnections`
(`k8s/monitoring/prometheus-rules-tunnel.yaml`) cover the connector.

**Google sign-in** needs `https://econgraph.com/idp/realms/econ-graph/broker/google/endpoint`
as an authorized redirect URI of the Google OAuth client (ECO-13).

### Production marker

Only the production cluster may run cloudflared or hold the tunnel token. The tunnel is
token-authenticated, and every cloudflared that runs with the token becomes one more
connector of the same tunnel: Cloudflare spreads the site's traffic across all of them
without any error, so a test cluster with the token would silently serve part of the real
site.

A cluster is production while it carries the marker written by:

```bash
./scripts/deploy/mark-production-cluster.sh            # mark this cluster
./scripts/deploy/mark-production-cluster.sh --dry-run  # show what it would do
./scripts/deploy/mark-production-cluster.sh --unmark   # stop cloudflared, remove the marker, offer to delete the token
```

Run bare, it works on the kind cluster `econ-graph` through that cluster's own kubectl and
kubeconfig (`scripts/deploy/lib/kind-env.sh`), context `kind-econ-graph`. For another
cluster, give its kubectl: `KUBECTL=/path/to/kubectl` (its current context). An exported
`KUBECONFIG` other than the kind cluster's (some runbooks export one) also selects plain
`kubectl` with it, and the script says so; unset it to reach the kind cluster. Every run,
dry runs included, shows the kubectl, `KUBECONFIG`, context, API server, nodes and
kube-system UID it reached.

The marker is the ConfigMap `kube-system/econgraph-cluster-role`, with `role: production`
and the UID of the cluster's `kube-system` namespace when it was marked. It counts only
while that UID matches the live one, so a marker copied or restored into another cluster
does not count, and neither does one on a recreated cluster: **mark the production cluster
again after creating or recreating it.** A node or etcd snapshot of production restored
elsewhere keeps the UID, the marker and the token, so run `--unmark` on such a clone before
starting its workloads, or don't restore it while production runs.

Before it changes anything, the script shows the cluster's identity (this host's name, the
nodes, the API server and the kube-system UID) and asks, at a terminal, for the first 8
characters of the kube-system UID: context names such as `kind-econ-graph` are the same on
every machine, the UID is not. It is idempotent (a marked cluster is left alone) and
deletes no data, volumes or clusters. `--unmark` scales cloudflared in `econ-graph` to 0
first, then removes the marker (kept if scaling fails), then offers to delete the Secret
`cloudflare-tunnel`, only if you type `delete`.

**Once on every cluster that is not production** (test, QA, integration, or anything that
ever got the token), run `--unmark` and delete the token when it offers to:

```bash
./scripts/deploy/mark-production-cluster.sh --unmark
```

How the scripts use the marker:

- `k8s/manifests/cloudflared.yaml` is committed with `replicas: 0`, so applying it, or all
  of `k8s/manifests`, never starts the connector, and on production stops it: run
  `deploy.sh` or `restart-k8s-rollout.sh` instead, or leave `cloudflared.yaml` out.
  `apply_cloudflared` in `scripts/deploy/lib/public-host.sh`, used by both scripts, is the
  only thing that scales it up, and only on a marked cluster with a `PUBLIC_HOST` and the
  token. On a cluster that is not marked it applies it with 0 replicas and says why. The
  alert `CloudflareTunnelStoppedOnProduction` fires when a marked cluster's cloudflared asks
  for no replica for 5 minutes.
- A non-empty `PUBLIC_HOST` on a cluster that is not marked stops the deploy before
  anything is applied, so it does not reach `apply_cloudflared` either: a connector already
  running there keeps running until `--unmark` or a deploy with `PUBLIC_HOST=` (empty).
- `create-secrets.sh` refuses to write `CLOUDFLARE_TUNNEL_TOKEN` (or
  `--prompt-tunnel-token`, before it prompts) into a cluster that is not marked.
- The check and every kubectl call of `deploy.sh`, `restart-k8s-rollout.sh`, the deploy
  library and `create-secrets.sh` use the same binary, `${KUBECTL:-kubectl}`, on one
  context: `deploy.sh` and `set-crawler-keys.sh` set `KUBECTL` to the kind cluster's own
  kubectl (`scripts/deploy/lib/kind-env.sh`) and pin `kind-econ-graph`;
  `restart-k8s-rollout.sh` uses plain `kubectl` with the kubeconfig it writes from
  MicroK8s and pins `microk8s`. Each replaces any `CLUSTER_ROLE_CONTEXT` in the
  environment, and scripts they start inherit the pin; `create-secrets.sh` follows an
  inherited one only while it is its kubectl's current context. So the check and the
  writes it allows cannot reach different clusters.
- If the cluster cannot be asked (kube-system or the marker unreadable), it counts as not
  production, and the scripts say so. The deploy then stops with cloudflared left as it is,
  as it does when the Secret `cloudflare-tunnel` cannot be looked up: a failed lookup never
  starts the connector, and never scales a running one to 0 either.

## 📊 Monitoring

### View Pods

```bash
kubectl get pods -n econ-graph
```

### View Services

```bash
kubectl get services -n econ-graph
```

### View Logs

```bash
# Backend logs
kubectl logs -f deployment/econ-graph-backend -n econ-graph

# Frontend logs
kubectl logs -f deployment/econ-graph-frontend -n econ-graph

# Admin UI logs
kubectl logs -f deployment/econ-graph-admin-frontend -n econ-graph
```

### Scale Deployments

```bash
# Scale backend to 3 replicas
kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph

# Scale frontend to 2 replicas
kubectl scale deployment econ-graph-frontend --replicas=2 -n econ-graph
```

## 🔧 Configuration

### Environment Variables

The application is configured via ConfigMap and Secrets:

- **ConfigMap**: Contains non-sensitive configuration
- **Secrets**: Store credentials outside the manifests. Current manifests contain no credentials; Google and Facebook credentials committed in earlier versions remain in Git history and must be rotated. See [Secrets](#secrets)

### Databases

The namespace `econ-graph` runs three PostgreSQL servers, each a one-replica StatefulSet on a
static PersistentVolume over a host directory
([host storage](../docs/deployment/HOST_STORAGE.md)):

| Database | StatefulSet | Service (5432) | Secret | Host directory |
| --- | --- | --- | --- | --- |
| Data, `econ_graph`: sources, series, observations, crawl state | `postgresql` (`postgres-deployment.yaml`) | `data-postgres-service`, and the alias `postgres-service` | `econ-graph-postgres` | `/srv/econgraph/data-plane/pg` |
| App, `econ_graph_app`: users and everything they create | `app-postgres` (`app-postgres.yaml`) | `app-postgres-service` | `econ-graph-app-postgres` | `/srv/econgraph/app-prod/app-pg` |
| Keycloak, `keycloak` | `keycloak-postgres` (`keycloak/postgres.yaml`) | `keycloak-postgres-service` | `econ-graph-keycloak` | `/srv/econgraph/app-prod/keycloak-pg` |

`postgres-service` has the same selector as `data-postgres-service`, for manifests that still
name it. The data and app databases each have a role `econgraph`, created by their init scripts
(`postgres-init.yaml`, `app-postgres-init.yaml`) from the `app-password` key of their Secret.

`create-secrets.sh` writes the connection URLs into the `econ-graph-secrets` Secret:

- `data-database-url`: `econ_graph` on `data-postgres-service`. The crawler-worker and the
  `crawler-migrate` Job read it as `DATA_DATABASE_URL`, and so does the backend (optional there).
- `app-database-url`: the backend reads it as `APP_DATABASE_URL` and applies the app migrations
  to it at startup. It names `econ_graph_app` on
  `app-postgres-service` (`APP_DATABASE=separate`), or, in a cluster deployed before the app
  database existed, the data database's URL (`APP_DATABASE=shared`). An existing cluster keeps
  its setting until `create-secrets.sh` runs with `APP_DATABASE` set; a new Secret gets
  `separate`. Moving an existing cluster to `separate` means moving its app rows into
  `econ_graph_app` first: the script only rewrites the URL.
- `database-url`: the same URL as `data-database-url`. No binary reads `DATABASE_URL`; the key is
  kept for manifests of earlier versions, which read it.

The data migrations go to the data database and the app migrations to the database
`app-database-url` names ([database migrations](../docs/development/MIGRATIONS.md)).

### Migrations

No service applies the data migrations at startup. `deploy.sh` runs the Job `crawler-migrate`
(`manifests/crawler-migrate-job.yaml`: `/app/crawler migrate` from the crawler-worker image,
reading `data-database-url`) after PostgreSQL is ready and before it applies the backend and
the crawler-worker. A failed Job stops the deploy and prints its logs. The Job runs only in a
namespace that owns a data database, i.e. whose `econ-graph-secrets` has the key
`data-database-url`; without that key `deploy.sh` skips it. The backend applies the app
migrations itself at startup. `deploy.sh --dry-run` prints every command of the deploy,
including the Job, and changes nothing.

### Secrets

Secret setup and value retrieval are human-only operations; agents never handle
secret values. See [Secrets Management](../docs/technical/SECRETS_MANAGEMENT.md)
for handling tiers and the current setup workflow.

Current manifests contain no credentials. Google and Facebook credentials committed in earlier versions remain in Git history and must be rotated. `scripts/deploy/create-secrets.sh` creates the Secrets below in the
`econ-graph` namespace from environment variables; `deploy.sh` runs it on every deploy. It is
idempotent: internal passwords are generated with `openssl rand` on the first run and kept on
later runs, optional keys that are unset keep their current value (or are omitted), and obvious
placeholders such as `password` or `your-...` are refused. A variable set to the empty string
counts as unset.

```bash
scripts/deploy/set-crawler-keys.sh  # Run yourself; hidden input, then restarts the worker
```

| Secret | Keys (environment variable) | Used by |
| --- | --- | --- |
| `econ-graph-secrets` | `app-database-url`, `data-database-url`, and `database-url` (the same URL as `data-database-url`); see [Databases](#databases). The retired in-house login keys (`jwt-secret`, `google-*`, `facebook-*`) are removed if an older run left them | backend (`app-database-url`, `data-database-url`), crawler-worker (`data-database-url`), manifests of earlier versions (`database-url`) |
| `econ-graph-postgres` | `postgres-password` (`POSTGRES_PASSWORD`, generated), `app-password` (`APP_DB_PASSWORD`, generated) | the data database (`postgresql`), its init script |
| `econ-graph-app-postgres` | `postgres-password` (`APP_POSTGRES_PASSWORD`, generated), `app-password` (`APP_POSTGRES_APP_PASSWORD`, generated) | the app database (`app-postgres`), its init script |
| `crawler-api-keys` | optional `fred-api-key`, `bls-api-key`, `bea-api-key`, `census-api-key` (`FRED_API_KEY`, ...) | crawler-worker |
| `econ-graph-keycloak` | `admin-username`, `admin-password`, `db-password`, optional `google-idp-client-id`, `google-idp-client-secret` (`KEYCLOAK_*`); the passwords are generated on the first run, the username defaults to `admin` | Keycloak |
| `grafana-admin` | `admin-password` (`GRAFANA_ADMIN_PASSWORD`, generated) | Grafana (user `admin`) |
| `monitoring-auth` | `auth`, one htpasswd line (`MONITORING_BASIC_AUTH`, e.g. `htpasswd -nB admin`, which prompts); only written when set | no manifest reads it |
| `cloudflare-tunnel` | optional `token` (`CLOUDFLARE_TUNNEL_TOKEN`, or hidden input with `--prompt-tunnel-token`), the Cloudflare Tunnel connector token | `cloudflared` (see [Public site](#-public-site-cloudflare-tunnel)) |

Cloudflare DNS-01 uses a separate Secret in `cert-manager`:
`cloudflare-api-token-secret/api-token`. The certificate setup script accepts
hidden terminal input or `CLOUDFLARE_API_TOKEN` from a secure environment source
and writes it through `create-secrets.sh --cloudflare-dns-only`; normal deploys
do not create this credential. Use a token scoped to one zone, with DNS edit
and zone read permissions and no account-wide grants. See the
[DNS-01 setup guide](../docs/deployment/LETSENCRYPT_CLOUDFLARE_DNS01_INTEGRATION.md).

Read a value back with, for example:

```bash
kubectl -n econ-graph get secret grafana-admin -o jsonpath='{.data.admin-password}' | base64 -d
```

Postgres and Grafana read their passwords only when their data volume is first initialised.
Changing `POSTGRES_PASSWORD`, `APP_DB_PASSWORD` or `GRAFANA_ADMIN_PASSWORD` later updates the
Secret but not the running service; also change the password there (for example
`ALTER ROLE econgraph PASSWORD ...` in psql). A Postgres volume created before these Secrets
existed has tables owned by `postgres` and an `econgraph` password that no longer matches: for a
local cluster, recreate it (`./scripts/deploy/teardown.sh`, then deploy again); to keep the data,
dump it with `pg_dump --no-owner` and restore it as `econgraph` into a fresh volume.
`create-secrets.sh` stops when it finds such a volume (a `postgresql-data-postgresql-*` PVC) but no
`econ-graph-postgres` Secret, or an `app-postgres-data-app-postgres-*` PVC but no
`econ-graph-app-postgres` Secret; set `ALLOW_EXISTING_DB_VOLUME=1` to continue anyway. It also
stops, without writing anything, when it cannot query the cluster. `--dry-run` runs every check
and prints the keys it would write, and writes nothing.

#### Upgrading a cluster deployed before these Secrets

Older deploys used committed credentials for Grafana and for the monitoring basic auth, and both
survive a redeploy: Grafana keeps its admin password on its volume, and the old `monitoring-auth`
Secret is never deleted by `kubectl apply`. Replace both; `create-secrets.sh` stops until you do.

```bash
# Grafana: choose a password, store it in grafana-admin, and set it in Grafana.
export GRAFANA_ADMIN_PASSWORD="$(openssl rand -hex 32)"
printf '%s' "$GRAFANA_ADMIN_PASSWORD" |
  kubectl -n econ-graph exec -i grafana-0 -- grafana cli admin reset-admin-password --password-from-stdin
# Monitoring basic auth: a new htpasswd line replaces the committed one.
export MONITORING_BASIC_AUTH="$(htpasswd -nB admin)"  # prompts for the password
./scripts/deploy/create-secrets.sh
```

If you already replaced them another way, run the script once with
`ALLOW_EXISTING_GRAFANA_VOLUME=1` or `ALLOW_MANIFEST_MONITORING_AUTH=1` to skip the check;
later runs don't need them.

## 🗑️ Cleanup

Every database lives on a host directory (`docs/deployment/HOST_STORAGE.md`). No script deletes
those directories or their PersistentVolumes, and each teardown below first dumps the data, app
and Keycloak databases to `~/econgraph-backups/<UTC timestamp>/`. Add `--dry-run` to see every
step without changing anything.

### Remove the app plane

```bash
./scripts/deploy/teardown.sh
```

Deletes the routes, the cloudflared connector, frontend, backend, chart API, Keycloak, and the
`app-postgres` and `keycloak-postgres` StatefulSets and Services, after you type the namespace
name (`--yes` answers that). Their Secrets and PVCs stay, so `deploy.sh` brings the same
databases back. The data plane keeps running: `postgresql`, `data-postgres-service`,
`postgres-service`, `econ-graph-postgres`, the crawler and the backup CronJob.

### Remove the data plane too

```bash
./scripts/deploy/teardown.sh --include-data-plane
```

Also deletes the data Postgres StatefulSet and its Services, the crawler, the backup CronJob,
the Secrets and the namespace with its PVCs, after you type `econ_graph` on a terminal. `--yes`
never answers this prompt, and without a terminal the script stops before changing anything.
The next `deploy.sh` needs the Secrets back first: `restore-kind-databases.sh --from <backup>`
recreates them from the backup the teardown took.

### Remove the cluster

```bash
./scripts/deploy/teardown.sh --include-data-plane --delete-cluster
```

Asks for the cluster name, typed on a terminal, before deleting the cluster. Deleting the
cluster loses what is stored only in its nodes: the in-cluster nightly backups and Grafana's
data. `scripts/deploy/recreate-kind-cluster.sh` replaces a cluster the same way (dump first,
then a confirmation). `terraform destroy` in `terraform/k8s/` does not delete the cluster.

### Released PersistentVolumes

```bash
./scripts/deploy/released-pvs.sh                  # report
./scripts/deploy/released-pvs.sh --delete PV_NAME # one PV object, after typing its name
```

A deleted PVC leaves its PV `Released`. The host-storage PVs (label
`econgraph.io/protected=true`) are re-armed by the next `deploy.sh` and are never deleted by
the script; it deletes only an unprotected PV with policy `Retain`, which leaves the data in
place.

## 🐛 Troubleshooting

### Common Issues

1. **Images not found**: Make sure you've built the images with `./scripts/deploy/build-images.sh`

2. **Database connection failed**: Ensure PostgreSQL is running and accessible

3. **Pods not starting**: Check logs with `kubectl logs <pod-name> -n econ-graph`

4. **Routes not answering**: check the gateway, its proxy and the routes' status:

   ```bash
   kubectl -n econ-graph-gateway get gateway econ-graph-gateway
   kubectl -n envoy-gateway-system get pods
   kubectl -n econ-graph get httproute -o wide
   ./scripts/deploy/test-gateway-routes.sh
   ```

### Debug Commands

```bash
# Describe a pod to see events
kubectl describe pod <pod-name> -n econ-graph

# Check cluster info
kubectl cluster-info

# Check node status
kubectl get nodes

# Check route status
kubectl -n econ-graph describe httproute
```

## 📈 Performance Tuning

### Resource Limits

The deployments include resource requests and limits. Adjust these in the deployment manifests based on your needs:

```yaml
resources:
  requests:
    memory: "256Mi"
    cpu: "100m"
  limits:
    memory: "512Mi"
    cpu: "500m"
```

### Scaling

For production use, consider:

- Horizontal Pod Autoscaler (HPA)
- Vertical Pod Autoscaler (VPA)
- Cluster Autoscaler

## 🔒 Security

### Current Security Measures

- Non-root containers
- Resource limits
- Health checks
- Security headers (frontend)

### Production Recommendations

- Use proper secrets management (e.g., HashiCorp Vault)
- Enable network policies
- Use TLS certificates
- Regular security scanning
- RBAC configuration

## 📚 Additional Resources

- [Kubernetes Documentation](https://kubernetes.io/docs/)
- [kind Documentation](https://kind.sigs.k8s.io/)
- [Terraform Documentation](https://www.terraform.io/docs/)
- [Envoy Gateway](https://gateway.envoyproxy.io/) and the [Gateway API](https://gateway-api.sigs.k8s.io/)

Admin audit client-IP attribution depends on the verified proxy chain. See
[Admin audit and trusted proxies](../docs/security/ADMIN_AUDIT.md) before setting
`TRUSTED_PROXIES`; the manifests default to the backend TCP peer.

Coding agents that need Kubernetes access should use the [read-only agent session](../docs/deployment/AGENT_ACCESS.md). Its namespaced identity can read workloads, pod logs and ConfigMaps but not Secrets, and cannot change anything; deployments and restarts stay with the operator scripts.
