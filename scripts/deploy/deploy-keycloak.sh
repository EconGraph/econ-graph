#!/bin/bash

# Deploy Keycloak to Kubernetes cluster
# This script deploys Keycloak with its dedicated PostgreSQL instance

set -e

echo "🔐 Deploying Keycloak to Kubernetes cluster..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Check if kubectl context is set
if ! kubectl config current-context >/dev/null 2>&1; then
    echo "❌ No kubectl context set. Please configure kubectl first."
    exit 1
fi

# Check if econ-graph namespace exists
if ! kubectl get namespace econ-graph >/dev/null 2>&1; then
    echo "❌ econ-graph namespace not found. Please deploy the main application first."
    exit 1
fi

echo "📋 Applying Keycloak Kubernetes manifests..."

# Deploy Keycloak PostgreSQL first
echo "🗄️  Deploying Keycloak PostgreSQL..."
kubectl apply -f k8s/manifests/keycloak-db-init.yaml
kubectl apply -f k8s/manifests/keycloak-postgres-deployment.yaml
kubectl apply -f k8s/manifests/keycloak-postgres-service.yaml

# Wait for Keycloak PostgreSQL to be ready
echo "⏳ Waiting for Keycloak PostgreSQL to be ready..."
kubectl wait --for=condition=ready pod -l app=keycloak-postgres -n econ-graph --timeout=300s

# Deploy Keycloak secrets
echo "🔒 Deploying Keycloak secrets..."
kubectl apply -f k8s/manifests/keycloak-secrets.yaml

# Deploy Keycloak
echo "🔐 Deploying Keycloak..."
kubectl apply -f k8s/manifests/keycloak-deployment.yaml
kubectl apply -f k8s/manifests/keycloak-service.yaml

# Deploy Keycloak ingress
echo "🌐 Deploying Keycloak ingress..."
kubectl apply -f k8s/manifests/keycloak-ingress.yaml

# Wait for Keycloak to be ready
echo "⏳ Waiting for Keycloak to be ready..."
kubectl wait --for=condition=ready pod -l app=keycloak -n econ-graph --timeout=300s

echo "✅ Keycloak deployment completed successfully!"
echo ""
echo "🌐 Keycloak URLs:"
echo "  Admin Console: http://auth.econ-graph.local/admin (admin/admin123)"
echo "  Auth Server: http://auth.econ-graph.local"
echo ""
echo "📋 Useful commands:"
echo "  kubectl get pods -l app=keycloak -n econ-graph"
echo "  kubectl logs -f deployment/keycloak -n econ-graph"
echo "  kubectl get pods -l app=keycloak-postgres -n econ-graph"
echo "  kubectl logs -f deployment/keycloak-postgres -n econ-graph"
echo ""
echo "🔧 Next steps:"
echo "  1. Add '127.0.0.1 auth.econ-graph.local' to /etc/hosts"
echo "  2. Run ./scripts/deploy/configure-keycloak.sh to set up realms and clients"
echo "  3. Update backend configuration to use Keycloak authentication"
