#!/bin/bash

echo "🔍 COMPREHENSIVE BACKEND DEBUGGING START"
echo "=========================================="

# Pre-startup debugging
echo "📋 Pre-startup environment check:"
echo "  - Current directory: $(pwd)"
echo "  - Docker version: $(docker --version)"
echo "  - Available images:"
docker images | grep econ-graph || echo "    No econ-graph images found"
echo "  - Port 8080 status:"
netstat -tlnp | grep :8080 || echo "    Port 8080 is free"
echo "  - PostgreSQL connectivity test:"
nc -zv host.docker.internal 5432 && echo "    ✅ PostgreSQL reachable" || echo "    ❌ PostgreSQL not reachable"

# Environment variables debugging
echo "📋 Environment variables being passed to container:"
echo "  - DATABASE_URL: postgresql://postgres:password@host.docker.internal:5432/econ_graph_test"
echo "  - BACKEND_PORT: 8080"
echo "  - Container image: econ-graph-e2e-optimized:latest"
echo "  - Binary path: ./backend/econ-graph-backend"

# Start the backend container
echo "🚀 Starting backend container..."
docker run --rm -d --name backend-server \
  -p 8080:8080 \
  -e DATABASE_URL="${DATABASE_URL}" \
  -e BACKEND_PORT=8080 \
  econ-graph-e2e-optimized:latest \
  ./backend/econ-graph-backend

# Get container ID for debugging
CONTAINER_ID=$(docker ps -q --filter name=backend-server)
echo "📋 Container started with ID: $CONTAINER_ID"

# Start real-time log tailing in background
echo "📋 Starting real-time backend log tailing..."
docker logs -f backend-server &
LOG_TAIL_PID=$!

# Function to stop log tailing
stop_log_tailing() {
  if [ ! -z "$LOG_TAIL_PID" ]; then
    echo "🛑 Stopping backend log tailing..."
    kill $LOG_TAIL_PID 2>/dev/null || true
    wait $LOG_TAIL_PID 2>/dev/null || true
  fi
}

# Set trap to stop log tailing on exit
trap stop_log_tailing EXIT

# Comprehensive startup monitoring
echo "⏳ Monitoring backend startup (30 attempts, 2s intervals)..."
BACKEND_READY=false

for i in {1..30}; do
  echo "🔍 Attempt $i/30 - Comprehensive health check:"

  # Check if container is still running
  if ! docker ps --filter name=backend-server --format "table {{.Names}}" | grep -q backend-server; then
    echo "  ❌ Container stopped running!"
    echo "  📋 Container exit code: $(docker inspect backend-server --format='{{.State.ExitCode}}' 2>/dev/null || echo 'unknown')"
    break
  fi

  # Check if port is listening
  if netstat -tlnp | grep -q :8080; then
    echo "  ✅ Port 8080 is listening"
  else
    echo "  ❌ Port 8080 not listening"
  fi

  # Check health endpoint
  if curl -f http://localhost:8080/health 2>/dev/null; then
    echo "  ✅ Health endpoint responding"
    echo "✅ Backend is ready!"
    BACKEND_READY=true
    stop_log_tailing
    break
  else
    echo "  ❌ Health endpoint not responding"
  fi

  # Show container status
  echo "  📋 Container status:"
  docker ps --filter name=backend-server --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

  echo "⏳ Waiting for backend... ($i/30)"
  sleep 2
done

# Final comprehensive failure analysis
if [ "$BACKEND_READY" = false ]; then
  echo "❌ BACKEND STARTUP FAILED - COMPREHENSIVE DIAGNOSIS:"
  echo "=================================================="

  # Container status
  echo "📋 Container Status:"
  docker ps -a --filter name=backend-server --format "table {{.Names}}\t{{.Status}}\t{{.ExitCode}}\t{{.Ports}}"

  # Container logs
  echo "📋 Container Logs (last 50 lines):"
  docker logs --tail 50 backend-server 2>&1 || echo "No logs available"

  # Container inspection
  echo "📋 Container Inspection:"
  docker inspect backend-server --format='{{json .State}}' | jq . 2>/dev/null || docker inspect backend-server

  # Network debugging
  echo "📋 Network Debugging:"
  echo "  - Port 8080 status:"
  netstat -tlnp | grep :8080 || echo "    Port 8080 not listening"
  echo "  - Docker network info:"
  docker network ls
  echo "  - Container network:"
  docker inspect backend-server --format='{{json .NetworkSettings}}' | jq . 2>/dev/null || echo "Network inspection failed"

  # Process debugging
  echo "📋 Process Debugging:"
  echo "  - Processes in container:"
  docker exec backend-server ps aux 2>/dev/null || echo "Cannot exec into container"
  echo "  - Container filesystem:"
  docker exec backend-server ls -la /app/backend/ 2>/dev/null || echo "Cannot list backend directory"

  # Environment debugging
  echo "📋 Environment Debugging:"
  echo "  - Environment variables in container:"
  docker exec backend-server env | grep -E "(DATABASE_URL|BACKEND_PORT|USER|PG)" 2>/dev/null || echo "Cannot get environment variables"

  # Database connectivity from container
  echo "📋 Database Connectivity from Container:"
  echo "  - Testing database connection from inside container:"
  docker exec backend-server nc -zv host.docker.internal 5432 2>&1 || echo "Cannot test database connectivity"
  echo "  - Testing with psql from container:"
  docker exec backend-server psql "postgresql://postgres:password@host.docker.internal:5432/econ_graph_test" -c "SELECT 1;" 2>&1 || echo "Cannot test database with psql"

  # Backend binary debugging
  echo "📋 Backend Binary Debugging:"
  echo "  - Binary exists:"
  docker exec backend-server ls -la /app/backend/econ-graph-backend 2>/dev/null || echo "Binary not found"
  echo "  - Binary is executable:"
  docker exec backend-server test -x /app/backend/econ-graph-backend && echo "    ✅ Executable" || echo "    ❌ Not executable"
  echo "  - Binary version:"
  docker exec backend-server /app/backend/econ-graph-backend --version 2>&1 || echo "Cannot get version"

  # System resources
  echo "📋 System Resources:"
  echo "  - Memory usage:"
  free -h
  echo "  - Disk usage:"
  df -h
  echo "  - Docker system info:"
  docker system df

  stop_log_tailing
  exit 1
fi

echo "✅ Backend startup completed successfully!"
