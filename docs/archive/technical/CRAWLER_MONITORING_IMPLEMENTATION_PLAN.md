# Crawler Monitoring Implementation Plan

## Overview

This document outlines the comprehensive plan for implementing unified monitoring for all crawler types in the EconGraph system using Prometheus metrics and Grafana dashboards. The system includes multiple crawler types that need consistent monitoring while allowing for crawler-specific metrics.

**Scope**: This implementation focuses on **simple monitoring** - providing visibility into crawler performance, health, and compliance. Advanced administrative features (like crawler configuration, scheduling, etc.) are covered by the separate Crawler Admin UI and are out of scope for this monitoring implementation.

## Current Crawler Types Identified

### 1. Economic Data Crawlers (econ-graph-crawler)
- **FRED** (Federal Reserve Economic Data)
- **BLS** (Bureau of Labor Statistics)
- **Census Bureau**
- **BEA** (Bureau of Economic Analysis)
- **World Bank**
- **IMF** (International Monetary Fund)
- **ECB** (European Central Bank)
- **OECD** (Organisation for Economic Co-operation and Development)
- **Bank of England**
- **WTO** (World Trade Organization)
- **Bank of Japan**
- **Reserve Bank of Australia**
- **Bank of Canada**
- **Swiss National Bank**
- **UN Statistics Division**
- **ILO** (International Labour Organization)
- **FHFA** (Federal Housing Finance Agency)

### 2. SEC EDGAR Crawler (econ-graph-sec-crawler)
- XBRL filing downloads
- Company submissions crawling
- Taxonomy component downloads
- Financial statement parsing

### 3. Queue-based Crawlers (crawl_queue system)
- Background processing of queued items
- Retry mechanisms
- Priority-based processing

## Unified Metrics Schema Design

### Core Metrics (All Crawler Types)
- **Request Metrics**: Total requests, duration, status codes
- **Data Collection**: Items collected, bytes downloaded
- **Error Tracking**: Errors by type, rate limit hits, retries, timeouts
- **Performance**: Active workers, queue status, processing rates
- **Timestamps**: Last successful crawl per source

### Economic Data Crawler Specific
- **Series Discovery**: Economic series discovered per source
- **Data Points**: Economic data points collected per source
- **API Quota**: API quota usage per source
- **Data Freshness**: Hours since last update per source

### SEC EDGAR Crawler Specific
- **Filings**: SEC filings downloaded, XBRL files processed
- **Taxonomy**: Taxonomy components downloaded
- **Processing**: Filing processing duration, XBRL parsing duration
- **Companies**: Companies crawled, filing sizes

### Bandwidth and Performance
- **Bandwidth**: Usage per source (bytes per second)
- **Throughput**: Requests per second per source
- **Response Times**: Percentiles per source
- **Resource Usage**: Memory and CPU usage per crawler type

### Politeness and Compliance
- **Request Delays**: Delay between requests per source
- **Robots.txt**: Compliance checks
- **User Agent**: Usage tracking
- **Concurrency**: Concurrent request limits per source

### Data Quality
- **Validation**: Data validation errors per source
- **Completeness**: Data completeness score per source
- **Accuracy**: Data accuracy score per source
- **Duplicates**: Duplicate data detection per source

## Implementation Plan

### Phase 1: Metrics Infrastructure and Testing (Week 1-2)

#### 1.1 Assess Current Metrics Testing
- [ ] **CRITICAL**: Review existing metrics endpoint tests in `backend/src/metrics.rs`
- [ ] Identify gaps in current metrics testing coverage
- [ ] Ensure basic metrics endpoint functionality is well-tested before modifications
- [ ] Add missing tests for existing metrics if needed
- [ ] Verify metrics endpoint returns valid Prometheus format

#### 1.2 Create Unified Metrics Module
- [ ] Create `backend/src/metrics/crawler_metrics.rs`
- [ ] Implement `CrawlerMetrics` struct with all metric types
- [ ] Add helper functions for recording metrics
- [ ] **Comprehensive test coverage** (see Testing Strategy below)
- [ ] Document metric naming conventions and labels

#### 1.3 Integrate with Existing Metrics System
- [ ] Update `backend/src/metrics.rs` to include crawler metrics
- [ ] Ensure compatibility with existing Prometheus registry
- [ ] Add crawler metrics to main metrics endpoint
- [ ] **Test metrics collection and export** with existing infrastructure

#### 1.4 Instrument Economic Data Crawlers
- [ ] Update `econ-graph-crawler` to use unified metrics
- [ ] Add metrics recording to all series discovery services
- [ ] Instrument FRED, BLS, Census, BEA, World Bank, IMF, ECB, OECD crawlers
- [ ] Add metrics to remaining crawlers (BoE, WTO, BoJ, RBA, BoC, SNB, UN, ILO, FHFA)
- [ ] **Test metrics collection for each crawler type**

#### 1.5 Instrument SEC EDGAR Crawler
- [ ] Update `econ-graph-sec-crawler` to use unified metrics
- [ ] Add metrics to filing downloads, XBRL processing, taxonomy downloads
- [ ] Instrument company crawling and parsing operations
- [ ] **Test SEC-specific metrics collection**

#### 1.6 Instrument Queue-based Crawlers
- [ ] Update `crawl_queue` system to use unified metrics
- [ ] Add metrics to queue processing, retry logic, priority handling
- [ ] Instrument worker management and job scheduling
- [ ] **Test queue metrics collection**

### Phase 2: Grafana Dashboards (Week 3-4)

#### 2.1 Core Crawler Dashboard
- [ ] Create `grafana-dashboards/crawler-overview.json`
- [ ] Include: Active crawlers, request rates, error rates, queue status
- [ ] Add data source dropdown for filtering by source
- [ ] Include time-based trends and basic alerts
- [ ] **Test dashboard functionality**

#### 2.2 Economic Data Crawler Dashboard
- [ ] Create `grafana-dashboards/economic-data-crawlers.json`
- [ ] Include: Series discovery rates, data collection rates, API quota usage
- [ ] Add per-source breakdowns with filtering
- [ ] Include data freshness monitoring
- [ ] Add bandwidth and performance metrics
- [ ] **Test economic data specific visualizations**

#### 2.3 SEC EDGAR Crawler Dashboard
- [ ] Create `grafana-dashboards/sec-edgar-crawler.json`
- [ ] Include: Filing download rates, XBRL processing times, company crawling
- [ ] Add taxonomy component tracking
- [ ] Include filing size distributions
- [ ] Add parsing performance metrics
- [ ] **Test SEC-specific visualizations**

#### 2.4 Politeness and Compliance Dashboard
- [ ] Create `grafana-dashboards/crawler-politeness.json`
- [ ] Include: Request delays, rate limit compliance, robots.txt checks
- [ ] Add user agent usage tracking
- [ ] Include concurrent request monitoring
- [ ] Add politeness policy compliance alerts
- [ ] **Test compliance monitoring**

### Phase 3: Testing and Validation (Week 5-6)

#### 3.1 Core Testing Strategy
- [ ] **Unit Tests**: Test all metric recording functions
- [ ] **Integration Tests**: Test metrics collection across all crawler types
- [ ] **End-to-End Tests**: Test complete metrics pipeline from crawler to Prometheus
- [ ] **Performance Tests**: Verify metrics collection doesn't impact crawler performance

#### 3.2 Documentation
- [ ] Create comprehensive monitoring documentation
- [ ] Document metric definitions and usage
- [ ] Create dashboard usage guides
- [ ] Document troubleshooting procedures

#### 3.3 Basic Alerting
- [ ] Create basic Prometheus alerting rules for critical metrics
- [ ] Add alerts for: High error rates, rate limit violations, queue backlogs
- [ ] Test alerting functionality
- [ ] Document alerting procedures

#### 3.4 Future Enhancements (Out of Scope)
- [ ] **Dashboard Testing**: Will be addressed in future phases
- [ ] **Performance Testing**: Will be addressed in future phases
- [ ] **Load Testing**: Will be addressed in future phases

## Testing Strategy

### Critical Testing Requirements

#### 1. Metrics Endpoint Testing (PRIORITY 1)
**Current Status**: Unknown - needs assessment
- [ ] **CRITICAL**: Review existing tests in `backend/src/metrics.rs`
- [ ] Verify metrics endpoint returns valid Prometheus format
- [ ] Test metrics endpoint under load
- [ ] Ensure metrics endpoint doesn't impact application performance
- [ ] Test metrics endpoint error handling

#### 2. Unit Testing Strategy
- [ ] **Metric Recording Functions**: Test all `record_*` functions
- [ ] **Metric Initialization**: Test `CrawlerMetrics::new()` with various configurations
- [ ] **Label Validation**: Test metric labels are properly formatted
- [ ] **Error Handling**: Test metric recording with invalid inputs
- [ ] **Concurrency**: Test metric recording from multiple threads

#### 3. Integration Testing Strategy
- [ ] **Crawler Integration**: Test metrics collection in actual crawler operations
- [ ] **Database Integration**: Test metrics with database operations
- [ ] **HTTP Integration**: Test metrics with HTTP requests
- [ ] **Queue Integration**: Test metrics with queue processing
- [ ] **Registry Integration**: Test metrics registration and collection

#### 4. End-to-End Testing Strategy
- [ ] **Metrics Pipeline**: Test complete flow from crawler → metrics → Prometheus
- [ ] **Data Accuracy**: Verify metrics match actual crawler behavior
- [ ] **Performance Impact**: Measure overhead of metrics collection

#### 5. Future Testing (Out of Scope)
- [ ] **Dashboard Testing**: Will be addressed in future phases
- [ ] **Performance Testing**: Will be addressed in future phases
- [ ] **Load Testing**: Will be addressed in future phases

### Test Implementation Plan

#### Phase 1: Foundation Testing
```rust
// Example test structure for metrics endpoint
#[tokio::test]
async fn test_metrics_endpoint_basic() {
    // Test basic metrics endpoint functionality
    // Verify Prometheus format
    // Test error handling
}

#[tokio::test]
async fn test_metrics_endpoint_performance() {
    // Test metrics endpoint under load
    // Measure response times
    // Test concurrent access
}
```

#### Phase 2: Crawler Metrics Testing
```rust
// Example test structure for crawler metrics
#[tokio::test]
async fn test_crawler_metrics_recording() {
    // Test all metric recording functions
    // Verify metric values
    // Test label combinations
}

#[tokio::test]
async fn test_crawler_metrics_integration() {
    // Test metrics with actual crawler operations
    // Verify metrics match crawler behavior
    // Test error scenarios
}
```

#### Phase 3: Dashboard Testing
```bash
# Example dashboard testing approach
# 1. Deploy test environment with sample data
# 2. Verify all dashboard panels load correctly
# 3. Test filtering and time range selection
# 4. Validate metric calculations
# 5. Test alerting functionality
```

### Testing Tools and Infrastructure

#### Required Testing Tools
- [ ] **Prometheus Test Server**: For testing metrics collection
- [ ] **Grafana Test Instance**: For testing dashboards
- [ ] **Load Testing Tools**: For performance testing
- [ ] **Mock Crawlers**: For controlled testing scenarios
- [ ] **Test Data Generators**: For realistic test data

#### Test Data Requirements
- [ ] **Sample Crawler Data**: Realistic crawler operation data
- [ ] **Error Scenarios**: Various error conditions to test
- [ ] **Load Scenarios**: High-volume data for performance testing
- [ ] **Edge Cases**: Boundary conditions and unusual scenarios
- [ ] **Historical Data**: Time-series data for dashboard testing

### Success Criteria for Testing

#### Functional Testing
- [ ] All metrics are collected correctly
- [ ] Dashboards display accurate data
- [ ] Alerting works as expected
- [ ] Error handling is robust
- [ ] Performance impact is minimal

#### Quality Assurance
- [ ] Test coverage >90% for metrics code
- [ ] All critical paths are tested
- [ ] Edge cases are covered
- [ ] Performance benchmarks are met
- [ ] Documentation is complete

## Technical Implementation Details

### Metric Naming Conventions
- **Prefix**: `econgraph_crawler_` for all crawler metrics
- **Labels**: Consistent use of `crawler_type`, `source`, `endpoint` labels
- **Types**: Use appropriate Prometheus metric types (Counter, Gauge, Histogram)
- **Buckets**: Define appropriate histogram buckets for each metric type

### Metrics Exposure Endpoint
- **Single Endpoint**: All metrics (app + crawler) are exposed via the existing `/metrics` endpoint using the same Prometheus registry
- **Rationale**: Simplifies scraping and dashboard configuration, avoids port proliferation, and aligns with Prometheus best practices
- **Compatibility**: Works with existing Prometheus scrape configs and Loki/Grafana stack without changes

### Dashboard Design Principles
- **Consistency**: Use consistent color schemes and layouts across dashboards
- **Filtering**: Include data source dropdowns and time range selectors
- **Alerting**: Integrate with Prometheus alerting for critical thresholds
- **Responsiveness**: Design for different screen sizes and devices
- **Accessibility**: Ensure dashboards are accessible to all team members

### Integration Points
- **Prometheus**: All metrics exposed via `/metrics` endpoint
- **Grafana**: Dashboards stored as JSON files in version control
- **Loki**: Log integration for comprehensive observability
- **AlertManager**: Alert routing and notification management
- **Kubernetes**: ServiceMonitor resources for automatic discovery

## Success Criteria

### Functional Requirements
- [ ] All crawler types have comprehensive metrics collection
- [ ] Dashboards provide real-time visibility into crawler performance
- [ ] Basic alerting system provides timely notifications of critical issues
- [ ] System is maintainable and extensible
- [ ] **Simple monitoring focus** - no complex administrative features

### Performance Requirements
- [ ] Metrics collection adds <5% overhead to crawler performance
- [ ] System can handle 1000+ metrics per second
- [ ] Storage requirements are reasonable (<1GB/day)
- [ ] **Future**: Dashboard performance and alerting latency will be addressed in future phases

### Operational Requirements
- [ ] Operations team can effectively monitor crawler health
- [ ] Issues are detected and resolved quickly
- [ ] System provides actionable insights for optimization
- [ ] Compliance requirements are met
- [ ] Documentation is comprehensive and up-to-date

## Risk Mitigation

### Technical Risks
- **Metric Cardinality**: Use appropriate label combinations to avoid high cardinality
- **Performance Impact**: Monitor metrics collection overhead and optimize as needed
- **Storage Growth**: Implement retention policies and data archiving
- **Integration Complexity**: Use incremental integration approach with thorough testing

### Operational Risks
- **Alert Fatigue**: Implement intelligent alerting with proper thresholds
- **Dashboard Complexity**: Keep dashboards focused and intuitive
- **Maintenance Overhead**: Automate dashboard updates and metric management
- **Team Adoption**: Provide comprehensive training and documentation

## Future Enhancements (Out of Scope)

**Note**: The following features are covered by the separate Crawler Admin UI and are out of scope for this monitoring implementation:

### Advanced Analytics
- [ ] Machine learning-based anomaly detection
- [ ] Predictive capacity planning
- [ ] Automated optimization recommendations
- [ ] Advanced correlation analysis

### Integration Improvements
- [ ] Integration with external monitoring systems
- [ ] API for programmatic access to metrics
- [ ] Custom dashboard creation tools
- [ ] Mobile monitoring applications

### Compliance Enhancements
- [ ] Automated compliance reporting
- [ ] Regulatory requirement tracking
- [ ] Audit trail generation
- [ ] Policy enforcement automation

### Administrative Features (Crawler Admin UI)
- [ ] Crawler configuration management
- [ ] Scheduling and job management
- [ ] User management and permissions
- [ ] Advanced reporting and analytics
- [ ] System administration tools

## Conclusion

This implementation plan provides a comprehensive approach to monitoring all crawler types in the EconGraph system. The unified metrics schema ensures consistency while allowing for crawler-specific monitoring needs. The phased approach minimizes risk while delivering value incrementally.

The plan emphasizes both technical excellence and operational effectiveness, ensuring that the monitoring system provides real value to the operations team while maintaining the high standards expected for a production system.

## Review Checklist

Before proceeding with implementation, please review:

- [ ] **Scope**: Are all crawler types covered appropriately?
- [ ] **Metrics**: Are the proposed metrics sufficient for monitoring needs?
- [ ] **Dashboards**: Do the dashboard designs meet operational requirements?
- [ ] **Timeline**: Is the 8-week timeline realistic for the team?
- [ ] **Resources**: Are the required resources available?
- [ ] **Dependencies**: Are there any missing dependencies or prerequisites?
- [ ] **Risks**: Are the identified risks and mitigation strategies adequate?
- [ ] **Success Criteria**: Are the success criteria measurable and achievable?

---

*This document should be reviewed and approved before beginning implementation. Any changes or concerns should be addressed before proceeding to the implementation phase.*
