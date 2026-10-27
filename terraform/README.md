# EconGraph Terraform modules

These modules describe the Kubernetes resources for EconGraph's PostgreSQL database,
Rust GraphQL API, React frontend, monitoring, and admin frontend. They collect application
configuration, persistent storage, autoscaling, and observability definitions in one
place for developers adapting EconGraph to an existing cluster.

## Choose a deployment path

For the repository's kind deployment, follow the [Kubernetes deployment guide](../k8s/README.md).
Its cluster configuration lives in [`terraform/k8s/`](k8s/), a separate Terraform
working directory; its application resources come from Kubernetes manifests and
the [deploy script](../scripts/deploy/deploy.sh). The [host storage guide](../docs/deployment/HOST_STORAGE.md)
explains that deployment's persistent data layout.

The top-level `terraform/` directory is **not a runnable deployment root**. Terraform
[loads all configuration files in a directory together](https://developer.hashicorp.com/terraform/language/files#directories-and-modules):
`main.tf` and `postgres-only.tf` both declare the Kubernetes provider, namespace and
password inputs, namespace resource, PostgreSQL module, and password output. They
cannot be selected as alternatives by filename. In addition, `main.tf` references
undeclared root inputs for its admin frontend call, including `image_repository`,
`image_tag`, `cluster_name`, and admin credentials and access settings. The
`admin_access` output also uses undeclared `admin_domain`. Child-module variables
do not declare root inputs. Removing one of the two files alone does not
produce a working full deployment.

Use this README as a map of the modules and their integration requirements. A
Terraform deployment requires a corrected root configuration and compatible runtime
settings before planning or applying resources. In particular:

- The backend requires `APP_DATABASE_URL` and optionally reads `DATA_DATABASE_URL`.
  The root supplies individual `DATABASE_*` fields instead of those URLs. The backend
  reads `BACKEND_PORT` (default 9876), while the module's HTTP Service and probes
  target 8080; the root's `SERVER_PORT` does not change the listener. The app plane
  stores users and application state; the data plane stores crawled and reference
  data. Both database URLs may name the same physical database.
- The admin frontend module targets port 3000, while the checked-in admin Nginx
  image listens on 3001.
- Gateway routes, monitoring targets, and worker dependencies need the
  integration described below. Resource declarations alone do not establish a
  functioning deployment.

## Module map

`main.tf` connects PostgreSQL to the backend, gives the backend a ConfigMap and
Secret, and places the frontend in front of the backend's Service. Monitoring is
configured alongside these resources; the admin frontend uses a separate namespace.
The crawl-queue worker is deployed separately, so these Terraform modules do not
start data collection.

The kind cluster configuration invokes the [Gateway installer](../scripts/deploy/install-gateway.sh).
The [application deploy script](../scripts/deploy/deploy.sh) also runs that installer
and applies application routes through [apply-gateway-routes.sh](../scripts/deploy/apply-gateway-routes.sh).
The HTTP entry point is Envoy Gateway using Gateway API. The top-level application
modules neither install that gateway nor connect their Services to it. The checked-in
[HTTPRoutes](../k8s/manifests/gateway/routes/httproutes.yaml) target namespace
`econ-graph`, frontend Service `econ-graph-frontend-service:3000`, and backend
Service `econ-graph-backend-service:9876`. These differ from the modules' default
namespace `econgraph` and Service names. Route attachment also requires the namespace
labels described in the [Gateway guide](../k8s/README.md#-gateway).

| Module | Resources and relationship |
|--------|----------------------------|
| [PostgreSQL](modules/postgresql/main.tf) | One PostgreSQL 18 StatefulSet, initialization/configuration, a Secret, normal and headless Services, and a persistent volume claim. Child inputs default to storage class `standard` and size `50Gi`. Persistence does not provide backups or database replication. |
| [Backend](modules/backend/main.tf) | API Deployment, HTTP and metrics Service ports, health probes, ServiceAccount/RBAC, HPA, disruption budget, and ServiceMonitor. Receives the root ConfigMap and Secret. |
| [Frontend](modules/frontend/main.tf) | React static files served by Nginx, caching/compression, security headers, and a `/graphql` proxy to `econgraph-backend`. Includes a Deployment, Service, ServiceAccount, HPA, and disruption budget. |
| [Monitoring](modules/monitoring/main.tf) | `kube-prometheus-stack` Helm release with Prometheus, Grafana, Alertmanager, custom dashboards and alert rules. See [monitoring integration](#monitoring-integration) before relying on their coverage. |
| [Admin frontend](modules/admin-frontend/) | Admin UI Deployment/Service, a separate namespace, authentication configuration, access restrictions, and monitoring definitions. Its root inputs and image port must be reconciled before use. |

## Cluster and runtime requirements

The top-level modules target an existing cluster through kubeconfig; they do not
create worker nodes or upgrade Kubernetes. `main.tf` declares Terraform >= 1.0,
Kubernetes provider `~> 2.23`, Helm provider `~> 2.11`, random provider `~> 3.4`, and
tls provider `~> 4.0`. Assess the providers and monitoring chart against the target
cluster when integrating the modules; their declarations alone do not establish
compatibility with a particular Kubernetes release.

The resources require persistent-volume provisioning for the configured storage
classes and cluster resource metrics for the HPAs. ServiceMonitor and PrometheusRule
resources require Prometheus Operator CRDs. The separate Gateway deployment requires Gateway API and Envoy Gateway
CRDs; its optional HTTPS certificate requires cert-manager. Account for those dependencies
when arranging installation order. Container images must also be available to the
cluster: the backend/frontend child modules default to `econgraph/backend:latest`
and `econgraph/frontend:latest`, rather than the local kind image names.

### Root declarations and credentials

This table describes the inputs declared in `main.tf`; it is a reference for
integration, not a tfvars recipe that makes the directory deployable.

| Input | Default and scope |
|-------|-------------------|
| `kubeconfig_path` | `~/.kube/config`; used by the Kubernetes and Helm providers. |
| `namespace` | `econgraph`; application namespace. `postgres-only.tf` separately declares `econ-graph`. |
| `environment` | `prod`; passed to backend/frontend and used in labels. It does not select a deployment profile. |
| `domain` | `econgraph.example.com`; passed to the frontend/monitoring modules and used for CORS. The `application_urls` output constructs frontend, API and Grafana URLs but provisions no routes for them. |
| `database_password` | No default. In `main.tf`, an explicit empty string selects a generated 32-character password; otherwise the trimmed input must have at least 16 characters. |
| `fred_api_key`, `bls_api_key` | Empty by default; sensitive values placed in the root application Secret. They do not configure the separately deployed worker's source Secret. |

`postgres-only.tf` requires a nonempty password of at least 16 trimmed characters;
it has no password-generation path. Both files reject known placeholder passwords,
including the placeholder in `terraform.tfvars.example`. These validation rules
describe conflicting declarations in the same directory, not selectable deployment
modes.

Provider keys can be obtained through the [FRED API](https://fred.stlouisfed.org/docs/api/api_key.html)
and [BLS API](https://www.bls.gov/developers/api_signature_v2.html).

Supply credentials through untracked tfvars files or `TF_VAR_*` environment
variables in the eventual deployment root. Never commit actual values.

Terraform state, saved plans, sensitive outputs, and Kubernetes Secret exports can
contain credentials even when terminal output is redacted. Restrict their storage
and access, keep them out of Git, and use the cluster's encryption-at-rest and
external secret-management facilities where appropriate.

### Customization and updates

Backend and frontend child modules accept `image_repository`, `image_tag`, and
`replicas`, but `main.tf` does not forward root inputs for those settings. Changing
them requires editing the module calls in a valid root. Assigning undeclared names
in tfvars does not add an interface; see [Terraform input variables](https://developer.hashicorp.com/terraform/language/values/variables#values-for-undeclared-variables).
There is no root `replicas` map, `enable_cert_manager`, `enable_monitoring`, or
`enable_backups` switch. The admin call's reference to `image_tag` is an undeclared
root variable, not a usable root input.

Both application Deployments initially request three replicas. Their HPAs range
from **2 to 10 replicas**, targeting 70% CPU and 80% memory utilization. Setting an
initial replica count of one does not lower the HPA minimum. Resource requests and
limits are literals in the child resources, not `cpu_request` or `memory_limit`
inputs:

| Container | CPU request / limit | Memory request / limit |
|-----------|---------------------|------------------------|
| Backend | `100m` / `500m` | `256Mi` / `1Gi` |
| Frontend | `50m` / `200m` | `64Mi` / `256Mi` |
| PostgreSQL | `250m` / `1000m` | `512Mi` / `2Gi` |

Adjust those definitions and HPA bounds together when sizing an installation.
Prometheus retention and storage size are child-module inputs, defaulting to `30d`
and `50Gi`; the root does not expose them. Application image changes and Helm chart
updates belong to the application configuration. Kubernetes upgrades belong to the
cluster's provisioning system. PostgreSQL major-version changes require a database
upgrade or dump/restore procedure; never attach PostgreSQL 18 to an older major
version's data directory.

## Crawler integration

Terraform does not deploy the worker that drains `crawl_queue`, so these modules
alone do not crawl. Follow the [crawler deployment guide](../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md)
for worker behavior and configuration, and [database migrations](../docs/development/MIGRATIONS.md)
for the distinction between app plane and data plane migrations.

The checked-in [worker manifest](../k8s/manifests/crawler-worker.yaml) belongs to the
kind deployment in namespace `econ-graph`. It waits for
`postgres-service.econ-graph.svc.cluster.local`, reads ConfigMap `econ-graph-config`,
and takes `DATA_DATABASE_URL` from Secret `econ-graph-secrets`, key
`data-database-url`. Source API keys come from the optional `crawler-api-keys`
Secret. These names and keys differ from this root's `econgraph-config`,
`econgraph-secrets`, and `postgresql` Service. Changing the namespace alone does not
provide the worker's dependencies.

The worker never applies migrations. The kind deploy script completes the
`crawler-migrate` Job before applying the worker. Its image uses
`imagePullPolicy: Never`; the [image build script](../scripts/deploy/build-images.sh)
loads images into kind. An installation using another cluster must provide its own
image distribution, database URL, configuration/Secret mapping, and completed data
plane migrations before starting a compatible worker.

Keep the worker at **one replica** with the manifest's `Recreate` strategy. Source
rate limits apply per process, so additional replicas multiply upstream request
rates, including during rolling updates. Increase `CRAWLER_CONCURRENCY` within the
database pool's capacity for more concurrency; scaling across processes requires
first dividing source rates or assigning disjoint sources.

## Monitoring integration

The monitoring module defines dashboards for system, database, and crawler metrics,
plus alerts for backend availability, HTTP latency, queue backlog, and database
connections. Check scrape targets and query names before treating these definitions
as operational coverage:

- Backend scrapes target `econgraph-backend:9090`, while the current backend serves
  `/metrics` on its application listener (default 9876). Reconcile the listener,
  Service, probes, and scrape configuration together.
- The crawler target is `econgraph-crawler:9090`. No such Service is provided here;
  the kind worker exposes pod metrics on 9102 with annotations and no Service.
  Queue queries use older `econgraph_queue_items` names; current worker series
  include `crawler_queue_items` and `crawler_jobs_total`.
- PostgreSQL's ServiceMonitor requests `/metrics` on the database protocol port
  5432. The PostgreSQL module does not install an HTTP exporter. The `pg_*` queries
  require a separately configured exporter and matching scrape target.
- The system dashboard's GraphQL query uses `graphql_request_duration_seconds`,
  while the backend emits `graphql_query_duration_seconds`. The database panel
  labelled “Table Sizes” counts tuple changes, not bytes. These custom dashboards
  do not provide certificate-expiration or resource-utilization panels.

The [monitoring guide](../docs/monitoring/README.md) describes current metrics and
related dashboards. An empty panel or absent alert series does not establish that a
queue is empty, a database is healthy, or a certificate will renew.

### Grafana access

The `prometheus` Helm release installs Grafana in `var.namespace` (`econgraph` by
default), while separately creating a `${namespace}-monitoring` namespace.
For an installation of that release, the local access command is:

```bash
kubectl port-forward -n econgraph svc/prometheus-grafana 3000:80
```

Grafana uses username `admin` and the generated `grafana_admin_password` sensitive
root output. An authorized operator must retrieve that named output through the
installation's protected Terraform workflow; ordinary output listings redact it.

Grafana ingress is disabled in the monitoring chart configuration, and no public
Grafana HTTPRoute is supplied here. Prometheus likewise has no public route.

## Gateway and TLS

For the repository's kind deployment, [localhost routes](../k8s/manifests/gateway/routes/httproutes.yaml)
serve the frontend and backend paths on one host. The route script uses `PUBLIC_HOST`
for optional public routes and the Gateway's HTTPS listener. It keeps localhost
routes when public routing is disabled. Follow the [public site guide](../k8s/README.md#-public-site-cloudflare-tunnel)
for DNS, Cloudflare Tunnel, and certificate configuration; the top-level modules'
`domain` input does not configure that script.

The HTTPS certificate and its Secret `econ-graph-tls` belong to namespace
`econ-graph-gateway`. `GATEWAY_TLS_ISSUER` selects the ClusterIssuer; when unset,
the script retains the existing issuer or defaults to `econ-graph-selfsigned`.
The tunnel connector accepts that gateway-side certificate without verification;
visitors see Cloudflare's certificate. This default does not establish publicly
trusted TLS for clients connecting directly to the gateway. A different issuer
requires its own configuration and working issuance prerequisites. Monitor
Certificate status and issuance errors when relying on cert-manager renewal.

## Backup and security responsibilities

The PostgreSQL module provisions persistent storage, but no backup CronJob, backup
storage, retention policy, or restore automation. Supply database dumps, appropriate
volume snapshots, or an external backup system separately, and verify recovery
before relying on it. Exported Kubernetes resources and Terraform state preserve
configuration, not PostgreSQL rows. Protect those exports as sensitive artifacts.
The [production backup runbook](../docs/deployment/BACKUPS.md) covers the repository's
host-backed kind installation; its storage paths and jobs are not installed by
these modules.

The backend, frontend, and PostgreSQL modules provide no NetworkPolicy to isolate
application or database traffic. The admin frontend module defines policy for its
separate namespace. Design database access policy explicitly for the target
installation and ensure the cluster enforces NetworkPolicy.

Backend and frontend containers are configured to run as non-root, reject privilege
escalation, and use read-only root filesystems with writable temporary volumes.
The backend ServiceAccount's namespace Role grants get/list/watch on ConfigMaps,
Secrets, and pods. The frontend has a ServiceAccount but no corresponding Role in
its module. The admin module uses a ClusterRoleBinding with name-restricted
ConfigMap/Secret permissions, so its RBAC is not confined to its namespace. Review
these actual permissions and the Helm charts' cluster-scoped resources rather than
assuming every component has minimal, namespace-only access.

## Troubleshooting module-based installations

These commands use the module names and default application namespace `econgraph`.
Use the installation's namespace and context; the kind manifests use different
names. The commands inspect an existing installation and do not repair the root
configuration or runtime mismatches described above.

1. **Pods remain Pending.** Inspect scheduling events, resource availability, and
   storage-class/PVC binding:

   ```bash
   kubectl describe pod <pod-name> -n econgraph
   kubectl get pvc -n econgraph
   ```

2. **Backend cannot connect or become ready.** Inspect backend logs and configuration
   wiring for the required database URLs and listener/probe ports; check database
   connectivity and access policies without printing credentials:

   ```bash
   kubectl logs -n econgraph -l app=econgraph-backend
   kubectl describe deployment econgraph-backend -n econgraph
   ```

For Gateway routing or certificate issues in the kind deployment, use the
[Gateway guide](../k8s/README.md#-gateway) and [public site guidance](../k8s/README.md#-public-site-cloudflare-tunnel).
Their Gateway and certificate namespace is `econ-graph-gateway`, and their
application routes use `econ-graph`; commands against `econgraph` do not inspect
those routes. Check route attachment and backend Service references, then inspect
Certificate status and cert-manager events for issuance failures.

For a reconciled backend installation, forward its HTTP Service with
`kubectl port-forward -n econgraph svc/econgraph-backend 8080:80`. This still forwards
to the module's configured pod port 8080; it cannot compensate for a listener on
9876. Use `kubectl top pods -n econgraph` to assess CPU/memory usage when resource
metrics are available, and verify actual Prometheus targets before using dashboards
to guide scaling or incident response.
