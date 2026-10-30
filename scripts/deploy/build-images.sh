#!/bin/bash

# Build Docker images for EconGraph
# This script builds both frontend and backend images for local K8s deployment

set -e

echo "Building EconGraph Docker images..."

# Get the project root directory
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$PROJECT_ROOT"

# Build on the Docker daemon the kind cluster runs on (the rootful one,
# whatever DOCKER_HOST the shell has; ECONGRAPH_DOCKER_HOST chooses another):
# `kind load docker-image` reads the images from that daemon.
# shellcheck source=scripts/deploy/lib/cluster-role.sh
source "$PROJECT_ROOT/scripts/deploy/lib/cluster-role.sh"
cluster_role_kind_docker_host
kind() { cluster_role_kind "$@"; }

# Version to tag images with, read from backend/Cargo.toml (the one place the
# version lives). k8s/manifests/backend-deployment.yaml, frontend-deployment.yaml
# and crawler-worker.yaml hard-code this same tag, so it can't be overridden
# independently: doing so would load images under a tag the manifests don't
# reference and pods would sit on ErrImageNeverPull.
WORKSPACE_VERSION="$(grep -m1 '^version' backend/Cargo.toml | sed -E 's/version[[:space:]]*=[[:space:]]*"([^"]+)"/\1/')"
EXPECTED_VERSION="v${WORKSPACE_VERSION}"
VERSION="${VERSION:-$EXPECTED_VERSION}"
if [ "${VERSION}" != "${EXPECTED_VERSION}" ]; then
  echo "❌ VERSION=${VERSION} but backend/Cargo.toml's version (${EXPECTED_VERSION}) is what the k8s manifests hard-code." >&2
  echo "   Update backend/Cargo.toml and the manifests' image tags together, or unset VERSION." >&2
  exit 1
fi

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
# The OIDC issuer is baked in at build time (Vite). The default is a path, which
# the app resolves against its own origin at runtime (frontend/src/auth/oidcConfig.ts):
# Keycloak is served under /idp on the same host as the app, so this one image
# works at http://localhost and at the public host (PUBLIC_HOST in deploy.sh,
# https://econgraph.com) without a rebuild per host. Set VITE_OIDC_ISSUER to a
# full URL only for a Keycloak on another origin; set it to the empty string
# to build with sign-in hidden.
VITE_OIDC_ISSUER="${VITE_OIDC_ISSUER-/idp/realms/econ-graph}"
VITE_OIDC_SIGN_IN_DISABLED=false
if [ -z "${VITE_OIDC_ISSUER}" ]; then
  echo "Warning: VITE_OIDC_ISSUER is empty; the frontend image will have sign-in hidden." >&2
  VITE_OIDC_SIGN_IN_DISABLED=true
fi
cd ../frontend
docker build \
  --build-arg VITE_API_URL="http://localhost" \
  --build-arg VITE_GRAPHQL_URL="/graphql" \
  --build-arg VITE_WS_URL="ws://localhost/graphql" \
  --build-arg VITE_OIDC_ISSUER="${VITE_OIDC_ISSUER}" \
  --build-arg VITE_OIDC_SIGN_IN_DISABLED="${VITE_OIDC_SIGN_IN_DISABLED}" \
  --build-arg VITE_OIDC_CLIENT_ID="${VITE_OIDC_CLIENT_ID:-econ-graph-web}" \
  --build-arg NODE_ENV="production" \
  -t econ-graph-frontend:${VERSION} -t econ-graph-frontend:latest .
echo "Frontend image built successfully"

# Build chart API service image
echo "Building chart API service image..."
cd ../chart-api-service
docker build -t econ-graph-chart-api:v1.0.0 -t econ-graph-chart-api:latest .
echo "Chart API service image built successfully"

# Admin frontend image: not built here (ECO-242). It signs in through the retired
# in-house login and sends `role` on createUser/updateUser (see #261/AUTH-5), so
# it gets 401 on every GraphQL call.

# Load images into the kind cluster (nodes can't pull local-only images from
# a registry, and the manifests set imagePullPolicy: Never for these)
echo "Loading images into kind cluster 'econ-graph'..."
KIND_CLUSTER_NAME="${KIND_CLUSTER_NAME:-econ-graph}"
kind load docker-image econ-graph-backend:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-crawler-worker:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-frontend:${VERSION} --name "${KIND_CLUSTER_NAME}"
kind load docker-image econ-graph-chart-api:v1.0.0 --name "${KIND_CLUSTER_NAME}"
# econ-graph-admin-frontend: not loaded (ECO-242).

echo "All images built and loaded successfully!"
echo ""
echo "Images available in kind cluster '${KIND_CLUSTER_NAME}':"
echo "  - econ-graph-backend:${VERSION}"
echo "  - econ-graph-crawler-worker:${VERSION}"
echo "  - econ-graph-frontend:${VERSION}"
echo "  - econ-graph-chart-api:v1.0.0"
echo ""
echo "Frontend sign-in issuer: ${VITE_OIDC_ISSUER:-<none, sign-in hidden>}"
echo "Public HTTPS is terminated by Cloudflare and reaches the cluster through the"
echo "cloudflared tunnel (k8s/manifests/cloudflared.yaml); see PUBLIC_HOST in deploy.sh."
