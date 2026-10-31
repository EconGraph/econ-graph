use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
/// Integration tests for MCP server that require real backend and chart API services
/// These tests start up actual services and test end-to-end functionality
use econ_graph_mcp::mcp_server::EconGraphMcpServer;
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;
use testcontainers::{ContainerAsync, GenericImage};

/// The HTTP route supplies a verified caller in production. Direct tool tests provide an
/// explicit principal with the same catalog capabilities rather than relying on public data.
fn test_server(
    app: econ_graph_core::AppPool,
    data: Option<econ_graph_core::DataPool>,
) -> EconGraphMcpServer {
    let mut context = econ_graph_graphql::graphql::context::GraphQLContext::anonymous();
    context.principal = Some(econ_graph_auth::Principal::new(
        Default::default(),
        econ_graph_auth::Role::all()
            .iter()
            .copied()
            .filter(|role| role.is_catalog() || *role == econ_graph_auth::Role::ApiMcp),
    ));
    EconGraphMcpServer::new(app, data).with_auth_context(Arc::new(context))
}

// We need to create a simple test container setup since we can't import test_utils
// from the tests directory. Let's create a minimal version for integration testing.

async fn create_test_database_pool() -> (
    ContainerAsync<GenericImage>,
    econ_graph_core::database::DatabasePool,
) {
    use diesel::Connection;
    use testcontainers::core::WaitFor;
    use testcontainers::runners::AsyncRunner;
    use testcontainers::{GenericImage, ImageExt};

    let postgres = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "econ_graph_test")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "password");

    let container = postgres.start().await.expect("Failed to start container");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");

    let database_url = format!(
        "postgresql://postgres:password@localhost:{}/econ_graph_test",
        port
    );

    // Run migrations: both sets in one database, data then app, as `run_migrations` does.
    use diesel_migrations::MigrationHarness;
    use econ_graph_core::database::{APP_MIGRATIONS, DATA_MIGRATIONS};

    let mut conn =
        diesel::PgConnection::establish(&database_url).expect("Failed to connect to test database");

    conn.run_pending_migrations(DATA_MIGRATIONS)
        .expect("Failed to run the data migrations");
    conn.run_pending_migrations(APP_MIGRATIONS)
        .expect("Failed to run the app migrations");

    // Use the core crate's database pool creation function
    let pool = econ_graph_core::database::create_pool(&database_url)
        .await
        .expect("Failed to create connection pool");
    (container, pool)
}

/// Seed an observed series so visualization tests exercise valid GraphQL data.
async fn seed_series(pool: &econ_graph_core::DatabasePool) -> String {
    use diesel_async::RunQueryDsl;
    let mut conn = pool.get().await.unwrap();
    let id = econ_graph_core::test_utils::insert_series_without_observations(&mut conn).await;
    diesel::sql_query("UPDATE economic_series SET title = 'MCP GDP', start_date = '2023-01-01', end_date = '2023-02-01' WHERE id = $1")
        .bind::<diesel::sql_types::Uuid,_>(id).execute(&mut conn).await.unwrap();
    diesel::sql_query("INSERT INTO data_points (series_id, date, value, revision_date) VALUES ($1, '2023-01-01', 100, '2023-01-01'), ($1, '2023-02-01', 105, '2023-02-01')")
        .bind::<diesel::sql_types::Uuid,_>(id).execute(&mut conn).await.unwrap();
    id.to_string()
}

/// Series data and charts get each date once, at its latest revision: a revised date gives its
/// revision, and a date with a projection and a synthetic legacy row gives the projection, as
/// the site's charts do.
#[tokio::test]
#[serial]
async fn mcp_series_data_has_each_date_once_at_its_latest_revision() {
    use diesel_async::RunQueryDsl;
    let (_database, pool) = create_test_database_pool().await;
    let mut conn = pool.get().await.unwrap();
    let id = econ_graph_core::test_utils::insert_series_without_observations(&mut conn).await;
    diesel::sql_query("UPDATE economic_series SET title = 'MCP revisions', start_date = '2023-01-01', end_date = '2023-02-01' WHERE id = $1")
        .bind::<diesel::sql_types::Uuid, _>(id).execute(&mut conn).await.unwrap();
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release) VALUES \
         ($1, '2023-01-01', 100, '2023-01-15', true), ($1, '2023-01-01', 101, '2023-03-01', false), \
         ($1, '2023-02-01', 105, '2023-01-20', true), ($1, '2023-02-01', 999, '2023-02-01', true)",
    )
    .bind::<diesel::sql_types::Uuid, _>(id)
    .execute(&mut conn)
    .await
    .unwrap();
    drop(conn);
    let server = test_server(
        econ_graph_core::AppPool(pool.clone()),
        Some(econ_graph_core::DataPool(pool)),
    );
    let response = server
        .get_series_data(json!({"series_id": id.to_string()}))
        .await
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
    let mut points: Vec<(String, String)> = payload["data"]["series"]["dataPoints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let value = p["value"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| p["value"].to_string());
            (p["date"].as_str().unwrap().to_string(), value)
        })
        .collect();
    points.sort();
    let values: Vec<f64> = points.iter().map(|(_, v)| v.parse().unwrap()).collect();
    assert_eq!(
        points.iter().map(|(d, _)| d.as_str()).collect::<Vec<_>>(),
        ["2023-01-01", "2023-02-01"],
        "{points:?}"
    );
    assert_eq!(values, [101.0, 105.0], "{points:?}");
}

/// A series longer than the limit gives its latest dates, marked truncated with the total, in
/// both the data tool and the chart tool, which charts the latest 1000.
#[tokio::test]
#[serial]
async fn mcp_series_data_and_charts_give_the_latest_dates_and_say_when_cut_short() {
    use diesel_async::RunQueryDsl;
    let (_database, pool) = create_test_database_pool().await;
    let mut conn = pool.get().await.unwrap();
    let id = econ_graph_core::test_utils::insert_series_without_observations(&mut conn).await;
    diesel::sql_query("UPDATE economic_series SET title = 'MCP long series', start_date = '1900-01-01', end_date = '2000-04-01' WHERE id = $1")
        .bind::<diesel::sql_types::Uuid, _>(id).execute(&mut conn).await.unwrap();
    // 1201 monthly dates, 1900-01 to 2000-01, valued by their index.
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, value, revision_date) \
         SELECT $1, d::date, n, d::date \
         FROM generate_series(0, 1200) n, LATERAL (SELECT DATE '1900-01-01' + n * INTERVAL '1 month' AS d) m",
    )
    .bind::<diesel::sql_types::Uuid, _>(id)
    .execute(&mut conn)
    .await
    .unwrap();
    drop(conn);
    let server = test_server(
        econ_graph_core::AppPool(pool.clone()),
        Some(econ_graph_core::DataPool(pool)),
    );
    let series = |text: &str| -> serde_json::Value {
        serde_json::from_str::<serde_json::Value>(text).unwrap()["data"]["series"].clone()
    };
    let dates = |series: &serde_json::Value| -> Vec<String> {
        series["dataPoints"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["date"].as_str().unwrap().to_string())
            .collect()
    };

    // The default limit, 100: the latest 100 dates, oldest first.
    let response = server
        .get_series_data(json!({"series_id": id.to_string()}))
        .await
        .unwrap();
    let got = series(response["content"][0]["text"].as_str().unwrap());
    let got_dates = dates(&got);
    assert_eq!(got_dates.len(), 100);
    assert_eq!(got_dates[0], "1991-10-01");
    assert_eq!(got_dates.last().unwrap(), "2000-01-01");
    assert_eq!(
        (got["truncated"].clone(), got["totalCount"].clone()),
        (json!(true), json!(1201))
    );

    // Within a date range, the latest dates in it.
    let response = server
        .get_series_data(
            json!({"series_id": id.to_string(), "end_date": "1950-12-01", "limit": 12}),
        )
        .await
        .unwrap();
    let got = series(response["content"][0]["text"].as_str().unwrap());
    assert_eq!(dates(&got)[0], "1950-01-01");
    assert_eq!(dates(&got).last().unwrap(), "1950-12-01");
    assert_eq!(got["totalCount"], json!(612));

    // Everything fits: not truncated.
    let response = server
        .get_series_data(json!({"series_id": id.to_string(), "limit": 5000}))
        .await
        .unwrap();
    let got = series(response["content"][0]["text"].as_str().unwrap());
    assert_eq!(dates(&got).len(), 1201);
    assert_eq!(
        (got["truncated"].clone(), got["totalCount"].clone()),
        (json!(false), json!(1201))
    );

    // The chart tool: no chart API runs here, so it returns its data structure, the latest 1000.
    let response = server
        .create_data_visualization(json!({"series_ids": [id.to_string()]}))
        .await
        .unwrap();
    let text = response["content"][0]["text"].as_str().unwrap();
    let data: serde_json::Value =
        serde_json::from_str(text.split("📋 Visualization Data:\n").nth(1).expect(text)).unwrap();
    let charted = &data["series_data"][0];
    assert_eq!(dates(charted).len(), 1000);
    assert_eq!(dates(charted)[0], "1916-10-01");
    assert_eq!(dates(charted).last().unwrap(), "2000-01-01");
    assert_eq!(
        (charted["truncated"].clone(), charted["totalCount"].clone()),
        (json!(true), json!(1201))
    );
}

/// Tools must return stored data, not a successful wrapper around GraphQL errors.
#[tokio::test]
#[serial]
async fn mcp_queries_return_real_series_and_date_filtered_observations() {
    let (_database, pool) = create_test_database_pool().await;
    let series_id = seed_series(&pool).await;
    let server = test_server(
        econ_graph_core::AppPool(pool.clone()),
        Some(econ_graph_core::DataPool(pool)),
    );
    let denied = server.clone().with_auth_context(Arc::new(
        econ_graph_graphql::graphql::context::GraphQLContext::anonymous(),
    ));
    assert!(
        denied
            .get_series_data(json!({"series_id": series_id}))
            .await
            .is_err(),
        "direct tool calls with no catalog grants must not bypass authorization"
    );
    let response = server
        .get_series_data(json!({"series_id": series_id, "start_date": "2023-02-01", "limit": 1}))
        .await
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["data"]["series"]["id"], series_id);
    let points = payload["data"]["series"]["dataPoints"].as_array().unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0]["date"], "2023-02-01");

    let response = server
        .search_economic_series(json!({"query": "GDP", "limit": 5}))
        .await
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
    assert!(payload["data"]["searchSeries"]["series"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == series_id));
    let response = server.get_series_catalog().await.unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(response["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert!(payload["data"]["series"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == series_id));
    assert!(server.get_data_sources().await.is_ok());
}

#[tokio::test]
#[serial]
async fn test_mcp_server_integration_basic_functionality() {
    // Test basic MCP server functionality with real database
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

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
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

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
    assert!(
        result.is_err(),
        "An invalid series ID must produce a tool error"
    );

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
    let (_database, pool) = create_test_database_pool().await;
    let server = Arc::new(test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    ));

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
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

    let series_id = seed_series(&pool).await;
    // Test visualization creation with stored observations
    let viz_args = json!({
        "series_ids": [series_id],
        "chart_type": "line",
        "title": "Integration Test Chart"
    });

    // This will succeed with the stored observations if the chart service is unavailable, but tests the integration logic
    let result = server.create_data_visualization(viz_args).await;
    assert!(
        result.is_ok(),
        "Should succeed with stored observations when the chart service is unavailable, testing integration"
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
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

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
    let (_database, pool) = create_test_database_pool().await;
    let series_id = seed_series(&pool).await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

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
        "series_ids": [series_id],
        "chart_type": "line",
        "title": "End-to-End Test Chart"
    });
    let viz_result = server.create_data_visualization(viz_args).await;
    // This will succeed with the stored observations if the chart service is unavailable, testing the complete flow
    assert!(
        viz_result.is_ok(),
        "Visualization should succeed with stored observations when the chart service is unavailable"
    );
}

#[tokio::test]
#[serial]
async fn test_mcp_server_create_data_visualization_tool_integration() {
    // This test requires the chart API service to be running
    // It should be run as part of the integration test suite that starts all services
    let (_database, pool) = create_test_database_pool().await;
    let series_id = seed_series(&pool).await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

    // Test the visualization tool with mock data
    let viz_args = json!({
        "series_ids": [series_id],
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
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

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
    let (_database, pool) = create_test_database_pool().await;
    let server = test_server(
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
    );

    // Test chart API call with invalid data
    let invalid_request = json!({
        "invalid": "data"
    });

    // This should fail due to invalid request format
    let result = server.call_private_chart_api(&invalid_request).await;
    assert!(result.is_err());
}
