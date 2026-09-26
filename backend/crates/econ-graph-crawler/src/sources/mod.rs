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
pub mod bea;
pub mod bls;
pub mod census;
pub mod fhfa;
pub mod fred;
pub mod imf;
pub mod static_catalogs;
pub mod world_bank;

/// Registry with every production adapter, pointed at the real upstream APIs.
pub fn default_registry() -> AdapterRegistry {
    #[allow(unused_mut)]
    let mut registry = AdapterRegistry::new();
    // registry.register(std::sync::Arc::new(<name>::XAdapter::default()));
    registry.register(std::sync::Arc::new(fred::FredAdapter::default()));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::default()));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::default()));
    registry.register(std::sync::Arc::new(imf::ImfAdapter::default()));
    registry.register(std::sync::Arc::new(census::CensusAdapter::default()));
    registry.register(std::sync::Arc::new(bea::BeaAdapter::default()));
    registry.register(std::sync::Arc::new(fhfa::FhfaAdapter::default()));
    // Static catalogs (no live integration): BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO.
    for adapter in static_catalogs::StaticCatalogAdapter::all() {
        registry.register(std::sync::Arc::new(adapter));
    }
    registry
}
