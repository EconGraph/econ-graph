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

use crate::dataset::SeriesDataset;
use crate::discovery_run::{ChunkDiscovery, ChunkedDiscovery, DiscoveryUnit, Enumeration};
use crate::error::CrawlError;
use crate::http::{HttpFetcher, Validators};
use crate::policy::SourcePolicy;
use crate::reference_file::{self, CodeList};
use crate::source::SourceId;
use crate::units::{StructuredUnit, UnitsContext, UnitsSource};

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
    /// WTO Timeseries API subscription key (`WTO_API_KEY`).
    pub wto: Option<String>,
}

impl ApiKeys {
    /// Reads `FRED_API_KEY`, `BLS_API_KEY`, `BEA_API_KEY`, `CENSUS_API_KEY` and `WTO_API_KEY`.
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
            wto: get("WTO_API_KEY"),
        }
    }

    /// The key for `source`, if that source has one configured.
    pub fn get(&self, source: SourceId) -> Option<&str> {
        match source {
            SourceId::Fred => self.fred.as_deref(),
            SourceId::Bls => self.bls.as_deref(),
            SourceId::Bea => self.bea.as_deref(),
            SourceId::Census => self.census.as_deref(),
            SourceId::Wto => self.wto.as_deref(),
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
            .field("wto", &r(&self.wto))
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
    /// When true, adapters ignore the validators stored with each series (`Last-Modified`,
    /// `ETag`, content hashes, source-reported versions) and fetch in full, as if the series had
    /// never been fetched. Set for `refetch_series` jobs (see [`crate::refetch`]) and
    /// `crawler fetch --full`; adapters still store the new validators they get back.
    pub bypass_validators: bool,
}

/// Series-level metadata returned alongside observations. Maps onto `economic_series` columns.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewSeriesMetadataLite {
    /// Human-readable title. Empty when the fetch carried none (BLS without a key): the stored
    /// series then keeps its discovered or existing title, as with the `None` fields below.
    pub title: String,
    /// Longer description / notes.
    pub description: Option<String>,
    /// Units of measure, as the source states them, describing the values as stored.
    pub units: Option<String>,
    /// Where `units` came from. The default, [`UnitsSource::Structured`], is a field of the
    /// source's own that holds units. Persisting stores [`UnitsSource::Unknown`] when there are
    /// no units.
    pub units_source: UnitsSource,
    /// The source's units text exactly as received, when `units` rewords it (BEA drops the
    /// scale that `UNIT_MULT` applied) or was parsed out of it. `None` means `units` is that
    /// text.
    pub units_raw: Option<String>,
    /// What the source's structured fields state about the stored values' units, beyond the
    /// text (BEA: the scale, since values are multiplied by `10^UNIT_MULT`). Compared with the
    /// parsed `units`, and preferred to it, when the series is stored (see
    /// [`units::check_and_record`](crate::units::check_and_record)).
    pub structured_unit: Option<Box<StructuredUnit>>,
    /// What the parser may assume about this source's units texts: the currency a bare
    /// `dollars` or `$` means ([`UnitsContext::for_source`]). Text that
    /// names its currency ignores it.
    pub units_context: UnitsContext,
    /// Observation frequency, as the source states it (e.g. `"Monthly"`).
    pub frequency: Option<String>,
    /// Seasonal adjustment, as the source states it (e.g. `"Seasonally Adjusted"`).
    pub seasonal_adjustment: Option<String>,
}

/// The result of fetching one series.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedSeries {
    /// Updated metadata, if the source returned any.
    pub metadata: Option<NewSeriesMetadataLite>,
    /// Observations, in any order.
    pub points: Vec<FetchedPoint>,
    /// The series' dataset and dimension values. Every series belongs to a dataset the adapter
    /// declares in [`SourceAdapter::datasets`].
    pub dataset: SeriesDataset,
    /// What the next fetch of this series can send or compare to skip unchanged data (see
    /// [`HttpFetcher::get_text_if_changed`]). Stored with the points, in the same transaction,
    /// so a validator is never kept for data that wasn't written; read back with
    /// [`persist::stored_fetch_state`](crate::persist::stored_fetch_state). `None` clears any
    /// stored validators: the next fetch is a full one.
    pub validators: Option<Validators>,
}

impl FetchedSeries {
    /// The result for a series the source reports unchanged since its last stored fetch: no
    /// points and no metadata, so persisting it only marks the series crawled (keeping its
    /// stored title, units and points) and stores `validators`. `dataset` is the series' stored
    /// dataset ([`persist::stored_fetch_state`](crate::persist::stored_fetch_state)), so the
    /// adapter needn't re-derive it from data it skipped.
    pub fn unchanged(dataset: SeriesDataset, validators: Validators) -> Self {
        Self {
            metadata: None,
            points: Vec::new(),
            dataset,
            validators: Some(validators),
        }
    }
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
    /// The series' dataset and dimension values (see [`FetchedSeries::dataset`]).
    pub dataset: SeriesDataset,
}

/// Outcome of [`SourceAdapter::discover_if_changed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// The source's catalog hasn't changed since the last discovery that stored a validator: the
    /// worker writes nothing and retires nothing.
    Unchanged,
    /// The catalog as [`SourceAdapter::discover`] lists it. `validator`, when given, is the
    /// `(url, validators)` pair the next discovery compares against; the worker stores it (see
    /// [`persist::url_validators`](crate::persist::url_validators)) in the same transaction as
    /// the discovered series.
    Changed {
        found: Vec<DiscoveredSeries>,
        validator: Option<(String, Validators)>,
    },
}

/// Per-series results of [`SourceAdapter::fetch_batch`], keyed by external id.
pub type BatchFetch = HashMap<String, Result<FetchedSeries, CrawlError>>;

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

    /// Codes of the datasets this adapter writes, each defined in the source's
    /// `datasets/<source>.toml` (see [`crate::dataset`]). At least one: every series belongs to
    /// a dataset, and every series the adapter returns must use one of these codes and the
    /// definition's dimension keys.
    fn datasets(&self) -> &[&str];

    /// Lists the series this source offers.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError>;

    /// The source's own files of codes and labels behind this adapter's dataset dimensions (a
    /// state list, a code file per field, ...), as opposed to the series `discover` returns.
    /// The default [`refresh_reference_data`](Self::refresh_reference_data) keeps them current,
    /// and `crawler record-reference-seeds` records them into a seed migration for new databases
    /// (see [`crate::reference_file`]). Empty by default.
    /// `keys` is for a source that wants its API key in the request URL
    /// ([`CodeList::request_url`]).
    fn code_lists(&self, _keys: &ApiKeys) -> Vec<CodeList> {
        Vec::new()
    }

    /// Refreshes this adapter's own reference data so its labels don't go stale as the source
    /// adds or renames codes. Called at worker startup and once per scheduled catalog discovery,
    /// before [`discover_if_changed`](Self::discover_if_changed).
    ///
    /// The default refreshes each of [`code_lists`](Self::code_lists) with a conditional GET
    /// ([`reference_file::refresh_code_lists`]), so a source that hasn't changed a file costs one
    /// small request, not a re-parse. Override it for reference data that isn't a dimension's
    /// code list, going through [`reference_file::refresh`]. Failure here does not fail the
    /// discovery job: the worker logs it and carries on with whatever labels are already stored.
    async fn refresh_reference_data(&self, ctx: &CrawlCtx) -> Result<(), CrawlError> {
        reference_file::refresh_code_lists(ctx, self.id(), &self.code_lists(&ctx.keys)).await
    }

    /// Discovery that can skip an unchanged catalog. The worker calls this, not
    /// [`discover`](Self::discover). The default always discovers, with no validator. An adapter
    /// that wraps another must forward this, or the wrapped adapter's check is bypassed.
    ///
    /// An adapter whose catalog comes from a resource it can check cheaply (a conditional GET, a
    /// source-reported version) overrides this: it reads the validator it stored last time with
    /// [`persist::url_validators`](crate::persist::url_validators), returns
    /// [`Discovery::Unchanged`] when the source confirms nothing changed, and otherwise returns
    /// the full list with the validator to store.
    async fn discover_if_changed(&self, ctx: &CrawlCtx) -> Result<Discovery, CrawlError> {
        Ok(Discovery::Changed {
            found: self.discover(ctx).await?,
            validator: None,
        })
    }

    /// Whether a successful [`discover`](Self::discover) lists every series this adapter crawls,
    /// so a series it no longer lists has been retired by the source. The worker then marks such
    /// series inactive (see [`persist::retire_unlisted`](crate::persist::retire_unlisted)); they
    /// are never deleted. Defaults to `false`, for discoveries that return a sample (FRED by
    /// popularity, World Bank by topic) or tolerate partial failure. A
    /// [`Discovery::Unchanged`] result retires nothing: the last changed discovery already did.
    ///
    /// It must not depend on what a walk found: for an adapter that
    /// [discovers in chunks](Self::chunked_discovery), the worker calls it at finalize, which may
    /// run on a worker that walked nothing.
    fn discovery_is_complete(&self) -> bool {
        false
    }

    /// Whether confirmed upstream NotFound results can accumulate publication lifecycle
    /// evidence. Opt in only when this adapter implements scoped catalog reconciliation,
    /// which can later reactivate a recovered series. This capability is independent of
    /// whether a worker has already cached its first successful catalog.
    fn tracks_catalog_lifecycle(&self) -> bool {
        false
    }

    /// Whether every history fetch checks the upstream restriction marker and refuses a
    /// marked series with [`CrawlError::Restricted`](crate::CrawlError::Restricted), so that a
    /// successful fetch is evidence no marker is present. Only then can an operator's review
    /// of a restriction (`crawler clear-restriction`) publish the series again.
    fn checks_restriction_markers(&self) -> bool {
        false
    }

    /// Verified publication facts from the last complete successful catalog. Facts are kept
    /// separate from fetch health, so a successful history fetch cannot revive a retired id.
    fn catalog_lifecycle(&self) -> crate::lifecycle::CatalogLifecycle {
        crate::lifecycle::CatalogLifecycle::default()
    }

    /// Retained source ids positively matched by the current declarative rules, including
    /// tables removed before rules-driven discovery was introduced. Called by the worker
    /// only after a complete successful scoped discovery. It must never return every stored
    /// source id without applying the dataset, family, frequency and exclusion rules.
    async fn catalog_scope_members(&self, _ctx: &CrawlCtx) -> Result<Vec<String>, CrawlError> {
        Ok(Vec::new())
    }

    /// Stable fingerprint of the declarative catalog selection rules. Only adapters that
    /// return this opt into conservative publication lifecycle tracking. Changing the rules
    /// establishes a new baseline rather than treating deselected series as upstream removals.
    fn catalog_scope(&self) -> Option<String> {
        None
    }

    /// Curated or configured entries (BEA tables, FRED series ids, ...) that the last successful
    /// [`discover`](Self::discover) on this adapter instance skipped because the source no longer
    /// lists them, each named as the skip warning names it. The worker reads this after each
    /// changed discovery, publishes its length as `crawler_curated_missing{source}`, and retires
    /// nothing for a discovery that skipped entries, so the stored series of an entry the source
    /// dropped or renamed stay as they are. Defaults to none, for adapters with no curated list.
    fn curated_missing(&self) -> Vec<String> {
        Vec::new()
    }

    /// Restricts [`discovery_is_complete`](Self::discovery_is_complete) retirement to
    /// `external_id`s starting with this prefix, when this adapter isn't the only thing that
    /// writes series under its [`SourceId`] (for example Census, whose BDS discovery is complete
    /// but whose data source also holds ACS series seeded outside the crawler). `None` (the
    /// default) scopes retirement to the whole source, which is correct whenever the adapter owns
    /// it exclusively.
    fn retirement_scope_prefix(&self) -> Option<&str> {
        None
    }

    /// Opts this adapter into chunked catalog discovery (see [`crate::discovery_run`]): the
    /// `discover_catalog` job calls [`enumerate_discovery`](Self::enumerate_discovery) instead of
    /// [`discover_if_changed`](Self::discover_if_changed), each chunk of
    /// [`ChunkedDiscovery::units_per_chunk`] units becomes its own `discover_chunk` job that calls
    /// [`discover_units`](Self::discover_units), and once every chunk has succeeded one
    /// `finalize_discovery` job commits the concatenated results as a single discovery would.
    /// `None`, the default, keeps single-job discovery. An adapter that wraps another must
    /// forward this and both methods.
    ///
    /// An adapter that tracks publication lifecycle returns its catalog scope in
    /// [`ChunkedDiscovery::catalog_scope`], computed without a walk, and reports its publication
    /// facts with its units and chunks ([`Enumeration::Units`], [`ChunkDiscovery::lifecycle`])
    /// rather than through [`catalog_scope`](Self::catalog_scope) and
    /// [`catalog_lifecycle`](Self::catalog_lifecycle): a run's finalize may run on a worker that
    /// walked nothing, so it reads them from the run.
    fn chunked_discovery(&self) -> Option<ChunkedDiscovery> {
        None
    }

    /// Lists the work units of a chunked discovery: the cheap listing step (one request for a
    /// table list, a curated id list, ...). Each unit carries whatever the listing already
    /// returned for it, so [`discover_units`](Self::discover_units) needn't fetch it again.
    /// Called only when [`chunked_discovery`](Self::chunked_discovery) is `Some`.
    async fn enumerate_discovery(&self, ctx: &CrawlCtx) -> Result<Enumeration, CrawlError> {
        let _ = ctx;
        Err(CrawlError::Permanent(format!(
            "{} does not support chunked discovery",
            self.id()
        )))
    }

    /// The details of one chunk of units from [`enumerate_discovery`](Self::enumerate_discovery),
    /// in unit order. Concatenating every chunk's `found`, in chunk order, must give what
    /// [`discover`](Self::discover) returns for the same catalog. Called by `discover_chunk` jobs,
    /// each retried on its own under the source's retry policy, so an error should describe this
    /// chunk only.
    async fn discover_units(
        &self,
        ctx: &CrawlCtx,
        units: &[DiscoveryUnit],
    ) -> Result<ChunkDiscovery, CrawlError> {
        let _ = (ctx, units);
        Err(CrawlError::Permanent(format!(
            "{} does not support chunked discovery",
            self.id()
        )))
    }

    /// Fetches observations for `external_id`, only those on or after `since` when given
    /// (adapters may return more if the source can't filter).
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError>;

    /// Whether this source publishes vintages (each observation's revisions, with real
    /// publication dates). The worker then passes the newest stored `revision_date` to
    /// [`fetch_series_incremental`](Self::fetch_series_incremental).
    fn tracks_vintages(&self) -> bool {
        false
    }

    /// Incremental fetch for a series already stored. `since` is as for
    /// [`fetch_series`](Self::fetch_series). `known_vintage` is the newest `revision_date` already
    /// stored for the series, given only when [`tracks_vintages`](Self::tracks_vintages) is true:
    /// every vintage published on or before it is stored, so the adapter asks only for later ones.
    ///
    /// The default ignores `known_vintage`. An adapter that wraps another must forward this and
    /// [`tracks_vintages`](Self::tracks_vintages), or the wrapped adapter's vintage handling is
    /// bypassed.
    async fn fetch_series_incremental(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
        known_vintage: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let _ = known_vintage;
        self.fetch_series(ctx, external_id, since).await
    }

    /// Groups series that one upstream request can fetch together: the worker only batches
    /// `fetch_series` jobs whose ids return the same key (up to the policy's
    /// [`max_batch`](SourcePolicy::max_batch)). `None`, the default, never batches the series.
    fn batch_key(&self, _external_id: &str) -> Option<String> {
        None
    }

    /// Fetches several series that share a [`batch_key`](Self::batch_key), only observations on
    /// or after `since` when given (the earliest `since` of the batch's series).
    ///
    /// `Err` means the whole request failed and applies to every series in the batch: return it
    /// only for source-wide failures (rate limits, auth, a transport or server error), since a
    /// non-retryable `Err` (`Parse`, `NotFound`, `Permanent`) fails every job in the batch
    /// permanently. `Ok` carries one result per requested id; an id missing from the map fails
    /// as [`CrawlError::NotFound`] and ids that weren't requested are ignored.
    ///
    /// The default calls [`fetch_series`](Self::fetch_series) once per id, in order, each with the
    /// batch's `since` (so a more recent series may re-fetch some history). After the first
    /// `RateLimited` it stops calling upstream and gives the remaining ids that (retryable)
    /// error, keeping the results already fetched. Other errors, `Auth` included, stay with
    /// their own series.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let mut out = BatchFetch::with_capacity(external_ids.len());
        let mut refused: Option<CrawlError> = None;
        for id in external_ids {
            let fetched = match &refused {
                Some(e) => Err(e.clone()),
                None => self.fetch_series(ctx, id, since).await,
            };
            if let Err(e @ CrawlError::RateLimited { .. }) = &fetched {
                refused.get_or_insert_with(|| e.clone());
            }
            out.insert(id.clone(), fetched);
        }
        Ok(out)
    }
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

    /// Unregisters `source`, returning its adapter, if it had one.
    pub fn remove(&mut self, source: SourceId) -> Option<Arc<dyn SourceAdapter>> {
        self.adapters.remove(&source)
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

    /// A fetch with no metadata and no points, in the dimensionless `test` dataset.
    fn empty_series() -> FetchedSeries {
        FetchedSeries {
            metadata: None,
            points: Vec::new(),
            dataset: SeriesDataset::new("test", Vec::<(String, String)>::new()),
            validators: None,
        }
    }

    #[async_trait]
    impl SourceAdapter for Dummy {
        fn id(&self) -> SourceId {
            self.0
        }
        fn datasets(&self) -> &[&str] {
            &["test"]
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
            Ok(empty_series())
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
        assert_eq!(r.remove(SourceId::Fred).unwrap().id(), SourceId::Fred);
        assert!(r.remove(SourceId::Fred).is_none());
        assert_eq!(r.ids(), vec![SourceId::Sec]);
    }

    #[test]
    fn registry_replaces_same_source() {
        let mut r = AdapterRegistry::new();
        r.register(Arc::new(Dummy(SourceId::Fred, "old")));
        assert!(r.register(Arc::new(Dummy(SourceId::Fred, "new"))).is_some());
        assert_eq!(r.ids(), vec![SourceId::Fred]);
    }

    #[test]
    fn default_adapter_does_not_batch() {
        assert_eq!(Dummy(SourceId::Bea, "").batch_key("X"), None);
    }

    #[tokio::test]
    async fn default_fetch_batch_calls_fetch_series_per_id() {
        let ctx = crate::testkit::test_ctx();
        let ids = vec!["A".to_string(), "B".to_string()];
        let out = Dummy(SourceId::Bea, "")
            .fetch_batch(&ctx, &ids, None)
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out["A"], Ok(empty_series()));
        assert_eq!(out["B"], Ok(empty_series()));
    }

    /// Rate limited on ids starting with `"limited"`, unauthorised on `"denied"`, else not found.
    struct Limited;

    #[async_trait]
    impl SourceAdapter for Limited {
        fn id(&self) -> SourceId {
            SourceId::Bea
        }
        fn datasets(&self) -> &[&str] {
            &["test"]
        }
        async fn discover(&self, _: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
            Ok(Vec::new())
        }
        async fn fetch_series(
            &self,
            _: &CrawlCtx,
            id: &str,
            _: Option<NaiveDate>,
        ) -> Result<FetchedSeries, CrawlError> {
            if id.starts_with("limited") {
                Err(CrawlError::RateLimited { retry_after: None })
            } else if id.starts_with("denied") {
                Err(CrawlError::Auth(id.into()))
            } else {
                Err(CrawlError::NotFound(id.into()))
            }
        }
    }

    #[tokio::test]
    async fn default_fetch_batch_stops_calling_upstream_after_a_rate_limit() {
        let ctx = crate::testkit::test_ctx();
        let ids = vec![
            "missing".to_string(),
            "limited".to_string(),
            "after".to_string(),
        ];
        let out = Limited.fetch_batch(&ctx, &ids, None).await.unwrap();
        let limited = Err(CrawlError::RateLimited { retry_after: None });
        // "after" would be NotFound had it been fetched.
        assert_eq!(out["missing"], Err(CrawlError::NotFound("missing".into())));
        assert_eq!(out["limited"], limited);
        assert_eq!(out["after"], limited);

        // An auth failure stays with its own series; later ids are still fetched.
        let ids = vec!["denied".to_string(), "after".to_string()];
        let out = Limited.fetch_batch(&ctx, &ids, None).await.unwrap();
        assert_eq!(out["denied"], Err(CrawlError::Auth("denied".into())));
        assert_eq!(out["after"], Err(CrawlError::NotFound("after".into())));
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
            "WTO_API_KEY" => Some("wto-key\n".into()),
            _ => None,
        });
        assert_eq!(keys.fred.as_deref(), Some("abc"));
        assert_eq!(keys.bls, None);
        assert_eq!(keys.bea, None);
        assert_eq!(keys.census, None);
        assert_eq!(keys.get(SourceId::Fred), Some("abc"));
        assert_eq!(keys.get(SourceId::Wto), Some("wto-key"));
        assert_eq!(keys.get(SourceId::Sec), None);
    }

    #[test]
    fn api_keys_debug_redacts() {
        let keys = ApiKeys {
            fred: Some("supersecret".into()),
            wto: Some("wtosecret".into()),
            ..ApiKeys::default()
        };
        let s = format!("{keys:?}");
        assert!(!s.contains("supersecret"));
        assert!(!s.contains("wtosecret"));
        assert!(s.contains("<redacted>"));
    }
}
