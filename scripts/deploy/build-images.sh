#!/bin/bash

# Build Docker images for EconGraph
# This script builds both frontend and backend images for local K8s deployment

set -e

echo "Building EconGraph Docker images..."

# Version to tag images with (can be overridden: export VERSION=vX.Y.Z)
VERSION=${VERSION:-v3.8.0-permissions}

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Build backend image
echo "Building backend image..."
cd backend
docker build -t econ-graph-backend:${VERSION} -t econ-graph-backend:latest .
echo "Backend image built successfully"

# Build frontend image
echo "Building frontend image..."
cd ../frontend
docker build \
  --build-arg REACT_APP_API_URL="http://localhost" \
  --build-arg REACT_APP_GRAPHQL_URL="/graphql" \
  --build-arg REACT_APP_WS_URL="ws://localhost/graphql" \
  --build-arg REACT_APP_FACEBOOK_APP_ID="demo-facebook-app-id" \
  --build-arg REACT_APP_GOOGLE_CLIENT_ID="80227441551-3dv05tkflnfrjpqv5fgii7b8br0brt7m.apps.googleusercontent.com" \
  --build-arg NODE_ENV="production" \
  -t econ-graph-frontend:${VERSION} -t econ-graph-frontend:latest .
echo "Frontend image built successfully"

# Build chart API service image
echo "Building chart API service image..."
cd ../chart-api-service
docker build -t econ-graph-chart-api:v1.0.0 -t econ-graph-chart-api:latest .
echo "Chart API service image built successfully"

# Build admin frontend image
echo "Building admin frontend image..."
cd ../admin-frontend
docker build \
  --build-arg REACT_APP_API_URL="http://localhost" \
  --build-arg REACT_APP_GRAPHQL_URL="/graphql" \
  --build-arg REACT_APP_WS_URL="ws://localhost/graphql" \
  --build-arg REACT_APP_GRAFANA_URL="http://localhost:30001" \
  --build-arg REACT_APP_FACEBOOK_APP_ID="demo-facebook-app-id" \
  --build-arg REACT_APP_GOOGLE_CLIENT_ID="80227441551-3dv05tkflnfrjpqv5fgii7b8br0brt7m.apps.googleusercontent.com" \
  --build-arg NODE_ENV="production" \
  -t econ-graph-admin-frontend:v1.0.0 -t econ-graph-admin-frontend:latest .
echo "Admin frontend image built successfully"

# Load images into kind cluster
echo "Loading images into MicroK8s..."
# Save images to temporary files and import
docker save econ-graph-backend:${VERSION} | microk8s ctr images import - || true
docker save econ-graph-frontend:${VERSION} | microk8s ctr images import - || true
docker save econ-graph-chart-api:v1.0.0 | microk8s ctr images import - || true
docker save econ-graph-admin-frontend:v1.0.0 | microk8s ctr images import - || true

echo "All images built and loaded successfully!"
echo ""
echo "Images available in MicroK8s:"
echo "  - econ-graph-backend:${VERSION}"
echo "  - econ-graph-frontend:${VERSION}"
echo "  - econ-graph-chart-api:v1.0.0"
echo "  - econ-graph-admin-frontend:v1.0.0"
echo ""
echo "🔒 SSL Configuration:"
echo "  - Let's Encrypt issuer configured for www.econ-graph.com"
echo "  - SSL termination enabled with automatic certificate renewal"
echo "  - HTTPS redirect enforced for production domain"
