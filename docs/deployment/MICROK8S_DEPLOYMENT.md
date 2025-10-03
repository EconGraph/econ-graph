# MicroK8s Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the EconGraph application to a local Kubernetes cluster using MicroK8s. MicroK8s is a lightweight, single-node Kubernetes distribution that's ideal for local development and testing.

The deployment includes:
- **Backend**: Rust API with GraphQL
- **Frontend**: React application
- **Admin Frontend**: Administrative interface  
- **Chart API Service**: Private chart generation service
- **Monitoring Stack**: Grafana + Loki + Prometheus + Promtail
- **Database**: PostgreSQL with persistent storage
- **SSL/TLS**: Let's Encrypt certificates with cert-manager
- **Security**: CORS, HTTP security headers, rate limiting, NetworkPolicy, PodSecurityPolicy

## Prerequisites

- Ubuntu 18.04+ or compatible Linux distribution
- 8GB+ RAM (16GB+ recommended for full stack)
- 20GB+ free disk space
- Internet connection for image pulls and certificate generation

## Installation

### Step 1: Install MicroK8s

```bash
# Install MicroK8s
sudo snap install microk8s --classic

# Add user to microk8s group
sudo usermod -aG microk8s $USER
newgrp microk8s

# Verify installation
microk8s status --wait-ready
```

### Step 2: Enable Required Addons

```bash
# Enable essential addons
microk8s enable dns
microk8s enable ingress
microk8s enable storage
microk8s enable metrics-server

# Wait for addons to be ready
microk8s status --wait-ready
```

### Step 3: Configure kubectl

```bash
# Configure kubectl to use MicroK8s
microk8s kubectl config view --raw > ~/.kube/config
kubectl config use-context microk8s

# Verify kubectl works
kubectl get nodes
```

## Quick Start

### One-Command Deployment

```bash
cd /path/to/EconGraph/FrontEnd-1
./scripts/deploy/restart-k8s-rollout.sh
```

This script will:
- ✅ Check/start MicroK8s
- ✅ Enable required addons
- ✅ Build Docker images with versioned tags
- ✅ Load images into MicroK8s
- ✅ Apply updated manifests
- ✅ Deploy monitoring stack
- ✅ Install cert-manager for SSL
- ✅ Configure Let's Encrypt issuer
- ✅ Deploy SSL ingress with security features
- ✅ Wait for rollout completion
- ✅ Display deployment status

## Manual Deployment

### Step 1: Environment Setup

```bash
cd /path/to/EconGraph/FrontEnd-1

# Load port configuration
source config/ports.env

# Verify MicroK8s is running
microk8s status
kubectl config current-context
```

### Step 2: Build and Load Images

```bash
# Build images with versioned tags
./scripts/deploy/build-images.sh

# Images are automatically loaded into MicroK8s
# No need for separate load commands like with kind
```

### Step 3: Deploy Application

```bash
# Apply all manifests
kubectl apply -f k8s/manifests/

# Deploy monitoring stack
kubectl apply -f k8s/monitoring/
```

### Step 4: Configure SSL and Security

```bash
# Install cert-manager
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.16.2/cert-manager.yaml

# Wait for cert-manager to be ready
kubectl wait --for=condition=ready pod -l app=cert-manager -n cert-manager --timeout=120s

# Apply Let's Encrypt issuer
kubectl apply -f k8s/manifests/letsencrypt-issuer.yaml

# Apply SSL ingress with security features
kubectl apply -f k8s/manifests/ssl-ingress.yaml
```

## Service URLs

### HTTPS Access (Production-like)
- **Frontend**: https://www.econgraph.com
- **Backend**: https://www.econgraph.com/api
- **GraphQL**: https://www.econgraph.com/graphql
- **Admin UI**: https://www.econgraph.com/admin
- **Grafana**: https://www.econgraph.com/grafana

### Local Development Access
Add to `/etc/hosts`:
```
127.0.0.1 www.econgraph.com econgraph.com
```

### Port Forwarding (Alternative)
```bash
# Frontend
kubectl port-forward service/econ-graph-frontend-service 3000:3000 -n econ-graph

# Backend
kubectl port-forward service/econ-graph-backend-service 9876:9876 -n econ-graph

# Admin Frontend
kubectl port-forward service/econ-graph-admin-frontend-service 3001:3001 -n econ-graph

# Grafana
kubectl port-forward service/grafana-service 3000:3000 -n econ-graph
```

## Security Features

### SSL/TLS Configuration
- **Let's Encrypt**: Automatic SSL certificate generation
- **TLS Termination**: SSL termination at ingress level
- **Certificate Management**: Automated certificate renewal

### HTTP Security Headers
- **X-Frame-Options**: DENY
- **X-Content-Type-Options**: nosniff
- **X-XSS-Protection**: 1; mode=block
- **Strict-Transport-Security**: max-age=31536000; includeSubDomains; preload
- **Content-Security-Policy**: Comprehensive CSP policy
- **Referrer-Policy**: strict-origin-when-cross-origin
- **Permissions-Policy**: Restricted permissions

### CORS Configuration
- **Allowed Origins**: https://www.econgraph.com, https://econgraph.com
- **Allowed Methods**: GET, POST, PUT, DELETE, OPTIONS, PATCH
- **Allowed Headers**: Comprehensive header list
- **Credentials**: Enabled
- **Max Age**: 86400 seconds

### Rate Limiting
- **API Endpoints**: 100 requests per minute
- **Admin Endpoints**: 5 requests per minute
- **Connection Limits**: 10 concurrent connections

### Network Security
- **NetworkPolicy**: Restrictive pod-to-pod communication
- **PodSecurityPolicy**: Non-root containers, read-only filesystems
- **RBAC**: Role-based access control

## Monitoring and Observability

### Grafana Dashboards
- **EconGraph Overview**: System-wide metrics and health
- **Database Statistics**: PostgreSQL performance and usage
- **Crawler Status**: Data collection and processing metrics
- **Logging Dashboard**: Centralized log analysis

### Log Aggregation
- **Loki**: Log aggregation system
- **Promtail**: Log collection agent
- **Prometheus**: Metrics collection and alerting

### Health Checks
```bash
# Test service accessibility
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com/api/health
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com/grafana
```

## Troubleshooting

### Common Issues

#### MicroK8s Not Starting
```bash
# Check MicroK8s status
microk8s status

# Start MicroK8s
microk8s start

# Check logs
microk8s logs
```

#### Permission Denied Errors
```bash
# Ensure user is in microk8s group
groups $USER

# Add user to group if needed
sudo usermod -aG microk8s $USER
newgrp microk8s
```

#### Pods Not Starting
```bash
# Check pod status and events
kubectl describe pod <pod-name> -n econ-graph
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
```

#### PostgreSQL Bus Error
**Problem**: PostgreSQL crashes with "Bus error (core dumped)" during initialization.

**Root Cause**: Memory alignment issues with hugepages or system-level memory management.

**Solutions**:

1. **Disable hugepages in PostgreSQL**:
```bash
kubectl patch statefulset postgresql -n econ-graph -p '{"spec":{"template":{"spec":{"containers":[{"name":"postgresql","env":[{"name":"POSTGRES_INITDB_ARGS","value":"--set huge_pages=off"}]}]}}}}'
```

2. **Use different PostgreSQL image**:
```bash
kubectl set image statefulset/postgresql postgresql=postgres:18-alpine -n econ-graph
```

3. **Reduce resource constraints**:
```bash
kubectl patch statefulset postgresql -n econ-graph -p '{"spec":{"template":{"spec":{"containers":[{"name":"postgresql","resources":{"limits":{"cpu":"100m","memory":"512Mi"},"requests":{"cpu":"50m","memory":"256Mi"}}}]}}}}'
```

#### SSL Certificate Issues
```bash
# Check certificate status
kubectl get certificate -n econ-graph

# Check cert-manager logs
kubectl logs -f deployment/cert-manager -n cert-manager

# Check ingress status
kubectl describe ingress econ-graph-ssl-ingress -n econ-graph
```

#### Memory Issues
```bash
# Check node memory usage
kubectl top nodes

# Check pod memory usage
kubectl top pods -n econ-graph

# Scale down resource-heavy services if needed
kubectl scale deployment --replicas=0 -n istio-system istio-pilot istio-policy istio-telemetry
```

### Debug Commands

```bash
# Get all resources
kubectl get all -n econ-graph

# Check resource usage
kubectl top pods -n econ-graph

# View all events
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'

# Check ingress status
kubectl get ingress -n econ-graph

# Check certificate status
kubectl get certificate -n econ-graph
```

## Performance Optimization

### Resource Management
- **Pod Limits**: Increased from 110 to 500 pods
- **Memory Allocation**: 125GB allocatable with swap support
- **CPU Allocation**: 64 cores available
- **Storage**: Persistent volumes with 10GB+ capacity

### Scaling
```bash
# Scale deployments
kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph
kubectl scale deployment econ-graph-frontend --replicas=2 -n econ-graph
```

### Resource Monitoring
```bash
# Monitor resource usage
kubectl top nodes
kubectl top pods -n econ-graph

# Check storage usage
kubectl get pvc -n econ-graph
```

## Restart Procedures

### Full Restart
```bash
./scripts/deploy/restart-k8s-rollout.sh
```

### Individual Service Restart
```bash
kubectl rollout restart deployment/econ-graph-backend -n econ-graph
kubectl rollout restart deployment/econ-graph-frontend -n econ-graph
kubectl rollout restart deployment/econ-graph-admin-frontend -n econ-graph
kubectl rollout restart deployment/chart-api-service -n econ-graph
```

### MicroK8s Restart
```bash
# Restart MicroK8s
microk8s stop
microk8s start

# Wait for readiness
microk8s status --wait-ready
```

## Cleanup

### Remove Application
```bash
kubectl delete -f k8s/manifests/
kubectl delete -f k8s/monitoring/
```

### Remove MicroK8s
```bash
# Stop MicroK8s
microk8s stop

# Remove MicroK8s
sudo snap remove microk8s
```

## MicroK8s vs Kind Comparison

| Feature | MicroK8s | Kind |
|---------|----------|------|
| **Installation** | `snap install microk8s` | `go install` or binary |
| **Resource Usage** | Lower overhead | Higher overhead |
| **Addons** | Built-in (dns, ingress, storage) | Manual configuration |
| **SSL Support** | Native ingress with SSL | Requires additional setup |
| **Production-like** | More production-like | Development-focused |
| **Port Access** | Ingress-based | NodePort-based |
| **Security** | Built-in security features | Manual configuration |

## Best Practices

### Development
- Use MicroK8s for production-like local development
- Enable all required addons before deployment
- Use SSL/TLS for realistic testing
- Monitor resource usage and scale as needed

### Production Preparation
- Test SSL certificate generation
- Verify security headers and CORS
- Test rate limiting and network policies
- Validate monitoring and logging

### Maintenance
- Regularly update MicroK8s: `sudo snap refresh microk8s`
- Monitor resource usage and scale as needed
- Backup database data periodically
- Review and update monitoring dashboards

## Support and Maintenance

### Regular Maintenance
- Monitor resource usage and scale as needed
- Update images regularly for security patches
- Backup database data periodically
- Review and update monitoring dashboards

### Log Management
- Logs are automatically rotated by Kubernetes
- Use Grafana for log analysis and troubleshooting
- Set up alerts for critical errors

For additional support, refer to the individual service documentation or check the troubleshooting section above.

## Additional Resources

- **[MicroK8s Troubleshooting Guide](MICROK8S_TROUBLESHOOTING.md)** - Detailed solutions for common issues
- **[Kubernetes Deployment Guide](KUBERNETES_DEPLOYMENT.md)** - General Kubernetes deployment information
- **[EconGraph Documentation](../README.md)** - Complete project documentation
