#!/bin/bash
set -e

# Docker Image Builder Script
# Builds all required Docker images for E2E testing

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")/docker"

cd "$DOCKER_DIR"

echo "🔨 Building Docker images for E2E testing..."

# Build backend image
echo "📦 Building backend image..."
docker-compose -f docker-compose.unified.yml build backend

# Build frontend image
echo "📦 Building frontend image..."
docker-compose -f docker-compose.unified.yml build frontend

# Build test runner image
echo "📦 Building test runner image..."
docker-compose -f docker-compose.unified.yml build test-runner

echo "✅ All Docker images built successfully!"
echo "📋 Available images:"
docker images | grep -E "(backend|frontend|test-runner)" | head -10
