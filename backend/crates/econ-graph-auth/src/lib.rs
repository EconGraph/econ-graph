// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # EconGraph Auth
//!
//! Authentication, authorization, and user management for the `EconGraph` system.
//! This crate provides secure authentication and authorization services with enterprise-grade
//! security features, OAuth integration, and comprehensive user management capabilities.
//!
//! ## Features
//!
//! - **OAuth Integration**: Support for Google, GitHub, and other OAuth providers
//! - **JWT Authentication**: Secure token-based authentication with configurable expiration
//! - **Role-Based Access Control**: Granular permissions and role management
//! - **User Management**: Complete user lifecycle management and profile handling
//! - **Security Middleware**: Request authentication and authorization middleware
//! - **Session Management**: Secure session handling and token refresh
//!
//! ## Architecture
//!
//! This crate follows a security-first architecture:
//! - **Authentication**: OAuth flows, JWT token management, and user verification
//! - **Authorization**: Role-based access control and permission validation
//! - **Middleware**: Request authentication and authorization middleware
//! - **Services**: User management and authentication business logic
//!
//! ## Usage
//!
//! ```rust,no_run
//! use econ_graph_auth::{AuthService, AuthMiddleware};
//! use econ_graph_auth::auth::handlers::AuthHandlers;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize authentication service
//!     let auth_service = AuthService::new();
//!     let auth_middleware = AuthMiddleware::new(auth_service);
//!
//!     // Use for request authentication
//!     Ok(())
//! }
//! ```

pub mod auth;

// Re-export commonly used auth types
pub use auth::*;
