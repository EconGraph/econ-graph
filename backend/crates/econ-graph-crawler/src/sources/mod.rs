//! Per-site [`SourceAdapter`](crate::SourceAdapter) implementations.
//!
//! Each adapter lives in its own module and holds only site-specific knowledge
//! (URLs, response parsing, policy overrides). Shared concerns — HTTP, rate
//! limiting, retries, persistence, queueing — live elsewhere in this crate.
//!
//! To add a source: add `pub mod <name>;` below and one `registry.register(..)`
//! line in [`default_registry`].

use crate::adapter::AdapterRegistry;

// Adapter modules (one per source) are declared here.

/// Registry with every production adapter, pointed at the real upstream APIs.
pub fn default_registry() -> AdapterRegistry {
    #[allow(unused_mut)]
    let mut registry = AdapterRegistry::new();
    // registry.register(std::sync::Arc::new(<name>::XAdapter::default()));
    registry
}
