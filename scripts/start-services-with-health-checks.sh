#!/bin/bash

# This script starts frontend and backend services with proper health checks
# It retries until both services are actually ready before proceeding

set -e

echo "🚀 Starting services with health checks..."

# Load CI port configuration if available
if [ -f "ci-ports.env" ]; then
  set -a  # automatically export all variables
  source ci-ports.env
  set +a  # stop automatically exporting
  echo "📋 Loaded CI port configuration"
  echo "  - FRONTEND_URL: $FRONTEND_URL"
  echo "  - BACKEND_URL: $BACKEND_URL"
  echo "  - BACKEND_PORT: $BACKEND_PORT"
else
  echo "⚠️  ci-ports.env not found, using defaults"
  FRONTEND_URL="http://localhost:3000"
  BACKEND_URL="http://localhost:9876"
  BACKEND_PORT="9876"
fi

# Function to wait for a service to be healthy
wait_for_service() {
  local service_name="$1"
  local health_url="$2"
  local max_attempts="$3"
  local sleep_interval="$4"

  echo "🔍 Waiting for $service_name to be healthy..."
  echo "  - Health check URL: $health_url"
  echo "  - Max attempts: $max_attempts"
  echo "  - Sleep interval: ${sleep_interval}s"

  for i in $(seq 1 $max_attempts); do
    echo "  - Attempt $i/$max_attempts: Testing $health_url"

    if curl -f -s --connect-timeout 5 "$health_url" > /dev/null; then
      echo "    ✅ $service_name is healthy!"
      return 0
    else
      echo "    ⏳ $service_name not yet healthy, waiting ${sleep_interval}s..."
      sleep $sleep_interval
    fi
  done

  echo "    ❌ $service_name failed to become healthy after $max_attempts attempts"
  return 1
}

# Function to start backend with health check
start_backend() {
  echo "🚀 Starting backend service..."

  # Start backend using the debug script
  chmod +x scripts/debug-backend-startup.sh
  ./scripts/debug-backend-startup.sh

  # Wait for backend to be healthy
  wait_for_service "Backend" "$BACKEND_URL/health" 30 2

  if [ $? -eq 0 ]; then
    echo "✅ Backend service is ready!"
  else
    echo "❌ Backend service failed to start"
    echo "📋 Backend container logs:"
    docker logs backend-server 2>&1 || echo "    Cannot get backend logs"
    exit 1
  fi
}

# Function to start frontend with health check
start_frontend() {
  echo "🚀 Starting frontend service..."

  # Start frontend server
  echo "  - Using container: econ-graph-e2e-optimized:latest"
  echo "  - Command: npx serve -s build -l 3000"
  echo "  - Network: host"

  docker run --rm -d --name frontend-server \
    --network host \
    econ-graph-e2e-optimized:latest \
    npx serve -s build -l 3000

  # Debug: Check if frontend container started
  echo "📋 Frontend container status:"
  docker ps --filter name=frontend-server --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

  # Wait for frontend to be healthy
  wait_for_service "Frontend" "$FRONTEND_URL" 30 2

  if [ $? -eq 0 ]; then
    echo "✅ Frontend service is ready!"

    # Additional frontend health checks
    echo "📋 Additional frontend health checks:"
    echo "  - Testing static files:"
    curl -f -s --connect-timeout 5 "$FRONTEND_URL/static/js/main.6b919b30.js" > /dev/null && echo "    ✅ Frontend JS bundle accessible" || echo "    ❌ Frontend JS bundle NOT accessible"
    echo "  - Testing CSS files:"
    curl -f -s --connect-timeout 5 "$FRONTEND_URL/static/css/main.*.css" > /dev/null && echo "    ✅ Frontend CSS accessible" || echo "    ❌ Frontend CSS NOT accessible"
  else
    echo "❌ Frontend service failed to start"
    echo "📋 Frontend container logs:"
    docker logs frontend-server 2>&1 || echo "    Cannot get frontend logs"
    exit 1
  fi
}

# Start services in order
echo "🔧 Starting services with health checks..."

# Start backend first
start_backend

# Start frontend second
start_frontend

# Final verification
echo "🔧 Final service verification:"
echo "  - Backend health: $BACKEND_URL/health"
curl -f -s --connect-timeout 5 "$BACKEND_URL/health" > /dev/null && echo "    ✅ Backend healthy" || echo "    ❌ Backend NOT healthy"

echo "  - Frontend health: $FRONTEND_URL"
curl -f -s --connect-timeout 5 "$FRONTEND_URL" > /dev/null && echo "    ✅ Frontend healthy" || echo "    ❌ Frontend NOT healthy"

echo "✅ All services are ready for E2E tests!"
