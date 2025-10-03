#!/bin/bash

# Complete MicroK8s setup for EconGraph development
# This script provides the same functionality as the kind setup script
#
# For detailed MicroK8s documentation, see: docs/deployment/MICROK8S_DEPLOYMENT.md
# For troubleshooting, see the troubleshooting section in the documentation

set -e

echo "🚀 Setting up MicroK8s for EconGraph development (complete setup)..."
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

# Create namespace
echo "📋 Creating econ-graph namespace..."
kubectl create namespace econ-graph --dry-run=client -o yaml | kubectl apply -f -

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
echo "🌐 Production URLs (with SSL):"
echo "  - Frontend: https://www.econ-graph.com"
echo "  - Backend:  https://www.econ-graph.com/api"
echo "  - Grafana:  https://www.econ-graph.com/grafana"
echo "  - Admin UI: https://www.econ-graph.com/admin"
echo ""
echo "🔧 For local development:"
echo "  - Check ingress status: kubectl get ingress -n econ-graph"
echo "  - Check services: kubectl get svc -n econ-graph"
echo "  - Check pods: kubectl get pods -n econ-graph"
echo ""
echo "🔧 MicroK8s commands:"
echo "  microk8s kubectl <command>  # Use microk8s kubectl"
echo "  microk8s status             # Check status"
echo "  microk8s stop               # Stop cluster"
echo "  microk8s start              # Start cluster"
echo ""
echo "📋 Next steps:"
echo "  1. Run: ./scripts/deploy/restart-k8s-rollout.sh"
echo "  2. Your application will be available at the production URLs above"
echo ""
echo "🔒 Security features enabled:"
echo "  - SSL/TLS termination with Let's Encrypt"
echo "  - CORS configuration"
echo "  - Security headers"
echo "  - Rate limiting"
echo "  - Network policies"
