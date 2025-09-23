#!/bin/bash

# Setup kind cluster with proper port mappings for NodePort access
# This solves the issue where NodePort services aren't accessible via localhost in kind

set -e

CLUSTER_NAME="econ-graph"

echo "🔧 Setting up kind cluster with port mappings..."

# Check if cluster already exists
if kind get clusters | grep -q "$CLUSTER_NAME"; then
    echo "⚠️  Cluster $CLUSTER_NAME already exists"
    echo "   To recreate with port mappings, run:"
    echo "   kind delete cluster --name $CLUSTER_NAME"
    echo "   Then run this script again"
    exit 1
fi

# Create kind cluster with port mappings
echo "🚀 Creating kind cluster with port mappings..."
kind create cluster --name "$CLUSTER_NAME" --config - <<EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 30000
    hostPort: 30000
    protocol: TCP
  - containerPort: 30080
    hostPort: 30080
    protocol: TCP
  - containerPort: 30001
    hostPort: 30001
    protocol: TCP
  - containerPort: 30002
    hostPort: 30002
    protocol: TCP
EOF

echo "✅ Kind cluster created with port mappings:"
echo "   - Frontend: localhost:30000"
echo "   - Backend:  localhost:30080"
echo "   - Grafana:  localhost:30001"
echo "   - Admin UI: localhost:30002"

echo "🔧 Setting up ingress controller..."
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml

echo "⏳ Waiting for ingress controller to be ready..."
kubectl wait --namespace ingress-nginx \
  --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller \
  --timeout=90s

echo "✅ Kind cluster setup complete!"
echo ""
echo "🌐 NodePort services will now be accessible via localhost:"
echo "   Frontend: http://localhost:30000"
echo "   Backend:  http://localhost:30080"
echo "   Grafana:  http://localhost:30001"
echo "   Admin UI: http://localhost:30002"
echo ""
echo "📋 Next steps:"
echo "   1. Run: ./scripts/deploy/restart-k8s-rollout.sh"
echo "   2. Access services via the URLs above"
