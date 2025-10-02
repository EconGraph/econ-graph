#!/bin/bash

# Script to increase MicroK8s pod limit
# This modifies the kubelet configuration to allow more pods per node

echo "🔧 Increasing MicroK8s pod limit..."

# Check if we're running as root or have sudo access
if [ "$EUID" -eq 0 ]; then
    SUDO_CMD=""
else
    SUDO_CMD="sudo"
fi

# Stop MicroK8s
echo "⏹️  Stopping MicroK8s..."
$SUDO_CMD microk8s stop

# Backup the original kubelet configuration
echo "💾 Backing up kubelet configuration..."
$SUDO_CMD cp /var/snap/microk8s/current/args/kubelet /var/snap/microk8s/current/args/kubelet.backup

# Add max-pods argument to kubelet configuration
echo "📝 Adding max-pods=500 to kubelet configuration..."
if ! grep -q "max-pods" /var/snap/microk8s/current/args/kubelet; then
    echo "--max-pods=500" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
else
    echo "⚠️  max-pods already configured"
fi

# Start MicroK8s
echo "🚀 Starting MicroK8s..."
$SUDO_CMD microk8s start

# Wait for MicroK8s to be ready
echo "⏳ Waiting for MicroK8s to be ready..."
$SUDO_CMD microk8s status --wait-ready

# Verify the new pod limit
echo "✅ Verifying new pod limit..."
kubectl get nodes -o jsonpath='{.items[0].status.capacity.pods}'
echo " pods available"

echo "✅ Pod limit increased successfully!"
echo "   New limit: 500 pods per node"
echo "   Previous limit: 110 pods per node"
