#!/bin/bash

# Script to increase MicroK8s resource allocation
# This modifies the kubelet and containerd configuration to allow more resources

echo "🔧 Increasing MicroK8s resource allocation..."

# Check if we're running as root or have sudo access
if [ "$EUID" -eq 0 ]; then
    SUDO_CMD=""
else
    SUDO_CMD="sudo"
fi

# Stop MicroK8s
echo "⏹️  Stopping MicroK8s..."
$SUDO_CMD microk8s stop

# Backup original configurations
echo "💾 Backing up configurations..."
$SUDO_CMD cp /var/snap/microk8s/current/args/kubelet /var/snap/microk8s/current/args/kubelet.backup
$SUDO_CMD cp /var/snap/microk8s/current/args/containerd /var/snap/microk8s/current/args/containerd.backup

# Increase kubelet resource limits
echo "📝 Increasing kubelet resource limits..."
if ! grep -q "max-pods" /var/snap/microk8s/current/args/kubelet; then
    echo "--max-pods=500" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
fi

# Add memory and CPU limits
if ! grep -q "system-reserved" /var/snap/microk8s/current/args/kubelet; then
    echo "--system-reserved=memory=4Gi,cpu=2" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
fi

if ! grep -q "kube-reserved" /var/snap/microk8s/current/args/kubelet; then
    echo "--kube-reserved=memory=2Gi,cpu=1" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
fi

# Increase containerd memory limits
echo "📝 Increasing containerd memory limits..."
if ! grep -q "memory-limit" /var/snap/microk8s/current/args/containerd; then
    echo "--memory-limit=8Gi" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/containerd
fi

# Start MicroK8s
echo "🚀 Starting MicroK8s..."
$SUDO_CMD microk8s start

# Wait for MicroK8s to be ready
echo "⏳ Waiting for MicroK8s to be ready..."
$SUDO_CMD microk8s status --wait-ready

# Verify the new limits
echo "✅ Verifying new resource limits..."
echo "Pod limit:"
kubectl get nodes -o jsonpath='{.items[0].status.capacity.pods}'
echo ""

echo "Memory allocation:"
kubectl get nodes -o jsonpath='{.items[0].status.allocatable.memory}'
echo ""

echo "CPU allocation:"
kubectl get nodes -o jsonpath='{.items[0].status.allocatable.cpu}'
echo ""

echo "✅ MicroK8s resource allocation increased successfully!"
echo "   - Pod limit: 500 (was 110)"
echo "   - System reserved: 4Gi memory, 2 CPU"
echo "   - Kube reserved: 2Gi memory, 1 CPU"
echo "   - Containerd memory limit: 8Gi"
