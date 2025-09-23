use once_cell::sync::Lazy;
pub use prometheus;
use prometheus::Registry;
use std::sync::Arc;

pub mod crawler;

/// Shared default registry used across crates
pub static DEFAULT_REGISTRY: Lazy<Arc<Registry>> =
    Lazy::new(|| Arc::new(prometheus::default_registry().clone()));
