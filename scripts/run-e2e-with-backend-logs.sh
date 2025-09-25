#!/bin/bash

# Script to run E2E tests with continuous backend log tailing
# This ensures we capture backend logs throughout the entire test duration

set -e

echo "🔍 E2E Test Runner with Backend Log Tailing"
echo "============================================="

# Check if backend container exists and is running
if ! docker ps --filter name=backend-server --format "table {{.Names}}" | grep -q backend-server; then
  echo "❌ Backend container not found or not running!"
  echo "📋 Available containers:"
  docker ps --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
  exit 1
fi

echo "✅ Backend container found, starting log tailing..."

# Start backend log tailing in background
echo "📋 Starting continuous backend log tailing..."
docker logs -f backend-server &
BACKEND_LOG_PID=$!

# Function to stop log tailing
stop_backend_logs() {
  if [ ! -z "$BACKEND_LOG_PID" ]; then
    echo "🛑 Stopping backend log tailing..."
    kill $BACKEND_LOG_PID 2>/dev/null || true
    wait $BACKEND_LOG_PID 2>/dev/null || true
  fi
}

# Set trap to stop log tailing on exit
trap stop_backend_logs EXIT

# Debug: Show what URLs Playwright will be using
echo "🔧 Playwright URL Configuration:"
echo "  - FRONTEND_URL: $FRONTEND_URL"
echo "  - BACKEND_URL: $BACKEND_URL"
echo "  - REACT_APP_GRAPHQL_ENDPOINT: $REACT_APP_GRAPHQL_ENDPOINT"
echo "  - REACT_APP_API_URL: $REACT_APP_API_URL"
echo "  - REACT_APP_GRAPHQL_URL: $REACT_APP_GRAPHQL_URL"
echo "  - REACT_APP_BACKEND_PORT: $REACT_APP_BACKEND_PORT"
echo ""

# Debug: Test if the URLs are actually reachable
echo "🔧 URL Reachability Testing:"
echo "  - Testing FRONTEND_URL: $FRONTEND_URL"
if curl -f -s --connect-timeout 5 "$FRONTEND_URL" > /dev/null; then
  echo "    ✅ Frontend URL is reachable"
else
  echo "    ❌ Frontend URL is NOT reachable (connection refused/timeout)"
fi

echo "  - Testing BACKEND_URL: $BACKEND_URL"
if curl -f -s --connect-timeout 5 "$BACKEND_URL/health" > /dev/null; then
  echo "    ✅ Backend URL is reachable"
else
  echo "    ❌ Backend URL is NOT reachable (connection refused/timeout)"
fi

echo "  - Testing GraphQL endpoint: $REACT_APP_GRAPHQL_ENDPOINT"
if curl -f -s --connect-timeout 5 -X POST \
  -H "Content-Type: application/json" \
  -d '{"query":"query { __typename }"}' \
  "$REACT_APP_GRAPHQL_ENDPOINT" > /dev/null; then
  echo "    ✅ GraphQL endpoint is reachable"
else
  echo "    ❌ GraphQL endpoint is NOT reachable (connection refused/timeout)"
fi

# Debug: Check what ports are actually listening
echo "🔧 Port Listening Check:"
echo "  - Ports listening on localhost:"
netstat -tlnp | grep -E ":(3000|51249|8080|9876)" || echo "    No expected ports found listening"
echo "  - All listening ports:"
netstat -tlnp | head -10 || echo "    Cannot get port information"

echo ""

# Run the actual Playwright tests
echo "🚀 Running Playwright E2E tests with backend logs..."
echo "📋 Backend logs will be streamed continuously during test execution"
echo "📋 Look for '🌐 Playwright Network Request:' messages to see what URLs are being hit"
echo ""

# Execute the provided command (Playwright test command)
# Use exec to ensure proper exit code propagation
exec "$@"
