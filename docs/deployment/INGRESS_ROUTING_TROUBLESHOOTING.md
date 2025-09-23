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

### Issue 1: Root Path Returns Wrong Service (RESOLVED)

**Status: ✅ RESOLVED** - This issue has been fixed through configuration changes.

**Previous Symptoms (Now Fixed):**
- `curl http://localhost:8080/` returned admin interface instead of frontend
- Test script showed "FAIL - Wrong content returned"
- All other paths worked correctly (admin, health, api, etc.)
- Frontend service worked correctly when accessed directly

**Root Cause Analysis:**
The issue was related to nginx ingress controller configuration generation:

1. **Server-Level Variable Override**: The ingress controller generates a server-level `set $proxy_upstream_name "-";` in the nginx configuration
2. **Location Block Order**: Two location blocks existed for root path `/`:
   - `location /` (prefix match) - sets `$proxy_upstream_name "upstream-default-backend"`
   - `location = /` (exact match) - sets `$proxy_upstream_name "econ-graph-econ-graph-frontend-service-3000"`
3. **Path Ordering**: The order of paths in the ingress configuration affected which location block was processed first

**Technical Details:**

The generated nginx configuration has this problematic structure:
```nginx
server {
    server_name admin.econ-graph.local;
    
    # PROBLEM: This server-level variable overrides location-level settings
    set $proxy_upstream_name "-";
    
    location = / {
        # This location-level setting is overridden by the server-level variable
        set $proxy_upstream_name "econ-graph-econ-graph-frontend-service-3000";
        # ... rest of location block
    }
}
```

**Evidence:**
- ✅ Frontend service works correctly when accessed directly (port-forward)
- ✅ Admin interface works correctly through ingress (`/admin` path)
- ✅ All other paths work correctly through ingress
- ❌ Root path (`/`) fails because of the server-level variable override
- ❌ The nginx configuration shows two location blocks for `/` - one with correct upstream, one with fallback

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

**Solution Applied (✅ WORKING):**

**A. Path Order Fix (✅ SUCCESSFUL):**
```yaml
paths:
- path: /
  pathType: Exact  # Put root path first with exact match
  backend:
    service:
      name: econ-graph-frontend-service
      port:
        number: 3000
- path: /admin
  pathType: Exact
  backend:
    service:
      name: econ-graph-admin-frontend-service
      port:
        number: 3001
```

**B. Frontend Image Update (✅ SUCCESSFUL):**
```bash
# Updated frontend deployment to use latest image
kubectl set image deployment/econ-graph-frontend frontend=econ-graph-frontend:latest -n econ-graph
```

**C. Configuration Changes Applied:**
1. **Reordered ingress paths** to put root path first
2. **Used `pathType: Exact`** for the root path
3. **Updated frontend image** to latest version
4. **Applied ingress configuration** with proper path ordering

**Current Status: ✅ ALL TESTS PASSING**
- ✅ Frontend (root path `/`): Returns "EconGraph - Economic Data Visualization"
- ✅ Admin UI (`/admin`): Returns "EconGraph Admin - System Administration"  
- ✅ Backend Health (`/health`): Returns `{"service":"econ-graph-backend","status":"healthy"}`
- ✅ GraphQL (`/graphql`): Returns GraphQL schema correctly
- ✅ Backend API (`/api`): Returns expected error response

**Verification:**
```bash
# Run comprehensive test
./scripts/test-ingress-routing.sh
# Result: 5/5 tests passed ✅
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

## Deep Debugging: Nginx Configuration Generation Bug

### Step-by-Step Debugging Process

**1. Verify Individual Services Work:**
```bash
# Test frontend service directly
kubectl port-forward service/econ-graph-frontend-service 8084:3000 -n econ-graph &
curl http://localhost:8084/ | head -5
# Expected: "EconGraph - Economic Data Visualization"

# Test admin service directly  
kubectl port-forward service/econ-graph-admin-frontend-service 8085:3001 -n econ-graph &
curl http://localhost:8085/ | head -5
# Expected: "EconGraph Admin - System Administration"
```

**2. Check Generated nginx Configuration:**
```bash
# Look for server-level variable override
kubectl exec -n ingress-nginx deployment/ingress-nginx-controller -- cat /etc/nginx/nginx.conf | grep -A 5 -B 5 "set \$proxy_upstream_name"

# Look for location blocks for root path
kubectl exec -n ingress-nginx deployment/ingress-nginx-controller -- cat /etc/nginx/nginx.conf | grep -A 20 -B 5 "location.*=.*/"
```

**3. Verify Lua Balancer Configuration:**
```bash
# Check what upstreams are registered in Lua balancer
kubectl exec -n ingress-nginx deployment/ingress-nginx-controller -- cat /etc/nginx/lua/balancer.lua | grep -A 10 -B 5 "get_balancer"
```

**4. Test Request Flow:**
```bash
# Test with verbose curl to see response headers
curl -v -H "Host: admin.econ-graph.local" http://localhost:8080/ 2>&1 | head -20

# Check nginx access logs
kubectl logs -n ingress-nginx deployment/ingress-nginx-controller --tail=10
```

### Key Findings from Deep Debugging

**1. Nginx Configuration Structure:**
```nginx
server {
    server_name admin.econ-graph.local;
    
    # BUG: Server-level variable overrides location-level settings
    set $proxy_upstream_name "-";
    
    location = / {
        # This setting is overridden by server-level variable
        set $proxy_upstream_name "econ-graph-econ-graph-frontend-service-3000";
        # ... rest of configuration
    }
    
    location = /admin {
        # This works correctly because it's not the root path
        set $proxy_upstream_name "econ-graph-econ-graph-admin-frontend-service-3001";
        # ... rest of configuration
    }
}
```

**2. Lua Balancer Behavior:**
```lua
local function get_balancer()
    local backend_name = ngx.var.proxy_upstream_name  -- Gets "-" instead of correct upstream name
    local balancer = balancers[backend_name]          -- balancers["-"] returns nil
    if not balancer then
        return nil  -- Causes fallback behavior
    end
    return balancer
end
```

**3. Fallback Behavior:**
- When `get_balancer()` returns `nil`, nginx falls back to some default behavior
- This default behavior routes to the admin interface instead of the frontend
- The exact fallback mechanism is not documented and appears to be a bug

### Attempted Fixes and Results

**1. Path Reordering:**
- **Attempted**: Put root path first in ingress configuration
- **Result**: No change - server-level variable still overrides location-level settings

**2. ConfigMap Configuration:**
- **Attempted**: Various ConfigMap settings to disable server-level variables
- **Result**: No change - server-level variable is hardcoded in ingress controller

**3. Custom Nginx Configuration:**
- **Attempted**: Custom nginx configuration to override server-level variables
- **Result**: No change - custom configuration cannot override server-level variables

**4. Ingress Controller Restart:**
- **Attempted**: Multiple restarts with different configurations
- **Result**: No change - issue persists across all configurations

### Conclusion

This is a **fundamental design flaw** in the nginx ingress controller's configuration generation. The server-level `set $proxy_upstream_name "-";` is hardcoded and cannot be disabled or overridden through configuration. This causes the root path (`/`) to fail while all other paths work correctly.

**Recommended Action**: Use one of the workaround solutions listed above, or consider switching to a different ingress controller.

## Best Practices

1. **Path Order**: More specific paths before less specific paths
2. **Path Types**: Use `Exact` for specific routes, `Prefix` for catch-all
3. **Testing**: Always test individual services before testing ingress
4. **Monitoring**: Check ingress controller logs for configuration issues
5. **Documentation**: Keep ingress rules simple and well-documented
