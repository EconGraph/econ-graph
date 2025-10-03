# MicroK8s Troubleshooting Guide

## Overview

This guide provides solutions for common issues encountered when using MicroK8s for EconGraph development. For general MicroK8s setup, see [MicroK8s Deployment Guide](MICROK8S_DEPLOYMENT.md).

## Common Issues and Solutions

### 1. Permission Denied Errors

#### Problem
```bash
Error: open servers store: open /var/snap/microk8s/3410/var/kubernetes/backend/cluster.yaml: permission denied
```

#### Solution
```bash
# Add user to microk8s group
sudo usermod -aG microk8s $USER

# Apply group changes
newgrp microk8s

# Verify group membership
groups $USER
```

#### Prevention
Always run `newgrp microk8s` after adding yourself to the microk8s group.

### 2. MicroK8s Not Starting

#### Problem
```bash
microk8s start
# Hangs or fails to start
```

#### Solution
```bash
# Check MicroK8s status
microk8s status

# Check logs
microk8s logs

# Reset MicroK8s if needed
microk8s reset

# Start fresh
microk8s start
```

#### Prevention
Ensure you have sufficient system resources (8GB+ RAM recommended).

### 3. kubectl Context Issues

#### Problem
```bash
kubectl get nodes
# Error: The connection to the server 127.0.0.1:16443 was refused
```

#### Solution
```bash
# Configure kubectl for MicroK8s
microk8s kubectl config view --raw > ~/.kube/config
kubectl config use-context microk8s

# Verify context
kubectl config current-context
```

### 4. Addon Installation Failures

#### Problem
```bash
microk8s enable ingress
# Fails or hangs
```

#### Solution
```bash
# Check MicroK8s status first
microk8s status --wait-ready

# Enable addons one by one
microk8s enable dns
microk8s enable storage
microk8s enable ingress
microk8s enable metrics-server

# Wait for each addon to be ready
microk8s status --wait-ready
```

### 5. Image Loading Issues

#### Problem
```bash
microk8s ctr images import <(docker save image)
# Error: ctr: open /dev/fd/63: no such file or directory
```

#### Solution
```bash
# Use piping instead of process substitution
docker save econ-graph-backend:latest | microk8s ctr images import -

# Or save to file first
docker save econ-graph-backend:latest > /tmp/backend.tar
microk8s ctr images import /tmp/backend.tar
```

### 6. PostgreSQL Bus Error

#### Problem
```bash
kubectl logs postgresql-0 -n econ-graph
# Bus error (core dumped)
```

#### Solution
```bash
# Configure PostgreSQL to disable hugepages
./scripts/configure-postgresql-no-hugepages.sh

# Or manually patch the StatefulSet
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

# Restart PostgreSQL
kubectl delete pod postgresql-0 -n econ-graph
```

### 7. Backend Pods Stuck in Init:0/1

#### Problem
```bash
kubectl get pods -n econ-graph | grep backend
# Shows: econ-graph-backend-xxx   0/1     Init:0/1   0          XXm

kubectl logs <backend-pod> -n econ-graph -c wait-for-postgres
# Shows: "PostgreSQL is unavailable - sleeping" repeatedly
```

#### Solution
```bash
# Check PostgreSQL status
kubectl get pods -n econ-graph | grep postgres
kubectl get svc -n econ-graph | grep postgres

# Test connectivity (if network policies are blocking)
kubectl delete networkpolicy --all -n econ-graph

# Restart backend deployment
kubectl rollout restart deployment/econ-graph-backend -n econ-graph

# Re-apply network policies after connectivity is established
kubectl apply -f k8s/manifests/network-policy.yaml
```

### 8. SSL Certificate Issues

#### Problem
```bash
kubectl get certificate -n econ-graph
# Shows "NotReady" or "Failed"
```

#### Solution
```bash
# Check cert-manager status
kubectl get pods -n cert-manager

# Check certificate details
kubectl describe certificate econ-graph-tls -n econ-graph

# Check cert-manager logs
kubectl logs -f deployment/cert-manager -n cert-manager

# Restart cert-manager if needed
kubectl rollout restart deployment/cert-manager -n cert-manager
```

### 9. Memory Issues

#### Problem
```bash
kubectl get pods -n econ-graph
# Pods stuck in Pending with "Insufficient memory"
```

#### Solution
```bash
# Check node resources
kubectl top nodes
kubectl describe node

# Scale down resource-heavy services
kubectl scale deployment --replicas=0 -n istio-system istio-pilot istio-policy istio-telemetry

# Check if kubeflow is consuming resources
kubectl get pods -A | grep kubeflow
# Delete kubeflow if present
kubectl delete namespace kubeflow
```

### 10. Ingress Not Working

#### Problem
```bash
curl https://www.econ-graph.com
# Connection refused or timeout
```

#### Solution
```bash
# Check ingress controller status
kubectl get pods -n ingress-nginx

# Check ingress configuration
kubectl get ingress -n econ-graph
kubectl describe ingress econ-graph-ssl-ingress -n econ-graph

# Check if ingress controller is ready
kubectl wait --for=condition=ready pod -l app.kubernetes.io/component=controller -n ingress-nginx --timeout=180s
```

### 11. Pod Limit Exceeded

#### Problem
```bash
kubectl describe pod <pod-name> -n econ-graph
# Warning: Too many pods
```

#### Solution
```bash
# Increase pod limit (requires sudo)
sudo microk8s stop

# Edit kubelet configuration
sudo nano /var/snap/microk8s/current/args/kubelet

# Add or modify:
# --max-pods=500

sudo microk8s start
```

## Debugging Commands

### System Information
```bash
# MicroK8s status
microk8s status

# System resources
free -h
df -h

# Docker status
docker ps
docker images
```

### Kubernetes Information
```bash
# Cluster info
kubectl cluster-info
kubectl get nodes
kubectl top nodes

# Namespace info
kubectl get all -n econ-graph
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
```

### Service Information
```bash
# Service status
kubectl get services -n econ-graph
kubectl get ingress -n econ-graph

# Pod logs
kubectl logs -f deployment/econ-graph-backend -n econ-graph
kubectl logs -f deployment/econ-graph-frontend -n econ-graph
```

## Performance Optimization

### Resource Management
```bash
# Check resource usage
kubectl top pods -n econ-graph
kubectl top nodes

# Scale services based on usage
kubectl scale deployment econ-graph-backend --replicas=2 -n econ-graph
```

### Memory Optimization
```bash
# Check memory usage
kubectl top nodes
kubectl describe node

# Scale down if needed
kubectl scale deployment --replicas=0 -n istio-system istio-pilot
```

## Recovery Procedures

### Complete Reset
```bash
# Stop MicroK8s
microk8s stop

# Reset MicroK8s
microk8s reset

# Start fresh
microk8s start
microk8s status --wait-ready

# Re-enable addons
microk8s enable dns ingress storage metrics-server
```

### Application Reset
```bash
# Delete application
kubectl delete -f k8s/manifests/
kubectl delete -f k8s/monitoring/

# Redeploy
./scripts/deploy/restart-k8s-rollout.sh
```

## Prevention Best Practices

### 1. Resource Management
- Monitor system resources regularly
- Scale services based on actual usage
- Use resource limits and requests

### 2. Regular Maintenance
- Update MicroK8s regularly: `sudo snap refresh microk8s`
- Clean up unused images: `docker system prune`
- Monitor disk usage

### 3. Backup Procedures
- Backup important configurations
- Document custom changes
- Test recovery procedures

## Getting Help

### Logs to Check
```bash
# MicroK8s logs
microk8s logs

# Application logs
kubectl logs -f deployment/econ-graph-backend -n econ-graph

# System logs
journalctl -u snap.microk8s.daemon-kubelite
```

### Community Resources
- [MicroK8s Documentation](https://microk8s.io/docs)
- [Kubernetes Troubleshooting](https://kubernetes.io/docs/tasks/debug-application-cluster/)
- [EconGraph Documentation](../README.md)

### Support Channels
- Check the [MicroK8s Deployment Guide](MICROK8S_DEPLOYMENT.md) for detailed setup
- Review the [Kubernetes Deployment Guide](KUBERNETES_DEPLOYMENT.md) for general issues
- Check application-specific documentation in the `docs/` directory
