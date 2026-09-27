//! Per-site [`SourceAdapter`](crate::SourceAdapter) implementations.
//!
//! Each adapter lives in its own module and holds only site-specific knowledge
//! (URLs, response parsing, policy overrides). Shared concerns — HTTP, rate
//! limiting, retries, persistence, queueing — live elsewhere in this crate.
//!
//! To add a source: add `pub mod <name>;` below and one `registry.register(..)`
//! line in each of [`default_registry`] and [`registry_at`].

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

/// Registry with every HTTP adapter pointed at `base_url` instead of its real upstream, plus the
/// static catalogs. For mock upstreams serving recorded fixtures (tests and `seed-fixtures`).
pub fn registry_at(base_url: &str) -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(std::sync::Arc::new(fred::FredAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::new(
        base_url,
    )));
    registry.register(std::sync::Arc::new(imf::ImfAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(census::CensusAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(bea::BeaAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(fhfa::FhfaAdapter::new(base_url)));
    for adapter in static_catalogs::StaticCatalogAdapter::all() {
        registry.register(std::sync::Arc::new(adapter));
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new adapter has to be added to both registries, or `seed-fixtures` can't load its fixtures.
    #[test]
    fn registry_at_covers_every_default_adapter() {
        assert_eq!(
            registry_at("http://127.0.0.1:1").ids(),
            default_registry().ids()
        );
    }
}
