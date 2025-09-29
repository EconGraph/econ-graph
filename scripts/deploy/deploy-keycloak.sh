#!/bin/bash

# Deploy Keycloak to local Kubernetes cluster
# This script deploys Keycloak alongside existing authentication system

set -e

echo "🔐 Deploying Keycloak to local Kubernetes cluster..."
echo "=================================================="

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found. Please run terraform first."
    echo "   cd terraform/k8s && terraform init && terraform apply"
    exit 1
fi

# Set kubectl context
kubectl config use-context kind-econ-graph

# Apply Keycloak manifests
echo "📋 Applying Keycloak Kubernetes manifests..."

# Apply in order - secrets first
echo "🔑 Creating Keycloak secrets..."
kubectl apply -f k8s/manifests/keycloak-secrets.yaml

# Apply PostgreSQL for Keycloak
echo "🗄️  Deploying Keycloak PostgreSQL..."
kubectl apply -f k8s/manifests/keycloak-postgres-service.yaml
kubectl apply -f k8s/manifests/keycloak-postgres-deployment.yaml

# Wait for Keycloak PostgreSQL to be ready
echo "⏳ Waiting for Keycloak PostgreSQL to be ready..."
kubectl wait --for=condition=ready pod -l app=keycloak-postgresql -n econ-graph --timeout=300s

# Apply Keycloak deployment
echo "🔐 Deploying Keycloak..."
kubectl apply -f k8s/manifests/keycloak-service.yaml
kubectl apply -f k8s/manifests/keycloak-deployment.yaml

# Wait for Keycloak to be ready
echo "⏳ Waiting for Keycloak deployment..."
echo "📊 Monitoring Keycloak pod status (updates every 10 seconds):"
kubectl get pods -l app=keycloak -n econ-graph
echo ""

# Start background monitoring
(
  while true; do
    sleep 10
    echo "📊 Keycloak pod status update:"
    kubectl get pods -l app=keycloak -n econ-graph
    echo ""
  done
) &
MONITOR_PID=$!

# Wait for Keycloak deployment to be available
kubectl wait --for=condition=available --timeout=600s deployment/keycloak -n econ-graph

# Stop monitoring
kill $MONITOR_PID 2>/dev/null || true

# Apply Keycloak ingress
echo "🌐 Configuring Keycloak ingress..."
kubectl apply -f k8s/manifests/keycloak-ingress.yaml

# Wait a bit for ingress to be ready
sleep 10

echo "✅ Keycloak deployment completed successfully!"
echo ""
echo "🔐 Keycloak URLs:"
echo "  Admin Console: http://auth.econ-graph.local (admin/admin123)"
echo "  Auth Endpoint: http://auth.econ-graph.local/realms/econ-graph"
echo "  Integration URL: http://admin.econ-graph.local/auth"
echo ""
echo "📊 Useful commands:"
echo "  kubectl get pods -l app=keycloak -n econ-graph"
echo "  kubectl logs -f deployment/keycloak -n econ-graph"
echo "  kubectl port-forward service/keycloak-service 8080:8080 -n econ-graph"
echo ""
echo "🔧 Keycloak Configuration:"
echo "  Realm: econ-graph"
echo "  Admin User: admin"
echo "  Admin Password: admin123"
echo "  Database: keycloak-postgresql (separate from main app database)"
echo ""
echo "⚠️  Note: Keycloak is running alongside existing authentication system"
echo "   Existing auth is still active - this is for parallel testing"
echo ""
echo "🌐 To access Keycloak admin console:"
echo "  1. Add '127.0.0.1 auth.econ-graph.local' to /etc/hosts"
echo "  2. Visit http://auth.econ-graph.local"
echo "  3. Login with admin/admin123"
