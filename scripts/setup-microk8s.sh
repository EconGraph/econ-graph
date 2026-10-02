#!/bin/bash

# Setup MicroK8s for EconGraph development
# This script configures MicroK8s with required addons

set -e

echo "🚀 Setting up MicroK8s for EconGraph development..."
echo ""

# Check if MicroK8s is installed
if ! command -v microk8s >/dev/null 2>&1; then
    echo "❌ MicroK8s is not installed."
    echo "Please install MicroK8s first:"
    echo "  sudo snap install microk8s --classic"
    exit 1
fi

# Add user to microk8s group if not already added
if ! groups | grep -q microk8s; then
    echo "🔧 Adding user to microk8s group..."
    sudo usermod -aG microk8s $USER
    echo "⚠️  You may need to logout and login again for group changes to take effect."
    echo "   Or run: newgrp microk8s"
    newgrp microk8s
fi

# Start MicroK8s
echo "🚀 Starting MicroK8s..."
microk8s start

# Wait for MicroK8s to be ready
echo "⏳ Waiting for MicroK8s to be ready..."
microk8s status --wait-ready

# Enable required addons
echo "🔧 Enabling required addons..."
microk8s enable dns
microk8s enable ingress
microk8s enable storage
microk8s enable metrics-server

# Wait for ingress controller to be ready
echo "⏳ Waiting for ingress controller to be ready..."
microk8s kubectl wait --namespace ingress-nginx \
  --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller \
  --timeout=90s || echo "⚠️  Ingress controller may still be starting..."

# Configure kubectl
echo "🔧 Configuring kubectl..."
microk8s kubectl config view --raw > ~/.kube/config
kubectl config use-context microk8s

# Check status
echo "📊 MicroK8s status:"
microk8s status

echo ""
echo "✅ MicroK8s setup complete!"
echo ""
echo "🌐 Access your cluster:"
echo "  kubectl get nodes"
echo "  kubectl get pods --all-namespaces"
echo ""
echo "🌐 Port mappings (MicroK8s uses ingress, not NodePort):"
echo "  - Frontend: https://www.econ-graph.com (SSL)"
echo "  - Backend:  https://www.econ-graph.com/api (SSL)"
echo "  - Admin UI: https://www.econ-graph.com/admin (SSL)"
echo "  - Grafana:  not on the public ingress; kubectl port-forward service/grafana-service 3000:3000 -n econ-graph"
echo ""
echo "🔧 For local development (if needed):"
echo "  - Check ingress status: kubectl get ingress -n econ-graph"
echo "  - Check services: kubectl get svc -n econ-graph"
echo ""
echo "🔧 MicroK8s commands:"
echo "  microk8s kubectl <command>  # Use microk8s kubectl"
echo "  microk8s status             # Check status"
echo "  microk8s stop               # Stop cluster"
echo "  microk8s start              # Start cluster"
echo ""
echo "📋 Next steps:"
echo "  1. Run: ./scripts/deploy/restart-k8s-rollout.sh"
echo "  2. Your application will be available at the configured URLs"
