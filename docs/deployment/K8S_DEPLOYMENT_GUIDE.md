# Kubernetes Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the EconGraph application to a Kubernetes cluster. The deployment includes a complete production-ready stack with SSL/TLS, security policies, monitoring, and automated certificate management.

### Architecture Components

- **Backend**: Rust API with GraphQL (v3.7.4)
- **Frontend**: React application (v3.7.4) 
- **Admin Frontend**: Administrative interface (v1.0.0)
- **Chart API Service**: Private chart generation service (v1.0.0)
- **Database**: PostgreSQL 18 with persistent storage
- **Monitoring Stack**: Grafana + Loki + Prometheus + Promtail
- **SSL/TLS**: Let's Encrypt certificates with cert-manager
- **Security**: Pod Security Standards, Network Policies, CORS, Rate Limiting

## Prerequisites

### System Requirements
- **OS**: Ubuntu 18.04+ or compatible Linux distribution
- **RAM**: 8GB+ (16GB+ recommended for full stack)
- **Storage**: 20GB+ free disk space
- **Network**: Internet connection for image pulls and certificate generation

### Required Software
- **MicroK8s**: `sudo snap install microk8s --classic`
- **Docker**: For building application images
- **kubectl**: Configured for MicroK8s

## Quick Start (Recommended)

### One-Command Deployment

```bash
cd /path/to/EconGraph/FrontEnd-1
./scripts/deploy/restart-k8s-rollout.sh
```

This automated script will:
- ✅ Check/start MicroK8s and enable required addons
- ✅ Build Docker images with proper versioned tags
- ✅ Load images into MicroK8s container runtime
- ✅ Apply all Kubernetes manifests with security contexts
- ✅ Deploy monitoring stack (Grafana, Loki, Prometheus, Promtail)
- ✅ Install cert-manager for SSL certificate management
- ✅ Configure Let's Encrypt staging and production issuers
- ✅ Deploy SSL ingress with comprehensive security features
- ✅ Apply Network Policies for secure pod communication
- ✅ Wait for all rollouts to complete
- ✅ Test HTTPS termination and display service URLs

## Manual Deployment

### Step 1: MicroK8s Setup

```bash
# Install MicroK8s
sudo snap install microk8s --classic

# Add user to microk8s group
sudo usermod -aG microk8s $USER
newgrp microk8s

# Enable required addons
microk8s enable dns ingress storage metrics-server

# Configure kubectl
microk8s kubectl config view --raw > ~/.kube/config
kubectl config use-context microk8s

# Verify setup
microk8s status --wait-ready
```

### Step 2: Build and Load Images

```bash
# Build images with versioned tags
./scripts/deploy/build-images.sh

# Images are automatically loaded into MicroK8s
# No separate load commands needed (unlike kind clusters)
```

### Step 3: Deploy Application Stack

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

# Apply Let's Encrypt issuers
kubectl apply -f k8s/manifests/letsencrypt-issuer.yaml

# Apply SSL ingress with security features
kubectl apply -f k8s/manifests/ssl-ingress.yaml
```

## Service URLs

### Production HTTPS Access
- **Frontend**: https://www.econgraph.com
- **Backend API**: https://www.econgraph.com/api
- **GraphQL**: https://www.econgraph.com/graphql
- **Admin UI**: https://www.econgraph.com/admin
- **Grafana**: https://www.econgraph.com/grafana

### Local Development Setup
Add to `/etc/hosts`:
```
127.0.0.1 www.econgraph.com econgraph.com
```

### Alternative: Port Forwarding
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

### Pod Security Standards
All pods run with restricted security contexts:
- **Non-root containers**: `runAsNonRoot: true`
- **Read-only filesystems**: `readOnlyRootFilesystem: true`
- **No privilege escalation**: `allowPrivilegeEscalation: false`
- **Dropped capabilities**: All capabilities dropped except required ones
- **Seccomp profiles**: Runtime default seccomp profiles

### Network Policies
Restrictive pod-to-pod communication:
- **Ingress controller access**: Only from `ingress` namespace
- **Inter-pod communication**: Only within `econ-graph` namespace
- **External access**: Only through defined services

### SSL/TLS Configuration
- **Let's Encrypt**: Automatic SSL certificate generation and renewal
- **TLS Termination**: SSL termination at ingress level
- **Certificate Management**: Automated certificate lifecycle management
- **Staging Environment**: `letsencrypt-staging` issuer for testing

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
- **Allowed Headers**: Comprehensive header list including CSRF tokens
- **Credentials**: Enabled for authenticated requests
- **Max Age**: 86400 seconds

### Rate Limiting
- **API Endpoints**: 100 requests per minute per client
- **Admin Endpoints**: 5 requests per minute per client
- **Connection Limits**: 10 concurrent connections per client

## Monitoring and Observability

### Grafana Dashboards
- **EconGraph Overview**: System-wide metrics and health
- **Database Statistics**: PostgreSQL performance and usage
- **Crawler Status**: Data collection and processing metrics
- **Logging Dashboard**: Centralized log analysis

### Log Aggregation
- **Loki**: Log aggregation system for centralized logging
- **Promtail**: Log collection agent running as DaemonSet
- **Prometheus**: Metrics collection and alerting system

### Health Checks
```bash
# Test service accessibility
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com/api/health
curl -k -s -o /dev/null -w "%{http_code}" https://www.econgraph.com/grafana
```

## Troubleshooting

### Common Issues

#### 1. Pod Security Standards Violations
**Symptoms**: Pods fail to start with security context violations
```bash
# Check pod events
kubectl describe pod <pod-name> -n econ-graph

# Common violations:
# - allowPrivilegeEscalation != false
# - unrestricted capabilities
# - runAsNonRoot != true
# - seccompProfile not set
```

**Solution**: All deployment manifests include proper security contexts. If issues persist:
```bash
# Check namespace labels
kubectl get namespace econ-graph -o yaml

# Verify Pod Security Standards are applied
kubectl get namespace econ-graph -o jsonpath='{.metadata.labels}'
```

#### 2. Network Policy Blocking Traffic
**Symptoms**: Services can't communicate, ACME challenges fail
```bash
# Check network policies
kubectl get networkpolicies -n econ-graph

# Test connectivity from ingress controller
kubectl exec -n ingress nginx-ingress-microk8s-controller-xxx -- wget -O- http://10.1.26.89:8089/.well-known/acme-challenge/test
```

**Solution**: Network policies are configured to allow ingress controller access. If issues persist:
```bash
# Check ingress namespace label
kubectl get namespace ingress -o yaml | grep -A5 labels

# Verify network policy allows ingress namespace
kubectl describe networkpolicy econ-graph-network-policy -n econ-graph
```

#### 3. SSL Certificate Issues
**Symptoms**: HTTPS endpoints return certificate errors
```bash
# Check certificate status
kubectl get certificates -n econ-graph

# Check ACME challenges
kubectl get challenges -n econ-graph

# Check cert-manager logs
kubectl logs -f deployment/cert-manager -n cert-manager
```

**Solution**: 
```bash
# Test ACME challenge accessibility
curl -s -o /dev/null -w "%{http_code}" http://econgraph.com/.well-known/acme-challenge/$(kubectl get challenge -n econ-graph -o jsonpath='{.items[0].spec.token}')

# Should return 200, not 504
```

#### 4. PostgreSQL Issues
**Symptoms**: Database pods crash or fail to start
```bash
# Check PostgreSQL logs
kubectl logs -f statefulset/postgresql -n econ-graph

# Check volume mounts
kubectl describe pod postgresql-0 -n econ-graph
```

**Solution**: PostgreSQL 18 configuration includes proper volume mounts and security contexts.

#### 5. Image Pull Issues
**Symptoms**: Pods stuck in `ErrImageNeverPull` or `ErrImagePull`
```bash
# Check available images in MicroK8s
microk8s ctr images list | grep econ-graph

# Rebuild and load images
./scripts/deploy/build-images.sh
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
kubectl get certificates -n econ-graph

# Check network policies
kubectl get networkpolicies -n econ-graph
```

## Performance Optimization

### Resource Management
- **Pod Limits**: Configured for optimal resource usage
- **Memory Allocation**: Appropriate limits based on service requirements
- **CPU Allocation**: Efficient CPU resource distribution
- **Storage**: Persistent volumes with adequate capacity

### Scaling
```bash
# Scale deployments
kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph
kubectl scale deployment econ-graph-frontend --replicas=2 -n econ-graph

# Check scaling status
kubectl get deployments -n econ-graph
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

## Security Best Practices

### Development
- Use MicroK8s for production-like local development
- Enable all required addons before deployment
- Use SSL/TLS for realistic testing
- Monitor resource usage and scale as needed

### Production Preparation
- Test SSL certificate generation with staging issuer
- Verify security headers and CORS configuration
- Test rate limiting and network policies
- Validate monitoring and logging setup

### Maintenance
- Regularly update MicroK8s: `sudo snap refresh microk8s`
- Monitor resource usage and scale as needed
- Backup database data periodically
- Review and update monitoring dashboards
- Keep security contexts and network policies updated

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

### Security Updates
- Regularly update base images for security patches
- Review and update security contexts
- Monitor network policies for effectiveness
- Test SSL certificate renewal processes

## Additional Resources

- **[MicroK8s Troubleshooting Guide](MICROK8S_TROUBLESHOOTING.md)** - Detailed solutions for common issues
- **[EconGraph Documentation](../README.md)** - Complete project documentation
- **[Kubernetes Documentation](https://kubernetes.io/docs/)** - Official Kubernetes guides
- **[MicroK8s Documentation](https://microk8s.io/docs)** - MicroK8s specific guides

For additional support, refer to the individual service documentation or check the troubleshooting section above.
