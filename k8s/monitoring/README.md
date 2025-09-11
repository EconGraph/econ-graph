# EconGraph Monitoring Stack

This directory contains the Kubernetes manifests for the monitoring stack deployed with EconGraph.

## Components

### Grafana
- **Purpose**: Dashboard and visualization platform
- **Access**: http://localhost:30001
- **Credentials**: admin/admin123
- **Features**: 
  - Pre-configured Loki datasource
  - Real-time log visualization
  - Custom dashboards for application monitoring

### Loki
- **Purpose**: Log aggregation system
- **Port**: 3100 (internal)
- **Features**:
  - Collects logs from all pods in the econ-graph namespace
  - Stores logs in filesystem backend
  - Provides log querying API for Grafana

### Promtail
- **Purpose**: Log collection agent
- **Deployment**: DaemonSet (runs on all nodes)
- **Features**:
  - Automatically discovers and collects logs from Kubernetes pods
  - Sends logs to Loki for aggregation
  - Configurable log parsing and labeling

## Usage

### Accessing Grafana
1. Navigate to http://localhost:30001
2. Login with admin/admin123
3. Go to "Explore" to query logs using Loki
4. Create dashboards for monitoring application health

### Log Queries
Example Loki queries for EconGraph:
```
# All backend logs
{app="econ-graph-backend"}

# Authentication errors
{app="econ-graph-backend"} |= "auth" |= "error"

# Database connection issues
{app="econ-graph-backend"} |= "database" |= "connection"

# All logs from econ-graph namespace
{namespace="econ-graph"}
```

### Monitoring Authentication Issues
To debug authentication problems:
1. Go to Grafana Explore
2. Select Loki datasource
3. Use query: `{app="econ-graph-backend"} |= "auth"`
4. Filter by time range to see recent authentication attempts

## Configuration

### Grafana Configuration
- Admin password: Set via `GF_SECURITY_ADMIN_PASSWORD` environment variable
- Loki datasource: Pre-configured via ConfigMap
- Storage: Uses emptyDir (data lost on pod restart)

### Loki Configuration
- Storage: Filesystem-based (emptyDir)
- Retention: Disabled (logs kept indefinitely)
- Schema: v11 with boltdb-shipper index

### Promtail Configuration
- Scrapes logs from all pods in econ-graph namespace
- Automatically labels logs with pod, namespace, and app information
- Sends logs to Loki service

## Deployment

The monitoring stack is automatically deployed when running:
```bash
./scripts/deploy/restart-k8s-rollout.sh
```

Or manually:
```bash
kubectl apply -f k8s/monitoring/
```

## Troubleshooting

### Grafana Not Accessible
1. Check if port-forward is running: `kubectl port-forward -n econ-graph service/grafana-service 30001:3000`
2. Verify pod status: `kubectl get pods -n econ-graph | grep grafana`
3. Check logs: `kubectl logs -n econ-graph deployment/grafana`

### No Logs in Loki
1. Check Promtail status: `kubectl get pods -n econ-graph | grep promtail`
2. Verify Loki is running: `kubectl get pods -n econ-graph | grep loki`
3. Check Promtail logs: `kubectl logs -n econ-graph daemonset/promtail`

### Performance Issues
- Grafana: Increase memory limits in deployment
- Loki: Adjust retention settings or increase storage
- Promtail: Reduce log collection scope if needed
