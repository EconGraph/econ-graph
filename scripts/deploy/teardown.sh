#!/bin/bash

# Teardown EconGraph from local Kubernetes cluster
# This script removes the application and optionally the cluster
#
# Deleting the econ-graph namespace deletes the PostgreSQL PVC (and, unless
# its PV's reclaim policy has been set to Retain via
# scripts/deploy/protect-postgres-pv.sh, the underlying data with it). By
# default this script asks for confirmation before that step; pass --yes to
# skip that prompt (e.g. for CI).
#
# The PostgreSQL and Keycloak PostgreSQL data live on host btrfs subvolumes
# (static PVs in k8s/manifests/host-storage.yaml, docs/deployment/HOST_STORAGE.md).
# This script never deletes those PVs or their directories; deleting the
# namespace only leaves them Released, and the next deploy.sh re-binds them.
# Before removing anything it pg_dumps both databases to
# ~/econgraph-backups/<UTC timestamp>/ (backup-kind-databases.sh).
#
# --yes does NOT also delete the kind cluster: the cluster's nodes still hold
# the in-cluster backup volume and Grafana's data. That needs its own
# --delete-cluster flag (or the interactive prompt), never implied by --yes.

set -e

ASSUME_YES="false"
DELETE_CLUSTER_FLAG="false"
for arg in "$@"; do
    case "$arg" in
        --yes|-y) ASSUME_YES="true" ;;
        --delete-cluster) DELETE_CLUSTER_FLAG="true" ;;
    esac
done

echo "🗑️  Tearing down EconGraph from local Kubernetes cluster..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# The kind cluster runs on the rootful Docker daemon; use its own kubectl (on
# this host `kubectl` is microk8s'). Sourced first: it sets DOCKER_HOST.
# shellcheck source=scripts/deploy/lib/kind-env.sh
source "$PROJECT_ROOT/scripts/deploy/lib/kind-env.sh"
check_docker_daemon

# Check if kind cluster exists
if ! kind_cluster_exists; then
    echo "❌ Kind cluster 'econ-graph' not found on ${DOCKER_HOST}."
    exit 1
fi
ensure_kind_kubectl
kubectl config use-context kind-econ-graph

# Off-cluster copy of both databases first, while they are still running.
if pod_running postgresql-0; then
    backup_databases "$(new_backup_dir)"
else
    echo "ℹ️  postgresql-0 is not running; no pre-teardown backup taken."
fi

# Preserve both data and backup PVs before any resource or namespace deletion.
# A failed patch stops teardown before it can remove the namespace.
"$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"

# Remove application resources
echo "📋 Removing application resources..."
kubectl delete -f k8s/manifests/admin-ingress.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/graphql-ingress.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/ingress.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/frontend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/frontend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/admin-frontend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/admin-frontend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/crawler-worker.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/backend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/backend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/chart-api-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/chart-api-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres-init.yaml --ignore-not-found=true
kubectl delete cronjob postgres-backup -n econ-graph --ignore-not-found=true
# Secrets created by scripts/deploy/create-secrets.sh (the namespace deletion
# below would remove them too; listed so the intent is explicit).
kubectl -n econ-graph delete secret econ-graph-secrets econ-graph-postgres crawler-api-keys \
    econ-graph-keycloak grafana-admin monitoring-auth --ignore-not-found=true
kubectl delete -f k8s/manifests/configmap.yaml --ignore-not-found=true

echo "✅ Application resources removed successfully!"

# Deleting the namespace deletes the PostgreSQL and backup PVC objects.
# Their bound PVs have been set to Retain above, so their contents survive.
# Still confirm: the in-cluster backup PV must be re-attached by hand.
DELETE_NAMESPACE="${ASSUME_YES}"
if [ "${ASSUME_YES}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the 'econ-graph' namespace deletes its PostgreSQL PVCs."
    echo "   Their PVs are kept (Retain); the next deploy.sh re-binds the host-storage ones."
    read -p "Delete the namespace (and its PVCs)? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        DELETE_NAMESPACE="true"
    fi
fi

if [ "${DELETE_NAMESPACE}" = "true" ]; then
    kubectl delete -f k8s/manifests/namespace.yaml --ignore-not-found=true
    echo "✅ Namespace and its PVCs removed."
else
    echo "ℹ️  Namespace kept, so its PVCs (including PostgreSQL data and backups) were not deleted."
    echo "   Delete it later with: kubectl delete -f k8s/manifests/namespace.yaml"
fi

# Ask if user wants to delete the cluster. This is separate from --yes: the
# databases survive it (host storage, plus the backup taken above), but the
# in-cluster backup volume and Grafana's data do not.
DELETE_CLUSTER="${DELETE_CLUSTER_FLAG}"
if [ "${DELETE_CLUSTER_FLAG}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the kind cluster destroys everything stored in its nodes:"
    echo "   the in-cluster nightly backups and Grafana's data. The databases on"
    echo "   /srv/econgraph are kept, and so is the backup taken above."
    read -p "Do you want to delete the entire kind cluster? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        DELETE_CLUSTER="true"
    fi
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo "🗑️  Deleting kind cluster..."
    kind delete cluster --name econ-graph --kubeconfig "${KUBECONFIG}"
    echo "✅ Kind cluster deleted successfully!"
else
    echo "ℹ️  Kind cluster kept running. You can delete it later with:"
    echo "   kind delete cluster --name econ-graph"
fi
