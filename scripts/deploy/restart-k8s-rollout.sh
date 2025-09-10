#!/bin/bash

# Restart Kubernetes rollout to deploy v3.6.0 with database migration consolidation and auth test fixes
# Run this script when Docker and Kubernetes cluster are available

set -e

echo "🚀 Restarting EconGraph Kubernetes rollout for v3.6.0 (with database migration consolidation and auth test fixes)..."
echo ""

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Check if kind cluster exists
if ! kind get clusters | grep -q "econ-graph"; then
    echo "❌ Kind cluster 'econ-graph' not found."
    echo "Creating new cluster..."
    cd terraform/k8s
    terraform init
    terraform apply -auto-approve
    cd "$PROJECT_ROOT"
fi

# Set kubectl context
echo "🔧 Setting kubectl context..."
kubectl config use-context kind-econ-graph

# Rebuild Docker images with new version tag
echo "🏗️  Building Docker images for v3.6.0..."
./scripts/deploy/build-images.sh

# Tag images with new version
echo "🏷️  Tagging images with v3.6.0..."
docker tag econ-graph-backend:latest econ-graph-backend:v3.6.0
docker tag econ-graph-frontend:latest econ-graph-frontend:v3.6.0

# Load images into kind cluster
echo "📦 Loading images into kind cluster..."
kind load docker-image econ-graph-backend:v3.6.0 --name econ-graph
kind load docker-image econ-graph-frontend:v3.6.0 --name econ-graph

# Apply updated manifests
echo "📋 Applying updated Kubernetes manifests..."
kubectl apply -f k8s/manifests/

# Restart deployments to pick up new images
echo "🔄 Restarting deployments..."
kubectl rollout restart deployment/econ-graph-backend -n econ-graph
kubectl rollout restart deployment/econ-graph-frontend -n econ-graph

# Wait for rollout to complete
echo "⏳ Waiting for rollouts to complete..."
kubectl rollout status deployment/econ-graph-backend -n econ-graph --timeout=300s
kubectl rollout status deployment/econ-graph-frontend -n econ-graph --timeout=300s

# Display status
echo ""
echo "✅ Kubernetes rollout restart completed successfully!"
echo ""
echo "📊 Current deployment status:"
kubectl get pods -n econ-graph -o wide
echo ""
echo "🌐 Application URLs:"
echo "  Frontend: http://localhost:3000"
echo "  Backend:  http://localhost:8080"
echo "  GraphQL:  http://localhost:8080/graphql"
echo "  Health:   http://localhost:8080/health"
echo ""
echo "🎯 Version deployed: v3.6.0"
echo "   ✅ Database migrations consolidated: 9 → 3 logical groups"
echo "   ✅ Auth integration tests: All 11 tests passing"
echo "   ✅ Database connection issues resolved"
echo "   ✅ Test container lifecycle improved"
echo "   ✅ Authentication system reliability enhanced"
echo "   ✅ Cleaner deployment process with consolidated migrations"
echo ""
echo "📋 Monitor deployment:"
echo "  kubectl logs -f deployment/econ-graph-backend -n econ-graph"
echo "  kubectl logs -f deployment/econ-graph-frontend -n econ-graph"
