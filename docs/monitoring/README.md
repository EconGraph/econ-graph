# EconGraph Grafana Dashboards

This directory contains comprehensive Grafana dashboards for monitoring the EconGraph platform. These dashboards provide real-time visibility into system performance, database health, and crawler operations.

## Dashboard Overview

### 1. EconGraph Platform Overview (`econgraph-overview.json`)

**Purpose**: High-level system monitoring and health overview  
**Refresh Rate**: 30 seconds  
**Time Range**: Last 1 hour

**Key Metrics**:
- System uptime and service availability
- API request rates and response times
- Resource utilization (CPU, memory, network)
- Database connection pool status
- Error rates and performance percentiles

**Panels**:
- **System Status**: Overall platform health indicator
- **API Request Rate**: HTTP requests per second with status code breakdown
- **Response Time Percentiles**: 50th, 95th, and 99th percentile latencies
- **Active Pods**: Running pod counts for each service
- **Database Connections**: Active vs. maximum connections
- **Memory Usage**: Memory consumption by service
- **CPU Usage**: CPU utilization across all services
- **Network I/O**: Inbound and outbound traffic rates

**Use Cases**:
- Operations team monitoring
- Performance troubleshooting
- Capacity planning
- SLA compliance tracking

### 2. Database Statistics (`database-statistics.json`)

**Purpose**: Comprehensive PostgreSQL monitoring for time series data  
**Refresh Rate**: 1 minute  
**Time Range**: Last 6 hours

**Key Metrics**:
- Database size and growth trends
- Table-level statistics and sizes
- Query performance and optimization
- Connection pool health
- I/O and WAL statistics

**Panels**:
- **Database Size**: Total database size with growth indicators
- **Total Records**: Count of records across all tables
- **Connection Pool Status**: Active connections vs. limits
- **Database Performance**: Cache hit ratio and efficiency metrics
- **Table Sizes**: Detailed breakdown of table sizes and row counts
- **Query Performance**: Mean and maximum query execution times
- **Database Activity**: Transaction rates (commits, rollbacks, DML operations)
- **I/O Statistics**: Disk reads vs. cache hits
- **Lock Statistics**: Database lock analysis by type
- **WAL Statistics**: Write-Ahead Log performance
- **Time Series Data Growth**: Growth trends for economic data tables

**Use Cases**:
- Database performance optimization
- Storage capacity planning
- Query optimization identification
- Data growth analysis
- Backup and maintenance scheduling

### 3. Crawler Status (`crawler-status.json`)

**Purpose**: Data crawler monitoring and queue processing analysis  
**Refresh Rate**: 15 seconds  
**Time Range**: Last 2 hours

**Key Metrics**:
- Crawler instance health and activity
- Queue processing rates and backlogs
- Data source crawl status and freshness
- Error rates and retry patterns
- API rate limiting compliance

**Panels**:
- **Active Crawler Instances**: Number of running crawler workers
- **Queue Items Pending**: Backlog of items waiting for processing
- **Processing Rate**: Items processed per second
- **Error Rate**: Percentage of failed processing attempts
- **Queue Processing Rate**: Processing throughput by data source
- **Queue Status Distribution**: Pie chart of queue item states
- **Data Source Crawl Status**: Table showing last crawl times and success rates
- **Crawler Worker Health**: CPU usage and health of individual workers
- **Queue Processing Time Distribution**: Heatmap of processing durations
- **API Rate Limiting Status**: Request rates vs. API limits
- **Data Freshness by Source**: Time since last successful crawl
- **Error Analysis by Source**: Error breakdown by source and type
- **Queue Backlog Trend**: Historical view of queue backlogs

**Use Cases**:
- Crawler performance monitoring
- Data freshness tracking
- Queue backlog management
- Error analysis and debugging
- API rate limit compliance
- Data source health monitoring

## Installation and Setup

### Automatic Installation (via Terraform)

The dashboards are automatically installed when deploying with Terraform:

```bash
cd terraform
terraform apply
```

The dashboards will be available at: `https://grafana.yourdomain.com`

### Manual Installation

1. **Access Grafana**:
   ```
   Username: admin
   Password: (see terraform output for generated password)
   ```

2. **Import Dashboards**:
   - Go to "+" → "Import" in Grafana
   - Upload each JSON file or paste the JSON content
   - Select "Prometheus" as the data source
   - Click "Import"

3. **Verify Data Sources**:
   - Ensure Prometheus is configured and accessible
   - Check that metrics are being collected from all services

## Metrics and Data Sources

### Required Prometheus Metrics

The dashboards expect the following metrics to be available:

#### Application Metrics
- `up{job="econgraph-backend"}` - Service availability
- `http_requests_total` - HTTP request counts
- `http_request_duration_seconds` - Request latencies
- `graphql_request_duration_seconds` - GraphQL-specific latencies

#### Crawler Metrics
- `econgraph_queue_items` - Queue item counts by status
- `econgraph_queue_items_processed_total` - Processing counters
- `econgraph_queue_items_failed_total` - Failure counters
- `econgraph_last_crawl_timestamp` - Last crawl timestamps
- `econgraph_crawl_errors_total` - Error counters
- `econgraph_api_requests_total` - API request tracking

#### Database Metrics (via postgres_exporter)
- `pg_database_size_bytes` - Database size
- `pg_stat_activity_count` - Connection counts
- `pg_stat_database_*` - Database statistics
- `pg_stat_user_tables_*` - Table statistics
- `pg_locks_count` - Lock information

#### Kubernetes Metrics
- `container_memory_usage_bytes` - Memory usage
- `container_cpu_usage_seconds_total` - CPU usage
- `container_network_*` - Network statistics

### Metric Labels

Ensure your metrics include appropriate labels:
- `job` - Service identifier (econgraph-backend, econgraph-crawler)
- `source` - Data source (fred, bls, census, worldbank)
- `status` - Queue item status (pending, processing, completed, failed)
- `pod` - Kubernetes pod name
- `error_type` - Error classification

## Alerting Rules

The dashboards work in conjunction with Prometheus alerting rules. Key alerts include:

### Critical Alerts
- **EconGraphBackendDown**: Backend service unavailable
- **EconGraphDatabaseDown**: Database connection failures
- **EconGraphQueueStuck**: Queue processing stopped

### Warning Alerts
- **EconGraphHighResponseTime**: API latency above threshold
- **EconGraphQueueBacklog**: Large queue backlog
- **EconGraphHighErrorRate**: Elevated error rates
- **EconGraphDataStale**: Data sources not updating

### Configuration

Alerts are configured in the Terraform monitoring module:
```hcl
# terraform/modules/monitoring/main.tf
resource "kubernetes_manifest" "econgraph_alerts" {
  # ... alert rules configuration
}
```

## Customization

### Adding Custom Panels

1. **Edit Dashboard JSON**:
   - Add new panel definition to the `panels` array
   - Increment the `id` field
   - Set appropriate `gridPos` for layout

2. **Example Custom Panel**:
   ```json
   {
     "id": 99,
     "title": "Custom Metric",
     "type": "graph",
     "gridPos": { "h": 8, "w": 12, "x": 0, "y": 0 },
     "targets": [
       {
         "expr": "your_custom_metric",
         "legendFormat": "Custom Legend",
         "refId": "A"
       }
     ]
   }
   ```

### Modifying Time Ranges

Update the dashboard `time` configuration:
```json
{
  "time": {
    "from": "now-6h",  // Start time
    "to": "now"        // End time
  },
  "refresh": "30s"     // Auto-refresh interval
}
```

### Adding Variables/Templates

Add template variables for filtering:
```json
{
  "templating": {
    "list": [
      {
        "name": "environment",
        "type": "query",
        "query": "label_values(up, environment)",
        "multi": true,
        "includeAll": true
      }
    ]
  }
}
```

## Troubleshooting

### Common Issues

1. **No Data Displayed**:
   - Check Prometheus data source configuration
   - Verify metrics are being scraped
   - Check metric names and labels

2. **Dashboard Import Errors**:
   - Validate JSON syntax
   - Ensure Grafana version compatibility
   - Check data source names

3. **Performance Issues**:
   - Reduce query frequency for heavy panels
   - Optimize Prometheus queries
   - Consider shorter time ranges

### Debugging Queries

1. **Test in Prometheus**:
   ```
   # Access Prometheus directly
   kubectl port-forward svc/prometheus-server 9090:80
   # Open http://localhost:9090
   ```

2. **Query Examples**:
   ```promql
   # Test basic connectivity
   up{job="econgraph-backend"}
   
   # Test rate calculations
   rate(http_requests_total[5m])
   
   # Test aggregations
   sum by (status) (econgraph_queue_items)
   ```

3. **Check Metric Labels**:
   ```promql
   # List all labels for a metric
   group by (__name__)({__name__=~"econgraph_.*"})
   ```

## Maintenance

### Regular Tasks

1. **Dashboard Updates**:
   - Review and update panels quarterly
   - Add new metrics as services evolve
   - Remove obsolete metrics and panels

2. **Performance Optimization**:
   - Monitor query performance in Grafana
   - Optimize slow-running queries
   - Adjust refresh rates based on usage

3. **Alert Tuning**:
   - Review alert thresholds monthly
   - Adjust based on operational experience
   - Add new alerts for emerging issues

### Backup and Recovery

1. **Export Dashboards**:
   ```bash
   # Export all dashboards
   curl -H "Authorization: Bearer $GRAFANA_API_KEY" \
        "$GRAFANA_URL/api/search?type=dash-db" | \
        jq -r '.[].uri' | \
        xargs -I {} curl -H "Authorization: Bearer $GRAFANA_API_KEY" \
        "$GRAFANA_URL/api/dashboards/{}" > dashboards-backup.json
   ```

2. **Version Control**:
   - Keep dashboard JSON files in version control
   - Track changes and maintain history
   - Use infrastructure as code for consistency

## Best Practices

### Dashboard Design
- **Logical Grouping**: Group related metrics together
- **Consistent Colors**: Use consistent color schemes across dashboards
- **Meaningful Names**: Use descriptive panel titles and legends
- **Appropriate Units**: Set correct units for all metrics
- **Time Alignment**: Use consistent time ranges across related panels

### Query Optimization
- **Rate Functions**: Use `rate()` for counter metrics
- **Aggregation**: Aggregate before calculating percentiles
- **Label Selection**: Be specific with label selectors
- **Recording Rules**: Use recording rules for complex calculations

### Monitoring Strategy
- **Layered Approach**: Start with overview, drill down to details
- **Proactive Monitoring**: Set up alerts before issues occur
- **Regular Reviews**: Review dashboards and metrics regularly
- **Documentation**: Keep dashboard documentation up to date

---

These dashboards provide comprehensive visibility into the EconGraph platform and support effective operations and troubleshooting. Regular review and updates ensure they remain valuable as the system evolves.
