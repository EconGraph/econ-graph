use anyhow::Result;
use econ_graph_financial_data::monitoring::{
    HealthCheck, HealthChecker, HealthStatus, MetricsCollector,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[tokio::test]
async fn test_metrics_collection() -> Result<()> {
    let metrics = MetricsCollector::new();

    // Test counter metrics
    metrics.increment_counter("test_counter", 5).await;
    metrics.increment_counter("test_counter", 3).await;

    // Test gauge metrics
    metrics.set_gauge("test_gauge", 42.5).await;

    // Test histogram metrics
    metrics.record_histogram("test_histogram", 1.0).await;
    metrics.record_histogram("test_histogram", 2.0).await;
    metrics.record_histogram("test_histogram", 3.0).await;

    // Test timing metrics
    metrics
        .record_timing("test_timer", Duration::from_millis(100))
        .await;

    // Test GraphQL operation metrics
    metrics
        .record_graphql_operation("query", Duration::from_millis(50), true)
        .await;
    metrics
        .record_graphql_operation("mutation", Duration::from_millis(75), false)
        .await;

    // Test storage operation metrics
    metrics
        .record_storage_operation("read", Duration::from_millis(25), true)
        .await;
    metrics
        .record_storage_operation("write", Duration::from_millis(30), true)
        .await;

    // Get metrics summary
    let summary = metrics.get_metrics_summary().await;

    // Verify counter
    assert_eq!(summary.get("test_counter"), Some(&"8".to_string()));

    // Verify gauge
    assert_eq!(summary.get("test_gauge"), Some(&"42.50".to_string()));

    // Verify histogram
    let histogram = summary.get("test_histogram");
    assert!(histogram.is_some());
    let histogram_str = histogram.unwrap();
    assert!(histogram_str.contains("count=3"));
    assert!(histogram_str.contains("avg=2.00"));
    assert!(histogram_str.contains("min=1.00"));
    assert!(histogram_str.contains("max=3.00"));

    // Verify timer
    assert_eq!(summary.get("test_timer"), Some(&"100ms".to_string()));

    // Verify GraphQL metrics
    assert_eq!(summary.get("graphql_query_total"), Some(&"1".to_string()));
    assert_eq!(summary.get("graphql_query_success"), Some(&"1".to_string()));
    assert_eq!(
        summary.get("graphql_mutation_total"),
        Some(&"1".to_string())
    );
    assert_eq!(
        summary.get("graphql_mutation_errors"),
        Some(&"1".to_string())
    );

    // Verify storage metrics
    assert_eq!(summary.get("storage_read_total"), Some(&"1".to_string()));
    assert_eq!(summary.get("storage_read_success"), Some(&"1".to_string()));
    assert_eq!(summary.get("storage_write_total"), Some(&"1".to_string()));
    assert_eq!(summary.get("storage_write_success"), Some(&"1".to_string()));

    // Verify uptime is present
    assert!(summary.contains_key("uptime_seconds"));

    println!("✅ Metrics collection test passed");
    println!(
        "   - Counter metrics: {}",
        summary.get("test_counter").unwrap()
    );
    println!("   - Gauge metrics: {}", summary.get("test_gauge").unwrap());
    println!(
        "   - Histogram metrics: {}",
        summary.get("test_histogram").unwrap()
    );
    println!("   - Timer metrics: {}", summary.get("test_timer").unwrap());
    println!(
        "   - GraphQL metrics: {} queries, {} mutations",
        summary.get("graphql_query_total").unwrap(),
        summary.get("graphql_mutation_total").unwrap()
    );
    println!(
        "   - Storage metrics: {} reads, {} writes",
        summary.get("storage_read_total").unwrap(),
        summary.get("storage_write_total").unwrap()
    );

    Ok(())
}

#[tokio::test]
async fn test_health_checking() -> Result<()> {
    let health_checker = HealthChecker::new();

    // Register some health checks
    let check1 = HealthCheck {
        name: "database_connectivity".to_string(),
        status: HealthStatus::Healthy,
        last_check: Instant::now(),
        message: Some("Database is responsive".to_string()),
    };

    let check2 = HealthCheck {
        name: "storage_health".to_string(),
        status: HealthStatus::Degraded,
        last_check: Instant::now(),
        message: Some("Storage is slow but functional".to_string()),
    };

    health_checker
        .register_check("database_connectivity".to_string(), check1)
        .await;
    health_checker
        .register_check("storage_health".to_string(), check2)
        .await;

    // Test overall health (should be degraded due to storage)
    let overall_health = health_checker.get_overall_health().await;
    assert_eq!(overall_health, HealthStatus::Degraded);

    // Test health report
    let health_report = health_checker.get_health_report().await;
    assert_eq!(health_report.len(), 2);
    assert!(health_report.contains_key("database_connectivity"));
    assert!(health_report.contains_key("storage_health"));

    // Update a health check
    health_checker
        .update_check(
            "storage_health",
            HealthStatus::Healthy,
            Some("Storage is now fully operational".to_string()),
        )
        .await;

    // Test overall health (should now be healthy)
    let overall_health_after_update = health_checker.get_overall_health().await;
    assert_eq!(overall_health_after_update, HealthStatus::Healthy);

    // Test unhealthy scenario
    health_checker
        .update_check(
            "database_connectivity",
            HealthStatus::Unhealthy,
            Some("Database connection failed".to_string()),
        )
        .await;

    let overall_health_unhealthy = health_checker.get_overall_health().await;
    assert_eq!(overall_health_unhealthy, HealthStatus::Unhealthy);

    println!("✅ Health checking test passed");
    println!("   - Initial health: {:?}", overall_health);
    println!("   - After update: {:?}", overall_health_after_update);
    println!("   - After failure: {:?}", overall_health_unhealthy);
    println!("   - Health checks registered: {}", health_report.len());

    Ok(())
}

#[tokio::test]
async fn test_metrics_performance() -> Result<()> {
    let metrics = MetricsCollector::new();
    let start = Instant::now();

    // Record many metrics quickly
    for i in 0..1000 {
        metrics.increment_counter("performance_test", 1).await;
        metrics.set_gauge("performance_gauge", i as f64).await;
        metrics
            .record_histogram("performance_histogram", i as f64)
            .await;
    }

    let duration = start.elapsed();
    let summary = metrics.get_metrics_summary().await;

    // Verify metrics were recorded
    assert_eq!(summary.get("performance_test"), Some(&"1000".to_string()));
    assert_eq!(
        summary.get("performance_gauge"),
        Some(&"999.00".to_string())
    );

    let histogram = summary.get("performance_histogram").unwrap();
    assert!(histogram.contains("count=1000"));

    println!("✅ Metrics performance test passed");
    println!("   - Recorded 1000 metrics in {:?}", duration);
    println!("   - Counter: {}", summary.get("performance_test").unwrap());
    println!("   - Gauge: {}", summary.get("performance_gauge").unwrap());
    println!("   - Histogram: {}", histogram);

    Ok(())
}

#[tokio::test]
async fn test_health_check_edge_cases() -> Result<()> {
    let health_checker = HealthChecker::new();

    // Test with no health checks
    let overall_health_empty = health_checker.get_overall_health().await;
    assert_eq!(overall_health_empty, HealthStatus::Healthy);

    // Test updating non-existent check
    health_checker
        .update_check(
            "non_existent",
            HealthStatus::Unhealthy,
            Some("This check doesn't exist".to_string()),
        )
        .await;

    // Should still be healthy since the check doesn't exist
    let overall_health_after_nonexistent = health_checker.get_overall_health().await;
    assert_eq!(overall_health_after_nonexistent, HealthStatus::Healthy);

    // Test with all healthy checks
    let healthy_check = HealthCheck {
        name: "healthy_check".to_string(),
        status: HealthStatus::Healthy,
        last_check: Instant::now(),
        message: Some("All good".to_string()),
    };

    health_checker
        .register_check("healthy_check".to_string(), healthy_check)
        .await;

    let overall_health_all_healthy = health_checker.get_overall_health().await;
    assert_eq!(overall_health_all_healthy, HealthStatus::Healthy);

    println!("✅ Health check edge cases test passed");
    println!("   - Empty health checker: {:?}", overall_health_empty);
    println!(
        "   - After non-existent update: {:?}",
        overall_health_after_nonexistent
    );
    println!("   - All healthy: {:?}", overall_health_all_healthy);

    Ok(())
}
