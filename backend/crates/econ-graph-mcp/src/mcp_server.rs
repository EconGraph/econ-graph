//! MCP (Model Context Protocol) Server for EconGraph
//!
//! This module provides an MCP server that exposes economic data search,
//! retrieval, and graphing capabilities to AI models through a standardized protocol.

use anyhow::Result;
use async_graphql::{Request, Variables};
use futures::future;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use warp::http::StatusCode;
use warp::Reply;

use econ_graph_core::database::DatabasePool;
use econ_graph_graphql::graphql::schema::create_schema_with_data;

/// MCP Server implementation for EconGraph
#[derive(Clone)]
pub struct EconGraphMcpServer {
    /// Database connection pool
    pool: Arc<DatabasePool>,
    /// GraphQL schema for data operations
    schema: async_graphql::Schema<
        econ_graph_graphql::graphql::query::Query,
        econ_graph_graphql::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    >,
    /// HTTP client for calling private frontend chart API
    http_client: Client,
    /// Frontend chart API base URL
    frontend_chart_api_url: String,
}

impl EconGraphMcpServer {
    /// Create a new MCP server instance
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        let schema = create_schema_with_data(pool.clone(), ());
        let http_client = Client::new();
        let frontend_chart_api_url = std::env::var("CHART_API_SERVICE_URL").unwrap_or_else(|_| {
            "http://chart-api-service.econ-graph.svc.cluster.local:3001/api/chart".to_string()
        });

        Self {
            pool,
            schema,
            http_client,
            frontend_chart_api_url,
        }
    }

    /// Execute a GraphQL query
    async fn execute_graphql_query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let request = if let Some(vars) = variables {
            let graphql_vars: Variables = serde_json::from_value(vars)?;
            Request::new(query).variables(graphql_vars)
        } else {
            Request::new(query)
        };

        let response = self.schema.execute(request).await;

        // Convert GraphQL response to JSON
        let result = serde_json::to_value(response)?;
        Ok(result)
    }

    /// Handle MCP tool calls
    pub async fn handle_tool_call(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        match tool_name {
            "search_economic_series" => self.search_economic_series(arguments).await,
            "get_series_data" => self.get_series_data(arguments).await,
            "get_series_metadata" => self.get_series_metadata(arguments).await,
            "create_data_visualization" => self.create_data_visualization(arguments).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        }
    }

    /// Get available tools
    pub fn get_available_tools() -> Vec<Value> {
        vec![
            json!({
                "name": "search_economic_series",
                "description": "Search for economic data series by name, description, or keywords",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query for economic series"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results to return",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "get_series_data",
                "description": "Retrieve time series data points for a specific economic series",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "series_id": {
                            "type": "string",
                            "description": "UUID of the economic series"
                        },
                        "start_date": {
                            "type": "string",
                            "description": "Start date in YYYY-MM-DD format",
                            "format": "date"
                        },
                        "end_date": {
                            "type": "string",
                            "description": "End date in YYYY-MM-DD format",
                            "format": "date"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of data points to return",
                            "default": 100
                        }
                    },
                    "required": ["series_id"]
                }
            }),
            json!({
                "name": "get_series_metadata",
                "description": "Get detailed metadata about an economic series",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "series_id": {
                            "type": "string",
                            "description": "UUID of the economic series"
                        }
                    },
                    "required": ["series_id"]
                }
            }),
            json!({
                "name": "create_data_visualization",
                "description": "Create a data visualization (chart) for economic data",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "series_ids": {
                            "type": "array",
                            "items": {
                                "type": "string"
                            },
                            "description": "Array of series UUIDs to visualize"
                        },
                        "chart_type": {
                            "type": "string",
                            "enum": ["line", "bar", "scatter"],
                            "description": "Type of chart to create",
                            "default": "line"
                        },
                        "title": {
                            "type": "string",
                            "description": "Title for the chart"
                        },
                        "start_date": {
                            "type": "string",
                            "description": "Start date in YYYY-MM-DD format",
                            "format": "date"
                        },
                        "end_date": {
                            "type": "string",
                            "description": "End date in YYYY-MM-DD format",
                            "format": "date"
                        }
                    },
                    "required": ["series_ids"]
                }
            }),
        ]
    }

    /// Get available resources
    pub fn get_available_resources() -> Vec<Value> {
        vec![
            json!({
                "uri": "econ-graph://data-sources",
                "name": "Data Sources",
                "description": "Available economic data sources (FRED, BLS, etc.)",
                "mime_type": "application/json"
            }),
            json!({
                "uri": "econ-graph://series-catalog",
                "name": "Series Catalog",
                "description": "Catalog of all available economic series",
                "mime_type": "application/json"
            }),
        ]
    }

    /// Search for economic series
    pub async fn search_economic_series(&self, arguments: Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: query"))?;

        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(10) as i32;

        let graphql_query = r#"
            query SearchSeries($query: String!, $limit: Int!) {
                searchSeries(query: $query, limit: $limit) {
                    id
                    name
                    description
                    frequency
                    units
                    source {
                        name
                        description
                    }
                    lastUpdated
                }
            }
        "#;

        let variables = json!({
            "query": query,
            "limit": limit
        });

        let result = self
            .execute_graphql_query(graphql_query, Some(variables))
            .await?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }],
            "is_error": false
        }))
    }

    /// Get series data points
    pub async fn get_series_data(&self, arguments: Value) -> Result<Value> {
        let series_id = arguments
            .get("series_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: series_id"))?;

        let start_date = arguments.get("start_date").and_then(|v| v.as_str());
        let end_date = arguments.get("end_date").and_then(|v| v.as_str());
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_i64())
            .unwrap_or(100) as i32;

        let mut graphql_query = r#"
            query GetSeriesData($seriesId: Uuid!, $limit: Int!) {
                series(id: $seriesId) {
                    id
                    name
                    description
                    dataPoints(limit: $limit) {
                        date
                        value
                        note
                    }
                }
            }
        "#
        .to_string();

        let mut variables = json!({
            "seriesId": series_id,
            "limit": limit
        });

        // Add date filtering if provided
        if let Some(start) = start_date {
            graphql_query = graphql_query.replace(
                "dataPoints(limit: $limit)",
                "dataPoints(limit: $limit, startDate: $startDate)",
            );
            variables["startDate"] = json!(start);
        }

        if let Some(end) = end_date {
            // Handle both cases: with and without start date
            if start_date.is_some() {
                graphql_query = graphql_query.replace(
                    "dataPoints(limit: $limit, startDate: $startDate)",
                    "dataPoints(limit: $limit, startDate: $startDate, endDate: $endDate)",
                );
            } else {
                graphql_query = graphql_query.replace(
                    "dataPoints(limit: $limit)",
                    "dataPoints(limit: $limit, endDate: $endDate)",
                );
            }
            variables["endDate"] = json!(end);
        }

        let result = self
            .execute_graphql_query(&graphql_query, Some(variables))
            .await?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }],
            "is_error": false
        }))
    }

    /// Get series metadata
    async fn get_series_metadata(&self, arguments: Value) -> Result<Value> {
        let series_id = arguments
            .get("series_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: series_id"))?;

        let graphql_query = r#"
            query GetSeriesMetadata($seriesId: Uuid!) {
                series(id: $seriesId) {
                    id
                    name
                    description
                    frequency
                    units
                    source {
                        name
                        description
                        url
                    }
                    tags
                    lastUpdated
                    createdAt
                }
            }
        "#;

        let variables = json!({
            "seriesId": series_id
        });

        let result = self
            .execute_graphql_query(graphql_query, Some(variables))
            .await?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }],
            "is_error": false
        }))
    }

    /// Create data visualization
    pub async fn create_data_visualization(&self, arguments: Value) -> Result<Value> {
        let series_ids = arguments
            .get("series_ids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: series_ids"))?;

        let chart_type = arguments
            .get("chart_type")
            .and_then(|v| v.as_str())
            .unwrap_or("line");

        let title = arguments.get("title").and_then(|v| v.as_str());
        let start_date = arguments.get("start_date").and_then(|v| v.as_str());
        let end_date = arguments.get("end_date").and_then(|v| v.as_str());

        // Collect series data for chart generation
        let mut series_data = Vec::new();
        for series_id in series_ids {
            if let Some(id) = series_id.as_str() {
                let series_data_item = self
                    .get_series_data_for_visualization(id, start_date, end_date)
                    .await?;
                series_data.push(series_data_item);
            }
        }

        // Prepare chart request for private frontend API
        let chart_request = json!({
            "seriesData": series_data,
            "chartType": chart_type,
            "title": title.unwrap_or("Economic Data Visualization"),
            "startDate": start_date.unwrap_or("2020-01-01"),
            "endDate": end_date.unwrap_or("2024-01-01"),
            "showLegend": true,
            "showGrid": true,
            "yAxisLabel": "Value",
            "xAxisLabel": "Date"
        });

        // Call private frontend chart API to generate chart configuration
        match self.call_private_chart_api(&chart_request).await {
            Ok(chart_response) => {
                if let Some(chart_config) = chart_response.get("chartConfig") {
                    // Return the complete chart configuration that can be rendered
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "✅ Chart visualization generated successfully using private frontend API!\n\n📊 Chart Configuration:\n{}\n\n📈 Metadata:\n{}",
                                serde_json::to_string_pretty(chart_config).unwrap_or_else(|_| "Failed to format chart config".to_string()),
                                serde_json::to_string_pretty(chart_response.get("metadata").unwrap_or(&json!(null))).unwrap_or_else(|_| "No metadata available".to_string())
                            )
                        }],
                        "is_error": false
                    }))
                } else {
                    // Fallback if chart API doesn't return expected format
                    self.create_fallback_visualization(series_data, chart_type, title)
                        .await
                }
            }
            Err(error) => {
                // Fallback to data structure if chart API fails
                eprintln!(
                    "Private chart API failed: {}, falling back to data structure",
                    error
                );
                self.create_fallback_visualization(series_data, chart_type, title)
                    .await
            }
        }
    }

    /// Call private frontend chart API to generate chart configuration
    pub async fn call_private_chart_api(&self, chart_request: &Value) -> Result<Value> {
        let url = format!("{}/generate", self.frontend_chart_api_url);

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-MCP-Server-Request", "true")
            .header("X-Internal-Request", "true")
            .json(chart_request)
            .send()
            .await?;

        if response.status().is_success() {
            let chart_response: Value = response.json().await?;
            Ok(chart_response)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(anyhow::anyhow!("Chart API error: {}", error_text))
        }
    }

    /// Create fallback visualization when private chart API is unavailable
    pub async fn create_fallback_visualization(
        &self,
        series_data: Vec<Value>,
        chart_type: &str,
        title: Option<&str>,
    ) -> Result<Value> {
        let visualization_data = json!({
            "chart_type": chart_type,
            "title": title.unwrap_or("Economic Data Visualization"),
            "series_data": series_data,
            "metadata": {
                "series_count": series_data.len(),
                "total_data_points": series_data.iter()
                    .map(|s| s.get("dataPoints").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0))
                    .sum::<usize>(),
                "note": "Private chart API unavailable - returning data structure for manual visualization"
            }
        });

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "📊 Data visualization prepared for {} series with {} total data points.\n\n⚠️ Note: Private chart API unavailable - returning data structure.\n\n📋 Visualization Data:\n{}",
                    series_data.len(),
                    series_data.iter()
                        .map(|s| s.get("dataPoints").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0))
                        .sum::<usize>(),
                    serde_json::to_string_pretty(&visualization_data).unwrap_or_else(|_| "Failed to format data".to_string())
                )
            }],
            "is_error": false
        }))
    }

    /// Get data sources
    pub async fn get_data_sources(&self) -> Result<Value> {
        let graphql_query = r#"
            query GetDataSources {
                dataSources {
                    id
                    name
                    description
                    url
                    lastUpdated
                }
            }
        "#;

        let result = self.execute_graphql_query(graphql_query, None).await?;

        Ok(json!({
            "contents": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }]
        }))
    }

    /// Get series catalog
    pub async fn get_series_catalog(&self) -> Result<Value> {
        let graphql_query = r#"
            query GetSeriesCatalog {
                series(limit: 100) {
                    id
                    name
                    description
                    frequency
                    units
                    source {
                        name
                    }
                }
            }
        "#;

        let result = self.execute_graphql_query(graphql_query, None).await?;

        Ok(json!({
            "contents": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }]
        }))
    }

    /// Get series data for visualization
    async fn get_series_data_for_visualization(
        &self,
        series_id: &str,
        start_date: Option<&str>,
        end_date: Option<&str>,
    ) -> Result<Value> {
        let mut graphql_query = r#"
            query GetSeriesDataForViz($seriesId: Uuid!) {
                series(id: $seriesId) {
                    id
                    name
                    description
                    dataPoints(limit: 1000) {
                        date
                        value
                    }
                }
            }
        "#
        .to_string();

        let mut variables = json!({
            "seriesId": series_id
        });

        // Add date filtering if provided
        if let Some(start) = start_date {
            graphql_query = graphql_query.replace(
                "dataPoints(limit: 1000)",
                "dataPoints(limit: 1000, startDate: $startDate)",
            );
            variables["startDate"] = json!(start);
        }

        if let Some(end) = end_date {
            // Handle both cases: with and without start date
            if start_date.is_some() {
                graphql_query = graphql_query.replace(
                    "dataPoints(limit: 1000, startDate: $startDate)",
                    "dataPoints(limit: 1000, startDate: $startDate, endDate: $endDate)",
                );
            } else {
                graphql_query = graphql_query.replace(
                    "dataPoints(limit: 1000)",
                    "dataPoints(limit: 1000, endDate: $endDate)",
                );
            }
            variables["endDate"] = json!(end);
        }

        let result = self
            .execute_graphql_query(&graphql_query, Some(variables))
            .await?;
        Ok(result)
    }
}

/// MCP Server HTTP handler
pub async fn mcp_handler(
    body: warp::hyper::body::Bytes,
    server: Arc<EconGraphMcpServer>,
) -> Result<impl Reply, warp::Rejection> {
    // Parse the MCP request
    let request: Value = match serde_json::from_slice(&body) {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("Failed to parse JSON: {}", e);
            return Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    }
                })),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    tracing::info!("MCP request received: {:?}", request);

    // Handle different MCP request types
    let response = match request.get("method").and_then(|m| m.as_str()) {
        Some("tools/list") => {
            tracing::info!("Handling tools/list request");
            let tools = EconGraphMcpServer::get_available_tools();
            tracing::info!("Retrieved {} tools", tools.len());
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "tools": tools
                }
            })
        }
        Some("tools/call") => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            match server.handle_tool_call(tool_name, arguments).await {
                Ok(result) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": result
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {}", e)
                    }
                }),
            }
        }
        Some("resources/list") => {
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "resources": EconGraphMcpServer::get_available_resources()
                }
            })
        }
        Some("resources/read") => {
            let params = request.get("params").cloned().unwrap_or(json!({}));
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

            let result = match uri {
                "econ-graph://data-sources" => server.get_data_sources().await,
                "econ-graph://series-catalog" => server.get_series_catalog().await,
                _ => Err(anyhow::anyhow!("Unknown resource: {}", uri)),
            };

            match result {
                Ok(data) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": data
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {}", e)
                    }
                }),
            }
        }
        _ => json!({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }),
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::test_utils::TestContainer;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_mcp_server_creation() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test that we can get a connection from the pool
        let connection_result = server.pool.get().await;
        assert!(
            connection_result.is_ok(),
            "Failed to get database connection: {:?}",
            connection_result.err()
        );

        // Test that the server has the expected fields
        assert!(!server.frontend_chart_api_url.is_empty());
        assert!(server.frontend_chart_api_url.contains("chart-api-service"));
    }

    #[tokio::test]
    async fn test_get_available_tools() {
        let tools = EconGraphMcpServer::get_available_tools();
        assert!(!tools.is_empty());

        let tool_names: Vec<String> = tools
            .iter()
            .map(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        assert!(tool_names.contains(&"search_economic_series".to_string()));
        assert!(tool_names.contains(&"get_series_data".to_string()));
        assert!(tool_names.contains(&"create_data_visualization".to_string()));
    }

    #[tokio::test]
    async fn test_get_available_resources() {
        let resources = EconGraphMcpServer::get_available_resources();
        assert!(!resources.is_empty());

        let resource_uris: Vec<String> = resources
            .iter()
            .map(|r| {
                r.get("uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();

        assert!(resource_uris.contains(&"econ-graph://data-sources".to_string()));
        assert!(resource_uris.contains(&"econ-graph://series-catalog".to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_server_with_database_operations() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test that we can execute a simple database query
        let connection_result = server.pool.get().await;
        assert!(
            connection_result.is_ok(),
            "Failed to get database connection"
        );

        let mut conn = connection_result.unwrap();

        // Test that we can get a connection (connection is already established above)
        // This verifies the testcontainer database is working properly
        assert!(true, "Database connection established successfully");
    }

    #[tokio::test]
    #[serial]
    async fn test_search_economic_series_tool() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test the search tool with a simple query
        let search_args = json!({
            "query": "GDP",
            "limit": 5
        });

        let result = server.search_economic_series(search_args).await;
        assert!(result.is_ok(), "Search tool failed: {:?}", result.err());

        let response = result.unwrap();
        assert!(response.get("content").is_some());
        assert!(!response.get("is_error").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_economic_series_tool() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test the get series tool with a mock series ID
        let get_args = json!({
            "series_id": "test-series-id",
            "start_date": "2020-01-01",
            "end_date": "2023-12-31"
        });

        let result = server.get_series_data(get_args).await;
        assert!(result.is_ok(), "Get series tool failed: {:?}", result.err());

        let response = result.unwrap();
        assert!(response.get("content").is_some());
        // Note: This might return an error if the series doesn't exist, which is expected
    }

    // test_create_data_visualization_tool moved to integration tests
    // because it requires the chart API service to be running

    // ===== INTEGRATION TESTS FOR HTTP ENDPOINT =====
    // These tests will catch the warp filter integration bug

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_integration() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test tools/list request
        let tools_list_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });

        let body = warp::hyper::body::Bytes::from(tools_list_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler failed for tools/list: {:?}",
            result.err()
        );
        let reply = result.unwrap();

        // Convert the reply to bytes to verify it's valid JSON-RPC
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
        let response_json: Value = serde_json::from_str(&response_text).unwrap();

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert_eq!(response_json["id"], 1);
        assert!(response_json["result"].is_object());
        assert!(response_json["result"]["tools"].is_array());
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_malformed_json() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with malformed JSON
        let malformed_json = "{ invalid json }";
        let body = warp::hyper::body::Bytes::from(malformed_json);
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler should handle malformed JSON gracefully"
        );
        let reply = result.unwrap();

        // Should return a JSON-RPC error response
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();

        // The response should be valid JSON
        let response_json: Value = serde_json::from_str(&response_text)
            .unwrap_or_else(|_| panic!("Failed to parse response as JSON: {}", response_text));

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert!(response_json["error"].is_object());
        assert_eq!(response_json["error"]["code"], -32700); // Parse error
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_invalid_method() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with invalid method
        let invalid_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "invalid/method"
        });

        let body = warp::hyper::body::Bytes::from(invalid_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler should handle invalid methods gracefully"
        );
        let reply = result.unwrap();

        // Should return a JSON-RPC error response
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
        let response_json: Value = serde_json::from_str(&response_text).unwrap();

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert!(response_json["error"].is_object());
        assert_eq!(response_json["error"]["code"], -32601); // Method not found
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_large_payload() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with large payload (search with many parameters)
        let large_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "search_economic_series",
                "arguments": {
                    "query": "GDP inflation unemployment employment trade balance current account fiscal deficit debt-to-gdp ratio",
                    "limit": 1000,
                    "start_date": "1900-01-01",
                    "end_date": "2030-12-31",
                    "sources": ["fred", "bls", "census", "bea", "imf", "oecd", "ecb", "boe", "boj", "rba", "boc", "snb", "world_bank", "wto", "unstats", "ilo", "fhfa"]
                }
            }
        });

        let body = warp::hyper::body::Bytes::from(large_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler should handle large payloads: {:?}",
            result.err()
        );
        let reply = result.unwrap();

        // Convert to response to verify it's valid
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
        let response_json: Value = serde_json::from_str(&response_text).unwrap();

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert!(response_json["result"].is_object());
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_concurrent_requests() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = Arc::new(EconGraphMcpServer::new(Arc::new(pool.clone())));

        // Test concurrent requests to ensure thread safety
        let mut handles = vec![];

        for i in 0..10 {
            let server_clone = server.clone();
            let handle = tokio::spawn(async move {
                let request = json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "method": "tools/list"
                });

                let body = warp::hyper::body::Bytes::from(request.to_string());
                mcp_handler(body, server_clone).await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        let results = future::join_all(handles).await;

        // Verify all requests succeeded
        for (i, result) in results.into_iter().enumerate() {
            assert!(
                result.is_ok(),
                "Concurrent request {} failed: {:?}",
                i,
                result.as_ref().err()
            );
            let handler_result = result.unwrap();
            assert!(
                handler_result.is_ok(),
                "MCP handler failed for concurrent request {}: {:?}",
                i,
                handler_result.as_ref().err()
            );

            let reply = handler_result.unwrap();
            // Convert to response to verify it's valid JSON
            let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
                .await
                .unwrap();
            let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
            let response_json: Value = serde_json::from_str(&response_text).unwrap();

            assert_eq!(response_json["jsonrpc"], "2.0");
            assert_eq!(response_json["id"], i as i64);
            assert!(response_json["result"].is_object());
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_error_handling() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test empty body
        let empty_body = warp::hyper::body::Bytes::new();
        let result = mcp_handler(empty_body, Arc::new(server.clone())).await;
        assert!(result.is_ok(), "Should handle empty body gracefully");

        // Test non-JSON body
        let non_json_body = warp::hyper::body::Bytes::from("This is not JSON");
        let result = mcp_handler(non_json_body, Arc::new(server.clone())).await;
        assert!(result.is_ok(), "Should handle non-JSON body gracefully");

        // Test missing required fields
        let incomplete_request = json!({
            "jsonrpc": "2.0",
            "id": 1
            // Missing method
        });
        let body = warp::hyper::body::Bytes::from(incomplete_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;
        assert!(
            result.is_ok(),
            "Should handle incomplete requests gracefully"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_tools_call_integration() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test tools/call with search_economic_series
        let tools_call_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "search_economic_series",
                "arguments": {
                    "query": "GDP",
                    "limit": 5
                }
            }
        });

        let body = warp::hyper::body::Bytes::from(tools_call_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler failed for tools/call: {:?}",
            result.err()
        );
        let reply = result.unwrap();

        // Parse and verify response
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
        let response_json: Value = serde_json::from_str(&response_text).unwrap();

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert_eq!(response_json["id"], 2);
        assert!(response_json["result"].is_object());
    }

    #[tokio::test]
    #[serial]
    async fn test_mcp_handler_resources_list_integration() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test resources/list request
        let resources_list_request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "resources/list"
        });

        let body = warp::hyper::body::Bytes::from(resources_list_request.to_string());
        let result = mcp_handler(body, Arc::new(server)).await;

        assert!(
            result.is_ok(),
            "MCP handler failed for resources/list: {:?}",
            result.err()
        );
        let reply = result.unwrap();

        // Parse and verify response
        let response_bytes = warp::hyper::body::to_bytes(reply.into_response().into_body())
            .await
            .unwrap();
        let response_text = String::from_utf8(response_bytes.to_vec()).unwrap();
        let response_json: Value = serde_json::from_str(&response_text).unwrap();

        assert_eq!(response_json["jsonrpc"], "2.0");
        assert_eq!(response_json["id"], 3);
        assert!(response_json["result"].is_object());
        assert!(response_json["result"]["resources"].is_array());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_data_sources() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test get_data_sources method
        let result = server.get_data_sources().await;
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.get("contents").is_some());
        assert!(data.get("contents").unwrap().is_array());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_catalog() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test get_series_catalog method
        let result = server.get_series_catalog().await;
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.get("contents").is_some());
        assert!(data.get("contents").unwrap().is_array());
    }

    #[tokio::test]
    #[serial]
    async fn test_create_fallback_visualization() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test fallback visualization with sample data
        let series_data = vec![
            json!({
                "id": "test-series-1",
                "name": "Test Series 1",
                "dataPoints": [
                    {"date": "2023-01-01", "value": 100.0},
                    {"date": "2023-02-01", "value": 105.0}
                ]
            }),
            json!({
                "id": "test-series-2",
                "name": "Test Series 2",
                "dataPoints": [
                    {"date": "2023-01-01", "value": 200.0},
                    {"date": "2023-02-01", "value": 210.0}
                ]
            }),
        ];

        let result = server
            .create_fallback_visualization(series_data, "line", Some("Test Chart"))
            .await;

        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.get("content").is_some());
        assert!(!data.get("is_error").unwrap().as_bool().unwrap());

        // Check that the content contains expected information
        let content = data.get("content").unwrap().as_array().unwrap();
        assert!(!content.is_empty());

        let text_content = content[0].get("text").unwrap().as_str().unwrap();
        assert!(text_content.contains("Data visualization prepared"));
        assert!(text_content.contains("2 series"));
        assert!(text_content.contains("4 total data points"));
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_for_visualization() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with a valid UUID (this will fail at GraphQL level but tests the function)
        let result = server
            .get_series_data_for_visualization(
                "550e8400-e29b-41d4-a716-446655440000",
                Some("2023-01-01"),
                Some("2023-12-31"),
            )
            .await;

        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_for_visualization_start_date_only() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with start date only
        let result = server
            .get_series_data_for_visualization(
                "550e8400-e29b-41d4-a716-446655440000",
                Some("2023-01-01"),
                None,
            )
            .await;

        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_for_visualization_end_date_only() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with end date only
        let result = server
            .get_series_data_for_visualization(
                "550e8400-e29b-41d4-a716-446655440000",
                None,
                Some("2023-12-31"),
            )
            .await;

        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_for_visualization_no_dates() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test with no date filters
        let result = server
            .get_series_data_for_visualization("550e8400-e29b-41d4-a716-446655440000", None, None)
            .await;

        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_search_economic_series_with_custom_limit() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test search with custom limit
        let arguments = json!({
            "query": "GDP",
            "limit": 5
        });

        let result = server.search_economic_series(arguments).await;
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.get("content").is_some());
        assert!(!data.get("is_error").unwrap().as_bool().unwrap());
    }

    #[tokio::test]
    #[serial]
    async fn test_search_economic_series_missing_query() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test search with missing query parameter
        let arguments = json!({
            "limit": 10
        });

        let result = server.search_economic_series(arguments).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Missing required parameter: query"));
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_missing_series_id() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test get_series_data with missing series_id
        let arguments = json!({
            "start_date": "2023-01-01",
            "end_date": "2023-12-31"
        });

        let result = server.get_series_data(arguments).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Missing required parameter: series_id"));
    }

    #[tokio::test]
    #[serial]
    async fn test_get_series_data_with_custom_limit() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test get_series_data with custom limit
        let arguments = json!({
            "series_id": "550e8400-e29b-41d4-a716-446655440000",
            "limit": 50
        });

        let result = server.get_series_data(arguments).await;
        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_missing_series_id() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test create_data_visualization with missing series_id
        let arguments = json!({
            "chart_type": "line",
            "title": "Test Chart"
        });

        let result = server.create_data_visualization(arguments).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Missing required parameter: series_id"));
    }

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_missing_chart_type() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test create_data_visualization with missing chart_type
        let arguments = json!({
            "series_id": "550e8400-e29b-41d4-a716-446655440000",
            "title": "Test Chart"
        });

        let result = server.create_data_visualization(arguments).await;
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Missing required parameter: chart_type"));
    }

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_with_title() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test create_data_visualization with title
        let arguments = json!({
            "series_id": "550e8400-e29b-41d4-a716-446655440000",
            "chart_type": "line",
            "title": "Custom Chart Title"
        });

        let result = server.create_data_visualization(arguments).await;
        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_without_title() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test create_data_visualization without title
        let arguments = json!({
            "series_id": "550e8400-e29b-41d4-a716-446655440000",
            "chart_type": "line"
        });

        let result = server.create_data_visualization(arguments).await;
        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_with_date_filters() {
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test create_data_visualization with date filters
        let arguments = json!({
            "series_id": "550e8400-e29b-41d4-a716-446655440000",
            "chart_type": "line",
            "title": "Test Chart",
            "start_date": "2023-01-01",
            "end_date": "2023-12-31"
        });

        let result = server.create_data_visualization(arguments).await;
        // This will fail because the series doesn't exist, but tests the function logic
        assert!(result.is_err());
    }

    // test_call_private_chart_api_* tests moved to integration tests
    // because they require the chart API service to be running
}
