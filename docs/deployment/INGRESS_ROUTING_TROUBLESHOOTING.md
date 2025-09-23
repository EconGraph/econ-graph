# Ingress Routing Troubleshooting Guide

## How Ingress Routing Works in Kubernetes

### 1. **Ingress Controller Architecture**

```
Internet/Client → Ingress Controller (nginx) → Kubernetes Services → Pods
```

**Components:**
- **Ingress Resource**: Defines routing rules (hosts, paths, backends)
- **Ingress Controller**: nginx pod that reads ingress rules and generates nginx config
- **Services**: Kubernetes services that route to pods
- **Pods**: Actual application containers

### 2. **Request Flow**

1. **Client Request**: `curl -H "Host: admin.econ-graph.local" http://localhost:8080/`
2. **Ingress Controller**: Receives request, matches against ingress rules
3. **Path Matching**: nginx processes location blocks in order
4. **Service Routing**: Forwards to appropriate Kubernetes service
5. **Pod Response**: Service routes to healthy pod, response returned

### 3. **Path Matching Rules**

**Order Matters**: nginx processes location blocks in the order they appear in the generated config.

**Path Types:**
- `Exact`: Matches exactly (e.g., `/admin` matches only `/admin`)
- `Prefix`: Matches prefix (e.g., `/admin` matches `/admin`, `/admin/users`, etc.)

**Critical Rule**: More specific paths should come BEFORE less specific paths.

## Common Issues and Solutions

### Issue 1: Root Path Returns Wrong Service

**Symptoms:**
- `curl http://localhost:8080/` returns admin interface instead of frontend
- Test script shows "FAIL - Wrong content returned"

**Root Cause:**
- Path order in ingress configuration
- nginx location block order in generated config
- Default backend interference

**Diagnosis Steps:**

1. **Check Ingress Configuration:**
```bash
kubectl get ingress econ-graph-ingress -n econ-graph -o yaml
```

2. **Check Generated nginx Config:**
```bash
kubectl exec -n ingress-nginx deployment/ingress-nginx-controller -- cat /etc/nginx/nginx.conf | grep -A 20 -B 5 "location / {"
```

3. **Verify Service Endpoints:**
```bash
kubectl get endpoints -n econ-graph
```

**Solutions:**

**A. Fix Path Order (Recommended):**
```yaml
paths:
- path: /admin
  pathType: Exact  # Use Exact instead of Prefix
  backend:
    service:
      name: econ-graph-admin-frontend-service
      port:
        number: 3001
- path: /
  pathType: Prefix
  backend:
    service:
      name: econ-graph-frontend-service
      port:
        number: 3000
```

**B. Check for Default Backend:**
```bash
kubectl get ingressclass nginx -o yaml
kubectl get configmap -n ingress-nginx
```

### Issue 2: NodePort Services Not Accessible via localhost

**Symptoms:**
- `curl http://localhost:30000` returns connection refused
- Services work via `kubectl port-forward` but not NodePort

**Root Cause:**
- kind cluster runs Kubernetes inside Docker
- NodePort services bind to kind node's internal network, not host localhost
- Missing `extraPortMappings` in kind cluster configuration

**Solutions:**

**A. Use Port Forwarding (Quick Fix):**
```bash
kubectl port-forward service/ingress-nginx-controller 8080:80 -n ingress-nginx
```

**B. Recreate kind Cluster with Port Mappings (Permanent Fix):**
```bash
# Delete existing cluster
kind delete cluster --name econ-graph

# Create with port mappings
kind create cluster --name econ-graph --config - <<EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 30000  # Frontend NodePort
    hostPort: 30000
    listenAddress: "127.0.0.1"
  - containerPort: 30080  # Backend NodePort
    hostPort: 30080
    listenAddress: "127.0.0.1"
  - containerPort: 30001  # Grafana NodePort
    hostPort: 30001
    listenAddress: "127.0.0.1"
  - containerPort: 30002  # Admin Frontend NodePort
    hostPort: 30002
    listenAddress: "127.0.0.1"
EOF
```

### Issue 3: MCP Endpoint Returns 405 Method Not Allowed

**Symptoms:**
- `curl -X POST http://localhost:8080/mcp` returns 405
- nginx error: "405 Not Allowed"

**Root Cause:**
- nginx blocking POST requests to certain paths
- Missing nginx configuration for POST methods
- CORS preflight issues

**Solutions:**

**A. Add nginx Annotations:**
```yaml
annotations:
  nginx.ingress.kubernetes.io/configuration-snippet: |
    location /mcp {
      proxy_method POST;
      proxy_pass http://econ-graph-backend-service:9876;
    }
```

**B. Check CORS Configuration:**
```yaml
annotations:
  nginx.ingress.kubernetes.io/cors-allow-methods: "GET, POST, PUT, DELETE, OPTIONS"
  nginx.ingress.kubernetes.io/enable-cors: "true"
```

### Issue 4: Ingress Controller Not Reloading Configuration

**Symptoms:**
- Changes to ingress don't take effect
- Old routing rules still active
- nginx config not updated

**Solutions:**

**A. Force Reload:**
```bash
kubectl rollout restart deployment/ingress-nginx-controller -n ingress-nginx
```

**B. Check Controller Logs:**
```bash
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller --tail=50
```

**C. Verify Ingress Status:**
```bash
kubectl describe ingress econ-graph-ingress -n econ-graph
```

## Verification Steps

### Step 1: Verify Ingress Controller is Running
```bash
kubectl get pods -n ingress-nginx
kubectl get services -n ingress-nginx
```

### Step 2: Verify Ingress Resource
```bash
kubectl get ingress -n econ-graph
kubectl describe ingress econ-graph-ingress -n econ-graph
```

### Step 3: Verify Services and Endpoints
```bash
kubectl get services -n econ-graph
kubectl get endpoints -n econ-graph
```

### Step 4: Test Individual Services
```bash
# Test frontend service directly
kubectl port-forward service/econ-graph-frontend-service 8081:3000 -n econ-graph &
curl http://localhost:8081 | grep -o "<title>[^<]*</title>"

# Test admin service directly
kubectl port-forward service/econ-graph-admin-frontend-service 8082:3001 -n econ-graph &
curl http://localhost:8082 | grep -o "<title>[^<]*</title>"
```

### Step 5: Test Ingress Routing
```bash
# Set up ingress port-forward
kubectl port-forward service/ingress-nginx-controller 8080:80 -n ingress-nginx &

# Test routing
curl -H "Host: admin.econ-graph.local" http://localhost:8080/ | grep -o "<title>[^<]*</title>"
curl -H "Host: admin.econ-graph.local" http://localhost:8080/admin | grep -o "<title>[^<]*</title>"
```

### Step 6: Run Comprehensive Test
```bash
./scripts/test-ingress-routing.sh
```

## Expected Results

### ✅ Working Configuration
- **Frontend** (`/`): Returns "EconGraph - Economic Data Visualization"
- **Admin UI** (`/admin`): Returns "EconGraph Admin - System Administration"
- **Backend Health** (`/health`): Returns "OK" or "healthy"
- **GraphQL** (`/graphql`): Returns GraphQL schema or error (not 404)
- **MCP** (`/mcp`): Accepts POST requests, returns JSON-RPC response

### ❌ Common Failures
- Root path returns admin interface (path order issue)
- NodePort services not accessible (kind cluster issue)
- MCP returns 405 (nginx POST method issue)
- Services return 502/503 (endpoint/pod issues)

## Debugging Commands

### Check nginx Configuration
```bash
kubectl exec -n ingress-nginx deployment/ingress-nginx-controller -- cat /etc/nginx/nginx.conf | grep -A 50 "admin.econ-graph.local"
```

### Check Ingress Controller Logs
```bash
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller --tail=100 | grep -i error
```

### Check Service Connectivity
```bash
kubectl exec -n econ-graph deployment/econ-graph-backend -- curl -s http://econ-graph-frontend-service:3000/health
```

### Check DNS Resolution
```bash
kubectl exec -n econ-graph deployment/econ-graph-backend -- nslookup econ-graph-frontend-service
```

## Best Practices

1. **Path Order**: More specific paths before less specific paths
2. **Path Types**: Use `Exact` for specific routes, `Prefix` for catch-all
3. **Testing**: Always test individual services before testing ingress
4. **Monitoring**: Check ingress controller logs for configuration issues
5. **Documentation**: Keep ingress rules simple and well-documented
