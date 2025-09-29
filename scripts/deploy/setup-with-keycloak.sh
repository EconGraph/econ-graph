#!/bin/bash

# Complete setup script for local Kubernetes deployment with Keycloak
# This script orchestrates the entire process including Keycloak integration

set -e

echo "🚀 Setting up local Kubernetes cluster with Keycloak for EconGraph..."
echo "=================================================================="

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Step 1: Create Kubernetes cluster with Terraform
echo "📋 Step 1: Creating Kubernetes cluster with Terraform..."
cd terraform/k8s

# Initialize Terraform if needed
if [ ! -d ".terraform" ]; then
    echo "Initializing Terraform..."
    terraform init
fi

# Apply Terraform configuration
echo "Applying Terraform configuration..."
terraform apply -auto-approve

cd "$PROJECT_ROOT"

# Step 2: Deploy PostgreSQL for main application
echo ""
echo "📋 Step 2: Deploying main PostgreSQL in Kubernetes..."
kubectl apply -f k8s/manifests/postgres-init.yaml
kubectl apply -f k8s/manifests/postgres-deployment.yaml
kubectl apply -f k8s/manifests/postgres.yaml

echo "⏳ Waiting for main PostgreSQL pod to be ready..."
kubectl wait --for=condition=ready pod -l app=postgresql -n econ-graph --timeout=300s

echo "✅ Main PostgreSQL is ready in Kubernetes cluster"

# Step 3: Build Docker images
echo ""
echo "📋 Step 3: Building Docker images..."
./scripts/deploy/build-images.sh

# Step 4: Deploy main application
echo ""
echo "📋 Step 4: Deploying main application to Kubernetes..."
./scripts/deploy/deploy.sh

# Step 5: Deploy Keycloak
echo ""
echo "📋 Step 5: Deploying Keycloak..."
./scripts/deploy/deploy-keycloak.sh

# Step 6: Configure Keycloak
echo ""
echo "📋 Step 6: Configuring Keycloak realm and clients..."
echo "⏳ Waiting for Keycloak to be fully ready..."
sleep 30

# Check if Keycloak is accessible
MAX_RETRIES=30
RETRY_COUNT=0
while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if curl -s -f http://auth.econ-graph.local/health/ready > /dev/null 2>&1; then
        echo "✅ Keycloak is ready"
        break
    fi
    echo "⏳ Waiting for Keycloak to be ready... (attempt $((RETRY_COUNT + 1))/$MAX_RETRIES)"
    sleep 10
    RETRY_COUNT=$((RETRY_COUNT + 1))
done

if [ $RETRY_COUNT -eq $MAX_RETRIES ]; then
    echo "⚠️  Keycloak may not be fully ready yet. You can configure it manually later."
    echo "   Run: ./scripts/deploy/configure-keycloak.sh"
else
    echo "🔧 Configuring Keycloak..."
    ./scripts/deploy/configure-keycloak.sh
fi

# Step 7: Apply network policies
echo ""
echo "📋 Step 7: Applying network policies..."
kubectl apply -f k8s/manifests/keycloak-network-policy.yaml

echo ""
echo "🎉 Complete setup with Keycloak completed successfully!"
echo "====================================================="
echo ""
echo "🌐 Your EconGraph application is now running at:"
echo "  Frontend: http://admin.econ-graph.local"
echo "  Backend:  http://admin.econ-graph.local/api"
echo "  GraphQL:  http://admin.econ-graph.local/graphql"
echo "  Playground: http://admin.econ-graph.local/playground"
echo ""
echo "🔐 Keycloak is running at:"
echo "  Admin Console: http://auth.econ-graph.local (admin/admin123)"
echo "  Auth Endpoint: http://auth.econ-graph.local/realms/econ-graph"
echo ""
echo "📊 Monitor your deployment:"
echo "  kubectl get pods -n econ-graph"
echo "  kubectl get services -n econ-graph"
echo ""
echo "🔧 Useful commands:"
echo "  View logs:     kubectl logs -f deployment/econ-graph-backend -n econ-graph"
echo "  View Keycloak: kubectl logs -f deployment/keycloak -n econ-graph"
echo "  Scale backend: kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph"
echo "  Teardown:      ./scripts/deploy/teardown.sh"
echo ""
echo "⚠️  Important Notes:"
echo "  - Existing authentication system is still active"
echo "  - Keycloak is running in parallel for testing"
echo "  - Add '127.0.0.1 auth.econ-graph.local' to /etc/hosts to access Keycloak"
echo "  - Add '127.0.0.1 admin.econ-graph.local' to /etc/hosts to access the application"
echo ""
echo "🔐 Authentication Systems:"
echo "  - Legacy Auth: Still active (Google/Facebook OAuth + JWT)"
echo "  - Keycloak Auth: New system (OIDC + fine-grained permissions)"
echo ""
echo "📋 Next Steps:"
echo "  1. Test existing authentication (should work as before)"
echo "  2. Test Keycloak authentication (new parallel system)"
echo "  3. Configure Keycloak clients and permissions"
echo "  4. Plan migration strategy from legacy to Keycloak"
