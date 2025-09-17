// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph MCP
//!
//! Model Context Protocol server implementation for the EconGraph system.
//! This crate provides AI model integration for economic data access.

pub mod mcp_server;

// Re-export commonly used MCP types
pub use mcp_server::*;
