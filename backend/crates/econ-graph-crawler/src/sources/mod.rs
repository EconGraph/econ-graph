//! Per-site [`SourceAdapter`](crate::SourceAdapter) implementations.
//!
//! Each adapter lives in its own module and holds only site-specific knowledge
//! (URLs, response parsing, policy overrides). Shared concerns — HTTP, rate
//! limiting, retries, persistence, queueing — live elsewhere in this crate.
//!
//! To add a source: add `pub mod <name>;` below and one `registry.register(..)`
//! line in each of [`default_registry`] and [`registry_at`].
//!
//! `SourceId::Imf` has no adapter (its series ids were made up); it is kept for a future SDMX
//! adapter.

use crate::adapter::{AdapterRegistry, ApiKeys};
use crate::source::SourceId;

// Adapter modules (one per source) are declared here.
pub mod bea;
pub mod bls;
pub mod boc;
pub mod census;
pub mod fhfa;
pub mod fred;
pub mod static_catalogs;
pub mod world_bank;
pub mod wto;

/// Sources the crawler worker leaves out entirely while their API key is unset, logging one
/// warning each at startup ([`registry_for_keys`]): no discovery, refreshes or coverage gauges,
/// rather than jobs that can only fail. For sources that cannot be used at all without a key and
/// whose key a deployment may not have yet. Other keyed sources (FRED, BEA, Census) stay
/// registered without their key, so their failed jobs and coverage alerts flag the gap.
pub const SKIPPED_WITHOUT_KEY: &[SourceId] = &[SourceId::Wto];

/// [`default_registry`] minus the [`SKIPPED_WITHOUT_KEY`] sources whose key `keys` lacks, each
/// logged as skipped with the environment variable that would enable it. The crawler worker's
/// registry.
pub fn registry_for_keys(keys: &ApiKeys) -> AdapterRegistry {
    let mut registry = default_registry();
    for &source in SKIPPED_WITHOUT_KEY {
        if keys.get(source).is_none() && registry.remove(source).is_some() {
            tracing::warn!(
                source = %source,
                env = crate::cli::api_key_env_var(source).unwrap_or("?"),
                "source skipped: its API key is not set, so it is not crawled, scheduled or \
                 counted in coverage; set the key and restart the worker to enable it"
            );
        }
    }
    registry
}

/// Registry with every production adapter, pointed at the real upstream APIs, plus the
/// development-only static catalogs when the `static_catalogs` build flag is on
/// (config/flags/README.md): on by default for the `dev` profile, off for `release`.
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    // registry.register(std::sync::Arc::new(<name>::XAdapter::default()));
    registry.register(std::sync::Arc::new(fred::FredAdapter::default()));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::default()));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::default()));
    registry.register(std::sync::Arc::new(census::CensusAdapter::default()));
    registry.register(std::sync::Arc::new(bea::BeaAdapter::default()));
    registry.register(std::sync::Arc::new(fhfa::FhfaAdapter::default()));
    registry.register(std::sync::Arc::new(wto::WtoAdapter::default()));
    registry.register(std::sync::Arc::new(boc::BocAdapter::default()));
    // Development only: hardcoded catalogs without live integrations.
    #[cfg(flag_static_catalogs)]
    for adapter in static_catalogs::StaticCatalogAdapter::all() {
        registry.register(std::sync::Arc::new(adapter));
    }
    registry
}

/// Registry with every HTTP adapter pointed at `base_url` instead of its real upstream, plus the
/// static catalogs. For mock upstreams serving recorded fixtures (tests and `seed-fixtures`).
/// Census reads its state file from `{base_url}/geo/state.txt`, not from www2.census.gov.
///
/// Unlike [`default_registry`], the static catalogs are unconditional here regardless of the
/// `static_catalogs` flag: `seed-fixtures` already requires the dev-only `testkit` feature, so
/// there is no release build of it to keep them out of. `SourceId::Imf` has no adapter to
/// register (see the module docs).
pub fn registry_at(base_url: &str) -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register(std::sync::Arc::new(fred::FredAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(bls::BlsAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(world_bank::WorldBankAdapter::new(
        base_url,
    )));
    registry.register(std::sync::Arc::new(
        census::CensusAdapter::new(base_url).with_states_url(format!("{base_url}/geo/state.txt")),
    ));
    registry.register(std::sync::Arc::new(bea::BeaAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(fhfa::FhfaAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(wto::WtoAdapter::new(base_url)));
    registry.register(std::sync::Arc::new(boc::BocAdapter::new(base_url)));
    for adapter in static_catalogs::StaticCatalogAdapter::all() {
        registry.register(std::sync::Arc::new(adapter));
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

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
                SourceId::Boc,
                SourceId::Wto,
                SourceId::Fhfa,
            ]
        );
    }

    /// Without its key a [`SKIPPED_WITHOUT_KEY`] source is left out of the worker's registry;
    /// with it, the registry is the default one. Other keyed sources stay either way.
    #[test]
    fn registry_for_keys_skips_keyless_optional_sources() {
        let none = registry_for_keys(&ApiKeys::default());
        assert!(none.get(SourceId::Wto).is_none());
        assert!(
            none.get(SourceId::Fred).is_some(),
            "FRED is kept without its key"
        );
        assert!(
            none.get(SourceId::Census).is_some(),
            "Census is kept without its key"
        );
        let with = registry_for_keys(&ApiKeys {
            wto: Some("k".into()),
            ..ApiKeys::default()
        });
        assert_eq!(with.ids(), default_registry().ids());
        for &s in SKIPPED_WITHOUT_KEY {
            assert!(
                crate::cli::api_key_env_var(s).is_some(),
                "{s} needs an env var to be enabled by"
            );
        }
    }

    #[test]
    fn static_catalogs_are_registered_per_the_flag() {
        let ids = default_registry().ids();
        let expected = cfg!(flag_static_catalogs);
        for s in static_catalogs::STATIC_CATALOG_SOURCES {
            assert_eq!(ids.contains(&s), expected, "{s}");
        }
    }

    /// `seed-fixtures` refreshes an adapter's reference data only when every code list points at
    /// its mock, so a reference-data URL `registry_at` leaves at the real upstream silently goes
    /// unseeded. BLS is the known exception: its code files are fixed URLs on download.bls.gov.
    #[test]
    fn registry_at_points_reference_data_at_the_base_url() {
        let base = "http://127.0.0.1:1";
        let registry = registry_at(base);
        for id in registry.ids() {
            if id == SourceId::Bls {
                continue;
            }
            let adapter = registry.get(id).unwrap();
            for list in adapter.code_lists(&crate::testkit::test_keys()) {
                assert!(
                    list.request_url().starts_with(base),
                    "{id}: {}",
                    list.request_url()
                );
            }
        }
    }

    /// A new HTTP adapter has to be added to both registries, or `seed-fixtures` can't load its
    /// fixtures. Equal only when the `static_catalogs` flag is on (the dev profile):
    /// `registry_at` always carries the static catalogs, `default_registry` only then.
    #[cfg(flag_static_catalogs)]
    #[test]
    fn registry_at_covers_every_default_adapter() {
        assert_eq!(
            registry_at("http://127.0.0.1:1").ids(),
            default_registry().ids()
        );
    }
}
