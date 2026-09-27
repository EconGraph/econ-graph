//! # GraphQL Module
//!
//! GraphQL schema, resolvers, and API logic for the EconGraph system.
//! This module provides the GraphQL API layer that bridges the core domain
//! models with the external API consumers.

pub mod context;
pub mod dataloaders;
pub mod global_analysis;
pub mod mutation;
pub mod query;
pub mod schema;

/// Serializes this crate's DB-backed tests, so migrations and table cleanups don't race.
#[cfg(test)]
pub(crate) static TEST_DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(test)]
mod annotation_visibility_tests;
#[cfg(test)]
mod authorization_tests;
#[cfg(test)]
pub mod n_plus_one_tests;

// Re-export commonly used types
pub use mutation::Mutation;
pub use query::Query;
pub use schema::{create_schema, create_schema_with_data, SchemaResources};
