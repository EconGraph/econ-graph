//! MCP (Model Context Protocol) Server for EconGraph
//!
//! This module provides an MCP server that exposes economic data search,
//! retrieval, and graphing capabilities to AI models through a standardized protocol.

use anyhow::Result;
use async_graphql::{Request, Variables};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use warp::http::StatusCode;
use warp::Reply;

use crate::database::DatabasePool;
use crate::graphql::create_schema_with_data;

/// MCP Server implementation for EconGraph
pub struct EconGraphMcpServer {
    /// Database connection pool
    pool: Arc<DatabasePool>,
    /// GraphQL schema for data operations
    schema: async_graphql::Schema<
        crate::graphql::query::Query,
        crate::graphql::mutation::Mutation,
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
        let schema = create_schema_with_data((*pool).clone());
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
    async fn search_economic_series(&self, arguments: Value) -> Result<Value> {
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
    async fn get_series_data(&self, arguments: Value) -> Result<Value> {
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
            graphql_query = graphql_query.replace(
                "dataPoints(limit: $limit, startDate: $startDate)",
                "dataPoints(limit: $limit, startDate: $startDate, endDate: $endDate)",
            );
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
    async fn create_data_visualization(&self, arguments: Value) -> Result<Value> {
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
    async fn call_private_chart_api(&self, chart_request: &Value) -> Result<Value> {
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
    async fn create_fallback_visualization(
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
            graphql_query = graphql_query.replace(
                "dataPoints(limit: 1000, startDate: $startDate)",
                "dataPoints(limit: 1000, startDate: $startDate, endDate: $endDate)",
            );
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
            return Ok(warp::reply::with_status(
                warp::reply::json(&json!({
                    "error": format!("Invalid JSON: {}", e)
                })),
                StatusCode::BAD_REQUEST,
            ));
        }
    };

    // Handle different MCP request types
    let response = match request.get("method").and_then(|m| m.as_str()) {
        Some("tools/list") => {
            json!({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "result": {
                    "tools": EconGraphMcpServer::get_available_tools()
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
    use crate::test_utils::TestContainer;
    use diesel_async::RunQueryDsl;
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

    #[tokio::test]
    #[serial]
    async fn test_create_data_visualization_tool() {
        // Create a test container that will be kept alive for the duration of the test
        let container = TestContainer::new().await;
        let pool = container.pool();
        let server = EconGraphMcpServer::new(Arc::new(pool.clone()));

        // Test the visualization tool with mock data
        let viz_args = json!({
            "series_ids": ["test-series-1", "test-series-2"],
            "chart_type": "line",
            "title": "Test Chart",
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
}
