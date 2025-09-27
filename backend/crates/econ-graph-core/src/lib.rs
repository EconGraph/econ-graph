// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # `EconGraph` Core
//!
//! Core data models, database schema, and shared utilities for the `EconGraph` system.
//! This crate provides the foundation layer that other crates depend on.

pub mod auth_models;
pub mod config;
pub mod database;
pub mod enums;
pub mod error;
pub mod models;
pub mod schema;

pub mod test_utils;

// Re-export commonly used types
pub use config::Config;
pub use database::{create_pool, run_migrations, DatabasePool};
pub use error::{AppError, AppResult};

// Re-export all models for convenience
pub use models::*;
