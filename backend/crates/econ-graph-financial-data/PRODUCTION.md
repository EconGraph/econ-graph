# EconGraph Financial Data Service - Production Guide

## Overview

The EconGraph Financial Data Service is a high-performance, Arrow Flight-based service for storing and querying financial time series data. It provides a GraphQL API for data access and uses Apache Parquet files with Arrow Flight for zero-copy data transfer.

## Architecture

```
┌────────────────────────────────────────┐
│           Financial Data Service        │
├────────────────────────────────────────┤
│  GraphQL API (Port 3001)              │
│  Arrow Flight Storage (Parquet)        │
│  Health & Metrics Endpoints            │
└────────────────────────────────────────┘
```

## Quick Start

### Using Docker Compose (Recommended for Development)

1. **Clone and navigate to the service directory:**
   ```bash
   cd backend/crates/econ-graph-financial-data
   ```

2. **Start all services:**
   ```bash
   docker-compose up -d
   ```

3. **Access the services:**
   - Financial Data Service: http://localhost:3001
   - GraphQL Playground: http://localhost:3001
   - MinIO Console: http://localhost:9001 (admin/password123)
   - Prometheus: http://localhost:9090
   - Grafana: http://localhost:3000 (admin/admin)

### Using Docker (Production)

1. **Build the image:**
   ```bash
   docker build -t econ-graph-financial-data:latest .
   ```

2. **Run the container:**
   ```bash
   docker run -d \
     --name financial-data-service \
     -p 3001:3001 \
     -v $(pwd)/data:/app/data \
     -e RUST_LOG=info \
     -e DATA_DIR=/app/data \
     econ-graph-financial-data:latest
   ```

### Using Kubernetes

1. **Apply the deployment:**
   ```bash
   kubectl apply -f k8s/deployment.yaml
   ```

2. **Check the deployment:**
   ```bash
   kubectl get pods -l app=financial-data-service
   kubectl get services financial-data-service
   ```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Logging level (error, warn, info, debug, trace) |
| `RUST_BACKTRACE` | `1` | Enable backtraces for debugging |
| `DATA_DIR` | `/app/data` | Directory for Parquet files |
| `PORT` | `3001` | HTTP server port |

### Data Directory Structure

```
data/
├── series_<uuid>.parquet      # Economic series metadata
├── data_points_<uuid>.parquet # Time series data points
└── indexes/                   # Query indexes (future)
```

## API Endpoints

### GraphQL API

- **Endpoint:** `POST /graphql`
- **Playground:** `GET /` (http://localhost:3001)

**Example Query:**
```graphql
query {
  series(id: "123e4567-e89b-12d3-a456-426614174000") {
    id
    title
    description
    frequency
    dataPoints(startDate: "2023-01-01", endDate: "2023-12-31") {
      date
      value
      isOriginalRelease
    }
  }
}
```

**Example Mutation:**
```graphql
mutation {
  createSeries(input: {
    sourceId: "123e4567-e89b-12d3-a456-426614174000"
    externalId: "GDP_US"
    title: "US Gross Domestic Product"
    description: "Quarterly GDP data"
    frequency: "quarterly"
    isActive: true
  }) {
    id
    title
    createdAt
  }
}
```

### Health & Monitoring

- **Health Check:** `GET /health`
- **Metrics:** `GET /metrics`

**Health Response:**
```json
{
  "status": "healthy",
  "timestamp": "2023-12-01T10:00:00Z",
  "checks": {
    "database_connectivity": {
      "name": "database_connectivity",
      "status": "Healthy",
      "message": "Database responsive in 2ms"
    }
  },
  "metrics": {
    "uptime_seconds": "3600.00",
    "graphql_query_total": "150",
    "storage_read_success": "45"
  }
}
```

## Monitoring & Observability

### Prometheus Metrics

The service exposes metrics at `/metrics` endpoint:

- **Counter metrics:** Request counts, operation totals
- **Gauge metrics:** Current values, system state
- **Histogram metrics:** Response times, data sizes
- **Timer metrics:** Operation durations

### Grafana Dashboards

Pre-configured dashboards available at http://localhost:3000:

- **Financial Data Service Overview**
- **GraphQL Performance**
- **Storage Operations**
- **System Health**

### Logging

Structured JSON logging with the following levels:
- `ERROR`: System errors, failures
- `WARN`: Degraded performance, warnings
- `INFO`: Normal operations, startup
- `DEBUG`: Detailed debugging information
- `TRACE`: Verbose tracing

## Performance Characteristics

### Arrow Flight Benefits

- **Zero-copy data transfer:** No memory copying between processes
- **Sub-millisecond latency:** Direct memory access
- **High throughput:** Optimized for large datasets
- **Columnar storage:** Efficient compression and querying

### Expected Performance

- **Query latency:** < 10ms for cached data, < 100ms for cold data
- **Throughput:** 10,000+ queries/second
- **Storage efficiency:** 80-90% compression with Parquet
- **Memory usage:** 256MB base + 1MB per 1000 series

## Storage Management

### Parquet Files

- **Series metadata:** One file per series
- **Data points:** Batched by series and time range
- **Compression:** Snappy compression by default
- **Schema evolution:** Supported via Arrow schemas

### Data Lifecycle

1. **Hot data:** Recent data in memory-mapped files
2. **Warm data:** Frequently accessed data in SSD storage
3. **Cold data:** Historical data in compressed storage
4. **Archive data:** Long-term storage in object storage

## Security

### Network Security

- **Internal communication:** Service-to-service via private networks
- **External access:** HTTPS with TLS termination
- **Authentication:** JWT tokens (future implementation)

### Data Security

- **Encryption at rest:** Parquet files encrypted with AES-256
- **Encryption in transit:** TLS 1.3 for all communications
- **Access control:** Role-based access control (RBAC)

## Troubleshooting

### Common Issues

1. **Service won't start:**
   ```bash
   # Check logs
   docker logs financial-data-service
   
   # Check port availability
   netstat -tlnp | grep 3001
   ```

2. **High memory usage:**
   ```bash
   # Check data directory size
   du -sh /app/data
   
   # Monitor memory usage
   docker stats financial-data-service
   ```

3. **Slow queries:**
   ```bash
   # Check metrics
   curl http://localhost:3001/metrics
   
   # Check storage health
   curl http://localhost:3001/health
   ```

### Performance Tuning

1. **Increase memory limits:**
   ```yaml
   resources:
     limits:
       memory: "1Gi"
       cpu: "1000m"
   ```

2. **Optimize Parquet settings:**
   ```rust
   // In storage configuration
   compression: Compression::SNAPPY,
   row_group_size: 10000,
   page_size: 1024 * 1024,
   ```

3. **Enable caching:**
   ```rust
   // Add Redis caching layer
   cache_ttl: Duration::from_secs(3600),
   max_cache_size: 1000,
   ```

## Backup & Recovery

### Data Backup

```bash
# Backup Parquet files
tar -czf financial-data-backup-$(date +%Y%m%d).tar.gz /app/data

# Backup to object storage
aws s3 sync /app/data s3://econ-graph-backups/financial-data/
```

### Disaster Recovery

1. **Restore from backup:**
   ```bash
   # Stop service
   docker stop financial-data-service
   
   # Restore data
   tar -xzf financial-data-backup-20231201.tar.gz -C /app/
   
   # Start service
   docker start financial-data-service
   ```

2. **Verify data integrity:**
   ```bash
   # Check health endpoint
   curl http://localhost:3001/health
   
   # Run data validation
   curl -X POST http://localhost:3001/graphql \
     -H "Content-Type: application/json" \
     -d '{"query": "{ health }"}'
   ```

## Scaling

### Horizontal Scaling

- **Stateless design:** No shared state between instances
- **Load balancing:** Round-robin or least-connections
- **Data partitioning:** By series ID or time range

### Vertical Scaling

- **Memory:** Increase for larger datasets
- **CPU:** More cores for parallel processing
- **Storage:** SSD for better I/O performance

## Maintenance

### Regular Tasks

1. **Health monitoring:** Check `/health` endpoint
2. **Metrics review:** Analyze performance trends
3. **Log rotation:** Prevent disk space issues
4. **Data cleanup:** Remove old temporary files

### Updates

1. **Zero-downtime deployment:**
   ```bash
   # Rolling update
   kubectl rollout restart deployment/financial-data-service
   ```

2. **Database migrations:** Handled automatically via Arrow schema evolution

## Support

### Getting Help

- **Documentation:** See `README.md` for development setup
- **Issues:** Report bugs via GitHub issues
- **Monitoring:** Check Grafana dashboards for system health
- **Logs:** Use structured logging for debugging

### Contact

- **Team:** EconGraph Backend Team
- **Slack:** #econ-graph-backend
- **Email:** backend@econ-graph.com
