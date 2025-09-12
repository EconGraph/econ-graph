//! Simple test binary to demonstrate MCP server functionality
//! This can be run without the full server to test the MCP implementation

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use econ_graph_backend::database::create_pool;
use econ_graph_backend::mcp_server::EconGraphMcpServer;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing EconGraph MCP Server");

    // Create a test database pool (this will fail if DB isn't running, but that's OK for demo)
    let pool = match create_pool("postgres://user:password@localhost/econ_graph").await {
        Ok(pool) => {
            println!("✅ Database connection established");
            Arc::new(pool)
        }
        Err(e) => {
            println!("⚠️  Database connection failed: {}", e);
            println!("   This is expected if the database isn't running.");
            println!("   The MCP server can still demonstrate its API structure.");
            return Ok(());
        }
    };

    // Create MCP server instance
    let mcp_server = EconGraphMcpServer::new(pool);
    println!("✅ MCP Server created successfully");

    // Test available tools
    println!("\n🔧 Available Tools:");
    let tools = EconGraphMcpServer::get_available_tools();
    for (i, tool) in tools.iter().enumerate() {
        let name = tool
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let description = tool
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");
        println!("  {}. {} - {}", i + 1, name, description);
    }

    // Test available resources
    println!("\n📚 Available Resources:");
    let resources = EconGraphMcpServer::get_available_resources();
    for (i, resource) in resources.iter().enumerate() {
        let name = resource
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let description = resource
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("No description");
        let uri = resource
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown URI");
        println!("  {}. {} ({}) - {}", i + 1, name, uri, description);
    }

    // Test a sample tool call (this will fail without DB, but shows the structure)
    println!("\n🔍 Testing Tool Call Structure:");
    let sample_args = json!({
        "query": "GDP",
        "limit": 5
    });

    match mcp_server
        .handle_tool_call("search_economic_series", sample_args)
        .await
    {
        Ok(result) => {
            println!("✅ Tool call successful:");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Err(e) => {
            println!("⚠️  Tool call failed (expected without database): {}", e);
            println!("   This demonstrates the error handling structure.");
        }
    }

    println!("\n🎉 MCP Server test completed!");
    println!("\nThe MCP server provides the following capabilities:");
    println!("• Data Search: Find economic series by keywords");
    println!("• Data Retrieval: Get time series data points");
    println!("• Metadata Access: Get detailed series information");
    println!("• Data Visualization: Create charts and graphs");
    println!("• Resource Access: Browse data sources and catalogs");

    println!("\nTo use this MCP server with an AI client:");
    println!("1. Start the full backend server: cargo run");
    println!("2. The MCP endpoint will be available at: http://localhost:9876/mcp");
    println!("3. Use JSON-RPC 2.0 protocol to communicate");
    println!("4. Supported methods: tools/list, tools/call, resources/list, resources/read");

    Ok(())
}
