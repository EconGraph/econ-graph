//! Storage module for Arrow-based financial data storage
//! 
//! This module provides implementations for storing financial data using
//! Arrow Flight, Parquet, and Iceberg formats.

// For now, we'll create stub implementations to get the crate structure working
// TODO: Add proper Arrow dependencies and implement the storage traits

pub mod parquet_storage;
pub mod iceberg_storage;
pub mod in_memory_storage;

// Re-export the main storage trait and implementations
pub use parquet_storage::{FinancialDataStorage, ParquetStorage};
pub use iceberg_storage::IcebergStorage;
pub use in_memory_storage::InMemoryStorage;