# Chart API Service - Separate Kubernetes Service

## Overview

The Chart API Service is now a **completely separate Kubernetes service** with its own deployment, service, and internal-only network access. This provides better security, scalability, and separation of concerns for the MCP server's chart generation capabilities.

## Architecture

```
┌─────────────────┐    Internal K8s Network    ┌──────────────────┐
│   MCP Server    │ ──────────────────────────► │  Chart API       │
│   (Backend)     │                             │   Service        │
│                 │                             │  (Separate Pod)  │
└─────────────────┘                             └──────────────────┘
         │                                               │
         │                                               │
         ▼                                               ▼
┌─────────────────┐                             ┌──────────────────┐
│   GraphQL API   │                             │   Chart.js       │
│   (Data Source) │                             │   Components     │
└─────────────────┘                             └──────────────────┘
```

## Service Structure

### Chart API Service Directory
```
chart-api-service/
├── src/
│   ├── server.js          # Main Express.js server
│   ├── chartApi.js        # Chart configuration generation
│   └── security.js        # Security and access control
├── k8s/
│   ├── deployment.yaml    # K8s deployment manifest
│   ├── service.yaml       # K8s service manifest (ClusterIP)
│   └── configmap.yaml     # K8s configuration
├── tests/
│   ├── chartApi.test.js   # Chart API tests
│   └── server.test.js     # Server tests
├── Dockerfile             # Container image
└── package.json           # Node.js dependencies
```

## Kubernetes Deployment

### Service Configuration
- **Type**: `ClusterIP` (internal only, not exposed externally)
- **Port**: `3001`
- **Namespace**: `econ-graph`
- **Replicas**: `2` (for high availability)

### Security Features
- **Internal Network Only**: No external access
- **Required Headers**: `X-MCP-Server-Request: true`, `X-Internal-Request: true`
- **Rate Limiting**: 1000 requests per 15 minutes
- **Security Headers**: Helmet.js protection
- **Non-root User**: Runs as user 1000

### Resource Limits
- **CPU**: 100m request, 500m limit
- **Memory**: 128Mi request, 512Mi limit
- **Readiness Probe**: `/health` endpoint
- **Liveness Probe**: `/health` endpoint

## API Endpoints

### 1. Health Check
**GET** `/health`
```json
{
  "success": true,
  "message": "Chart API Service is healthy",
  "timestamp": "2024-01-15T10:30:00.000Z",
  "version": "1.0.0",
  "environment": "production",
  "uptime": 3600
}
```

### 2. Chart Generation
**POST** `/api/chart/generate`
```json
{
  "seriesData": [...],
  "chartType": "line",
  "title": "Economic Data Visualization",
  "showLegend": true,
  "showGrid": true,
  "yAxisLabel": "Value",
  "xAxisLabel": "Date"
}
```

### 3. Chart Types
**GET** `/api/chart/types`
```json
{
  "success": true,
  "chartTypes": [
    {
      "type": "line",
      "description": "Line charts for time series data and trends",
      "supportedFeatures": ["multiple series", "time scale", "smooth curves"]
    },
    {
      "type": "bar",
      "description": "Bar charts for categorical comparisons",
      "supportedFeatures": ["grouped bars", "stacked bars", "categorical data"]
    },
    {
      "type": "scatter",
      "description": "Scatter plots for correlation analysis",
      "supportedFeatures": ["point markers", "trend lines", "correlation analysis"]
    }
  ]
}
```

### 4. Service Info
**GET** `/api/info`
```json
{
  "success": true,
  "service": "EconGraph Chart API",
  "version": "1.0.0",
  "description": "Private chart generation service for MCP server integration",
  "endpoints": [...],
  "security": {
    "internalNetworkOnly": true,
    "requiredHeaders": ["X-MCP-Server-Request", "X-Internal-Request"],
    "rateLimit": "1000 requests per 15 minutes"
  }
}
```

## MCP Server Integration

### Environment Configuration
The MCP server connects to the chart API service using the internal Kubernetes service URL:

```bash
CHART_API_SERVICE_URL=http://chart-api-service.econ-graph.svc.cluster.local:3001/api/chart
```

### Request Flow
1. **AI Model** calls MCP server's `create_data_visualization` tool
2. **MCP Server** collects economic data via GraphQL API
3. **MCP Server** calls chart API service with chart request
4. **Chart API Service** generates professional Chart.js configuration
5. **MCP Server** returns complete chart configuration to AI model

### Security Headers
All requests from MCP server include:
```http
X-MCP-Server-Request: true
X-Internal-Request: true
Content-Type: application/json
```

## Deployment

### Build and Deploy
```bash
# Build all images including chart API service
./scripts/deploy/build-images.sh

# Deploy to Kubernetes
./scripts/deploy/deploy.sh
```

### Manual Deployment
```bash
# Build chart API service image
cd chart-api-service
docker build -t econ-graph-chart-api:v1.0.0 .

# Load into kind cluster
kind load docker-image econ-graph-chart-api:v1.0.0 --name econ-graph

# Deploy to Kubernetes
kubectl apply -f k8s/manifests/chart-api-deployment.yaml
kubectl apply -f k8s/manifests/chart-api-service.yaml
```

## Monitoring and Logging

### Health Checks
```bash
# Check service health
kubectl get pods -l app=chart-api-service -n econ-graph

# Check service logs
kubectl logs -f deployment/chart-api-service -n econ-graph

# Test health endpoint
kubectl port-forward svc/chart-api-service 3001:3001 -n econ-graph
curl -H "X-MCP-Server-Request: true" http://localhost:3001/health
```

### Service Discovery
```bash
# List all services
kubectl get services -n econ-graph

# Get service details
kubectl describe service chart-api-service -n econ-graph

# Test internal connectivity
kubectl exec -it deployment/econ-graph-backend -n econ-graph -- \
  curl -H "X-MCP-Server-Request: true" \
       -H "X-Internal-Request: true" \
       http://chart-api-service:3001/health
```

## Testing

### Unit Tests
```bash
cd chart-api-service
npm test
```

### Integration Tests
```bash
# Test chart generation
curl -X POST \
  -H "Content-Type: application/json" \
  -H "X-MCP-Server-Request: true" \
  -H "X-Internal-Request: true" \
  -d '{"seriesData":[{"id":"test","name":"Test","dataPoints":[{"date":"2020-01-01","value":100}]}],"chartType":"line"}' \
  http://chart-api-service.econ-graph.svc.cluster.local:3001/api/chart/generate
```

## Security Considerations

### Network Security
- **ClusterIP Service**: Only accessible within Kubernetes cluster
- **No External Access**: No ingress or load balancer configured
- **Internal DNS**: Uses Kubernetes service discovery

### Access Control
- **Header Validation**: Requires specific security headers
- **IP Validation**: Validates against internal network ranges
- **Rate Limiting**: Prevents abuse and DoS attacks

### Container Security
- **Non-root User**: Runs as user 1000
- **Read-only Filesystem**: Prevents file system modifications
- **Security Context**: Drops all capabilities
- **Resource Limits**: Prevents resource exhaustion

## Scaling and Performance

### Horizontal Scaling
```bash
# Scale chart API service
kubectl scale deployment chart-api-service --replicas=3 -n econ-graph
```

### Resource Monitoring
```bash
# Check resource usage
kubectl top pods -l app=chart-api-service -n econ-graph

# Check service metrics
kubectl get hpa -n econ-graph  # If HPA is configured
```

## Troubleshooting

### Common Issues

1. **Service Not Accessible**
   ```bash
   # Check if service is running
   kubectl get pods -l app=chart-api-service -n econ-graph
   
   # Check service endpoints
   kubectl get endpoints chart-api-service -n econ-graph
   ```

2. **Connection Refused**
   ```bash
   # Check service configuration
   kubectl describe service chart-api-service -n econ-graph
   
   # Test DNS resolution
   kubectl exec -it deployment/econ-graph-backend -n econ-graph -- nslookup chart-api-service
   ```

3. **Authentication Errors**
   ```bash
   # Check if required headers are present
   kubectl logs deployment/chart-api-service -n econ-graph | grep "Access denied"
   ```

### Debug Mode
```bash
# Enable debug logging
kubectl set env deployment/chart-api-service LOG_LEVEL=debug -n econ-graph

# Check logs
kubectl logs -f deployment/chart-api-service -n econ-graph
```

## Benefits of Separate Service

### Security
- **Isolation**: Chart API runs in separate pods with its own security context
- **Network Segmentation**: Internal-only access with no external exposure
- **Reduced Attack Surface**: Smaller, focused service with minimal dependencies

### Scalability
- **Independent Scaling**: Can scale chart API service independently from backend
- **Resource Optimization**: Dedicated resource limits and requests
- **Load Distribution**: Multiple replicas for high availability

### Maintainability
- **Separation of Concerns**: Chart generation logic isolated from main backend
- **Independent Deployment**: Can deploy chart API service updates independently
- **Technology Flexibility**: Can use different Node.js versions or frameworks

### Monitoring
- **Service-specific Metrics**: Dedicated monitoring for chart generation performance
- **Independent Logging**: Separate log streams for chart API operations
- **Health Checks**: Dedicated health endpoints for chart service

## Future Enhancements

### Advanced Features
- **Chart Caching**: Cache generated chart configurations
- **Image Export**: Generate PNG/SVG chart images
- **Batch Processing**: Process multiple chart requests
- **Custom Themes**: Support for different chart themes

### Performance Optimizations
- **Connection Pooling**: Optimize HTTP client connections
- **Compression**: Enable response compression
- **CDN Integration**: Cache chart assets

### Security Enhancements
- **mTLS**: Mutual TLS for service-to-service communication
- **Service Mesh**: Integrate with Istio or Linkerd
- **RBAC**: Role-based access control for service accounts

## Conclusion

The Chart API Service as a separate Kubernetes service provides:

- ✅ **Enhanced Security**: Internal-only access with proper network segmentation
- ✅ **Better Scalability**: Independent scaling and resource management
- ✅ **Improved Maintainability**: Separation of concerns and independent deployment
- ✅ **Production Ready**: Proper health checks, monitoring, and security controls
- ✅ **Kubernetes Native**: Full integration with Kubernetes service discovery and networking

This architecture ensures that the MCP server can generate professional charts while maintaining the highest security standards and operational excellence.
