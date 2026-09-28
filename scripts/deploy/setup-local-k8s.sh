#!/bin/bash

# Complete setup script for local Kubernetes deployment
# This script orchestrates the entire process from cluster creation to deployment

set -e

echo "🚀 Setting up local Kubernetes cluster for EconGraph..."
echo "=================================================="

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

# Step 2: Deploy PostgreSQL in Kubernetes
# The StatefulSet reads its superuser password from this Secret.
kubectl apply -f k8s/manifests/namespace.yaml
kubectl apply -f k8s/manifests/secret.yaml
echo ""
echo "📋 Step 2: Deploying PostgreSQL in Kubernetes..."
kubectl apply -f k8s/manifests/postgres-init.yaml
kubectl apply -f k8s/manifests/postgres-deployment.yaml
kubectl apply -f k8s/manifests/postgres.yaml

echo "⏳ Waiting for PostgreSQL pod to be ready..."
kubectl wait --for=condition=ready pod -l app=postgresql -n econ-graph --timeout=300s

echo "✅ PostgreSQL is ready in Kubernetes cluster"

echo ""
echo "📋 Step 2b: Setting up PostgreSQL backups..."
kubectl apply -f k8s/manifests/postgres-backup-pvc.yaml
kubectl apply -f k8s/manifests/postgres-backup-configmap.yaml
kubectl apply -f k8s/manifests/postgres-backup-cronjob.yaml
"$PROJECT_ROOT/scripts/deploy/protect-postgres-pv.sh"
echo "✅ Nightly backups configured (see docs/deployment/POSTGRES_BACKUP_RESTORE.md)"

# Step 3: Build Docker images
echo ""
echo "📋 Step 3: Building Docker images..."
./scripts/deploy/build-images.sh

# Step 4: Deploy application
echo ""
echo "📋 Step 4: Deploying application to Kubernetes..."
./scripts/deploy/deploy.sh

echo ""
echo "🎉 Local Kubernetes setup completed successfully!"
echo "=================================================="
echo ""
echo "🌐 Your EconGraph application is now running at:"
echo "  Frontend: http://localhost/"
echo "  Backend:  http://localhost:9876"
echo "  GraphQL:  http://localhost/graphql"
echo "  Playground: http://localhost/playground"
echo ""
echo "📊 Monitor your deployment:"
echo "  kubectl get pods -n econ-graph"
echo "  kubectl get services -n econ-graph"
echo ""
echo "🔧 Useful commands:"
echo "  View logs:     kubectl logs -f deployment/econ-graph-backend -n econ-graph"
echo "  Scale backend: kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph"
echo "  Teardown:      ./scripts/deploy/teardown.sh"
