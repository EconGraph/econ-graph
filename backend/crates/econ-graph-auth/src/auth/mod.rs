/**
 * REQUIREMENT: OAuth authentication system for multi-user collaboration
 * PURPOSE: Provide secure authentication with Google and Facebook OAuth backends
 * This enables professional chart collaboration with proper user management
 */
pub mod handlers;
pub mod keycloak;
pub mod middleware;
pub mod routes;
pub mod services;
pub mod simple_test;
pub mod unified_auth;

// Re-export models from core
pub use econ_graph_core::auth_models as models;

#[cfg(test)]
pub mod integration_tests;

#[cfg(test)]
pub mod comprehensive_integration_tests;
