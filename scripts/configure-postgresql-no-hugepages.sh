#!/bin/bash

# This script configures PostgreSQL to disable hugepages during initialization.
# This can help resolve "Bus error (core dumped)" issues during initdb.

set -e

echo "🔧 Configuring PostgreSQL to disable hugepages..."

# Check if MicroK8s is running
if ! microk8s status >/dev/null 2>&1; then
    echo "❌ MicroK8s is not running. Please start it first:"
    echo "   microk8s start"
    exit 1
fi

# Check if kubectl is configured
if ! kubectl get nodes >/dev/null 2>&1; then
    echo "❌ kubectl is not configured. Please run:"
    echo "   microk8s kubectl config view --raw > ~/.kube/config"
    echo "   kubectl config use-context microk8s"
    exit 1
fi

# Check if PostgreSQL StatefulSet exists
if ! kubectl get statefulset postgresql -n econ-graph >/dev/null 2>&1; then
    echo "❌ PostgreSQL StatefulSet not found in econ-graph namespace."
    echo "   Please deploy the application first."
    exit 1
fi

echo "📝 Patching PostgreSQL StatefulSet to disable hugepages..."

# Patch the StatefulSet to add POSTGRES_INITDB_ARGS environment variable
kubectl patch statefulset postgresql -n econ-graph -p '{
  "spec": {
    "template": {
      "spec": {
        "containers": [
          {
            "name": "postgresql",
            "env": [
              {
                "name": "POSTGRES_INITDB_ARGS",
                "value": "--set huge_pages=off"
              }
            ]
          }
        ]
      }
    }
  }
}'

echo "✅ PostgreSQL StatefulSet patched successfully!"

echo "🔄 Restarting PostgreSQL to apply changes..."

# Delete the existing pod to force recreation with new configuration
kubectl delete pod postgresql-0 -n econ-graph

echo "⏳ Waiting for PostgreSQL to be ready..."
kubectl wait --for=condition=ready pod postgresql-0 -n econ-graph --timeout=300s

echo "✅ PostgreSQL configured with hugepages disabled!"
echo ""
echo "📋 What this does:"
echo "   - Sets POSTGRES_INITDB_ARGS='--set huge_pages=off'"
echo "   - Forces PostgreSQL to disable hugepages during initialization"
echo "   - May resolve 'Bus error (core dumped)' issues"
echo ""
echo "🔍 To verify the configuration:"
echo "   kubectl describe pod postgresql-0 -n econ-graph"
echo "   kubectl logs postgresql-0 -n econ-graph"
