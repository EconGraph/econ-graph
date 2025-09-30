//! # EconGraph Backend
//!
//! The main backend application for the `EconGraph` system, providing the core server
//! infrastructure, metrics collection, and integration testing capabilities. This crate
//! serves as the primary backend service that orchestrates all other components.
//!
//! ## Features
//!
//! - **Server Infrastructure**: Core HTTP server with routing and middleware
//! - **Metrics Collection**: Application performance and health monitoring
//! - **Integration Testing**: Comprehensive end-to-end testing capabilities
//! - **Service Orchestration**: Coordination of all backend services and components
//! - **Health Monitoring**: System health checks and status reporting
//! - **Configuration Management**: Centralized configuration and environment handling
//!
//! ## Architecture
//!
//! This crate serves as the main application entry point:
//! - **Server**: HTTP server with GraphQL, REST, and WebSocket endpoints
//! - **Metrics**: Application metrics collection and monitoring
//! - **Integration**: End-to-end testing and validation
//! - **Orchestration**: Service coordination and lifecycle management
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_backend::metrics::MetricsCollector;
//! use econ_graph_backend::integration_tests::TestRunner;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize metrics collection
//!     let metrics = MetricsCollector::new();
//!
//!     // Start backend server
//!     Ok(())
//! }
//! ```

pub mod integration_tests;
pub mod metrics;

// Re-export commonly used types if needed
// pub use crate::metrics::*;
