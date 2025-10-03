# Kubernetes Deployment Guide

## Overview

**📚 For the most up-to-date and comprehensive deployment guide, see [K8S_DEPLOYMENT_GUIDE.md](K8S_DEPLOYMENT_GUIDE.md)**

This guide provides comprehensive instructions for deploying the EconGraph application to a local Kubernetes cluster. We support two deployment options:

1. **MicroK8s** (Recommended for production-like development)
2. **Kind** (Kubernetes in Docker)

The deployment includes:

- **Backend**: Rust API with GraphQL
- **Frontend**: React application
- **Admin Frontend**: Administrative interface  
- **Chart API Service**: Private chart generation service
- **Monitoring Stack**: Grafana + Loki + Prometheus + Promtail
- **Database**: PostgreSQL with persistent storage

## Choosing Between MicroK8s and Kind

### MicroK8s (Recommended)
**Best for:**
- Production-like local development
- SSL/TLS testing
- Security feature testing
- Performance testing
- Learning Kubernetes concepts

**Advantages:**
- Built-in addons (dns, ingress, storage, metrics-server)
- Native SSL/TLS support with cert-manager
- Production-like networking
- Lower resource overhead
- Easy snap installation

**Requirements:**
- Ubuntu 18.04+ or compatible Linux
- 8GB+ RAM recommended
- 20GB+ free disk space

### Kind (Alternative)
**Best for:**
- Docker-based development
- CI/CD pipelines
- Cross-platform development
- Quick testing

**Advantages:**
- Works on any platform with Docker
- Easy to reset and recreate
- Good for CI/CD environments
- Familiar Docker workflow

**Requirements:**
- Docker Desktop
- kubectl and kind installed
- More resource intensive

## Prerequisites

### For MicroK8s (Recommended)
- Ubuntu 18.04+ or compatible Linux distribution
- 8GB+ RAM (16GB+ recommended for full stack)
- 20GB+ free disk space
- Internet connection for image pulls and certificate generation

### For Kind (Deprecated)
- Docker Desktop running
- kubectl installed
- kind installed
- terraform installed (for cluster setup)

**Note**: Kind deployment is deprecated in favor of MicroK8s for better security and SSL support.

## Quick Start

### MicroK8s Deployment (Recommended)

```bash
cd /path/to/EconGraph/FrontEnd-1
./scripts/deploy/restart-k8s-rollout.sh
```

This script will:
- ✅ Check/start MicroK8s
- ✅ Enable required addons (dns, ingress, storage, metrics-server)
- ✅ Build Docker images with versioned tags
- ✅ Load images into MicroK8s
- ✅ Apply updated manifests
- ✅ Deploy monitoring stack
- ✅ Install cert-manager for SSL
- ✅ Configure Let's Encrypt issuer
- ✅ Deploy SSL ingress with security features
- ✅ Wait for rollout completion
- ✅ Display deployment status

**Access URLs:**
- Frontend: https://www.econgraph.com
- Backend: https://www.econgraph.com/api
- Admin: https://www.econgraph.com/admin
- Grafana: https://www.econgraph.com/grafana

**For detailed MicroK8s setup and troubleshooting, see**: [MicroK8s Deployment Guide](MICROK8S_DEPLOYMENT.md)

### Kind Deployment (Alternative)

```bash
cd /Users/josephmalicki/src/econ-graph5
./scripts/deploy/restart-k8s-rollout.sh
```

This script will:
- ✅ Check/create kind cluster
- ✅ Build Docker images with latest tags
- ✅ Load images into cluster
- ✅ Apply updated manifests
- ✅ Deploy monitoring stack
- ✅ Restart all deployments
- ✅ Wait for rollout completion
- ✅ Display deployment status

## Manual Deployment

### Step 1: Environment Setup

```bash
cd /Users/josephmalicki/src/econ-graph5
export KUBECONFIG=~/.kube/config

# Load port configuration
source ports.env

# Verify cluster exists
kind get clusters
kubectl config current-context
```

### Step 2: Cluster Setup

If no cluster exists, create one:

```bash
cd terraform/k8s
terraform init
terraform apply -auto-approve
cd ../..
```

### Step 3: Build Images

```bash
# Automated build
./scripts/deploy/build-images.sh

# Or manual build:
cd backend && docker build -t econ-graph-backend:latest . && cd ..
cd frontend && docker build --build-arg REACT_APP_API_URL="http://localhost" --build-arg REACT_APP_GRAPHQL_URL="/graphql" --build-arg REACT_APP_WS_URL="ws://localhost/graphql" --build-arg NODE_ENV="production" -t econ-graph-frontend:latest . && cd ..
cd admin-frontend && docker build --build-arg REACT_APP_API_URL="http://localhost" --build-arg REACT_APP_GRAPHQL_URL="/graphql" --build-arg REACT_APP_WS_URL="ws://localhost/graphql" --build-arg REACT_APP_GRAFANA_URL="http://localhost:${GRAFANA_NODEPORT}" --build-arg NODE_ENV="production" -t econ-graph-admin-frontend:latest . && cd ..
cd chart-api-service && docker build -t econ-graph-chart-api:latest . && cd ..
```

### Step 4: Load Images into Cluster

```bash
kind load docker-image econ-graph-backend:latest --name econ-graph
kind load docker-image econ-graph-frontend:latest --name econ-graph
kind load docker-image econ-graph-admin-frontend:latest --name econ-graph
kind load docker-image econ-graph-chart-api:latest --name econ-graph
```

### Step 5: Deploy Application

```bash
# Set kubectl context
kubectl config use-context kind-econ-graph

# Apply manifests
kubectl apply -f k8s/manifests/

# Deploy monitoring stack
kubectl apply -f k8s/monitoring/
```

### Step 6: Monitor Deployment

```bash
# Watch pod status
watch kubectl get pods -n econ-graph

# Check deployment status
kubectl get deployments -n econ-graph

# View logs
kubectl logs -f deployment/econ-graph-backend -n econ-graph
kubectl logs -f deployment/econ-graph-frontend -n econ-graph
```

## Service URLs

Load port configuration first:
```bash
source ports.env
```

### External Access (NodePort)
- **Frontend**: http://localhost:${FRONTEND_NODEPORT}
- **Backend**: http://localhost:${BACKEND_NODEPORT}
- **GraphQL**: http://localhost:${BACKEND_NODEPORT}/graphql
- **Health Check**: http://localhost:${BACKEND_NODEPORT}/health
- **Grafana**: http://localhost:${GRAFANA_NODEPORT} (admin/admin123)

### Internal Services (ClusterIP)
- **Chart API Service**: `chart-api-service.econ-graph.svc.cluster.local:3001`
- **Database**: `postgresql.econ-graph.svc.cluster.local:5432`

## Port Configuration

All ports are centralized in `ports.env`:

```bash
# Internal container ports
BACKEND_PORT=9876
FRONTEND_PORT=3000
ADMIN_FRONTEND_PORT=3001
DATABASE_PORT=5432

# External NodePort mappings
BACKEND_NODEPORT=30080
FRONTEND_NODEPORT=30000
GRAFANA_NODEPORT=30001

# Service URLs
BACKEND_URL=http://localhost:${BACKEND_NODEPORT}
FRONTEND_URL=http://localhost:${FRONTEND_NODEPORT}
GRAFANA_URL=http://localhost:${GRAFANA_NODEPORT}
```

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
curl -s -o /dev/null -w "%{http_code}" http://localhost:${FRONTEND_NODEPORT}
curl -s -o /dev/null -w "%{http_code}" http://localhost:${BACKEND_NODEPORT}/health
curl -s -o /dev/null -w "%{http_code}" http://localhost:${GRAFANA_NODEPORT}
```

## Troubleshooting

### ⚠️ CRITICAL: Ingress Routing Bug

**Problem**: Root path (`/`) returns admin interface instead of frontend when accessing through ingress.

**Root Cause**: This is a **fundamental design flaw** in the nginx ingress controller's configuration generation. The controller generates a server-level `set $proxy_upstream_name "-";` that overrides location-level settings, causing incorrect routing.

**Impact**: 
- ✅ All other paths work correctly (`/admin`, `/health`, `/api`, etc.)
- ✅ Services work correctly when accessed directly (port-forward)
- ❌ Root path (`/`) fails through ingress

**Workarounds**:

**Option 1: Use Separate Ingress for Frontend**
```bash
# Access frontend via separate ingress
curl -H "Host: frontend.econ-graph.local" http://localhost:8080/
```

**Option 2: Use Direct Service Access**
```bash
# Port-forward to frontend service directly
kubectl port-forward service/econ-graph-frontend-service 3000:3000 -n econ-graph
# Access via http://localhost:3000
```

**Option 3: Use Admin Path for Admin Interface**
```bash
# Access admin via /admin path
curl -H "Host: admin.econ-graph.local" http://localhost:8080/admin
```

**For detailed debugging information, see**: [Ingress Routing Troubleshooting Guide](INGRESS_ROUTING_TROUBLESHOOTING.md)

### Common Issues

#### Pods Not Starting
```bash
# Check pod status and events
kubectl describe pod <pod-name> -n econ-graph
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'
```

#### Service Not Accessible
```bash
# Check service endpoints
kubectl get endpoints -n econ-graph
kubectl describe service <service-name> -n econ-graph
```

#### NodePort Services Not Accessible (Kind Cluster Issue)
**Problem**: NodePort services (ports 30000, 30080, 30001, 30002) may not be accessible via `localhost` in kind clusters.

**Root Cause**: Kind runs Kubernetes inside Docker containers, and NodePort services are bound to the kind node's internal network, not the host's localhost.

**Solutions**:

1. **Use Port Forwarding (Recommended)**:
```bash
# Frontend
kubectl port-forward service/econ-graph-frontend-service 3000:3000 -n econ-graph
# Backend  
kubectl port-forward service/econ-graph-backend-service 9876:9876 -n econ-graph
# Admin Frontend
kubectl port-forward service/econ-graph-admin-frontend-service 3001:3001 -n econ-graph
# Grafana
kubectl port-forward service/grafana 3001:3000 -n econ-graph
```

2. **Configure Kind with Extra Port Mappings**:
```bash
# Create kind cluster with port mappings
kind create cluster --name econ-graph --config - <<EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 30000
    hostPort: 30000
  - containerPort: 30080
    hostPort: 30080
  - containerPort: 30001
    hostPort: 30001
  - containerPort: 30002
    hostPort: 30002
EOF
```

3. **Use Ingress (For Admin UI)**:
```bash
# Add to /etc/hosts (or equivalent)
echo "127.0.0.1 admin.econ-graph.local" >> /etc/hosts

# Access via ingress controller port
curl -H "Host: admin.econ-graph.local" http://localhost:30466/admin
```

#### Database Connection Issues
```bash
# Check PostgreSQL status
kubectl logs -f statefulset/postgresql -n econ-graph

# Test database connectivity
kubectl exec -it postgresql-0 -n econ-graph -- psql -U postgres -d econ_graph
```

### Debug Commands

```bash
# Get all resources
kubectl get all -n econ-graph

# Check resource usage
kubectl top pods -n econ-graph

# View all events
kubectl get events -n econ-graph --sort-by='.metadata.creationTimestamp'

# Port forward for debugging
kubectl port-forward service/econ-graph-backend 8080:9876 -n econ-graph
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

### Monitoring Restart
```bash
kubectl rollout restart statefulset/grafana -n econ-graph
kubectl rollout restart deployment/prometheus -n econ-graph
kubectl rollout restart daemonset/promtail -n econ-graph
```

## Security Considerations

### Network Security
- **Internal Services**: Chart API service only accessible from cluster internal networks
- **External Access**: Only NodePort services exposed externally
- **Authentication**: Required headers for internal API access

### Data Security
- **Secrets Management**: Kubernetes secrets for sensitive configuration
- **Network Policies**: Restrict inter-pod communication
- **RBAC**: Role-based access control for cluster resources

## Performance Optimization

### Resource Limits
All deployments include resource requests and limits:
- **CPU**: Appropriate limits based on service requirements
- **Memory**: Memory limits to prevent resource exhaustion
- **Storage**: Persistent volumes for database and logs

### Scaling
```bash
# Scale deployments
kubectl scale deployment econ-graph-backend --replicas=3 -n econ-graph
kubectl scale deployment econ-graph-frontend --replicas=2 -n econ-graph
```

## Cleanup

### Remove Application
```bash
kubectl delete -f k8s/manifests/
kubectl delete -f k8s/monitoring/
```

### Remove Cluster
```bash
kind delete cluster --name econ-graph
```

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
