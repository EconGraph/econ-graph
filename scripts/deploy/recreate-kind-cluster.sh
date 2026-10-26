#!/bin/bash

# Recreate kind cluster with proper port mappings for monitoring stack
set -e

echo "🔄 Recreating kind cluster with monitoring port mappings..."

# terraform/k8s/kind-config.yaml bind-mounts the host btrfs subvolumes that hold
# the Postgres data into econ-graph-worker (docs/deployment/HOST_STORAGE.md).
# Docker would create a missing bind-mount source as an empty root-owned
# directory, so check them before the old cluster is deleted.
for dir in /srv/econgraph/data-plane /srv/econgraph/app-prod; do
    if ! mountpoint -q "$dir"; then
        echo "❌ $dir is not mounted. Create the subvolumes first:"
        echo "   sudo scripts/host/setup-subvolumes.sh --dry-run (then without --dry-run)"
        exit 1
    fi
done

# Delete existing cluster
echo "🗑️  Deleting existing kind cluster..."
kind delete cluster --name econ-graph

# Create new cluster with updated configuration
echo "🏗️  Creating new kind cluster with port mappings..."
kind create cluster --name econ-graph --config terraform/k8s/kind-config.yaml

# Wait for cluster to be ready
echo "⏳ Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=300s

echo "✅ Kind cluster recreated successfully!"
echo ""
# The new cluster has a new kube-system UID, so an earlier production marker no
# longer counts (scripts/deploy/lib/cluster-role.sh).
echo "🚇 If this is the production cluster (the one serving econgraph.com), mark it again"
echo "   before deploying with PUBLIC_HOST: ./scripts/deploy/mark-production-cluster.sh"
echo ""
echo "📋 Available ports:"
echo "  - Frontend: http://localhost:3000"
echo "  - Backend:  http://localhost:8080"
echo "  - Grafana:  ClusterIP only; kubectl port-forward service/grafana-service 3000:3000 -n econ-graph"
echo "  - HTTP:     http://localhost:80"
echo "  - HTTPS:    https://localhost:443"
echo ""
echo "🔒 Internal Services (not exposed externally):"
echo "  - Chart API Service: chart-api-service.econ-graph.svc.cluster.local:3001"
echo ""
echo "🚀 Ready to deploy the application stack!"
