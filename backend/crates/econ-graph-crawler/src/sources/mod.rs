//! Per-site [`SourceAdapter`](crate::SourceAdapter) implementations.
//!
//! Each adapter lives in its own module and holds only site-specific knowledge
//! (URLs, response parsing, policy overrides). Shared concerns — HTTP, rate
//! limiting, retries, persistence, queueing — live elsewhere in this crate.
//!
//! To add a source: add `pub mod <name>;` below and one `registry.register(..)`
//! line in each of [`default_registry`] and [`registry_at`].
//!
//! `SourceId::Imf` has no adapter (its series ids were made up); it is kept for the SDMX work
//! in release train 3.

use crate::adapter::AdapterRegistry;

// Adapter modules (one per source) are declared here.
pub mod bea;
pub mod bls;
pub mod census;
pub mod fhfa;
pub mod fred;
pub mod static_catalogs;
pub mod world_bank;

/// Registry with every production adapter, pointed at the real upstream APIs, plus the
/// development-only static catalogs in a dev build (`debug_assertions`) or with the
/// `static-catalogs` feature explicitly on (for a `--release` build that still wants them, or a
/// `--release` test run).
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    // registry.register(std::sync::Arc::new(<name>::XAdapter::default()));
    registry.register(std::sync::Arc::new(fred::FredAdapter::default()));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::default()));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::default()));
    registry.register(std::sync::Arc::new(census::CensusAdapter::default()));
    registry.register(std::sync::Arc::new(bea::BeaAdapter::default()));
    registry.register(std::sync::Arc::new(fhfa::FhfaAdapter::default()));
    // Development only: hardcoded catalogs without live integrations. `--release` (the Dockerfile
    // and every deployment) leaves debug_assertions off, so this only needs the feature there;
    // a plain `cargo build`/`cargo test` (debug profile) gets them without any flag.
    #[cfg(any(feature = "static-catalogs", debug_assertions))]
    for adapter in static_catalogs::StaticCatalogAdapter::all() {
        registry.register(std::sync::Arc::new(adapter));
    }
    registry
}

/// Registry with every HTTP adapter pointed at `base_url` instead of its real upstream, plus the
/// static catalogs. For mock upstreams serving recorded fixtures (tests and `seed-fixtures`).
///
/// Unlike [`default_registry`], the static catalogs are unconditional here: `seed-fixtures`
/// already requires the dev-only `testkit` feature, so there is no release build of it to keep
/// them out of. `SourceId::Imf` has no adapter to register (see the module docs).
pub fn registry_at(base_url: &str) -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(std::sync::Arc::new(fred::FredAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::new(
        base_url,
    )));
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
    use crate::source::SourceId;

    #[test]
    fn default_registry_holds_exactly_the_live_sources() {
        let live: Vec<SourceId> = default_registry()
            .ids()
            .into_iter()
            .filter(|&s| !static_catalogs::is_static_catalog_source(s))
            .collect();
        assert_eq!(
            live,
            vec![
                SourceId::Fred,
                SourceId::Bls,
                SourceId::Bea,
                SourceId::Census,
                SourceId::WorldBank,
                SourceId::Fhfa,
            ]
        );
    }

    #[test]
    fn static_catalogs_are_registered_in_dev_or_with_the_feature() {
        let ids = default_registry().ids();
        let expected = cfg!(feature = "static-catalogs") || cfg!(debug_assertions);
        for s in static_catalogs::STATIC_CATALOG_SOURCES {
            assert_eq!(ids.contains(&s), expected, "{s}");
        }
    }

    /// A new HTTP adapter has to be added to both registries, or `seed-fixtures` can't load its
    /// fixtures. Equal only under `--all-features`: `registry_at` always carries the static
    /// catalogs, `default_registry` only with the `static-catalogs` feature.
    #[cfg(feature = "static-catalogs")]
    #[test]
    fn registry_at_covers_every_default_adapter() {
        assert_eq!(
            registry_at("http://127.0.0.1:1").ids(),
            default_registry().ids()
        );
    }
}
