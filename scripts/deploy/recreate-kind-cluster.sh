#!/bin/bash

# Recreate kind cluster with proper port mappings for monitoring stack
set -e

echo "🔄 Recreating kind cluster with monitoring port mappings..."

# Delete existing cluster
echo "🗑️  Deleting existing kind cluster..."
kind delete cluster --name econ-graph

# Create new cluster with updated configuration
echo "🏗️  Creating new kind cluster with port mappings..."
kind create cluster --name econ-graph --config terraform/k8s/kind-config.yaml

# Wait for cluster to be ready
echo "⏳ Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=300s

echo "✅ Kind cluster recreated successfully!"
echo ""
echo "📋 Available ports:"
echo "  - Frontend: http://localhost:3000"
echo "  - Backend:  http://localhost:8080"
echo "  - Grafana:  http://localhost:30001"
echo "  - HTTP:     http://localhost:80"
echo "  - HTTPS:    https://localhost:443"
echo ""
echo "🚀 Ready to deploy the application stack!"
