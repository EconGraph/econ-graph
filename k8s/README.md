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
written by `scripts/deploy/create-secrets.sh` (from the secrets PR, which also makes
`deploy.sh` run it). That script generates the admin and database passwords on its
first run and keeps them afterwards. `deploy.sh` skips Keycloak while the Secret
does not exist.

```bash
# Optional, for Google sign-in
# (redirect URI http://localhost/idp/realms/econ-graph/broker/google/endpoint):
export KEYCLOAK_GOOGLE_CLIENT_ID=... KEYCLOAK_GOOGLE_CLIENT_SECRET=...
./scripts/deploy/create-secrets.sh
./scripts/deploy/deploy.sh
```

- Issuer: `http://localhost/idp/realms/econ-graph`. Only `/idp/realms/econ-graph`
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

## 🌐 Accessing the Application

After deployment, the application will be available at:

- **Frontend**: <http://localhost>
- **Backend API**: <http://localhost/api>
- **GraphQL**: <http://localhost/graphql>
- **Health Check**: <http://localhost/health>

**Note**: `k8s/manifests/ingress.yaml` and `k8s/manifests/keycloak/ingress.yaml` route on
`host: localhost`, so no `/etc/hosts` entry is needed. The admin frontend isn't deployed by
`deploy.sh` (re-enabled by ECO-242, train 2); `/admin` has no backend until then.

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
`econ-graph-secrets` Secret. From train 2 (`v4.1.0`) there are two databases with their own
URLs; see [Two databases](#two-databases) below.

### Two databases

From train 2 (`v4.1.0`), EconGraph keeps users' work and crawled data in separate databases.
Names below are from `docs/development/TRAIN2_CONTRACTS.md` (sections 1, 3, 6 and 9); the
migration rules are in [MIGRATIONS.md](../docs/development/MIGRATIONS.md#two-databases).

| Database | Holds | Postgres (StatefulSet / Service) | Database name | URL (Secret `econ-graph-secrets` key, env var) | Migrations |
| --- | --- | --- | --- | --- | --- |
| Data | Sources, series, observations, crawl state, SEC data | `postgresql` / `data-postgres-service` | `econ_graph` (the existing database) | `data-database-url`, `DATA_DATABASE_URL` | `migrations/data`, applied only by the `crawler-migrate` Job |
| App | Users, sessions, annotations, comments, preferences, audit logs | `app-postgres` / `app-postgres-service` | `econ_graph_app` | `app-database-url`, `APP_DATABASE_URL` | `migrations/app`, applied by the backend at startup |

`data-database-url` and `app-database-url` replace `database-url`. No binary reads
`DATABASE_URL` any more; one that finds it set and its own variable unset exits at startup.
The crawler-worker, `crawler`, `sec-crawler` and `sec-company-crawler` read only the data URL.
The backend reads the app URL and, where its namespace owns a data database, the data URL.

**Which namespace owns which.** The production namespace `econ-graph` owns the primary data
database and production's app database, both on host storage
(`docs/deployment/HOST_STORAGE.md`). A branch namespace `econ-graph-br-<slug>` owns only its
own app database (and Keycloak), on disposable storage; it has no data URL and no network path
to `data-postgres-service`, and reads data through `/data/graphql`, which the gateway routes to
production's backend with credentials stripped. A branch deployed with `--local-data` also owns
its own data database, seeded from the latest verified primary dump, and never writes to the
primary. Branch namespaces are created and removed by `scripts/deploy/deploy-branch.sh`; see
its section of this README.

**`crawler migrate`.** No service applies data migrations at startup. `deploy.sh` runs the Job
`crawler-migrate` (`/app/crawler migrate` from the `econ-graph-crawler-worker` image) before the
crawler-worker rolls out, and only in a namespace that owns a data database. It applies pending
data migrations and prints each version. To see what is pending without changing anything:

```bash
kubectl -n econ-graph exec deploy/crawler-worker -- /app/crawler migrate --dry-run
kubectl -n econ-graph logs job/crawler-migrate
```

Until the Job has run, a backend with a data URL keeps failing `/readyz` if its binary
expects data migrations the database doesn't have yet.

**Teardown keeps the data database.** `teardown.sh` removes the app plane (backend, frontend,
app Postgres). The data Postgres StatefulSet, its Service and its Secret are removed only with
`--include-data-plane` plus a typed confirmation, never by `--yes` alone. No script deletes a
PersistentVolume or a host data directory, and backups are never deleted automatically.
Changes to the data plane that lose rows need Joe's sign-off, a dump first and a planned
recrawl ([critical changes](../docs/development/MIGRATIONS.md#critical-changes)).

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

From train 2 this keeps the data database; see
[Teardown keeps the data database](#two-databases).

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
