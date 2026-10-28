# EconGraph Kubernetes Deployment

This directory contains all the necessary files to deploy EconGraph to a local Kubernetes cluster using Terraform and kind (Kubernetes in Docker).

## 🚀 Quick Start

### Prerequisites

- Docker
- Terraform
- kubectl
- kind (will be installed automatically)

### One-Command Setup

```bash
./scripts/deploy/setup-local-k8s.sh
```

This script will:

1. Create a local Kubernetes cluster using Terraform
2. Build Docker images for frontend and backend
3. Deploy the application to the cluster
4. Set up ingress and services

## 📁 Directory Structure

```text
k8s/
├── manifests/           # Kubernetes deployment manifests
│   ├── namespace.yaml   # Namespace definition
│   ├── configmap.yaml   # Configuration
│   ├── postgres.yaml    # PostgreSQL service
│   ├── backend-deployment.yaml  # Backend deployment
│   ├── frontend-deployment.yaml # Frontend deployment
│   ├── admin-frontend-deployment.yaml # Admin UI deployment
│   ├── admin-frontend-service.yaml    # Admin UI service
│   ├── ingress.yaml     # Ingress configuration
│   └── graphql-ingress.yaml # /graphql, rate-limited per client IP
└── README.md           # This file

terraform/k8s/          # Terraform configuration
├── main.tf            # Main Terraform configuration
├── variables.tf       # Variables
└── outputs.tf         # Outputs

scripts/deploy/         # Deployment scripts
├── setup-local-k8s.sh # Complete setup script
├── build-images.sh    # Build Docker images
├── create-secrets.sh  # Create the Secrets (run by deploy.sh)
├── deploy.sh          # Deploy to K8s
├── mark-production-cluster.sh # Mark the cluster that serves the public site
└── teardown.sh        # Cleanup
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

## 🔐 Keycloak

`scripts/deploy/deploy.sh` deploys Keycloak from `k8s/manifests/keycloak/` (its own
Postgres, then Keycloak with the `econ-graph` realm from `config/keycloak/`). Its
credentials are never committed: they live in the Secret `econ-graph-keycloak`,
written by `scripts/deploy/create-secrets.sh` (from the secrets PR, which also makes
`deploy.sh` run it). That script generates the admin and database passwords on its
first run and keeps them afterwards. `deploy.sh` skips Keycloak while the Secret
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
  and `/idp/resources` are routed by the ingress; the master realm is not reachable
  from outside the cluster.
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

**Note**: `k8s/manifests/ingress.yaml` and `k8s/manifests/keycloak/ingress.yaml` route on
`host: localhost`, so no `/etc/hosts` entry is needed. The admin frontend isn't deployed by
`deploy.sh`, and `/admin` has no backend in this release (ECO-242).

## 🌍 Public site (Cloudflare Tunnel)

Production is served at <https://econgraph.com> (no hyphen) through a Cloudflare Tunnel:

```text
browser --https--> Cloudflare edge --tunnel--> cloudflared (pod) --https--> ingress-nginx --> app, Keycloak
```

TLS for visitors ends at Cloudflare. The `cloudflared` Deployment
(`k8s/manifests/cloudflared.yaml`) dials out to Cloudflare, so no port is opened for it on
the host, and it runs in the cluster, so it comes back with the cluster after a reboot or a
recreate. It reaches ingress-nginx over https, so the scheme ingress-nginx passes on is the
connection's own.

**The host name is one setting, `PUBLIC_HOST`.** `deploy.sh` and `restart-k8s-rollout.sh`
apply it through `scripts/deploy/lib/public-host.sh` and
`scripts/deploy/render-public-host.py`. The committed manifests stay localhost-only, and
`http://localhost` keeps working on the host:

| What | Value for `PUBLIC_HOST=econgraph.com` |
| --- | --- |
| Ingresses | a separate `<name>-public` Ingress per localhost one, for `econgraph.com`, labelled `econgraph.io/public-host=true`, with `force-ssl-redirect` and without `/admin`, `/api/admin` and `/playground` |
| `www.econgraph.com` | permanent redirect to `https://econgraph.com`, same path (ingress-nginx `from-to-www-redirect`) |
| Keycloak `KC_HOSTNAME` | `https://econgraph.com/idp`; the admin console stays on the port-forward (`KC_HOSTNAME_ADMIN`) |
| Realm client `econ-graph-web` | redirect URIs and web origins for `https://econgraph.com` and `http://localhost` |
| Backend `OIDC_ISSUER` | `https://econgraph.com/idp/realms/econ-graph`; the signing keys are still fetched in-cluster (`OIDC_JWKS_URL`) |
| Backend `CORS_ALLOWED_ORIGINS` | `https://econgraph.com` added to `http://localhost,http://localhost:3000` |
| ingress-nginx controller ConfigMap | `enable-real-ip`, `forwarded-for-header: CF-Connecting-IP`, `proxy-real-ip-cidr: <the nodes' pod CIDRs>` |

`PUBLIC_HOST=` (empty) removes every piece again: the public Ingresses, the controller
settings, and cloudflared's replicas. With `PUBLIC_HOST` unset, a cluster that serves no
public host is deployed localhost-only, and on one that does, the scripts stop rather than
take the site down. A non-empty `PUBLIC_HOST` is accepted only on a cluster marked
production (see [Production marker](#production-marker)); anywhere else the deploy stops
before applying anything.

**What comes from client headers.** Only the visitor's address, from `CF-Connecting-IP`,
and only when the request comes from a pod address (cloudflared). `use-forwarded-headers`
is off, so ingress-nginx overwrites `X-Forwarded-Proto`, `-Host` and `-Port` with the
connection's own scheme, `Host` and port, whatever a client sends. A plain-http request
for `econgraph.com` (say from the LAN to the host's port 80) is redirected to https and
never reaches the app or Keycloak in clear text. Keycloak builds its URLs from the full
`KC_HOSTNAME`. `KC_PROXY_HEADERS=xforwarded` is kept only so that its events and
brute-force detection see the visitor's address (`X-Forwarded-For`, which ingress-nginx
sets). Any pod can claim an address in `CF-Connecting-IP`, so addresses logged for
in-cluster callers are not audit-grade.

The frontend image needs no rebuild per host: `build-images.sh` builds it with the issuer
`/idp/realms/econ-graph`, which the app resolves against the page's origin at runtime.
Images built before ECO-457 have an absolute issuer baked in, so rebuild them once.

www is redirected rather than served because sign-in uses one Keycloak issuer that the
backend checks exactly. A second origin would need its own redirect URIs, web origins and
CORS entry, and would split sessions between hosts.

The old `k8s/manifests/ssl-ingress.yaml` is retired. It served econgraph.com and www without
the `/graphql` rate limits, and the deploy scripts delete its Ingress
(`econ-graph-ssl-ingress`) if a cluster still has it.

**Tunnel token.** The token is the Secret `cloudflare-tunnel` (key `token`). Enter it
yourself with hidden input, on the production cluster only, after marking it. It is never
committed, printed, or given to an agent:

```bash
scripts/deploy/create-secrets.sh --prompt-tunnel-token
```

It reads from the terminal only, refuses piped input, and refuses a cluster that is not
marked production. Don't put the token on a command line (`CLOUDFLARE_TUNNEL_TOKEN=...`),
where shell history keeps it. Without the Secret, `deploy.sh` applies `cloudflared` with 0
replicas and says so. If a cluster that is not marked already holds the Secret,
`create-secrets.sh` keeps it and warns; delete it there with
`scripts/deploy/mark-production-cluster.sh --unmark`.

**Cloudflare dashboard** (Zero Trust > Networks > Tunnels > the tunnel > Public hostnames),
one entry per host name:

| Subdomain | Domain | Path | Service type | URL |
| --- | --- | --- | --- | --- |
| (empty) | `econgraph.com` | (empty) | `HTTPS` | `ingress-nginx-controller.ingress-nginx.svc.cluster.local:443` |
| `www` | `econgraph.com` | (empty) | `HTTPS` | `ingress-nginx-controller.ingress-nginx.svc.cluster.local:443` |

Under *Additional application settings*, turn *TLS > No TLS Verify* on: ingress-nginx
answers with its default certificate, and this hop stays inside the cluster. Leave *HTTP
Settings > HTTP Host Header* empty, so that cloudflared sends the visitor's host
(`econgraph.com` or `www.econgraph.com`), which is what the ingress rules match on. When
ingress-nginx gives way to the Gateway API (ECO-433), the service URL changes to the
Gateway's Service.

**Checks.** `scripts/deploy/check-public-site.sh --host econgraph.com` runs against the local
ingress without DNS or the tunnel, connecting over https as cloudflared does. It checks:

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

Which cluster it reaches:

- `mark-production-cluster.sh` and `create-secrets.sh` work on the current context of
  `KUBECTL` and its `KUBECONFIG`. Run by hand without `KUBECTL`, they use the kind
  cluster's own kubectl kept at `~/.cache/econgraph/bin/kubectl`, with an exported
  `KUBECONFIG` if there is one (some runbooks export one; unset it to get the kind
  cluster's) and `~/.cache/econgraph/kubeconfig-econ-graph` if not, and say which. They
  never fall back to plain `kubectl`, which may be another cluster's (the microk8s snap
  wrapper ignores `KUBECONFIG` and `~/.kube/config`): when a copy is missing they stop and
  say which, with the commands that copy both out of the cluster on the rootful Docker
  daemon (`docker cp` of its kubectl, `kind get kubeconfig`), or set `KUBECTL`.
- `deploy.sh` switches the current context of `${KUBECTL:-kubectl}`'s kubeconfig to
  `kind-econ-graph` and pins it (`create-secrets.sh`, run from it, stays on it). So
  `KUBECONFIG` has to be the production cluster's for that reason too: the context is
  switched in whichever kubeconfig is in effect. `create-secrets.sh` follows such an
  inherited context (`CLUSTER_ROLE_CONTEXT`) only while it is its kubectl's current
  context; otherwise it stops before asking for or writing anything.
- `restart-k8s-rollout.sh` uses plain `kubectl` with the kubeconfig it writes from
  MicroK8s (`~/.kube/config`), whatever `KUBECTL` or `KUBECONFIG` says, and pins `microk8s`.

To choose the cluster explicitly, set both, for example:

```bash
KUBECTL=~/.cache/econgraph/bin/kubectl KUBECONFIG=~/.cache/econgraph/kubeconfig-econ-graph \
  ./scripts/deploy/mark-production-cluster.sh
```

Every run of the mark script, dry runs included, shows the kubectl, `KUBECONFIG`, context,
API server, nodes and kube-system UID it reached; check them before typing the UID.

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
  context: `deploy.sh` pins `kind-econ-graph` and `restart-k8s-rollout.sh` pins `microk8s`
  (replacing any `CLUSTER_ROLE_CONTEXT` in the environment), and scripts they start
  inherit it. So the check and the writes it allows cannot reach different clusters.
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
- **Secrets**: Store credentials outside the manifests. Current manifests contain no credentials; Google and Facebook credentials committed before #227 remain in Git history and must be rotated. See [Secrets](#secrets)

### Database Connection

PostgreSQL runs in the cluster (`postgres-deployment.yaml`). The backend and crawler-worker connect as the
`econgraph` user through `DATABASE_URL`, which `create-secrets.sh` builds and stores in the
`econ-graph-secrets` Secret.

### Secrets

Current manifests contain no credentials. Google and Facebook credentials committed before #227 remain in Git history and must be rotated. `scripts/deploy/create-secrets.sh` creates the Secrets below in the
`econ-graph` namespace from environment variables; `deploy.sh` runs it on every deploy. It is
idempotent: internal passwords are generated with `openssl rand` on the first run and kept on
later runs, optional keys that are unset keep their current value (or are omitted), and obvious
placeholders such as `password` or `your-...` are refused. A variable set to the empty string
counts as unset.

```bash
FRED_API_KEY=... ./scripts/deploy/create-secrets.sh
kubectl -n econ-graph rollout restart deployment/econ-graph-backend deployment/crawler-worker
```

| Secret | Keys (environment variable) | Used by |
| --- | --- | --- |
| `econ-graph-secrets` | `database-url` (built from `app-password`); the retired in-house login keys (`jwt-secret`, `google-*`, `facebook-*`) are removed if an older run left them | backend, crawler-worker (`database-url`) |
| `econ-graph-postgres` | `postgres-password` (`POSTGRES_PASSWORD`, generated), `app-password` (`APP_DB_PASSWORD`, generated) | postgres, its init script |
| `crawler-api-keys` | optional `fred-api-key`, `bls-api-key`, `bea-api-key`, `census-api-key` (`FRED_API_KEY`, ...) | crawler-worker |
| `econ-graph-keycloak` | `admin-username`, `admin-password`, `db-password`, optional `google-idp-client-id`, `google-idp-client-secret` (`KEYCLOAK_*`); the passwords are generated on the first run, the username defaults to `admin` | Keycloak (manifests arrive with the Keycloak deploy PR) |
| `grafana-admin` | `admin-password` (`GRAFANA_ADMIN_PASSWORD`, generated) | Grafana (user `admin`) |
| `monitoring-auth` | `auth`, one htpasswd line (`MONITORING_BASIC_AUTH`, e.g. `htpasswd -nB admin`, which prompts); only written when set | `ingress-cloudflare-dns01.yaml` basic auth |
| `cloudflare-tunnel` | optional `token` (`CLOUDFLARE_TUNNEL_TOKEN`, or hidden input with `--prompt-tunnel-token`), the Cloudflare Tunnel connector token | `cloudflared` (see [Public site](#-public-site-cloudflare-tunnel)) |

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
`econ-graph-postgres` Secret; set `ALLOW_EXISTING_DB_VOLUME=1` to continue anyway. It also stops,
without writing anything, when it cannot query the cluster.

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

Without `monitoring-auth`, the monitoring host in `ingress-cloudflare-dns01.yaml` returns 503
until the Secret exists; `create-secrets.sh` warns when `MONITORING_BASIC_AUTH` is unset and the
Secret is missing.

## 🗑️ Cleanup

### Remove Application Only

```bash
./scripts/deploy/teardown.sh
```

### Remove Entire Cluster

```bash
kind delete cluster --name econ-graph
```

## 🐛 Troubleshooting

### Common Issues

1. **Images not found**: Make sure you've built the images with `./scripts/deploy/build-images.sh`

2. **Database connection failed**: Ensure PostgreSQL is running and accessible

3. **Pods not starting**: Check logs with `kubectl logs <pod-name> -n econ-graph`

4. **Ingress not working**: Verify NGINX ingress controller is running:

   ```bash
   kubectl get pods -n ingress-nginx
   ```

### Debug Commands

```bash
# Describe a pod to see events
kubectl describe pod <pod-name> -n econ-graph

# Check cluster info
kubectl cluster-info

# Check node status
kubectl get nodes

# Check ingress status
kubectl get ingress -n econ-graph
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
- [NGINX Ingress Controller](https://kubernetes.github.io/ingress-nginx/)
