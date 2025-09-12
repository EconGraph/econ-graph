# EconGraph Terraform Deployment

This directory contains Terraform configurations for deploying the EconGraph platform to Kubernetes.

## Overview

The Terraform configuration deploys a complete production-ready environment including:

- **PostgreSQL Database** - Optimized for time series data with persistence
- **Backend API** - Rust GraphQL server with auto-scaling
- **Crawler Service** - Data collection with queue processing
- **Frontend** - React application with Nginx
- **Monitoring Stack** - Prometheus and Grafana with custom dashboards
- **Ingress Controller** - NGINX with SSL termination

## Prerequisites

### Required Tools
- [Terraform](https://www.terraform.io/downloads) >= 1.0
- [kubectl](https://kubernetes.io/docs/tasks/tools/) configured for your cluster
- [Helm](https://helm.sh/docs/intro/install/) >= 3.0

### Required Infrastructure
- Kubernetes cluster (1.24+) with:
  - At least 3 worker nodes
  - 8 CPU cores and 16GB RAM total
  - Storage class for persistent volumes
  - LoadBalancer service support

### API Keys (Optional but Recommended)
- [FRED API Key](https://fred.stlouisfed.org/docs/api/api_key.html)
- [BLS API Key](https://www.bls.gov/developers/api_signature_v2.html)

## Quick Start

1. **Clone and navigate to the terraform directory:**
   ```bash
   cd terraform
   ```

2. **Create terraform.tfvars file:**
   ```hcl
   # terraform.tfvars
   domain            = "econgraph.yourdomain.com"
   database_password = "your-secure-password"
   fred_api_key     = "your-fred-api-key"
   bls_api_key      = "your-bls-api-key"
   environment      = "prod"
   ```

3. **Initialize and deploy:**
   ```bash
   terraform init
   terraform plan
   terraform apply
   ```

4. **Access the application:**
   - Frontend: `https://econgraph.yourdomain.com`
   - Grafana: `https://grafana.econgraph.yourdomain.com`
   - API: `https://api.econgraph.yourdomain.com/graphql`

## Configuration Variables

### Required Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `domain` | Domain name for the application | `"econgraph.example.com"` |
| `database_password` | PostgreSQL password | `"secure-password-123"` |

### Optional Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `namespace` | Kubernetes namespace | `"econgraph"` |
| `environment` | Environment name | `"prod"` |
| `kubeconfig_path` | Path to kubeconfig | `"~/.kube/config"` |
| `fred_api_key` | FRED API key | `""` |
| `bls_api_key` | BLS API key | `""` |

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Load Balancer │    │  Ingress NGINX  │    │   Cert Manager  │
│                 │────│                 │────│                 │
│  (Cloud Provider)│    │  SSL Termination│    │  Let's Encrypt  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                │
                ┌───────────────┼───────────────┐
                │               │               │
        ┌───────▼───────┐ ┌─────▼──────┐ ┌─────▼──────┐
        │   Frontend    │ │  Backend   │ │  Grafana   │
        │               │ │            │ │            │
        │ React + Nginx │ │ Rust + GraphQL│ │ Monitoring │
        │  (3 replicas) │ │ (3 replicas)│ │            │
        └───────────────┘ └─────┬──────┘ └────────────┘
                                │
                        ┌───────▼───────┐
                        │   PostgreSQL  │
                        │               │
                        │ Time Series DB│
                        │  (StatefulSet)│
                        └───────┬───────┘
                                │
                        ┌───────▼───────┐
                        │   Crawler     │
                        │               │
                        │ Queue Processing│
                        │  (2 replicas) │
                        └───────────────┘
```

## Modules

### PostgreSQL Module (`modules/postgresql/`)
- StatefulSet with persistent storage
- Optimized configuration for time series data
- Health checks and monitoring
- Automated backups (configurable)

### Backend Module (`modules/backend/`)
- Deployment with horizontal pod autoscaling
- Health checks and readiness probes
- Service mesh integration ready
- Metrics endpoint for Prometheus

### Crawler Module (`modules/crawler/`)
- Background service for data collection
- CronJobs for scheduled crawling
- Queue processing with SKIP LOCKED
- Rate limiting and error handling

### Frontend Module (`modules/frontend/`)
- Nginx-based static file serving
- Optimized caching and compression
- API proxy configuration
- Security headers

### Monitoring Module (`modules/monitoring/`)
- Prometheus with custom metrics
- Grafana with pre-built dashboards
- AlertManager for notifications
- ServiceMonitors for auto-discovery

### Ingress Module (`modules/ingress/`)
- NGINX Ingress Controller
- SSL/TLS termination with Let's Encrypt
- Security headers and rate limiting
- Network policies

## Monitoring and Observability

### Grafana Dashboards

1. **EconGraph System Metrics**
   - Backend pod status and health
   - API response times and error rates
   - Database connection metrics
   - Resource utilization

2. **Database Statistics**
   - Database size and growth
   - Table sizes and query performance
   - Connection pool status
   - Slow query analysis

3. **Crawler Status**
   - Active crawler instances
   - Queue processing rates
   - Data source crawl status
   - Error rates and retry metrics

### Prometheus Alerts

- Backend service downtime
- High API response times
- Database connection issues
- Crawler queue backlog
- Resource exhaustion

### Accessing Monitoring

- **Grafana**: `https://grafana.yourdomain.com`
  - Username: `admin`
  - Password: (generated, see terraform output)
- **Prometheus**: Internal cluster access only

## SSL/TLS Configuration

The deployment automatically provisions SSL certificates using Let's Encrypt:

1. **cert-manager** installs and configures certificate management
2. **ClusterIssuer** handles Let's Encrypt ACME challenges
3. **Ingress** resources automatically request certificates
4. **Automatic renewal** prevents certificate expiration

To use custom certificates, disable cert-manager:
```hcl
# In terraform.tfvars
enable_cert_manager = false
```

## Scaling Configuration

### Horizontal Pod Autoscaling (HPA)

- **Backend**: 2-10 replicas based on CPU/memory
- **Frontend**: 2-10 replicas based on CPU/memory
- **Crawler**: Fixed 2 replicas (can be adjusted)

### Vertical Scaling

Resource limits can be adjusted in module variables:

```hcl
module "backend" {
  # Override default resource limits
  cpu_request    = "200m"
  memory_request = "512Mi"
  cpu_limit      = "1000m"
  memory_limit   = "2Gi"
}
```

## Backup and Recovery

### Database Backups

PostgreSQL backups can be configured using:

1. **pg_dump CronJob** (included in postgresql module)
2. **Volume snapshots** (cloud provider dependent)
3. **External backup solutions** (Velero, etc.)

### Configuration Backup

```bash
# Backup Kubernetes resources
kubectl get all,configmaps,secrets,ingress -n econgraph -o yaml > econgraph-backup.yaml

# Backup Terraform state
terraform state pull > terraform.tfstate.backup
```

## Troubleshooting

### Common Issues

1. **Pods stuck in Pending state**
   ```bash
   kubectl describe pod <pod-name> -n econgraph
   # Check resource availability and node affinity
   ```

2. **Database connection errors**
   ```bash
   kubectl logs -n econgraph -l app=econgraph-backend
   # Check database credentials and network policies
   ```

3. **SSL certificate issues**
   ```bash
   kubectl describe certificate -n econgraph
   kubectl logs -n cert-manager deployment/cert-manager
   ```

4. **Ingress not accessible**
   ```bash
   kubectl get ingress -n econgraph
   kubectl describe ingress econgraph-ingress -n econgraph
   ```

### Debugging Commands

```bash
# Check all resources
kubectl get all -n econgraph

# View logs
kubectl logs -n econgraph -l app=econgraph-backend -f
kubectl logs -n econgraph -l app=econgraph-crawler -f

# Port forwarding for local access
kubectl port-forward -n econgraph svc/econgraph-backend 8080:80
kubectl port-forward -n econgraph-monitoring svc/prometheus-grafana 3000:80

# Execute into pods
kubectl exec -it -n econgraph deployment/econgraph-backend -- /bin/bash
```

## Security Considerations

### Network Policies
- Ingress traffic restricted to ingress controller
- Inter-pod communication controlled
- Database access limited to application pods

### RBAC
- Minimal service account permissions
- Separate roles for each component
- No cluster-admin privileges

### Secrets Management
- All sensitive data in Kubernetes secrets
- Secrets encrypted at rest (cluster dependent)
- Consider external secret management (Vault, etc.)

### Security Headers
- CSP, HSTS, and other security headers configured
- Rate limiting on API endpoints
- Basic auth on monitoring endpoints

## Maintenance

### Updates

1. **Application Updates**
   ```bash
   # Update image tags in terraform.tfvars
   image_tag = "v1.1.0"
   terraform apply
   ```

2. **Kubernetes Updates**
   ```bash
   # Test in staging first
   terraform plan
   terraform apply
   ```

3. **Certificate Renewal**
   - Automatic with cert-manager
   - Monitor certificate expiration in Grafana

### Monitoring Health

```bash
# Check cluster health
kubectl get nodes
kubectl top nodes
kubectl top pods -n econgraph

# Check application health
curl -k https://yourdomain.com/health
curl -k https://api.yourdomain.com/health
```

## Cost Optimization

### Resource Requests
- Set appropriate resource requests/limits
- Use HPA to scale down during low usage
- Consider spot instances for development

### Storage
- Monitor database growth
- Implement data retention policies
- Use appropriate storage classes

### Monitoring
- Review Grafana dashboards for optimization opportunities
- Set up alerts for cost anomalies
- Regular resource utilization reviews

## Development vs Production

### Development Configuration
```hcl
# terraform.tfvars for dev
environment = "dev"
replicas = {
  backend  = 1
  frontend = 1
  crawler  = 1
}
enable_cert_manager = false
```

### Production Configuration
```hcl
# terraform.tfvars for prod
environment = "prod"
replicas = {
  backend  = 3
  frontend = 3
  crawler  = 2
}
enable_cert_manager = true
enable_monitoring = true
enable_backups = true
```

## Support

For issues and questions:

1. Check the troubleshooting section above
2. Review Kubernetes and application logs
3. Consult the monitoring dashboards
4. Contact the development team directly

---

**Note**: This deployment assumes a managed Kubernetes cluster. For self-managed clusters, additional configuration may be required for load balancers, storage classes, and networking.
