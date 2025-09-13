#!/bin/bash

# Teardown EconGraph from local Kubernetes cluster
# This script removes the application and optionally the cluster

set -e

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

# Remove application resources
echo "📋 Removing application resources..."
kubectl delete -f k8s/manifests/admin-ingress.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/ingress.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/frontend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/frontend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/admin-frontend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/admin-frontend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/backend-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/backend-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/chart-api-service.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/chart-api-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres-deployment.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/postgres-init.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/secret.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/configmap.yaml --ignore-not-found=true
kubectl delete -f k8s/manifests/namespace.yaml --ignore-not-found=true

echo "✅ Application resources removed successfully!"

# Ask if user wants to delete the cluster
read -p "Do you want to delete the entire kind cluster? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🗑️  Deleting kind cluster..."
    kind delete cluster --name econ-graph
    echo "✅ Kind cluster deleted successfully!"
else
    echo "ℹ️  Kind cluster kept running. You can delete it later with:"
    echo "   kind delete cluster --name econ-graph"
fi
