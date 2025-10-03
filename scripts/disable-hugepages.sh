#!/bin/bash

# Script to disable hugepages in MicroK8s kubelet configuration
# This may resolve PostgreSQL bus errors caused by memory alignment issues

echo "🔧 Disabling hugepages in MicroK8s kubelet configuration..."

# Check if we're running as root or have sudo access
if [ "$EUID" -eq 0 ]; then
    SUDO_CMD=""
else
    SUDO_CMD="sudo"
fi

# Stop MicroK8s
echo "⏹️  Stopping MicroK8s..."
$SUDO_CMD microk8s stop

# Backup original kubelet configuration
echo "💾 Backing up kubelet configuration..."
$SUDO_CMD cp /var/snap/microk8s/current/args/kubelet /var/snap/microk8s/current/args/kubelet.backup

# Add hugepages disable argument to kubelet configuration
echo "📝 Disabling hugepages in kubelet configuration..."
if ! grep -q "disable-hugepages" /var/snap/microk8s/current/args/kubelet; then
    echo "--disable-hugepages" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
else
    echo "⚠️  hugepages already disabled"
fi

# Start MicroK8s
echo "🚀 Starting MicroK8s..."
$SUDO_CMD microk8s start

# Wait for MicroK8s to be ready
echo "⏳ Waiting for MicroK8s to be ready..."
$SUDO_CMD microk8s status --wait-ready

# Verify hugepages are disabled
echo "✅ Verifying hugepages configuration..."
kubectl get nodes -o jsonpath='{.items[0].status.capacity.hugepages-2Mi}'
echo " hugepages-2Mi available"

echo "✅ Hugepages disabled successfully!"
echo "   This may resolve PostgreSQL bus errors caused by memory alignment issues"
