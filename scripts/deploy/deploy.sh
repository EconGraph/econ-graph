#!/bin/bash

# Deploy EconGraph to local Kubernetes cluster
# This script deploys the application using the K8s manifests
#
# Usage: deploy.sh [--dry-run]
#   --dry-run  print the kubectl and kind commands and the helper scripts this deploy runs,
#              in order, and contact no cluster. Lookups succeed with no output, so optional
#              steps (Keycloak, the crawler-migrate Job) are shown, and nothing is waited for.

set -e

DRY_RUN=false
case "${1:-}" in
  "") ;;
  --dry-run) DRY_RUN=true ;;
  *) echo "Usage: $0 [--dry-run]" >&2; exit 2 ;;
esac
# Runs a helper script, or prints it in a dry run.
run_script() {
  if [ "$DRY_RUN" = true ]; then echo "[dry-run] $*"; else "$@"; fi
}
if [ "$DRY_RUN" = true ]; then
  echo "🔎 Dry run: nothing is applied."
  kubectl() { echo "[dry-run] kubectl $*" >&2; }
  kind() { echo "[dry-run] kind $*" >&2; echo econ-graph; }
  # Exported, so the linters below never reach a cluster either.
  export -f kubectl kind
fi

echo "🚀 Deploying EconGraph to local Kubernetes cluster..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Run linter checks before deployment
echo "🔍 Running linter checks before deployment..."
echo ""

# Run Grafana dashboard linter. Only the dashboard-file checks, not the full suite
# (which also ends with a --step=k8s-deployment check of the live cluster): this
# runs before anything is applied, so on a cluster that has never been deployed to
# before, Grafana/Loki/Prometheus etc. can never exist yet, and the full suite
# would always abort here before deploying anything.
if [ -f "scripts/test-grafana-dashboards.sh" ]; then
    echo "📊 Running Grafana dashboard linter..."
    linter_failed=false
    for step in json-syntax dashboard-structure datasource-consistency promql-queries logql-queries configmap-structure; do
        ./scripts/test-grafana-dashboards.sh --step="$step" || linter_failed=true
    done
    if [ "$linter_failed" = false ]; then
        echo "✅ Grafana dashboard linter passed"
    else
        echo "❌ Grafana dashboard linter failed - aborting deployment"
        exit 1
    fi
    echo ""
else
    echo "⚠️  Grafana dashboard linter not found, skipping..."
fi


# Load port configuration
if [ -f "ports.env" ]; then
    echo "📋 Loading port configuration from ports.env..."
    source ports.env
else
    echo "⚠️  ports.env not found, using default ports"
    BACKEND_NODEPORT=30080
    FRONTEND_NODEPORT=30000
fi

# The kind cluster runs on the rootful Docker daemon, and its own kubectl and
# kubeconfig are used (on this host `kubectl` is the microk8s snap wrapper);
# see scripts/deploy/lib/kind-env.sh. Sourced first: it sets DOCKER_HOST.
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
check_docker_daemon

# Check if kind cluster exists
if ! kind_cluster_exists; then
    echo "❌ Kind cluster '${KIND_CLUSTER}' not found on ${DOCKER_HOST}."
    echo "   Create it with scripts/deploy/recreate-kind-cluster.sh --if-missing"
    exit 1
fi
ensure_kind_kubectl

# One kubectl, one cluster for the whole deploy: the kind kubectl that
# ensure_kind_kubectl put first on PATH (exported as KUBECTL, so a KUBECTL from
# the environment cannot point elsewhere), switched to kind-econ-graph and
# pinned (replacing any CLUSTER_ROLE_CONTEXT from the environment). Every
# kubectl call below, including kind-env.sh's, then goes through kc, the same
# binary and context the production check uses (scripts/deploy/lib/cluster-role.sh).
# Otherwise PUBLIC_HOST= meant for this cluster could remove another cluster's
# public routes and stop its tunnel connector.
# In a dry run, kc only prints (CLUSTER_ROLE_DRY_RUN) and kubectl stays the
# printing stub above: neither reaches the cluster in a dry run. The
# libraries are sourced first: sourcing lib/cluster-role.sh resets
# CLUSTER_ROLE_DRY_RUN to false, so only --dry-run, below, turns it on, never
# a value from the environment.
export KUBECTL="${ECONGRAPH_KUBE_DIR:-${ECONGRAPH_CACHE}}/bin/kubectl"

# Public host name (ECO-457): PUBLIC_HOST=econgraph.com also serves
# https://econgraph.com through the cloudflared tunnel; empty serves
# http://localhost only. See scripts/deploy/lib/public-host.sh (which sources
# lib/cluster-role.sh), shared with restart-k8s-rollout.sh, for the rules and
# render_manifest. The frontend image needs no rebuild for it: it resolves its
# sign-in issuer against the page's origin.
# shellcheck source=scripts/deploy/lib/public-host.sh
source "$PROJECT_ROOT/scripts/deploy/lib/public-host.sh"
CLUSTER_ROLE_DRY_RUN="$DRY_RUN"
if ! cluster_role_use_context "kind-${KIND_CLUSTER}"; then
    echo "❌ ${KUBECTL} has no context kind-${KIND_CLUSTER}"
    exit 1
fi
if [ "$DRY_RUN" != true ]; then
    kubectl() { kc "$@"; }
fi

# Monitoring stack linter: static checks only (--lint-only contacts no cluster).
# Run after the kind cluster's kubectl and context are set, so nothing before
# this point can reach another cluster. A failure stops the deploy; a checkout
# without the script skips it.
if [ -f scripts/test-monitoring.sh ]; then
    echo "🔧 Running monitoring stack linter..."
    if ! ./scripts/test-monitoring.sh --lint-only; then
        echo "❌ Monitoring stack linter failed - aborting deployment"
        exit 1
    fi
else
    echo "⚠️  scripts/test-monitoring.sh not found, skipping the monitoring stack linter"
fi
echo "✅ All linter checks passed - proceeding with deployment"
echo ""

# Secrets from an earlier version may lack keys this one needs: stop before applying anything.
# shellcheck source=scripts/deploy/lib/keycloak-secret.sh
source "$PROJECT_ROOT/scripts/deploy/lib/keycloak-secret.sh"
require_anonymous_client_secret || exit 1

public_host_init

# Apply all manifests
echo "📋 Applying Kubernetes manifests..."

# Apply in order
kubectl apply -f k8s/manifests/namespace.yaml
# Static PVs on the host btrfs subvolumes for the data, app and Keycloak Postgres
# databases. Runs before create-secrets.sh: it refuses to continue if a host data
# directory already holds a database whose password Secret is missing.
run_script ./scripts/deploy/apply-host-storage.sh
render_manifest k8s/manifests/configmap.yaml | kubectl apply -f -
# Credentials live in Secrets created out of band (never committed). The script
# keeps existing values and generates missing internal passwords; see k8s/README.md.
run_script ./scripts/deploy/create-secrets.sh

# Deploy PostgreSQL: the data database (StatefulSet postgresql, Services
# data-postgres-service and its alias postgres-service) and the app database
# (StatefulSet app-postgres, Service app-postgres-service).
echo "🗄️  Deploying PostgreSQL..."
kubectl apply -f k8s/manifests/postgres-init.yaml
kubectl apply -f k8s/manifests/postgres-deployment.yaml
kubectl apply -f k8s/manifests/postgres.yaml
kubectl apply -f k8s/manifests/app-postgres-init.yaml
kubectl apply -f k8s/manifests/app-postgres.yaml

# Wait for PostgreSQL to be ready
echo "⏳ Waiting for PostgreSQL to be ready..."
kubectl wait --for=condition=ready pod -l app=postgresql -n econ-graph --timeout=300s
kubectl -n econ-graph rollout status statefulset/app-postgres --timeout=300s

# Deploy PostgreSQL backups
echo "🗄️  Deploying PostgreSQL backup CronJob..."
kubectl apply -f k8s/manifests/postgres-backup-pvc.yaml
kubectl apply -f k8s/manifests/postgres-backup-configmap.yaml
kubectl apply -f k8s/manifests/postgres-backup-cronjob.yaml
run_script "$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"

# Deploy Keycloak (its own Postgres, then Keycloak with the econ-graph realm).
# Its credentials are in the Secret econ-graph-keycloak, written by
# scripts/deploy/create-secrets.sh; without that Secret, Keycloak is skipped.
KEYCLOAK_DEPLOYED=false
if kubectl -n econ-graph get secret econ-graph-keycloak >/dev/null 2>&1; then
  echo "🔐 Deploying Keycloak..."
  kubectl -n econ-graph create configmap keycloak-realm \
    --from-file=econ-graph-realm.json=config/keycloak/econ-graph-realm.json \
    --from-file=render-realm.sh=scripts/keycloak/render-realm.sh \
    --from-file=preserve-catalog-grants.py=scripts/keycloak/preserve-catalog-grants.py \
    --dry-run=client -o yaml | kubectl apply -f -
  kubectl apply -f k8s/manifests/keycloak/postgres.yaml
  echo "⏳ Waiting for Keycloak's PostgreSQL to be ready..."
  kubectl -n econ-graph rollout status statefulset/keycloak-postgres --timeout=300s
  render_manifest k8s/manifests/keycloak/deployment.yaml | kubectl apply -f -
  # Wait for the new Keycloak pod, so the realm import below talks to the server this
  # deploy just applied, after any schema migration it ran on start. The Deployment
  # uses Recreate, so no old pod stays ready meanwhile. Under --dry-run kubectl only
  # prints, so this is printed and not waited for.
  echo "⏳ Waiting for the Keycloak rollout..."
  if ! kubectl -n econ-graph rollout status deployment/keycloak --timeout=900s; then
    echo "❌ Keycloak did not become ready; not applying the realm. Its pods:"
    kubectl -n econ-graph get pods -l app=keycloak || true
    kubectl -n econ-graph logs deployment/keycloak --tail=100 || true
    exit 1
  fi
  # The realm-import Job applies the realm file on every deploy; a Job cannot be
  # re-run in place, so the previous one goes first.
  kubectl -n econ-graph delete job keycloak-realm-import --ignore-not-found
  render_manifest k8s/manifests/keycloak/realm-import-job.yaml | kubectl apply -f -
  KEYCLOAK_DEPLOYED=true
else
  echo "⚠️  Secret econ-graph-keycloak not found, skipping Keycloak."
  echo "   Create it with scripts/deploy/create-secrets.sh (see the Keycloak section of k8s/README.md)."
fi

# Data migrations: the crawler-migrate Job applies backend/migrations/data and must
# complete before the backend and crawler-worker are applied; no service applies them at
# startup. It runs only in a namespace that owns a data database: one whose Secret
# econ-graph-secrets has the key data-database-url (create-secrets.sh writes it). Without
# that key the Job is skipped. See k8s/manifests/crawler-migrate-job.yaml.
# create-secrets.sh has just written the Secret, so a failed read stops the deploy (set -e)
# rather than skipping the migrations.
# crawler-worker.yaml is applied only after this Job completes, so a worker stopped for the
# deploy (scripts/deploy/precheck-db-migration.sh --stop-crawler) stays stopped while the data
# migrations run, and stays stopped if the Job fails (a migration that gave up on its table
# locks, say): the deploy stops here.
has_data_database="$(kubectl -n econ-graph get secret econ-graph-secrets \
  -o jsonpath='{.data.data-database-url}')"
if [ "$DRY_RUN" = true ] || [ -n "$has_data_database" ]; then
  echo "🗄️  Applying data migrations (crawler-migrate Job)..."
  # A Job cannot be re-run in place, so the previous one goes first.
  kubectl -n econ-graph delete job crawler-migrate --ignore-not-found
  kubectl apply -f k8s/manifests/crawler-migrate-job.yaml
  if [ "$DRY_RUN" = false ]; then
    # Poll for either terminal condition: `kubectl wait --for=condition=complete` alone
    # would sit out its whole timeout on a failed Job. The Job's activeDeadlineSeconds
    # (30 minutes) fails it before this gives up.
    migrate_status=""
    for _ in $(seq 1 372); do
      migrate_status="$(kubectl -n econ-graph get job crawler-migrate \
        -o jsonpath='{.status.conditions[?(@.status=="True")].type}' 2>/dev/null || true)"
      case "$migrate_status" in
        *Complete*|*Failed*) break ;;
      esac
      sleep 5
    done
    case "$migrate_status" in
      *Complete*) echo "✅ Data migrations applied" ;;
      *)
        echo "❌ crawler-migrate did not complete (status: ${migrate_status:-none}); the backend and crawler-worker were not applied:"
        kubectl -n econ-graph logs job/crawler-migrate --all-containers || true
        exit 1
        ;;
    esac
  fi
else
  echo "⚠️  Secret econ-graph-secrets has no data-database-url: no data database here, skipping crawler-migrate."
fi

# Deploy application
render_manifest k8s/manifests/backend-deployment.yaml | kubectl apply -f -
kubectl apply -f k8s/manifests/backend-service.yaml
# Queue worker (no Service). API keys come from the optional Secret crawler-api-keys,
# written by create-secrets.sh from FRED_API_KEY, BLS_API_KEY, BEA_API_KEY, CENSUS_API_KEY.
kubectl apply -f k8s/manifests/crawler-worker.yaml
kubectl apply -f k8s/manifests/frontend-deployment.yaml
kubectl apply -f k8s/manifests/frontend-service.yaml
# admin-frontend-deployment/service and admin-ingress: not applied here (ECO-242).
# See the note in build-images.sh for why. Delete any
# admin-frontend resources a previous deploy left running, so a stale, broken
# (401-on-everything) admin frontend doesn't keep serving traffic.
kubectl delete -f k8s/manifests/admin-frontend-deployment.yaml --ignore-not-found
kubectl delete -f k8s/manifests/admin-frontend-service.yaml --ignore-not-found
# The gateway: Gateway API CRDs, Envoy Gateway, and its proxy on host ports
# 80/443 (k8s/manifests/gateway/; idempotent). The routes, with the public
# host's, follow once cert-manager is installed (apply_public_edge below).
run_script ./scripts/deploy/install-gateway.sh --context "$CLUSTER_ROLE_CONTEXT"
# The tunnel connector (scripts/deploy/lib/public-host.sh).
apply_cloudflared

# Deploy chart API service (internal only)
echo "📊 Deploying chart API service..."
kubectl apply -f k8s/manifests/chart-api-deployment.yaml
kubectl apply -f k8s/manifests/chart-api-service.yaml

echo "⏳ Waiting for deployments to be ready..."
echo "📊 Monitoring pod status (updates every 10 seconds):"
kubectl get pods -n econ-graph
echo ""

# Start background monitoring
(
  while true; do
    sleep 10
    echo "📊 Pod status update:"
    kubectl get pods -n econ-graph
    echo ""
  done
) &
MONITOR_PID=$!

# Wait for backend deployment
echo "Waiting for backend deployment..."
kubectl wait --for=condition=available --timeout=300s deployment/econ-graph-backend -n econ-graph

# Wait for crawler worker (the crawler-migrate Job above applied the data migrations)
echo "Waiting for crawler-worker deployment..."
kubectl wait --for=condition=available --timeout=300s deployment/crawler-worker -n econ-graph

# Wait for frontend deployment
echo "Waiting for frontend deployment..."
kubectl wait --for=condition=available --timeout=300s deployment/econ-graph-frontend -n econ-graph

# Admin frontend deployment: not waited on here (ECO-242).

# Wait for chart API service deployment
echo "Waiting for chart API service deployment..."
kubectl wait --for=condition=available --timeout=300s deployment/chart-api-service -n econ-graph

if [ "$KEYCLOAK_DEPLOYED" = true ] && [ "$DRY_RUN" = false ]; then
  echo "Waiting for the Keycloak realm import..."
  # `kubectl wait --for=condition=complete` alone would sit out its whole timeout on
  # a failed Job, so poll for either terminal condition.
  realm_import_status=""
  for _ in $(seq 1 120); do
    realm_import_status="$(kubectl -n econ-graph get job keycloak-realm-import \
      -o jsonpath='{.status.conditions[?(@.status=="True")].type}' 2>/dev/null || true)"
    case "$realm_import_status" in
      *Complete*|*Failed*) break ;;
    esac
    sleep 5
  done
  case "$realm_import_status" in
    *Complete*) echo "✅ Keycloak realm applied" ;;
    *)
      echo "❌ Keycloak realm import did not complete (status: ${realm_import_status:-none}):"
      kubectl -n econ-graph logs job/keycloak-realm-import --all-containers || true
      kill $MONITOR_PID 2>/dev/null || true
      exit 1
      ;;
  esac
fi

wait_cloudflared

# Stop monitoring
kill $MONITOR_PID 2>/dev/null || true

# Install cert-manager first: k8s/monitoring/letsencrypt-cloudflare-dns01.yaml
# defines ClusterIssuer/Certificate resources whose CRDs cert-manager provides.
# Its Gateway API support (--enable-gateway-api, needed for the gateway's https
# listener certificate) is switched on in the manifest before it is applied, so
# the controller starts once with it.
echo "🔐 Installing cert-manager (provides the CRDs used by k8s/monitoring/)..."
cert_manager_manifest="$(mktemp)"
curl -fsSL --retry 3 -o "$cert_manager_manifest" \
  https://github.com/cert-manager/cert-manager/releases/download/v1.21.3/cert-manager.yaml
awk '{ print } /^ *- --acme-http01-solver-image=/ { sub(/-.*/, "- --enable-gateway-api"); print }' \
  "$cert_manager_manifest" > "${cert_manager_manifest}.patched"
grep -q -- '- --enable-gateway-api$' "${cert_manager_manifest}.patched" || {
  echo "❌ Could not add --enable-gateway-api to the cert-manager manifest"
  exit 1
}
kubectl apply -f "${cert_manager_manifest}.patched"
rm -f "$cert_manager_manifest" "${cert_manager_manifest}.patched"
echo "⏳ Waiting for cert-manager to be ready..."
kubectl wait --for=condition=available --timeout=180s deployment/cert-manager -n cert-manager
kubectl wait --for=condition=available --timeout=180s deployment/cert-manager-webhook -n cert-manager
kubectl wait --for=condition=available --timeout=180s deployment/cert-manager-cainjector -n cert-manager

# The gateway's routes, and for PUBLIC_HOST its https listener, certificate and
# public routes, or their removal without one (scripts/deploy/lib/public-host.sh).
if [ "$DRY_RUN" = true ]; then
  echo "[dry-run] scripts/deploy/apply-gateway-routes.sh (PUBLIC_HOST=${PUBLIC_HOST:-none})"
else
  apply_public_edge
fi

# Deploy monitoring stack
echo "📊 Deploying monitoring stack (Grafana + Loki + Prometheus)..."
# The prometheus-rules-*.yaml files are plain ConfigMaps consumed via prometheus-config.yaml's
# rule_files, not PrometheusRule CRDs, so they apply like any other manifest here. The glob
# applies them after prometheus-deployment.yaml; the pod waits for its projected rules
# volume until they exist.
#
# letsencrypt-cloudflare-dns01.yaml is applied separately below, with a retry: the
# cert-manager webhook can report "available" slightly before its CA bundle is
# injected, so its ClusterIssuer/Certificate can transiently fail to apply even
# though the deployments are ready.
for f in k8s/monitoring/*.yaml; do
    case "$(basename "$f")" in
        letsencrypt-cloudflare-dns01.yaml)
            continue
            ;;
    esac
    kubectl apply -f "$f"
done

cert_manager_resources_applied=false
for attempt in 1 2 3 4 5; do
    if kubectl apply -f k8s/monitoring/letsencrypt-cloudflare-dns01.yaml; then
        cert_manager_resources_applied=true
        break
    fi
    echo "⏳ cert-manager webhook not ready yet, retrying ($attempt/5)..."
    sleep 10
done
if [ "$cert_manager_resources_applied" != true ]; then
    echo "❌ Failed to apply k8s/monitoring/letsencrypt-cloudflare-dns01.yaml after 5 attempts"
    exit 1
fi

# Configure Grafana dashboards
echo "📋 Configuring Grafana dashboards..."
# Create ConfigMap with proper JSON embedding to avoid truncation
cat > /tmp/grafana-dashboards.yaml << 'EOF'
apiVersion: v1
kind: ConfigMap
metadata:
  name: grafana-dashboards
  namespace: econ-graph
  labels:
    grafana_dashboard: "1"
data:
  econgraph-overview.json: |
EOF

# Append the full JSON content with proper indentation
cat grafana-dashboards/econgraph-overview.json | sed 's/^/    /' >> /tmp/grafana-dashboards.yaml

# Add the logging dashboard
cat >> /tmp/grafana-dashboards.yaml << 'EOF'
  logging-dashboard.json: |
EOF

# Extract and append the JSON content from the YAML file
yq -r '.data."logging-dashboard.json"' k8s/monitoring/grafana-logging-dashboard.yaml | sed 's/^/    /' >> /tmp/grafana-dashboards.yaml

# Apply the ConfigMap
kubectl apply -f /tmp/grafana-dashboards.yaml
rm -f /tmp/grafana-dashboards.yaml

# Wait for monitoring stack to be ready
echo "⏳ Waiting for monitoring stack to be ready..."
echo "📊 Monitoring pod status (updates every 10 seconds):"
kubectl get pods -n econ-graph
echo ""

# Start background monitoring
(
  while true; do
    sleep 10
    echo "📊 Pod status update:"
    kubectl get pods -n econ-graph
    echo ""
  done
) &
MONITOR_PID=$!

kubectl wait --for=condition=ready pod -l app=grafana -n econ-graph --timeout=300s
kubectl wait --for=condition=ready pod -l app=loki -n econ-graph --timeout=300s
kubectl wait --for=condition=ready pod -l app=prometheus -n econ-graph --timeout=300s

# Stop monitoring
kill $MONITOR_PID 2>/dev/null || true

# Show final pod status
echo "📊 Final pod status:"
kubectl get pods -n econ-graph

echo "✅ Deployment completed successfully!"
echo ""
echo "🌐 Application URLs:"
if [ -n "$PUBLIC_HOST" ]; then
  echo "  Public:   https://${PUBLIC_HOST} (through the Cloudflare Tunnel; www redirects here)"
  echo "            check: scripts/deploy/check-public-site.sh --host ${PUBLIC_HOST} [--via-cloudflare]"
fi
# The gateway's routes (k8s/manifests/gateway/routes/) answer on host localhost.
echo "  Frontend: http://localhost"
# Admin UI: not deployed here (ECO-242).
echo "  Backend:  http://localhost/api"
echo "  GraphQL:  http://localhost/graphql"
echo "  Playground: off (set ENABLE_GRAPHQL_PLAYGROUND=true on the backend to serve /playground)"
echo "  Grafana:  kubectl port-forward service/grafana-service 3000:3000 -n econ-graph"
echo "            (admin / password: kubectl -n econ-graph get secret grafana-admin -o jsonpath={.data.admin-password} | base64 -d)"
echo ""
echo "📊 Useful commands:"
echo "  kubectl get pods -n econ-graph"
echo "  kubectl get services -n econ-graph"
echo "  kubectl logs -f deployment/econ-graph-backend -n econ-graph"
echo "  kubectl logs -f deployment/crawler-worker -n econ-graph"
echo "  kubectl logs -f deployment/econ-graph-frontend -n econ-graph"
echo "  kubectl logs -f deployment/chart-api-service -n econ-graph"
echo ""
echo "🔒 Internal Services (not exposed externally):"
echo "  Chart API Service: chart-api-service.econ-graph.svc.cluster.local:3001"
