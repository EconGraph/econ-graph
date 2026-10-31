//! Real Apache Iceberg observation storage with caller-transaction SQL publication.
//! Decimal values are exact nullable strings; objects and metadata are immutable. Preparing
//! a commit never changes the authoritative catalog pointer. Publication borrows the crawl
//! transaction so its existing lease fence, summaries, attempts and completion remain atomic.
pub mod catalog;
pub mod config;
pub mod encoding;
pub mod store;
pub use catalog::{CatalogCheckpoint, PreparedCommit, ReachableObject, SqlCatalog, TablePointer};
pub use config::StorageConfig;
pub use store::IcebergStore;
