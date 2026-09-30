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
# --yes does NOT also delete the kind cluster: on kind, the cluster's nodes
# are the storage backing every PV, Retain policy included, so deleting the
# cluster destroys the database and its backups regardless. That needs its
# own --delete-cluster flag (or the interactive prompt), never implied by
# --yes.

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

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found."
    exit 1
fi

# Set kubectl context
kubectl config use-context kind-econ-graph

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
# Still confirm because reattaching Released volumes requires manual work.
DELETE_NAMESPACE="${ASSUME_YES}"
if [ "${ASSUME_YES}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the 'econ-graph' namespace deletes its PostgreSQL PVCs."
    echo "   Their retained PVs must be manually reattached after deletion."
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

# Ask if user wants to delete the cluster. This is separate from --yes: on
# kind, the cluster's node holds every PV's actual data (Retain policy
# included), so this step destroys the database and its backups no matter
# what happened above.
DELETE_CLUSTER="${DELETE_CLUSTER_FLAG}"
if [ "${DELETE_CLUSTER_FLAG}" != "true" ]; then
    echo ""
    echo "⚠️  Deleting the kind cluster destroys ALL its data, including the"
    echo "   PostgreSQL volume and its backups (Retain policy doesn't help here:"
    echo "   the cluster's node is what actually stores the data)."
    read -p "Do you want to delete the entire kind cluster? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        DELETE_CLUSTER="true"
    fi
fi

if [ "${DELETE_CLUSTER}" = "true" ]; then
    echo "🗑️  Deleting kind cluster..."
    kind delete cluster --name econ-graph
    echo "✅ Kind cluster deleted successfully!"
else
    echo "ℹ️  Kind cluster kept running. You can delete it later with:"
    echo "   kind delete cluster --name econ-graph"
fi
