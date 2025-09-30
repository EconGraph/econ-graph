// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph MCP
//!
//! Model Context Protocol server implementation for the `EconGraph` system.
//! This crate provides AI model integration for economic data access, enabling
//! AI assistants to interact with economic data through a standardized protocol.
//!
//! ## Features
//!
//! - **MCP Server**: Full Model Context Protocol server implementation
//! - **AI Integration**: Seamless integration with AI models and assistants
//! - **Economic Data Access**: Secure access to economic data through AI interfaces
//! - **Protocol Compliance**: Full compliance with MCP specification
//! - **Authentication**: Secure authentication for AI model access
//! - **Data Filtering**: Intelligent data filtering and access control
//!
//! ## Architecture
//!
//! This crate implements the MCP protocol:
//! - **Server**: MCP server implementation with economic data endpoints
//! - **Protocol**: Full MCP protocol compliance and message handling
//! - **Integration**: AI model integration and data access control
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_mcp::McpServer;
//! use econ_graph_mcp::McpConfig;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize MCP server
//!     let config = McpConfig::new();
//!     let server = McpServer::new(config);
//!
//!     // Start MCP server
//!     server.start().await?;
//!     Ok(())
//! }
//! ```

pub mod mcp_server;

// Re-export commonly used MCP types
pub use mcp_server::*;
