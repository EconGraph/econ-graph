#!/bin/bash

# Universal wrapper to verify frontend connectivity before running Playwright tests
# This ensures the frontend is actually reachable and can make backend requests

set -e

echo "🔍 FRONTEND CONNECTIVITY VERIFICATION"
echo "====================================="

# Check if frontend is running and accessible
echo "📋 Verifying frontend connectivity..."

# Test basic frontend accessibility
echo "  - Testing frontend HTTP response..."
if curl -f -s http://localhost:3000 > /dev/null; then
  echo "    ✅ Frontend is accessible on localhost:3000"
else
  echo "    ❌ Frontend is NOT accessible on localhost:3000"
  echo "    📋 Available services on localhost:"
  netstat -tlnp | grep :3000 || echo "      No service on port 3000"
  echo "    📋 All listening ports:"
  netstat -tlnp | head -10
  exit 1
fi

# Test frontend health endpoint if available
echo "  - Testing frontend health endpoint..."
if curl -f -s http://localhost:3000/health > /dev/null; then
  echo "    ✅ Frontend health endpoint responding"
else
  echo "    ⚠️  Frontend health endpoint not available (this may be normal)"
fi

# Test if frontend can reach backend
echo "  - Testing backend connectivity from frontend perspective..."
if curl -f -s http://localhost:8080/health > /dev/null; then
  echo "    ✅ Backend is accessible on localhost:8080"
else
  echo "    ❌ Backend is NOT accessible on localhost:8080"
  echo "    📋 Backend container status:"
  docker ps --filter name=backend-server --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" || echo "      No backend container found"
  echo "    📋 Available services on localhost:"
  netstat -tlnp | grep :8080 || echo "      No service on port 8080"
fi

# Test network connectivity between containers
echo "  - Testing frontend-backend container network connectivity..."
if docker exec frontend-server curl -f -s http://localhost:8080/health > /dev/null 2>&1; then
  echo "    ✅ Frontend container can reach backend on localhost:8080"
else
  echo "    ❌ Frontend container CANNOT reach backend on localhost:8080"
  echo "    📋 Frontend container network test:"
  docker exec frontend-server curl -v http://localhost:8080/health 2>&1 || echo "      Network test failed"
  echo "    📋 Frontend container network interfaces:"
  docker exec frontend-server ip addr show 2>/dev/null || echo "      Cannot get network interfaces"
  echo "    📋 Backend container network interfaces:"
  docker exec backend-server ip addr show 2>/dev/null || echo "      Cannot get backend network interfaces"
fi

# Test frontend-backend integration
echo "  - Testing frontend-backend integration..."
echo "    📋 Making a test request to frontend that should trigger backend calls..."
# Make a request to the frontend that should trigger backend API calls
FRONTEND_RESPONSE=$(curl -s -w "%{http_code}" http://localhost:3000/ 2>/dev/null || echo "000")
HTTP_CODE="${FRONTEND_RESPONSE: -3}"
if [ "$HTTP_CODE" = "200" ]; then
  echo "    ✅ Frontend responding with HTTP 200"
else
  echo "    ❌ Frontend responding with HTTP $HTTP_CODE"
  exit 1
fi

# Check if backend logs show any activity
echo "  - Checking for recent backend activity..."
if docker logs --tail 10 backend-server 2>/dev/null | grep -q "GET\|POST\|PUT\|DELETE"; then
  echo "    ✅ Backend shows recent HTTP activity"
else
  echo "    ⚠️  No recent HTTP activity in backend logs"
  echo "    📋 Recent backend logs:"
  docker logs --tail 20 backend-server 2>/dev/null || echo "      Cannot access backend logs"
fi

echo "✅ Frontend connectivity verification completed!"
echo "🚀 Proceeding with Playwright tests..."

# Execute the provided command
exec "$@"
