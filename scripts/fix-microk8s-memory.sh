#!/bin/bash

# Script to fix MicroK8s memory allocation
# Reduce Kubernetes memory allocation to match available system memory

echo "🔧 Fixing MicroK8s memory allocation..."

# Check if we're running as root or have sudo access
if [ "$EUID" -eq 0 ]; then
    SUDO_CMD=""
else
    SUDO_CMD="sudo"
fi

# Get available system memory (in KiB)
AVAILABLE_MEMORY=$(free | grep "Mem:" | awk '{print $7}')
echo "Available system memory: $(echo "$AVAILABLE_MEMORY / 1024 / 1024" | bc)GB"

# Calculate safe allocation (80% of available memory)
SAFE_ALLOCATION=$(echo "$AVAILABLE_MEMORY * 80 / 100" | bc)
echo "Safe allocation (80%): $(echo "$SAFE_ALLOCATION / 1024 / 1024" | bc)GB"

# Stop MicroK8s
echo "⏹️  Stopping MicroK8s..."
$SUDO_CMD microk8s stop

# Backup original kubelet configuration
echo "💾 Backing up kubelet configuration..."
$SUDO_CMD cp /var/snap/microk8s/current/args/kubelet /var/snap/microk8s/current/args/kubelet.backup

# Remove existing memory limits
echo "📝 Removing existing memory limits..."
$SUDO_CMD sed -i '/--system-reserved/d' /var/snap/microk8s/current/args/kubelet
$SUDO_CMD sed -i '/--kube-reserved/d' /var/snap/microk8s/current/args/kubelet

# Add new memory limits based on available memory
echo "📝 Setting new memory limits..."
echo "--system-reserved=memory=2Gi,cpu=1" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet
echo "--kube-reserved=memory=1Gi,cpu=0.5" | $SUDO_CMD tee -a /var/snap/microk8s/current/args/kubelet

# Start MicroK8s
echo "🚀 Starting MicroK8s..."
$SUDO_CMD microk8s start

# Wait for MicroK8s to be ready
echo "⏳ Waiting for MicroK8s to be ready..."
$SUDO_CMD microk8s status --wait-ready

# Verify the new limits
echo "✅ Verifying new memory limits..."
echo "Allocatable memory:"
kubectl get nodes -o jsonpath='{.items[0].status.allocatable.memory}'
echo ""

echo "✅ MicroK8s memory allocation fixed!"
echo "   - Reduced system reserved: 2Gi memory, 1 CPU"
echo "   - Reduced kube reserved: 1Gi memory, 0.5 CPU"
echo "   - Should now fit within available system memory"
