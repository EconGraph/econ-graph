# EconGraph Kubernetes Deployment

This directory contains the manifests for deploying EconGraph to the host-backed kind
(Kubernetes in Docker) cluster. Terraform creates the cluster; the deployment scripts
build images and deploy the application.

## 🚀 Quick Start

### Prerequisites

Run the commands in this guide from the repository root unless a step says otherwise.
Before creating the cluster, prepare the host storage described in
[Host Storage](../docs/deployment/HOST_STORAGE.md#layout): mounted btrfs subvolumes at
`/srv/econgraph/data-plane` and `/srv/econgraph/app-prod`, with their roots owned by the
operator account. Terraform bind-mounts these paths into the worker node; the database
manifests require this storage layout.

- Access to the rootful Docker daemon at `unix:///var/run/docker.sock`, as the operator
  account without sudo. See [Which Docker daemon](../docs/deployment/HOST_STORAGE.md#which-docker-daemon).
- Terraform
- kubectl
- yq and jq, used by deployment and dashboard validation
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
├── release-deploy.sh          # Load, back up, check, deploy and verify a release in order
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

## 🛠️ Setup

After preparing the host storage and tools above, run these steps from the repository root.

### 1. Create Kubernetes Cluster

```bash
terraform -chdir=terraform/k8s init
terraform -chdir=terraform/k8s apply
```

Only on the production cluster (the one that serves the public site), mark it as
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
`./scripts/deploy/release-deploy.sh --tag vX.Y.Z-rc.N` runs the database backup, the load,
the Keycloak backup, the pre-deploy check, `deploy.sh` (with `PUBLIC_HOST` from the host's
deploy config) and the verify in order, from a clean checkout of the tag; see [Deploying a release](../docs/deployment/RELEASE_DEPLOY.md).

## 🔐 Keycloak

`scripts/deploy/deploy.sh` deploys Keycloak from `k8s/manifests/keycloak/` (its own
Postgres, then Keycloak with the `econ-graph` realm from `config/keycloak/`). Its
credentials are never committed: they live in the Secret `econ-graph-keycloak`,
written by `scripts/deploy/create-secrets.sh`, which `deploy.sh` runs. That script generates the
admin and database passwords on its first run and keeps them afterwards. `deploy.sh` skips Keycloak while the Secret
does not exist.

```bash
# Optional, for Google sign-in (redirect URI https://<MAIN_HOST>/idp/realms/econ-graph/broker/google/endpoint,
# or http://localhost/idp/... without a PUBLIC_HOST):
export KEYCLOAK_GOOGLE_CLIENT_ID=... KEYCLOAK_GOOGLE_CLIENT_SECRET=...
./scripts/deploy/create-secrets.sh
./scripts/deploy/deploy.sh
```

- Issuer: `http://localhost/idp/realms/econ-graph`, or `https://<MAIN_HOST>/idp/realms/econ-graph`
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
- The backend, Keycloak and the realm import read the anonymous catalog reader's credential
  from `econ-graph-keycloak/anonymous-client-secret`. A Secret written before that key
  existed stops `deploy.sh` before it applies anything, with
  `Run scripts/deploy/create-secrets.sh`. Run it (every existing value is kept and the key is
  generated), then deploy again:
  `./scripts/deploy/create-secrets.sh && ./scripts/deploy/deploy.sh`.
- The realm import also stops while the `catalog-public` role holds anything but
  `econ-graph-api` `catalog.*:read` roles; remove the composites its log names, in the admin
  console, and deploy again. See [catalog access](../config/keycloak/README.md#catalog-access-including-anonymous-visitors).

## 🌐 Accessing the Application

After deployment, the application will be available at:

- **Frontend**: <http://localhost>
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

The public site is served through a Cloudflare Tunnel, at the main address `MAIN_HOST`, by
default <https://www.econgraph.com> for `PUBLIC_HOST=econgraph.com` (no hyphen):

```text
browser --https--> Cloudflare edge --tunnel--> cloudflared (pod) --https--> gateway (listener https) --> app, Keycloak
```

TLS for visitors ends at Cloudflare. The `cloudflared` Deployment
(`k8s/manifests/cloudflared.yaml`) dials out to Cloudflare, so no port is opened for it on
the host, and it runs in the cluster, so it comes back with the cluster after a reboot or a
recreate. It reaches the gateway's https listener, so the scheme the gateway passes on is the
connection's own.

**`PUBLIC_HOST` is the base domain; `MAIN_HOST` is the main address.** With
`PUBLIC_HOST=econgraph.com`, `MAIN_HOST` defaults to `www.econgraph.com`. Set
`MAIN_HOST=econgraph.com` explicitly to use the apex instead. `deploy.sh` and
`restart-k8s-rollout.sh` apply these settings through
`scripts/deploy/lib/public-host.sh` and `scripts/deploy/render-public-host.py`.
The committed manifests stay localhost-only. `http://localhost` still serves the app
on the host, but a public deployment accepts sign-in redirects only on its main HTTPS
origin.

With `PUBLIC_HOST=econgraph.com`, for each `MAIN_HOST`:

| What | `MAIN_HOST=www.econgraph.com` (the default) | `MAIN_HOST=econgraph.com` |
| --- | --- | --- |
| Gateway | listener `https` (port 443, no host name, so any TLS server name gets the certificate), certificate `econ-graph-tls` for `econgraph.com` and `www.econgraph.com` from ClusterIssuer `GATEWAY_TLS_ISSUER` (default `econ-graph-selfsigned`: only the tunnel connector sees it, unverified) | the same |
| HTTPRoutes | a separate `<name>-public` route per localhost one, for `www.econgraph.com` on listener `https` only, labelled `econgraph.io/public-host=true`, without `/admin`, `/api/admin` and `/playground`; the rate limits cover them | the same, for `econgraph.com` |
| Redirects (`econ-graph-www-redirect`, `econ-graph-public-redirect`), path and query kept | https on `econgraph.com` and http on both names to `https://www.econgraph.com`: 302 until `MAIN_HOST_PERMANENT` names www (`switch-main-host.sh --finalize`), then 301; this includes http on `www.econgraph.com` itself | https on `www.econgraph.com` and http on both names to `https://econgraph.com`: 301 |
| Keycloak `KC_HOSTNAME` and realm `frontendUrl` | `https://www.econgraph.com/idp`; the admin console stays on the port-forward (`KC_HOSTNAME_ADMIN`) | `https://econgraph.com/idp` |
| Realm client `econ-graph-web` | redirect URIs and web origins for `https://www.econgraph.com` only (`KC_WEB_BASE_URL` and `KC_WEB_E2E_BASE_URL`) | for `https://econgraph.com` only |
| Backend `OIDC_ISSUER` | `https://www.econgraph.com/idp/realms/econ-graph`; the signing keys are still fetched in-cluster (`OIDC_JWKS_URL`) | `https://econgraph.com/idp/realms/econ-graph` |
| Backend `CORS_ALLOWED_ORIGINS` | `https://econgraph.com,https://www.econgraph.com,http://localhost` | the same |

Without a public host, the realm client allows `http://localhost` and CORS
`http://localhost,http://localhost:3000`.

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
An empty `VITE_OIDC_ISSUER` only warns and builds an image with sign-in hidden. The app's canonical link uses the
main page origin and path, without a query or fragment.

The other public name redirects to the main host before the app starts, so sign-in
uses one Keycloak issuer that the backend checks exactly. Allowing both public CORS origins does not create a
second issuer. Changing the main host changes the issuer and forces users to sign in
again; existing sessions and old-issuer tokens are no longer usable. See
[Main public address](../docs/deployment/MAIN_HOST.md) for the operator commands and
rollback.

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
writes the token only, with `KUBECTL` on its current context. Run by hand without
`KUBECTL`, it uses the kind cluster's own kubectl kept at `~/.cache/econgraph/bin/kubectl`,
with an exported `KUBECONFIG` if there is one (some runbooks export one) and
`~/.cache/econgraph/kubeconfig-econ-graph` if not, and says which. It never falls back to
plain `kubectl`, which may be another cluster's (the microk8s snap wrapper ignores
`KUBECONFIG` and `~/.kube/config`): when a copy is missing it stops and says which, and how
to get it (any `scripts/deploy/lib/kind-env.sh` script keeps them) or to set `KUBECTL`.
Don't put the token on a command line
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
gateway without DNS or the tunnel, connecting over https as cloudflared does. It checks the main
host the deploy config names for that domain (`MAIN_HOST`, else `www.econgraph.com`), so it
needs no flag before or after a switch; `--main-host` or `MAIN_HOST` overrides it. It checks:

- the frontend and a `/graphql` search;
- the Keycloak discovery document (issuer `https://<MAIN_HOST>/idp/realms/econ-graph`) and the sign-in page;
- both allowed public CORS origins, refused-origin preflights (the backend's 403), and the full redirect matrix (301 or 302, see [Main public address](../docs/deployment/MAIN_HOST.md#redirect-status));
- that spoofed `X-Forwarded-Proto`, `-Host` and `-Port` headers change nothing, and that plain http is redirected (locally by the gateway, straight to the main host; through Cloudflare by its Always Use HTTPS, a 301 to https on the same name, whose https answer is then checked);
- that `/idp/admin`, `/idp/realms/master`, `/metrics`, `/playground`, `/api/admin` and encoded traversals of them are not served.

Add `--via-cloudflare` to go through Cloudflare the way a visitor does. The Prometheus alerts
`CloudflareTunnelDown` and `CloudflareTunnelNoConnections`
(`k8s/monitoring/prometheus-rules-tunnel.yaml`) cover the connector.

**Google sign-in** needs `https://<MAIN_HOST>/idp/realms/econ-graph/broker/google/endpoint`
as an authorized redirect URI of the Google OAuth client (ECO-13); keep both names
authorized while a rollback of the main host is possible.

**Facebook sign-in** is disabled ([ECO-470](https://linear.app/econgraph/issue/ECO-470)):
`scripts/deploy/set-crawler-keys.sh --facebook` refuses, and the realm's `facebook`
provider stays off while `econ-graph-keycloak` lacks its two keys. See
[Facebook sign-in](../config/keycloak/README.md#facebook-sign-in) for the provider, the
Meta app URLs and how to remove keys a cluster still holds.

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

A release lab ([docs/deployment/RELEASE_LAB.md](../docs/deployment/RELEASE_LAB.md)) carries
the ConfigMap `kube-system/econgraph-lab-owner`, and `mark-production-cluster.sh` refuses
to mark it. A cluster carrying both that ConfigMap and a valid production marker has
conflicting markers: `cluster_is_marked_production` reports it as unknown (status 2), as
when the cluster can't be asked, and the scripts refuse to act on it.

Scripts that delete a cluster (`recreate-kind-cluster.sh`, `migrate-to-host-storage.sh`)
read the marker more broadly, so a damaged one still protects: any ConfigMap
`kube-system/econgraph-cluster-role` counts as a production marker there, whatever its role
or UID, an empty one included, except one whose role is exactly `none`, with no
`kube-system-uid` or one without whitespace. A role or UID with surrounding whitespace or a
newline in it counts as production. `--unmark` removes any such ConfigMap, empty ones too.

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
  `app-postgres-service` (`APP_DATABASE=separate`), or the data database's URL
  (`APP_DATABASE=shared`) while the app records are still in `econ_graph`, as on a cluster
  deployed before the app database existed. With `APP_DATABASE` unset, the key's current
  setting is kept; a cluster without `econ-graph-secrets` gets `separate`; an existing Secret
  without the key needs `APP_DATABASE`. `create-secrets.sh` refuses `separate` while
  `econ_graph` holds the app tables, and `shared` once they are only in `econ_graph_app`
  (`scripts/deploy/lib/app-database-guard.sh`). `scripts/deploy/convert-app-data.sh` moves
  the app rows and rewrites the key ([the runbook](../docs/deployment/APP_DATA_CONVERSION.md)).
- `database-url`: the same URL as `data-database-url`. No binary reads `DATABASE_URL`; the key is
  kept for manifests of earlier versions, which read it.

The data migrations go to the data database and the app migrations to the database
`app-database-url` names ([database migrations](../docs/development/MIGRATIONS.md)).

### Migrations

No service applies the data migrations at startup. `deploy.sh` runs the Job `crawler-migrate`
(`manifests/crawler-migrate-job.yaml`: `/app/crawler migrate` from the crawler-worker image,
reading `data-database-url`) after PostgreSQL is ready and before it applies the backend and
the crawler-worker. A failed Job stops the deploy and prints its logs. Without the key
`data-database-url` in `econ-graph-secrets`, `deploy.sh` stops before applying the backend:
with sign-in on and no data database, the backend refuses to start (it could not apply catalog
grants to annotations). `scripts/deploy/create-secrets.sh` writes the key. The backend applies the app
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
| `crawler-api-keys` | optional `fred-api-key`, `bls-api-key`, `bea-api-key`, `census-api-key`, `wto-api-key` (`FRED_API_KEY`, ...) | crawler-worker |
| `econ-graph-keycloak` | `admin-username`, `admin-password`, `db-password`, `anonymous-client-secret`, optional `google-idp-client-id`, `google-idp-client-secret` (`KEYCLOAK_*`), and `facebook-idp-client-id`, `facebook-idp-client-secret`, kept if present but never written: Facebook sign-in is disabled and `KEYCLOAK_FACEBOOK_*` is refused; the passwords and the anonymous client secret are generated on the first run, the username defaults to `admin` | Keycloak, the realm import and the backend (`anonymous-client-secret`) |
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
`ALTER ROLE econgraph PASSWORD ...` in psql).

A Postgres volume created before these Secrets existed may have tables owned by `postgres`
or an `econgraph` password that does not match the Secret. Reconcile the existing database's
password and object ownership, or preserve the data with `pg_dump --no-owner` and restore it
as `econgraph` into a fresh destination. Omitting owners from the dump does not choose the
restore owner: restoring as `postgres` alone leaves the tables owned by `postgres`. See
[restore ownership](../docs/deployment/POSTGRES_BACKUP_RESTORE.md#ownership) for the role and
extension handling, and [host-storage migration](../docs/deployment/HOST_STORAGE.md#moving-the-live-deployment)
and [production recovery](../docs/deployment/HOST_STORAGE.md#restoring-production-from-a-backup)
for moving or recovering host-backed databases with their saved Secrets. Teardown and
redeployment retain the host data; they do not reset database credentials or ownership.

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

Every database lives on a [host directory](../docs/deployment/HOST_STORAGE.md). No script deletes
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
data. `scripts/deploy/recreate-kind-cluster.sh` replaces a cluster with a confirmation, then a
dump. On a cluster with a production marker (any ConfigMap `kube-system/econgraph-cluster-role`
except `none` or `none <uid>`, exactly; see [Production marker](#production-marker)), or one
whose marker can't be read, the confirmation is the first 8 characters of its kube-system UID, typed or given
as `--confirm-production`; `--yes` does not answer it. `terraform destroy` in `terraform/k8s/`
does not delete the cluster.

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

- Follow the [human-only Secret setup and rotation workflow](../docs/technical/SECRETS_MANAGEMENT.md#current-workflow)
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

### Disposable branch environments

A branch runs in `econ-graph-br-<slug>` with its own app database, Keycloak,
Keycloak database and users. The slug combines a readable branch name with a
hash, so branch names that normalize to the same text do not share a namespace.
The frontend is served at `http://<slug>.localhost`; `/graphql` writes only to the
branch app database, while `/data/graphql` reads the shared primary through the
Gateway. Browser credentials are removed on that data route. The primary
backend's authorization guards reject anonymous mutations; removing credentials
is not a substitute for those guards.

Catalog access in a branch follows two Keycloaks. Data read through
`/data/graphql` is read anonymously, so every branch visitor, signed in or not,
sees the catalogs the primary grants anonymous visitors. The branch Keycloak's
realm import sets up catalog roles, including its own anonymous catalog
reader, whose credential `up` generates with the other branch Secrets.
Changes to catalog grants in the primary's Keycloak do not reach branches. For
annotations and their comments, the branch backend asks the primary
anonymously which catalog a series belongs to, then applies the branch caller's
own grants. A series the primary grants only to signed-in users therefore cannot be
annotated in a primary-mode branch: it is refused as if it did not exist. In
local-data mode the branch backend checks its own data database and only the
branch's grants apply. Branch pods are not Prometheus scrape targets:
`branch-isolation` admits no traffic from `econ-graph`, where Prometheus runs.

The shared Gateway must already be installed and the primary data plane must be
available. **An enforcing NetworkPolicy CNI is required.** CNI names or installed NetworkPolicy objects alone do not prove enforcement.
`up` creates temporary probe pods only in the branch namespace and checks positive connectivity, an isolated client's
negative connectivity, blocked direct primary access and backend-only gateway
access before creating any credentials or database workloads. A failed probe
stops deployment and cleans up its pods; the empty branch namespace and policies
remain for diagnosis. The script never installs a CNI or changes cluster-wide configuration.
For local crawler egress, the cluster's pod/service networks must use private
IPv4 ranges; HTTPS egress excludes private, loopback, link-local and reserved
ranges, and no IPv6 internet egress is granted.

The operator needs Python 3.11+, PyYAML, kubectl and a kubeconfig authorized for
branch namespaces, reading the primary Service metadata and creating exactly the
branch's ReferenceGrant in `econ-graph`. Use an explicit context on every command;
the script never switches the current context or reads production Secrets.
It never runs a plain `kubectl` from `PATH`, and chooses one as the other deploy
scripts do (`cluster_role_default_kubectl` in `scripts/deploy/lib/cluster-role.sh`).
`KUBECTL` is used when set. Otherwise the kind cluster's kubectl kept in
`~/.cache/econgraph/bin` is used, with `KUBECONFIG` when it is set and with
`~/.cache/econgraph/kubeconfig-econ-graph` when it is not. Without them the
script stops before contacting any cluster. A `--dry-run` and `build-frontend`
need no kubectl.

```bash
scripts/deploy/deploy-branch.sh build-frontend feature/my-change \
  --context kind-econ-graph --frontend-image econ-graph-frontend:my-change
# build-frontend builds on the kind cluster's Docker daemon (ECONGRAPH_DOCKER_HOST,
# default unix:///var/run/docker.sock), not the caller's DOCKER_HOST. Build the
# backend from this checkout on the same daemon, then load both into kind.
# load-kind-images.sh compares image IDs, so a stale image under the same tag
# is replaced.
DOCKER_HOST="${ECONGRAPH_DOCKER_HOST:-unix:///var/run/docker.sock}" \
  docker build -t econ-graph-backend:my-change backend
scripts/deploy/load-kind-images.sh \
  econ-graph-backend:my-change econ-graph-frontend:my-change
scripts/deploy/deploy-branch.sh up feature/my-change --context kind-econ-graph \
  --backend-image econ-graph-backend:my-change \
  --frontend-image econ-graph-frontend:my-change --dry-run
# Remove --dry-run to deploy after reviewing the plan.
scripts/deploy/deploy-branch.sh list --context kind-econ-graph
scripts/deploy/deploy-branch.sh down feature/my-change --context kind-econ-graph
```

`build-frontend` sets the branch OIDC issuer and `VITE_DATA_GRAPHQL_URL=/data/graphql`
at build time. Runtime environment variables cannot change an existing Vite
bundle. Rebuild the frontend for each branch hostname. The frontend image must
match the branch passed to `up`. No Google or Facebook credentials or seeded QA
users are copied into branches. To create two test users, port-forward the branch
Keycloak Service to port 18080, open `http://localhost:18080/idp/admin`, and create
them in the `econ-graph` realm. The operator retrieves the generated admin
credentials from that branch's `econ-graph-keycloak` Secret privately. This does
not enable the password grant or change production user seeding.

The branch Namespace has `econgraph.io/env=branch`, `econgraph.io/branch=<slug>`,
`econgraph.io/data-mode=primary` and `econgraph.io/gateway-access=true`. Network
policies allow namespace-local traffic and DNS; only backend pods may call the
shared gateway. Direct access to the primary backend or database is denied. All
branch Services are ClusterIP, all database PVCs are disposable local-path claims,
and branches never bind production PVs. The sole primary-namespace write is
`ReferenceGrant/data-graphql-from-<slug>`, limited to the primary backend Service.
`down` requires the typed namespace, checks ownership and removes only that grant
and namespace. It never calls `kind delete`, removes a shared controller or
changes host storage. Deleting a branch permanently discards its users and app
records through its disposable PVCs. Production data is never deleted. A
namespace-local ConfigMap lock prevents concurrent deploy/delete operations from
racing credentials or restores. If an operator kills the process, inspect the
branch before manually removing `branch-deploy-lock` and retrying.

For crawler development, use `up ... --local-data --data-image
<worker>:v<release> --seed-dir <verified-primary-dump-directory>`. The directory
must contain `VERIFIED`, `SHA256SUMS` and `econ_graph.dump`; the dump checksum is
checked and dumps still containing app tables are refused. If backups are
encrypted, the operator decrypts the data-only dump into a private staging
bundle first. With the encrypted host-bundle tooling installed, use
`open-backup.sh <bundle> -- scripts/deploy/deploy-branch.sh up <branch> ... --seed-dir @BACKUP@`;
the wrapper authenticates the bundle and holds plaintext in private tmpfs only
for that command. The branch tool receives no age identity. `pg_restore` is
required locally for archive inspection and must support the archive version:
production PostgreSQL 18 dumps need a PostgreSQL 18-compatible client. After
installing that client, select it for this command through `PATH` (for example,
`PATH=/usr/lib/postgresql/18/bin:$PATH scripts/deploy/deploy-branch.sh ...`)
instead of changing the system-wide client default. The dump
is streamed exclusively into the branch's data Postgres before data migrations
run. An existing nonempty, unmarked database is refused. Subsequent deploys
require the same seed checksum and never restore over existing data. The data
plane always runs a tagged release image, independently of the branch backend
image. `/data/graphql` then routes to the branch backend and no ReferenceGrant
is created. The scheduler is disabled; deliberate test crawls may be queued by
a branch administrator. Source API keys may be entered privately into the branch
`crawler-api-keys` Secret only in this mode. A branch's data mode cannot be changed
in place; delete/recreate the disposable namespace to switch modes.

`k8s/base` is a namespace-free application base shared by the production rendering
and branch renderer. `kubectl kustomize k8s/overlays/production` renders the same
application objects as the existing production manifest inputs; CI checks this
parity. Production's existing ordered deployment remains responsible for shared
controllers, host PVs, monitoring, credentials and Jobs. They are deliberately
outside the application base and are never installed by a branch. The checked
`k8s/overlays/branch` is an example overlay for inspection with `kubectl kustomize`;
`deploy-branch.sh` renders actual branch names, images and data mode. After changing
a production application manifest, refresh the base with
`python3 scripts/deploy/render-branch.py --refresh-base` and review the parity test.

Before relying on a new cluster, validate two branches side by side: sign in with
two manually created users, read real series, create an annotation in each branch,
and confirm the records remain in their respective app databases. Verify that
above-limit GraphQL requests receive 429s and that each route's BackendTrafficPolicy
is accepted; the shared Gateway's burst policy applies to branch routes too.
Finally remove one branch and confirm the other branch and primary row counts are
unchanged. Offline rendering/guard tests cannot replace these runtime checks.

## Tunnel origin certificate verification

The optional private-CA flow authenticates the tunnel connector's TLS connection to the
gateway. Stage only its public trust certificate with
`scripts/deploy/stage-tunnel-origin-ca.sh --context <context> --dry-run`, then follow
[Tunnel origin TLS](../docs/deployment/TUNNEL_ORIGIN_TLS.md) for issuer selection,
certificate checks, the remote dashboard transition and rollback. Staging does not change
the gateway issuer or disable the dashboard's existing No TLS Verify setting. A deploy whose
gateway issuer is `econ-graph-origin-ca` runs the staging itself before issuing the
certificate. The issuer comes from `GATEWAY_TLS_ISSUER` in the environment, else the
host's deploy config (`ECONGRAPH_DEPLOY_ENV`, else `$ECONGRAPH_CONFIG_DIR/deploy.env`, else
`~/.config/econgraph/deploy.env`) when it matches the existing
certificate's issuer or there is none, else the existing issuer; a config line that would
change the issuer is refused without an explicit choice.
