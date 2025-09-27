//! Iceberg Catalog Integration Module
//!
//! This module provides integration with Apache Iceberg's catalog system
//! for the financial data service, replacing our previous custom catalog implementation.
//!
//! ## Key Features
//!
//! - **Custom Partitioning**: Maintains our time-based partitioning scheme
//! - **Iceberg Catalog**: Uses Iceberg for metadata management and ACID transactions
//! - **Zero-Copy Access**: Preserves direct Parquet file access for performance
//!
//! ## Architecture
//!
//! ```
//! Financial Data Service
//! ├── Custom Time-Based Partitioning (year/month/day)
//! │   ├── year=2024/month=01/day=15/
//! │   └── series_*.parquet files
//! └── Iceberg Catalog Integration
//!     ├── Metadata Management
//!     ├── ACID Transactions
//!     └── Schema Evolution
//! ```

pub mod iceberg_catalog;

// Re-export the main types
pub use iceberg_catalog::{CatalogStats, IcebergCatalog, SeriesMetadata};
