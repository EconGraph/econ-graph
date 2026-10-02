#!/bin/bash

# Setup port forwarding for EconGraph services
# This script sets up port forwarding for all services that need external access

set -e

echo "🔗 Setting up port forwarding for EconGraph services..."
echo ""

# Check if kubectl is available and cluster is accessible
if ! kubectl cluster-info >/dev/null 2>&1; then
    echo "❌ Cannot connect to Kubernetes cluster. Please ensure kubectl is configured."
    exit 1
fi

# Set kubectl context
echo "🔧 Setting kubectl context..."
kubectl config use-context kind-econ-graph

# Check if services exist
echo "🔍 Checking if services are available..."
if ! kubectl get service grafana-service -n econ-graph >/dev/null 2>&1; then
    echo "❌ Grafana service not found. Please deploy the monitoring stack first."
    exit 1
fi

# Kill any existing port forwarding processes
echo "🧹 Cleaning up existing port forwarding processes..."
pkill -f "kubectl port-forward.*grafana-service" || true
pkill -f "kubectl port-forward.*econ-graph-backend-service" || true

# Start Grafana port forwarding
echo "📊 Starting Grafana port forwarding..."
kubectl port-forward -n econ-graph service/grafana-service 3000:3000 &
GRAFANA_PID=$!
echo "  ✅ Grafana port forwarding started (PID: $GRAFANA_PID)"
echo "     URL: http://localhost:3000"
echo "     Username: admin"
echo "     Password: kubectl -n econ-graph get secret grafana-admin -o jsonpath={.data.admin-password} | base64 -d"

# Start Backend port forwarding (if needed)
echo "🔧 Starting Backend port forwarding..."
kubectl port-forward -n econ-graph service/econ-graph-backend-service 9876:9876 &
BACKEND_PID=$!
echo "  ✅ Backend port forwarding started (PID: $BACKEND_PID)"
echo "     URL: http://localhost:9876"

# Wait a moment for port forwarding to establish
sleep 3

# Test connectivity
echo ""
echo "🧪 Testing connectivity..."
if curl -s -o /dev/null -w "%{http_code}" http://localhost:3000 | grep -q "302\|200"; then
    echo "  ✅ Grafana is accessible"
else
    echo "  ❌ Grafana is not accessible"
fi

if curl -s -o /dev/null -w "%{http_code}" http://localhost:9876/health | grep -q "200"; then
    echo "  ✅ Backend is accessible"
else
    echo "  ❌ Backend is not accessible"
fi

echo ""
echo "🎉 Port forwarding setup complete!"
echo ""
echo "📋 Service URLs:"
echo "  Frontend: http://localhost/"
echo "  Backend:  http://localhost:9876"
echo "  Grafana:  http://localhost:3000 (user admin, password in Secret grafana-admin)"
echo ""
echo "🛑 To stop port forwarding:"
echo "  kill $GRAFANA_PID"
echo "  kill $BACKEND_PID"
echo ""
echo "💡 Tip: Run this script again if port forwarding stops working"
