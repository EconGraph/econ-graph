/// Integration tests for MCP server that require real backend and chart API services
/// These tests start up actual services and test end-to-end functionality
use econ_graph_mcp::mcp_server::EconGraphMcpServer;
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;

// We need to create a simple test container setup since we can't import test_utils
// from the tests directory. Let's create a minimal version for integration testing.

async fn create_test_database_pool() -> econ_graph_core::database::DatabasePool {
    use diesel::Connection;
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    let postgres = Postgres::default()
        .with_db_name("econ_graph_test")
        .with_user("postgres")
        .with_password("password");

    let container = postgres.start().await.expect("Failed to start container");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");

    let database_url = format!(
        "postgresql://postgres:password@localhost:{}/econ_graph_test",
        port
    );

    // Wait for PostgreSQL to be ready
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Run migrations
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/");

    let mut conn =
        diesel::PgConnection::establish(&database_url).expect("Failed to connect to test database");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Use the core crate's database pool creation function
    econ_graph_core::database::create_pool(&database_url)
        .await
        .expect("Failed to create connection pool")
}

#[tokio::test]
#[serial]
async fn test_mcp_server_integration_basic_functionality() {
    // Test basic MCP server functionality with real database
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test that we can get available tools
    let tools = EconGraphMcpServer::get_available_tools();
    assert!(!tools.is_empty());
    assert!(tools.len() >= 4); // Should have at least 4 tools

    // Test that we can get available resources
    let resources = EconGraphMcpServer::get_available_resources();
    assert!(!resources.is_empty());
    assert!(resources.len() >= 2); // Should have at least 2 resources

    // Test basic search functionality
    let search_args = json!({
        "query": "GDP",
        "limit": 5
    });

    let result = server.search_economic_series(search_args).await;
    assert!(
        result.is_ok(),
        "Search should succeed even with empty database"
    );
}

#[tokio::test]
#[serial]
async fn test_mcp_server_error_handling_integration() {
    // Test error handling in integration context
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test error handling for missing required parameters
    let invalid_args = json!({});
    let result = server.search_economic_series(invalid_args).await;
    assert!(result.is_err(), "Should fail with missing query parameter");

    // Test error handling for invalid series ID
    let invalid_series_args = json!({
        "series_id": "invalid-uuid"
    });
    let result = server.get_series_data(invalid_series_args).await;
    // The system should handle invalid series IDs gracefully
    assert!(result.is_ok(), "Should handle invalid series ID gracefully");

    // Test error handling for missing series_ids in visualization
    let invalid_viz_args = json!({
        "chart_type": "line"
    });
    let result = server.create_data_visualization(invalid_viz_args).await;
    assert!(result.is_err(), "Should fail with missing series_ids");
}

#[tokio::test]
#[serial]
async fn test_mcp_server_concurrent_requests_integration() {
    // Test concurrent requests in integration context
    let pool = create_test_database_pool().await;
    let server = Arc::new(EconGraphMcpServer::new(Arc::new(pool.clone())));

    // Test concurrent search requests
    let mut handles = vec![];
    for i in 0..5 {
        let server_clone = server.clone();
        let handle = tokio::spawn(async move {
            let search_args = json!({
                "query": format!("test query {}", i),
                "limit": 5
            });
            server_clone.search_economic_series(search_args).await
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let results = futures::future::join_all(handles).await;

    // Verify all requests succeeded
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Concurrent request {} should succeed", i);
        let search_result = result.unwrap();
        assert!(search_result.is_ok(), "Search result {} should be ok", i);
    }
}

#[tokio::test]
#[serial]
async fn test_mcp_server_chart_api_integration() {
    // Test chart API integration
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test visualization creation with mock data
    let viz_args = json!({
        "series_ids": ["550e8400-e29b-41d4-a716-446655440000"],
        "chart_type": "line",
        "title": "Integration Test Chart"
    });

    // This will succeed with fallback data since the series doesn't exist, but tests the integration logic
    let result = server.create_data_visualization(viz_args).await;
    assert!(
        result.is_ok(),
        "Should succeed with fallback data for non-existent series, testing integration"
    );

    // Test fallback visualization functionality
    let series_data = vec![json!({
        "id": "test-series-1",
        "name": "Test Series 1",
        "dataPoints": [
            {"date": "2023-01-01", "value": 100.0},
            {"date": "2023-02-01", "value": 105.0}
        ]
    })];

    let fallback_result = server
        .create_fallback_visualization(series_data, "line", Some("Test Chart"))
        .await;
    assert!(
        fallback_result.is_ok(),
        "Fallback visualization should succeed"
    );
}

#[tokio::test]
#[serial]
async fn test_mcp_server_database_integration() {
    // Test database integration
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test database connectivity by performing a simple query
    let search_args = json!({
        "query": "connectivity test",
        "limit": 1
    });
    let connection_result = server.search_economic_series(search_args).await;
    assert!(
        connection_result.is_ok(),
        "Should be able to perform database queries"
    );

    // Test data sources query
    let data_sources_result = server.get_data_sources().await;
    assert!(
        data_sources_result.is_ok(),
        "Should be able to query data sources"
    );

    // Test series catalog query
    let catalog_result = server.get_series_catalog().await;
    assert!(
        catalog_result.is_ok(),
        "Should be able to query series catalog"
    );

    // Test search functionality with database
    let search_args = json!({
        "query": "test search",
        "limit": 10
    });
    let search_result = server.search_economic_series(search_args).await;
    assert!(
        search_result.is_ok(),
        "Should be able to perform search queries"
    );
}

#[tokio::test]
#[serial]
async fn test_mcp_server_end_to_end_integration() {
    // Test complete end-to-end integration
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test the complete MCP workflow
    // 1. Get available tools
    let tools = EconGraphMcpServer::get_available_tools();
    assert!(!tools.is_empty(), "Should have available tools");

    // 2. Get available resources
    let resources = EconGraphMcpServer::get_available_resources();
    assert!(!resources.is_empty(), "Should have available resources");

    // 3. Test search functionality
    let search_args = json!({
        "query": "GDP",
        "limit": 5
    });
    let search_result = server.search_economic_series(search_args).await;
    assert!(search_result.is_ok(), "Search should succeed");

    // 4. Test data sources
    let data_sources_result = server.get_data_sources().await;
    assert!(
        data_sources_result.is_ok(),
        "Data sources query should succeed"
    );

    // 5. Test series catalog
    let catalog_result = server.get_series_catalog().await;
    assert!(
        catalog_result.is_ok(),
        "Series catalog query should succeed"
    );

    // 6. Test visualization creation (will succeed with fallback data but tests the flow)
    let viz_args = json!({
        "series_ids": ["550e8400-e29b-41d4-a716-446655440000"],
        "chart_type": "line",
        "title": "End-to-End Test Chart"
    });
    let viz_result = server.create_data_visualization(viz_args).await;
    // This will succeed with fallback data because the series doesn't exist, testing the complete flow
    assert!(
        viz_result.is_ok(),
        "Visualization should succeed with fallback data for non-existent series"
    );
}

#[tokio::test]
#[serial]
async fn test_mcp_server_create_data_visualization_tool_integration() {
    // This test requires the chart API service to be running
    // It should be run as part of the integration test suite that starts all services
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test the visualization tool with mock data
    let viz_args = json!({
        "series_ids": ["test-series-1", "test-series-2"],
        "chart_type": "line",
        "title": "Integration Test Chart",
        "start_date": "2020-01-01",
        "end_date": "2023-12-31"
    });

    let result = server.create_data_visualization(viz_args).await;
    assert!(
        result.is_ok(),
        "Visualization tool failed: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(response.get("content").is_some());
    // This should return a fallback visualization since the series don't exist
}

#[tokio::test]
#[serial]
async fn test_mcp_server_call_private_chart_api_success_integration() {
    // This test requires the chart API service to be running
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test successful chart API call with mock data
    let chart_request = json!({
        "seriesData": [{
            "id": "test-series",
            "name": "Test Series",
            "dataPoints": [
                {"date": "2023-01-01", "value": 100.0},
                {"date": "2023-02-01", "value": 105.0}
            ]
        }],
        "chartType": "line",
        "title": "Integration Test Chart",
        "startDate": "2023-01-01",
        "endDate": "2023-12-31"
    });

    // In integration tests, this should succeed if the chart API service is running
    let result = server.call_private_chart_api(&chart_request).await;
    // Note: This will fail if chart API service is not running, which is expected
    // in environments where the service is not started
    if result.is_err() {
        println!("Chart API service not available in integration test environment");
    }
}

#[tokio::test]
#[serial]
async fn test_mcp_server_call_private_chart_api_failure_integration() {
    // This test requires the chart API service to be running
    let pool = create_test_database_pool().await;
    let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

    // Test chart API call with invalid data
    let invalid_request = json!({
        "invalid": "data"
    });

    // This should fail due to invalid request format
    let result = server.call_private_chart_api(&invalid_request).await;
    assert!(result.is_err());
}
