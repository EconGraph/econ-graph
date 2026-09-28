#!/bin/bash

# Build Docker images for EconGraph
# This script builds both frontend and backend images for local K8s deployment

set -e

echo "Building EconGraph Docker images..."

# Version to tag images with (can be overridden: export VERSION=vX.Y.Z)
VERSION=${VERSION:-v3.7.4}

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Build backend image
echo "Building backend image..."
cd backend
docker build -t econ-graph-backend:${VERSION} -t econ-graph-backend:latest .
echo "Backend image built successfully"

# Build crawler-worker image (same Dockerfile, separate stage; reuses the builder stage)
echo "Building crawler-worker image..."
docker build --target crawler-worker \
  -t econ-graph-crawler-worker:${VERSION} -t econ-graph-crawler-worker:latest .
echo "Crawler-worker image built successfully"

# Build frontend image
echo "Building frontend image..."
if [ -z "${VITE_OIDC_ISSUER:-}" ]; then
  echo "Warning: VITE_OIDC_ISSUER is not set; the frontend image will have sign-in hidden." >&2
fi
cd ../frontend
docker build \
  --build-arg VITE_API_URL="http://localhost" \
  --build-arg VITE_GRAPHQL_URL="/graphql" \
  --build-arg VITE_WS_URL="ws://localhost/graphql" \
  --build-arg VITE_OIDC_ISSUER="${VITE_OIDC_ISSUER:-}" \
  --build-arg VITE_OIDC_CLIENT_ID="${VITE_OIDC_CLIENT_ID:-econ-graph-web}" \
  --build-arg NODE_ENV="production" \
  -t econ-graph-frontend:${VERSION} -t econ-graph-frontend:latest .
echo "Frontend image built successfully"

# Build chart API service image
echo "Building chart API service image..."
cd ../chart-api-service
docker build -t econ-graph-chart-api:v1.0.0 -t econ-graph-chart-api:latest .
echo "Chart API service image built successfully"

# Admin frontend image: not built here (re-enabled by ECO-242, train 2). It still
# signs in through the retired in-house login and sends `role` on createUser/
# updateUser (see #261/AUTH-5), so it gets 401 on every GraphQL call as of that PR.

# Load images into the kind cluster (nodes can't pull local-only images from
# a registry, and the manifests set imagePullPolicy: Never for these)
echo "Loading images into kind cluster 'econ-graph'..."
KIND_CLUSTER_NAME="${KIND_CLUSTER_NAME:-econ-graph}"
kind load docker-image econ-graph-backend:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-crawler-worker:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-frontend:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-chart-api:v1.0.0 --name "${KIND_CLUSTER_NAME}"
# econ-graph-admin-frontend: not loaded (re-enabled by ECO-242, train 2).

echo "All images built and loaded successfully!"
echo ""
echo "Images available in kind cluster '${KIND_CLUSTER_NAME}':"
echo "  - econ-graph-backend:${VERSION}"
echo "  - econ-graph-crawler-worker:${VERSION}"
echo "  - econ-graph-frontend:${VERSION}"
echo "  - econ-graph-chart-api:v1.0.0"
echo ""
echo "🔒 SSL Configuration:"
echo "  - Let's Encrypt issuer configured for www.econ-graph.com"
echo "  - SSL termination enabled with automatic certificate renewal"
echo "  - HTTPS redirect enforced for production domain"
