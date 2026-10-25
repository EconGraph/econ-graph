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
4. Install the gateway and the app's routes on it

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
│   └── gateway/         # The gateway (Envoy Gateway): see "Gateway" below
│       ├── envoy-proxy.yaml, gatewayclass.yaml, gateway.yaml
│       ├── gateway-https.yaml   # optional listener https
│       ├── ratelimit-burst.yaml # bursts of 100 for the GraphQL rate limits
│       └── routes/          # this namespace's HTTPRoutes and rate limits
└── README.md           # This file

terraform/k8s/          # Terraform configuration
├── main.tf            # Main Terraform configuration
├── variables.tf       # Variables
└── outputs.tf         # Outputs

scripts/deploy/         # Deployment scripts
├── setup-local-k8s.sh # Complete setup script
├── build-images.sh    # Build Docker images
├── create-secrets.sh  # Create the Secrets (run by deploy.sh)
├── install-gateway.sh # Install the gateway (run by deploy.sh)
├── test-gateway-routes.sh # Check every route on the gateway
├── cutover-to-gateway.sh  # Move a cluster from ingress-nginx to the gateway
├── deploy.sh          # Deploy to K8s
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
written by `scripts/deploy/create-secrets.sh`, which `deploy.sh` runs. That script generates the
admin and database passwords on its first run and keeps them afterwards. `deploy.sh` skips Keycloak while the Secret
does not exist.

```bash
# Optional, for Google sign-in
# (redirect URI http://localhost/idp/realms/econ-graph/broker/google/endpoint):
export KEYCLOAK_GOOGLE_CLIENT_ID=... KEYCLOAK_GOOGLE_CLIENT_SECRET=...
./scripts/deploy/create-secrets.sh
./scripts/deploy/deploy.sh
```

- Issuer: `http://localhost/idp/realms/econ-graph`. Only `/idp/realms/econ-graph`
  and `/idp/resources` are routed by the gateway; the master realm and the admin
  console are not reachable from outside the cluster.
- Admin console: `kubectl -n econ-graph port-forward svc/keycloak-service 18080:8080`,
  then <http://localhost:18080/idp/admin>. The password is in the Secret:
  `kubectl -n econ-graph get secret econ-graph-keycloak -o jsonpath='{.data.admin-password}' | base64 -d`
- Every deploy re-applies `config/keycloak/econ-graph-realm.json` through the Job
  `keycloak-realm-import` (keycloak-config-cli), so the realm, its roles and the
  Google client follow the repository. Users and their role assignments are kept.
  Logs: `kubectl -n econ-graph logs job/keycloak-realm-import -c keycloak-config-cli`.
- Seeded test users exist only in the local docker-compose stack, never in the cluster.

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
| Gateway API CRDs | v1.5.1, standard channel | |
| Envoy Gateway | v1.8.5 (Envoy Proxy 1.38) | supports Kubernetes v1.32 to v1.35 |

| Object | Name | Where |
| --- | --- | --- |
| EnvoyProxy | `econ-graph-proxy` | `envoy-gateway-system`: one proxy replica on the control-plane node (`ingress-ready=true`), holding host ports 80/443 as hostPorts; Service `econ-graph-gateway-proxy` (ClusterIP) |
| GatewayClass | `econ-graph` | cluster |
| Gateway | `econ-graph-gateway` | namespace `econ-graph-gateway`. Listener `http` (port 80) admits routes from namespaces labelled `econgraph.io/gateway-access: "true"`; listener `https` (port 443) exists only with `GATEWAY_HTTPS_HOSTNAME` |

Routes in `econ-graph` (`k8s/manifests/gateway/routes/`), on host `localhost`:

| HTTPRoute | Paths | Backend |
| --- | --- | --- |
| `econ-graph-frontend` | `/`, `/auth/callback`, `/silent-callback.html` | frontend; sets the security headers (CSP, HSTS, X-Frame-Options; only `/silent-callback.html` may be framed, by this origin) |
| `econ-graph-backend` | `/api`, `/auth`, `/health` | backend |
| `econ-graph-graphql` | `/graphql` | backend; 20 requests a second per client address, bursts of 100, then 429 |
| `econ-graph-data-graphql` | `/data/graphql` | backend `/graphql`, with `Authorization` and `Cookie` removed; same rate limit |
| `econ-graph-idp` | `/idp/realms/econ-graph`, `/idp/resources` | Keycloak |

The rate limit counts per client address as the proxy sees it: through the hostPorts that is the
client's own address, except that requests from the host itself all arrive from the Docker bridge
gateway. In-cluster callers reach the gateway at
`http://econ-graph-gateway-proxy.envoy-gateway-system.svc.cluster.local/` with a `Host` header
that one of the routes matches; a NetworkPolicy admitting them must allow the proxy pod's port
10080 (listener `http`), not the Service port 80.

Check every route with `./scripts/deploy/test-gateway-routes.sh [--rate-limit] [BASE_URL]`.

**HTTPS.** `GATEWAY_HTTPS_HOSTNAME=<host> ./scripts/deploy/deploy.sh` adds listener `https` for
that host name, with a certificate in Secret `econ-graph-tls` (namespace `econ-graph-gateway`)
issued by cert-manager from ClusterIssuer `GATEWAY_TLS_ISSUER` (default `letsencrypt-staging`,
`k8s/manifests/letsencrypt-issuer.yaml`, HTTP-01 through listener `http`). `deploy.sh` then turns
on cert-manager's Gateway API support and adds the host name to the routes. Without the variable
there is no `https` listener.

**Moving a cluster from ingress-nginx.** `./scripts/deploy/cutover-to-gateway.sh --dry-run`
prints the steps; without `--dry-run` it installs the gateway next to ingress-nginx, checks every
route through the proxy Service, asks for confirmation, removes the Ingresses and ingress-nginx,
moves the proxy onto ports 80/443 and checks the routes again. It saves the Ingresses first;
`--rollback <dir>` restores ingress-nginx and them. Its header lists every step.

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

### Database Connection

PostgreSQL runs in the cluster (`postgres-deployment.yaml`). The backend and crawler-worker connect as the
`econgraph` user through `DATABASE_URL`, which `create-secrets.sh` builds and stores in the
`econ-graph-secrets` Secret. That one database holds both migration sets, data and app; the
backend applies them when it starts, the data set first
([database migrations](../docs/development/MIGRATIONS.md)).

### Secrets

Current manifests contain no credentials. Google and Facebook credentials committed in earlier versions remain in Git history and must be rotated. `scripts/deploy/create-secrets.sh` creates the Secrets below in the
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
| `econ-graph-keycloak` | `admin-username`, `admin-password`, `db-password`, optional `google-idp-client-id`, `google-idp-client-secret` (`KEYCLOAK_*`); the passwords are generated on the first run, the username defaults to `admin` | Keycloak |
| `grafana-admin` | `admin-password` (`GRAFANA_ADMIN_PASSWORD`, generated) | Grafana (user `admin`) |
| `monitoring-auth` | `auth`, one htpasswd line (`MONITORING_BASIC_AUTH`, e.g. `htpasswd -nB admin`, which prompts); only written when set | no manifest reads it |

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
