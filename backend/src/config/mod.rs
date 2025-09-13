//! Configuration management for the EconGraph backend
//!
//! This module provides centralized configuration management including
//! API keys, database settings, and other application configuration.

pub mod api_keys;
pub mod app_config;

pub use api_keys::ApiKeyConfig;
pub use app_config::Config;
