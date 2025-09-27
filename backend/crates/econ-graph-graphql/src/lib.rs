// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph GraphQL
//!
//! GraphQL schema, resolvers, and API logic for the EconGraph system.
//! This crate provides the GraphQL API layer with enterprise-grade quality standards.
//!
//! # Architecture
//!
//! This crate follows a clean architecture pattern:
//! - **Types**: GraphQL-specific type definitions and conversions
//! - **Resolvers**: Query and mutation resolvers with proper error handling
//! - **Context**: Authentication and authorization context management
//! - **Imports**: Centralized import management for maintainability
//!
//! # Quality Standards
//!
//! - **Zero Compilation Errors**: All code must compile without warnings
//! - **Comprehensive Documentation**: Google-style documentation for all public APIs
//! - **Type Safety**: Strong typing with minimal use of `unwrap()` or `expect()`
//! - **Error Handling**: Proper error propagation and user-friendly error messages
//! - **Performance**: Optimized queries and efficient data loading
//!
//! # Examples
//!
//! ```rust,no_run
//! use econ_graph_graphql::imports::*;
//! use econ_graph_graphql::graphql::schema::create_schema;
//!
//! #[tokio::main]
//! async fn main() -> AppResult<()> {
//!     let pool = DatabasePool::new();
//!     let schema = create_schema(Arc::new(pool));
//!
//!     // Use schema for GraphQL operations
//!     Ok(())
//! }
//! ```

pub mod graphql;
pub mod imports;
pub mod security;
pub mod types;

// Re-export commonly used GraphQL types
pub use graphql::*;
