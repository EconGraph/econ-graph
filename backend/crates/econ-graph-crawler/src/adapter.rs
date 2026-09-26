// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The per-source adapter contract and the registry the worker dispatches through.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::DatabasePool;

use crate::error::CrawlError;
use crate::http::HttpFetcher;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// API keys for sources that use them. `Debug` redacts the values.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApiKeys {
    /// FRED (`FRED_API_KEY`).
    pub fred: Option<String>,
    /// BLS (`BLS_API_KEY`).
    pub bls: Option<String>,
    /// BEA (`BEA_API_KEY`).
    pub bea: Option<String>,
    /// Census (`CENSUS_API_KEY`).
    pub census: Option<String>,
}

impl ApiKeys {
    /// Reads `FRED_API_KEY`, `BLS_API_KEY`, `BEA_API_KEY` and `CENSUS_API_KEY`.
    /// Unset, empty or whitespace-only variables become `None`; values are trimmed.
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let get = |name: &str| {
            lookup(name)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        Self {
            fred: get("FRED_API_KEY"),
            bls: get("BLS_API_KEY"),
            bea: get("BEA_API_KEY"),
            census: get("CENSUS_API_KEY"),
        }
    }

    /// The key for `source`, if that source has one configured.
    pub fn get(&self, source: SourceId) -> Option<&str> {
        match source {
            SourceId::Fred => self.fred.as_deref(),
            SourceId::Bls => self.bls.as_deref(),
            SourceId::Bea => self.bea.as_deref(),
            SourceId::Census => self.census.as_deref(),
            _ => None,
        }
    }
}

impl fmt::Debug for ApiKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = |k: &Option<String>| k.as_ref().map(|_| "<redacted>");
        f.debug_struct("ApiKeys")
            .field("fred", &r(&self.fred))
            .field("bls", &r(&self.bls))
            .field("bea", &r(&self.bea))
            .field("census", &r(&self.census))
            .finish()
    }
}

/// Everything an adapter needs to do its work. Cheap to clone.
#[derive(Clone)]
pub struct CrawlCtx {
    /// Shared rate-limited HTTP client.
    pub http: HttpFetcher,
    /// Database pool (for adapters that need lookups; persistence is done by the worker).
    pub pool: DatabasePool,
    /// Upstream API keys.
    pub keys: ApiKeys,
}

/// Series-level metadata returned alongside observations. Maps onto `economic_series` columns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewSeriesMetadataLite {
    /// Human-readable title.
    pub title: String,
    /// Longer description / notes.
    pub description: Option<String>,
    /// Units of measure, as the source states them.
    pub units: Option<String>,
    /// Observation frequency, as the source states it (e.g. `"Monthly"`).
    pub frequency: Option<String>,
    /// Seasonal adjustment, as the source states it (e.g. `"Seasonally Adjusted"`).
    pub seasonal_adjustment: Option<String>,
}

/// The result of fetching one series.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FetchedSeries {
    /// Updated metadata, if the source returned any.
    pub metadata: Option<NewSeriesMetadataLite>,
    /// Observations, in any order.
    pub points: Vec<FetchedPoint>,
}

/// One observation of a series.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedPoint {
    /// Observation date (period start for non-daily frequencies).
    pub date: NaiveDate,
    /// Value; `None` for a missing observation (e.g. FRED's `"."`).
    pub value: Option<BigDecimal>,
    /// Date this vintage of the value was published.
    pub revision_date: NaiveDate,
    /// Whether this is the first published value for `date`.
    pub is_original_release: bool,
}

/// A series found during catalog discovery, to be upserted and enqueued for fetching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSeries {
    /// The source's identifier for the series (becomes `crawl_queue.series_id`).
    pub external_id: String,
    /// Human-readable title.
    pub title: String,
    /// Longer description.
    pub description: Option<String>,
    /// Units of measure.
    pub units: Option<String>,
    /// Observation frequency.
    pub frequency: Option<String>,
    /// Link to the series on the source's website or API.
    pub data_url: Option<String>,
}

/// One external data source. Implementations are stateless apart from configuration and
/// do all HTTP through [`CrawlCtx::http`].
#[async_trait]
pub trait SourceAdapter: Send + Sync {
    /// Which source this adapter handles.
    fn id(&self) -> SourceId;

    /// Rate/retry policy for this source.
    fn policy(&self) -> SourcePolicy {
        SourcePolicy::default_for(self.id())
    }

    /// Lists the series this source offers.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError>;

    /// Fetches observations for `external_id`, only those on or after `since` when given
    /// (adapters may return more if the source can't filter).
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError>;
}

/// Maps each [`SourceId`] to its adapter.
#[derive(Clone, Default)]
pub struct AdapterRegistry {
    adapters: HashMap<SourceId, Arc<dyn SourceAdapter>>,
}

impl fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("sources", &self.ids())
            .finish()
    }
}

impl AdapterRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `adapter` under `adapter.id()`, returning the adapter it replaced, if any.
    pub fn register(&mut self, adapter: Arc<dyn SourceAdapter>) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters.insert(adapter.id(), adapter)
    }

    /// The adapter for `source`, if registered.
    pub fn get(&self, source: SourceId) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters.get(&source).cloned()
    }

    /// Registered sources, sorted.
    pub fn ids(&self) -> Vec<SourceId> {
        let mut ids: Vec<_> = self.adapters.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy(SourceId, &'static str);

    #[async_trait]
    impl SourceAdapter for Dummy {
        fn id(&self) -> SourceId {
            self.0
        }
        async fn discover(&self, _: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
            Err(CrawlError::Permanent(self.1.into()))
        }
        async fn fetch_series(
            &self,
            _: &CrawlCtx,
            _: &str,
            _: Option<NaiveDate>,
        ) -> Result<FetchedSeries, CrawlError> {
            Ok(FetchedSeries::default())
        }
    }

    #[test]
    fn registry_register_get_ids() {
        let mut r = AdapterRegistry::new();
        assert!(r.ids().is_empty());
        assert!(r.register(Arc::new(Dummy(SourceId::Sec, "a"))).is_none());
        assert!(r.register(Arc::new(Dummy(SourceId::Fred, "b"))).is_none());
        assert_eq!(r.ids(), vec![SourceId::Fred, SourceId::Sec]);
        assert_eq!(r.get(SourceId::Fred).unwrap().id(), SourceId::Fred);
        assert!(r.get(SourceId::Bls).is_none());
    }

    #[test]
    fn registry_replaces_same_source() {
        let mut r = AdapterRegistry::new();
        r.register(Arc::new(Dummy(SourceId::Fred, "old")));
        assert!(r.register(Arc::new(Dummy(SourceId::Fred, "new"))).is_some());
        assert_eq!(r.ids(), vec![SourceId::Fred]);
    }

    #[test]
    fn default_adapter_policy_is_source_default() {
        let d = Dummy(SourceId::Bea, "");
        assert_eq!(d.policy(), SourcePolicy::default_for(SourceId::Bea));
    }

    #[test]
    fn api_keys_from_lookup() {
        let keys = ApiKeys::from_lookup(|n| match n {
            "FRED_API_KEY" => Some(" abc ".into()),
            "BLS_API_KEY" => Some("   ".into()),
            "BEA_API_KEY" => Some(String::new()),
            _ => None,
        });
        assert_eq!(keys.fred.as_deref(), Some("abc"));
        assert_eq!(keys.bls, None);
        assert_eq!(keys.bea, None);
        assert_eq!(keys.census, None);
        assert_eq!(keys.get(SourceId::Fred), Some("abc"));
        assert_eq!(keys.get(SourceId::Sec), None);
    }

    #[test]
    fn api_keys_debug_redacts() {
        let keys = ApiKeys {
            fred: Some("supersecret".into()),
            ..ApiKeys::default()
        };
        let s = format!("{keys:?}");
        assert!(!s.contains("supersecret"));
        assert!(s.contains("<redacted>"));
    }
}
