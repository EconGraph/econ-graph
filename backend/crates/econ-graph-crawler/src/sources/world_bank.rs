// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! World Bank Indicators API (v2) adapter: a curated set of World Development Indicators for
//! every country and aggregate. Every request is a `GET` with `format=json`; the API needs no key.
//!
//! # Series and ids
//!
//! The indicators are the rows of `wdi_indicators.csv` in the crawler's data directory
//! ([`crate::reference::wdi_indicators`]), which also gives each one's unit. There is one series
//! per indicator and area, in dataset `wdi` (`datasets/world_bank.toml`) with dimensions
//! `indicator` (the WDI code) and `area` (the `key` of `econ-graph-core`'s `countries.csv`: ISO
//! alpha-3 for countries, the World Bank code for aggregates such as `WLD` and `EMU`). The
//! external id is the dataset's canonical id, `wdi/{indicator}.{area}` (for example
//! `wdi/NY.GDP.PCAP.CD.USA`).
//!
//! # Requests
//!
//! Discovery and fetching both use one request per indicator for every area:
//! `{base}/country/all/indicator/{id}?format=json&per_page=20000&page=N`. One page holds the
//! whole indicator today (about 266 areas times 65 years); further pages are followed when the
//! response's `pages` says so, up to [`MAX_PAGES`].
//!
//! - **Discovery** makes that request for each indicator and lists the areas with at least one
//!   value. A series with no values is never created. `Auth` and `RateLimited` abort discovery;
//!   another failure skips that indicator with a warning, unless every indicator failed.
//! - **Fetching** is batched by indicator ([`SourceAdapter::batch_key`]): one request serves every
//!   area of an indicator, up to [`MAX_BATCH`] series. A lone `fetch_series` makes the same request
//!   and keeps its own area.
//!
//! # Indicator names and descriptions
//!
//! `wdi_indicators.csv` carries only each indicator's id and unit. Its name and description
//! (`sourceNote`) are not shipped: each indicator's own `/indicator/{id}?format=json` carries
//! both, one indicator per URL, so [`code_lists`](SourceAdapter::code_lists) lists one
//! [`CodeList`] per listed indicator rather than one shared file. The default
//! `refresh_reference_data` (`reference_file::refresh_code_lists`) fetches every one of them,
//! merging each indicator's name and description into the `wdi` dataset's `indicator` dimension
//! codes ([`persist::merge_dataset_dimension_code_entries`]); being `code_lists` entries, these can
//! also be recorded as a seed migration by `crawler record-reference-seeds` (no WDI seed is
//! recorded yet), so a new database could have usable indicator names from its first deploy rather
//! than only after its first crawl.
//!
//! [`refresh_reference_data`](WorldBankAdapter) is still overridden, but only to keep the
//! in-process [`indicator_meta`](WorldBankAdapter::indicator_meta) cache (shared by every clone of
//! the adapter) fresh from the DB after that refresh, for building a series' title and
//! description without a DB round trip per series. `discover` and `fetch_batch` reseed the cache
//! from the same DB-held codes first, every call (so a freshly started process, a fetch job that
//! runs before this process's first discovery, or a cache only partly filled by an earlier call's
//! row-carried fallbacks, all pick up whatever is currently in the DB rather than seeing only the
//! id for an indicator this process hasn't merged itself); an indicator still missing after that
//! falls back to the name the data response itself carries for each row
//! ([`series_metadata`](WorldBankAdapter::series_metadata)), and only then to its bare id; that
//! row-carried fallback is itself merged into the dataset's `indicator` codes (and the in-process
//! cache), at most once per indicator per `discover`/`fetch_batch` call, so a brand new database
//! gets a usable indicator picker (the world map's indicator list reads these codes) from the very
//! first discovery or fetch, not just a usable series title, before `refresh_reference_data` has
//! ever run.
//!
//! # Rows
//!
//! Each row names its area by `countryiso3code` (ISO alpha-3, or the World Bank's code for an
//! aggregate) and `country.id` (ISO alpha-2, or a two-character aggregate id). The area is looked
//! up in the shared country table ([`econ_graph_core::reference::areas`]) by World Bank code, then
//! ISO alpha-3, then (for a row without `countryiso3code`) ISO alpha-2. Rows for an area the table does not list (regional aggregates
//! outside it, the Channel Islands) are skipped, with one warning per request naming them.
//!
//! Dates are years (`2023`), or `2023Q2` / `2023M05` for the rare quarterly or monthly indicator,
//! stored as the period's first day. A `null` value is no observation and is dropped.
//!
//! Every point's `revision_date` is the response's `lastupdated` (the date the World Bank last
//! updated the database), so each database update is stored as a new vintage of the whole series;
//! `is_original_release` is `true`, as for the other sources without vintage history. `since` is
//! ignored: the full history comes in the same single request, and returning all of it keeps
//! revisions to old years. The row's `obs_status` and `decimal` are dropped: `data_points` has no
//! attribute columns in train 1 (the dataset still declares them, for the schema). WDI leaves
//! `obs_status` empty for nearly every row.
//!
//! # Re-crawls
//!
//! The World Bank updates WDI a few times a year, while annual series are refreshed every 30 days
//! and discovery runs weekly, and each run would otherwise download every area of every
//! indicator. Both first ask for a one-row page of the same request (`per_page=1`), whose
//! `lastupdated` says whether the indicator changed:
//!
//! - **A fetch batch** stores the indicator's `lastupdated` as each series' validator `version`,
//!   with a fingerprint of the indicator's cached name and description, so a renamed indicator
//!   still rewrites its series' titles.
//!   When every requested series of an indicator stored the same one and the probe returns it,
//!   each gets [`FetchedSeries::unchanged`] (marked crawled, points kept) without the full
//!   request. A series never fetched, a different stored value, or a failed probe means a full
//!   fetch (an `Auth` or `RateLimited` probe fails the indicator's series instead).
//! - **Discovery** joins every listed indicator's `indicator=version` pair into one version,
//!   stored for the indicator endpoint (see [`persist::url_validators`]). With a version stored,
//!   it probes every indicator first, and a matching version is [`Discovery::Unchanged`]; with
//!   none (the first run), it discovers in full and takes the version from the full requests. The
//!   version is stored only by a discovery in which every indicator succeeded, so a skipped
//!   indicator is retried next time. A failed probe means a full discovery.
//!
//! Every stored version starts with [`PARSE_VERSION`], so a release that changes how the data is
//! read rewrites stored series once instead of waiting for the World Bank's next update.
//!
//! QA verifies live that a `per_page=1` page carries the same `lastupdated` as the full request.
//!
//! # Errors
//!
//! Successful responses are a two-element array `[meta, rows]` (`rows` may be `null`). Errors
//! come back as HTTP 200 with a one-element array `[{"message": [{"id", "key", "value"}]}]`;
//! [`classify_world_bank_message`] maps "Invalid value" / unknown-indicator messages (ids 120 and
//! 175) to `NotFound` and anything else to `Permanent`. A response without `lastupdated` is a
//! `Parse` error. An id that is not a `wdi` id of a listed indicator is `NotFound` without a
//! request.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::models::Code;
use econ_graph_core::reference::{areas, Area};
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{
    ApiKeys, BatchFetch, CrawlCtx, DiscoveredSeries, Discovery, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::http::Validators;
use crate::persist::{self, StoredFetchState};
use crate::policy::SourcePolicy;
use crate::reference::{self, WdiIndicator};
use crate::reference_file::{self, CodeList, ParseCodes};
use crate::source::SourceId;

/// The real World Bank API root.
pub const DEFAULT_BASE_URL: &str = "https://api.worldbank.org/v2";

/// The dataset every series belongs to.
pub const DATASET: &str = "wdi";

/// Human-facing indicator page, used for `data_url` (never requested).
const WEB_INDICATOR_URL: &str = "https://data.worldbank.org/indicator";

/// Rows per page. One page holds a whole indicator for every area.
const PER_PAGE: &str = "20000";

/// Upper bound on pages per indicator (a response claiming more is cut off with a warning).
pub const MAX_PAGES: u64 = 10;

/// How this adapter reads indicator data, prefixed to every stored `lastupdated` version (see
/// [Re-crawls](self#re-crawls)). Bump it whenever a change to parsing, the indicator list's units
/// or the area table should rewrite already-stored series before the World Bank's next update.
pub const PARSE_VERSION: &str = "wb-1";

/// Most series fetched per request: every area of one indicator (the country table has about 260
/// rows), with room to spare.
pub const MAX_BATCH: usize = 400;

/// One World Bank indicator's own name and description, as last merged into the DB by a
/// [`code_lists`](WorldBankAdapter::code_lists) refresh, or, without one yet, a row-carried
/// fallback name with no description ([`series_metadata`](WorldBankAdapter::series_metadata)).
#[derive(Debug, Clone)]
struct IndicatorMeta {
    name: String,
    description: Option<String>,
}

/// The dataset dimension indicator names and descriptions are merged into.
const INDICATOR_DIMENSION: &str = "indicator";

/// World Bank adapter. See the module docs for endpoints, ids and error mapping.
#[derive(Debug, Clone)]
pub struct WorldBankAdapter {
    base_url: String,
    /// Indicator id -> name/description, last loaded by [`seed_cache_from_db`](Self::seed_cache_from_db)
    /// (called after `refresh_reference_data`'s refresh, and at the start of every `discover`/
    /// `fetch_batch` call) or, failing that, by a row-carried fallback name
    /// ([`series_metadata`](Self::series_metadata)). Shared by every clone (the worker clones the
    /// adapter per job; the registry holds one `Arc` of it), so a fetch or discover job sees
    /// whatever was last loaded in this process. Empty until the first such load.
    indicator_meta: Arc<Mutex<HashMap<String, IndicatorMeta>>>,
}

impl WorldBankAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            indicator_meta: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Every row of `indicator` for every area, following pages.
    async fn fetch_indicator(
        &self,
        ctx: &CrawlCtx,
        indicator: &str,
    ) -> Result<IndicatorData, CrawlError> {
        let path = format!("/country/all/indicator/{indicator}");
        let url = format!("{}{path}", self.base_url);
        let mut rows = Vec::new();
        let mut last_updated = None;
        let mut page = 1u64;
        loop {
            let page_s = page.to_string();
            let body: Value = ctx
                .http
                .get_json(
                    SourceId::WorldBank,
                    &url,
                    &[
                        ("format", "json"),
                        ("per_page", PER_PAGE),
                        ("page", &page_s),
                    ],
                )
                .await?;
            let (meta, items) = parse_list(&path, body)?;
            if page == 1 {
                last_updated = meta.last_updated;
            }
            let got = items.len();
            rows.extend(parse_rows(&path, items)?);
            let pages = meta.pages.unwrap_or(1);
            if got == 0 || page >= pages {
                break;
            }
            if page >= MAX_PAGES {
                tracing::warn!(
                    indicator,
                    pages,
                    "World Bank indicator has more pages than MAX_PAGES; the rest are skipped"
                );
                break;
            }
            page += 1;
        }
        let last_updated = last_updated.ok_or_else(|| {
            CrawlError::Parse(format!("World Bank {path}: response has no lastupdated"))
        })?;
        Ok(IndicatorData::group(indicator, last_updated, rows))
    }

    /// Overwrites the in-process cache with whatever the `wdi` dataset's `indicator` dimension
    /// currently holds in the DB (the last successful merge, by this process or another one).
    /// Errors (e.g. no DB reachable, as in tests) are swallowed: the cache is best-effort and
    /// callers fall back to a row-carried name, and only then to the indicator's id (see
    /// [`series_metadata`](Self::series_metadata)).
    ///
    /// Called after [`refresh_reference_data`](SourceAdapter::refresh_reference_data) refreshes
    /// the `code_lists`, so the cache reflects whatever merged even when some indicators failed;
    /// also called at the start of every `discover`/`fetch_batch` call, unconditionally (not just
    /// when the cache is still empty), so an indicator another call's row-carried fallback merged
    /// in the meantime is picked up too, rather than being merged all over again with its
    /// `sourceNote` dropped. A freshly started process, or a fetch job that races this process's
    /// first discovery, sees whatever an earlier process last found rather than only the id.
    async fn seed_cache_from_db(&self, ctx: &CrawlCtx) {
        let codes = match persist::dataset_dimension_codes(
            &ctx.pool,
            SourceId::WorldBank,
            DATASET,
            INDICATOR_DIMENSION,
        )
        .await
        {
            Ok(codes) => codes,
            Err(e) => {
                tracing::warn!(error = %e, "World Bank: could not seed indicator cache from the DB");
                return;
            }
        };
        let mut cache = self.indicator_meta.lock().unwrap();
        for (id, code) in codes {
            cache.insert(
                id,
                IndicatorMeta {
                    name: code.label,
                    description: code.description,
                },
            );
        }
    }

    /// `indicator`'s name and description as last cached (by a fetch in this process or
    /// [`seed_cache_from_db`](Self::seed_cache_from_db) from the DB). Without a cache entry yet
    /// (a brand new database, or a fetch that races the first scheduled
    /// `refresh_reference_data`), the name falls back to `rows`' own `indicator.value` from the
    /// data response itself, and only then to the indicator's bare id; the description has no
    /// such fallback (`sourceNote` is only ever on `/indicator/{id}`). A row-carried fallback name
    /// is also merged into the `wdi` dataset's `indicator` dimension codes (best-effort; a DB
    /// error here is logged and otherwise ignored), so a fresh database that has not yet run
    /// `refresh_reference_data` still gets a usable indicator picker from the first discovery or
    /// fetch alone, not just a usable series title. `attempted` tracks, for the whole `discover`
    /// or `fetch_batch` call this is part of, which indicators this fallback merge was already
    /// tried for: a cache miss that leaves the cache empty (`Ok(false)` or `Err`) would otherwise
    /// repeat the same pooled transaction and `FOR UPDATE` query once per area of that indicator.
    /// Kept local to the call so a later `discover`/`fetch_batch` can still retry.
    async fn series_metadata(
        &self,
        ctx: &CrawlCtx,
        indicator: &WdiIndicator,
        area: &Area,
        rows: &[Row],
        attempted: &mut HashSet<String>,
    ) -> NewSeriesMetadataLite {
        let row_name = rows.first().and_then(|r| r.indicator_name.clone());
        let (name, description, fallback_to_merge) = {
            let cache = self.indicator_meta.lock().unwrap();
            match cache.get(&indicator.id) {
                Some(cached) => (cached.name.clone(), cached.description.clone(), None),
                None => (
                    row_name.clone().unwrap_or_else(|| indicator.id.clone()),
                    None,
                    row_name.clone(),
                ),
            }
        };
        // Merged (and cached) at most once per process per indicator: once this succeeds, later
        // calls for the same indicator find the cache populated and skip straight past this. A
        // failed or no-op attempt is only skipped for the rest of this `discover`/`fetch_batch`
        // call (via `attempted`), not forever, so a later call can retry.
        if let Some(name) = fallback_to_merge {
            if attempted.insert(indicator.id.clone()) {
                match persist::merge_dataset_dimension_code_entries(
                    &ctx.pool,
                    SourceId::WorldBank,
                    DATASET,
                    INDICATOR_DIMENSION,
                    &[Code::new(&indicator.id, &name)],
                )
                .await
                {
                    // Only cache a name the DB actually stored: `false` (the dataset row isn't
                    // synced yet, or this dimension uses a shared codelist) must not make later
                    // calls in this process believe the DB has a label it doesn't.
                    Ok(true) => {
                        self.indicator_meta
                            .lock()
                            .unwrap()
                            .entry(indicator.id.clone())
                            .or_insert(IndicatorMeta {
                                name,
                                description: None,
                            });
                    }
                    Ok(false) => {}
                    Err(e) => {
                        tracing::warn!(
                            indicator = %indicator.id,
                            error = %e,
                            "World Bank: could not merge the row-carried indicator name into the dataset codes"
                        );
                    }
                }
            }
        }
        let description = description.unwrap_or_else(|| {
            format!(
                "World Development Indicators {} for {}",
                indicator.id, area.name
            )
        });
        NewSeriesMetadataLite {
            title: format!("{name}: {}", area.name),
            description: Some(description),
            units: Some(indicator.unit.clone()),
            frequency: Some(
                rows.first()
                    .map_or(Frequency::Annual, |r| r.frequency)
                    .label()
                    .to_string(),
            ),
            seasonal_adjustment: None,
        }
    }

    /// `lastupdated` of `indicator`, from a one-row page of the same request
    /// [`fetch_indicator`](Self::fetch_indicator) makes (see [Re-crawls](self#re-crawls)).
    async fn probe_last_updated(
        &self,
        ctx: &CrawlCtx,
        indicator: &str,
    ) -> Result<NaiveDate, CrawlError> {
        let path = format!("/country/all/indicator/{indicator}");
        let body: Value = ctx
            .http
            .get_json(
                SourceId::WorldBank,
                &format!("{}{path}", self.base_url),
                &[("format", "json"), ("per_page", "1"), ("page", "1")],
            )
            .await?;
        parse_list(&path, body)?.0.last_updated.ok_or_else(|| {
            CrawlError::Parse(format!("World Bank {path}: response has no lastupdated"))
        })
    }

    /// Key under which discovery stores [`catalog_version`](Self::catalog_version)'s result (see
    /// [`persist::url_validators`]): the indicator endpoint, without an indicator.
    fn catalog_key(&self) -> String {
        format!("{}/country/all/indicator", self.base_url)
    }

    /// One string naming every listed indicator and its `lastupdated`, so a discovery can tell
    /// that neither the list nor any indicator changed. `None` if a probe failed with anything
    /// but `Auth` or `RateLimited` (which abort): discovery then runs in full.
    async fn catalog_version(
        &self,
        ctx: &CrawlCtx,
        indicators: &[WdiIndicator],
    ) -> Result<Option<String>, CrawlError> {
        let mut parts = Vec::with_capacity(indicators.len());
        for indicator in indicators {
            match self.probe_last_updated(ctx, &indicator.id).await {
                Ok(d) => parts.push(format!(
                    "{}={}",
                    indicator.id,
                    self.series_version(&indicator.id, d)
                )),
                Err(e @ (CrawlError::Auth(_) | CrawlError::RateLimited { .. })) => return Err(e),
                Err(e) => {
                    tracing::warn!(indicator = %indicator.id, error = %e,
                        "World Bank lastupdated probe failed; discovering in full");
                    return Ok(None);
                }
            }
        }
        parts.sort();
        Ok(Some(format!("{PARSE_VERSION}|{}", parts.join(";"))))
    }

    /// A series' stored `version`: [`PARSE_VERSION`], its indicator's `lastupdated`, and a
    /// fingerprint of the indicator's name and description as currently cached (see
    /// [`seed_cache_from_db`](Self::seed_cache_from_db)), so a renamed or redescribed indicator
    /// rewrites its series' titles even when its data hasn't moved.
    fn series_version(&self, indicator: &str, last_updated: NaiveDate) -> String {
        let meta = self.indicator_meta.lock().unwrap().get(indicator).map(|m| {
            let mut bytes = m.name.clone().into_bytes();
            bytes.push(0);
            bytes.extend(m.description.as_deref().unwrap_or_default().as_bytes());
            bytes
        });
        let tag = meta.map_or_else(String::new, |b| {
            use sha2::{Digest, Sha256};
            Sha256::digest(b)[..6]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        });
        format!("{PARSE_VERSION}:{last_updated}:{tag}")
    }

    /// Discovered series, whether every indicator's request succeeded, and (when they all did)
    /// the catalog version their `lastupdated` values give (as [`catalog_version`](Self::catalog_version)).
    async fn discover_all(
        &self,
        ctx: &CrawlCtx,
    ) -> Result<(Vec<DiscoveredSeries>, bool, Option<String>), CrawlError> {
        self.seed_cache_from_db(ctx).await;
        let indicators = reference::wdi_indicators()?;
        let def = wdi_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut found = Vec::new();
        let mut attempted = HashSet::new();
        let mut parts = Vec::with_capacity(indicators.len());
        let (mut any_ok, mut last_err) = (false, None);
        for indicator in indicators {
            match self.fetch_indicator(ctx, &indicator.id).await {
                Ok(data) => {
                    any_ok = true;
                    for (area, rows) in data.resolve(areas) {
                        let (external_id, dataset) = series_id(def, &indicator.id, &area.key)?;
                        let meta = self
                            .series_metadata(ctx, indicator, area, &rows, &mut attempted)
                            .await;
                        found.push(DiscoveredSeries {
                            external_id,
                            title: meta.title,
                            description: meta.description,
                            units: meta.units,
                            frequency: meta.frequency,
                            data_url: Some(data_url(&indicator.id, &rows)),
                            dataset,
                        });
                    }
                    // After the titles: a row-carried fallback name may have just been cached.
                    let version = self.series_version(&indicator.id, data.last_updated);
                    parts.push(format!("{}={version}", indicator.id));
                }
                Err(e @ (CrawlError::Auth(_) | CrawlError::RateLimited { .. })) => return Err(e),
                Err(e) => {
                    tracing::warn!(indicator = %indicator.id, error = %e, "World Bank indicator failed; skipping");
                    last_err = Some(e);
                }
            }
        }
        match (any_ok, last_err) {
            (false, Some(e)) => Err(e),
            (_, last_err) => {
                tracing::info!(series = found.len(), "World Bank discovery finished");
                let complete = last_err.is_none();
                parts.sort();
                let version = complete.then(|| format!("{PARSE_VERSION}|{}", parts.join(";")));
                Ok((found, complete, version))
            }
        }
    }
}

/// The validators every one of `ids` stored, if they all stored the same ones (their indicator's
/// `lastupdated` at their last fetch). Otherwise `None`: fetch the indicator in full.
fn shared_validators<'a>(
    ids: impl IntoIterator<Item = &'a str>,
    stored: &HashMap<String, StoredFetchState>,
) -> Option<Validators> {
    let mut shared: Option<&Validators> = None;
    for id in ids {
        let v = stored.get(id)?.validators.as_ref()?;
        v.version.as_ref()?;
        match shared {
            None => shared = Some(v),
            Some(s) if s == v => {}
            Some(_) => return None,
        }
    }
    shared.cloned()
}

impl Default for WorldBankAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for WorldBankAdapter {
    fn id(&self) -> SourceId {
        SourceId::WorldBank
    }

    fn policy(&self) -> SourcePolicy {
        SourcePolicy {
            max_batch: MAX_BATCH,
            ..SourcePolicy::default_for(SourceId::WorldBank)
        }
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// One [`CodeList`] per listed indicator: World Bank indicator names and `sourceNote`s each
    /// live at their own `/indicator/{id}?format=json`, never in one shared file. See the module
    /// docs.
    fn code_lists(&self, _keys: &ApiKeys) -> Vec<CodeList> {
        let indicators = match reference::wdi_indicators() {
            Ok(indicators) => indicators,
            Err(e) => {
                // `discover`/`fetch_batch` surface this same error themselves; here there is no
                // `Result` to return it through, so at least log it rather than silently
                // publishing no code lists.
                tracing::warn!(error = %e, "World Bank: could not list indicators for code_lists");
                return Vec::new();
            }
        };
        indicators
            .iter()
            .map(|indicator| {
                let id = indicator.id.clone();
                let url = format!("{}/indicator/{id}?format=json", self.base_url);
                let parse: ParseCodes = Arc::new(move |body: &str| {
                    let value: Value = serde_json::from_str(body).map_err(|e| {
                        CrawlError::Parse(format!("World Bank indicator {id}: {e}"))
                    })?;
                    let (_, items) = parse_list(&format!("/indicator/{id}"), value)?;
                    let Some(row) = items.first() else {
                        return Err(CrawlError::Parse(format!(
                            "World Bank indicator {id}: response has no rows"
                        )));
                    };
                    let name = row
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if name.is_empty() {
                        return Err(CrawlError::Parse(format!(
                            "World Bank indicator {id}: response has no name"
                        )));
                    }
                    let description = row
                        .get("sourceNote")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let mut code = Code::new(&id, name);
                    code.description = description;
                    Ok(vec![code])
                });
                CodeList::new(url, DATASET, INDICATOR_DIMENSION, parse)
            })
            .collect()
    }

    /// The default `code_lists` refresh (every indicator attempted and failures aggregated, except
    /// `Auth`/`RateLimited`, which stop the refresh at once), then
    /// [`seed_cache_from_db`](Self::seed_cache_from_db) unconditionally, so the in-process cache
    /// reflects whatever merged even when the refresh as a whole failed.
    async fn refresh_reference_data(&self, ctx: &CrawlCtx) -> Result<(), CrawlError> {
        let result =
            reference_file::refresh_code_lists(ctx, self.id(), &self.code_lists(&ctx.keys)).await;
        self.seed_cache_from_db(ctx).await;
        result
    }

    /// One request per indicator; see the module docs. Series come out indicator by indicator
    /// (in file order), areas sorted by key, so the queue holds each indicator's series together
    /// for batching.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        Ok(self.discover_all(ctx).await?.0)
    }

    /// [`Discovery::Unchanged`] when no indicator's `lastupdated` (and not the list) has changed
    /// since the last discovery that read every indicator (see [Re-crawls](self#re-crawls)).
    async fn discover_if_changed(&self, ctx: &CrawlCtx) -> Result<Discovery, CrawlError> {
        let key = self.catalog_key();
        let stored = persist::url_validators(&ctx.pool, SourceId::WorldBank, &key)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "World Bank: reading stored validators failed");
                None
            })
            .and_then(|v| v.version);
        // The probes give the version to store after a full discovery too, but with nothing stored
        // they can't save one: the full requests below carry the same `lastupdated` values.
        self.seed_cache_from_db(ctx).await;
        let version = match stored {
            Some(_) => {
                self.catalog_version(ctx, reference::wdi_indicators()?)
                    .await?
            }
            None => None,
        };
        if version.is_some() && version == stored {
            return Ok(Discovery::Unchanged);
        }
        let (found, complete, full_version) = self.discover_all(ctx).await?;
        let version = full_version.or(version);
        // Only a discovery that read every indicator may vouch for the catalog. An incomplete one
        // keeps the last complete discovery's version: an indicator that moved since then (the
        // reason this one ran in full) still differs from it, so the next discovery runs in full
        // too, and one that didn't move was listed by that discovery.
        let validator = version.filter(|_| complete).map(|version| {
            (
                key,
                Validators {
                    version: Some(version),
                    ..Validators::default()
                },
            )
        });
        Ok(Discovery::Changed { found, validator })
    }

    /// The same request as a batch: every area of the indicator, keeping `external_id`'s.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let mut out = self
            .fetch_batch(ctx, &[external_id.to_string()], since)
            .await?;
        out.remove(external_id).unwrap_or_else(|| {
            Err(CrawlError::NotFound(format!(
                "World Bank {external_id}: no values for this area"
            )))
        })
    }

    /// The indicator: every area of an indicator comes from one request.
    fn batch_key(&self, external_id: &str) -> Option<String> {
        parse_id(external_id).map(|(indicator, _)| indicator.to_string())
    }

    /// One request per distinct indicator among `external_ids` (one, when the worker batches by
    /// [`batch_key`](Self::batch_key)). An id whose area has no values is left out of the map
    /// (so it fails as `NotFound`); an id that is not a listed indicator's is `NotFound` without a
    /// request. A failed request fails that indicator's ids.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        _since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        self.seed_cache_from_db(ctx).await;
        let indicators = reference::wdi_indicators()?;
        let def = wdi_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut attempted = HashSet::new();
        let mut out = BatchFetch::with_capacity(external_ids.len());
        // Requested areas per indicator, in first-requested order.
        let mut wanted: Vec<(&WdiIndicator, BTreeMap<&str, &str>)> = Vec::new();
        for id in external_ids {
            let listed = parse_id(id)
                .and_then(|(ind, area)| Some((indicators.iter().find(|i| i.id == ind)?, area)));
            let Some((indicator, area)) = listed else {
                out.insert(
                    id.clone(),
                    Err(CrawlError::NotFound(format!(
                        "World Bank {id}: not a {DATASET} id of a listed indicator"
                    ))),
                );
                continue;
            };
            match wanted.iter_mut().find(|(i, _)| i.id == indicator.id) {
                Some((_, ids)) => {
                    ids.insert(area, id);
                }
                None => wanted.push((indicator, BTreeMap::from([(area, id.as_str())]))),
            }
        }
        let requested: Vec<String> = wanted
            .iter()
            .flat_map(|(_, ids)| ids.values().map(|id| (*id).to_string()))
            .collect();
        let stored = if requested.is_empty() {
            HashMap::new()
        } else {
            persist::stored_fetch_state(&ctx.pool, SourceId::WorldBank, &requested)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "World Bank: reading stored validators failed; full fetch");
                    HashMap::new()
                })
        };
        for (indicator, ids) in wanted {
            if let Some(known) = shared_validators(ids.values().copied(), &stored) {
                match self.probe_last_updated(ctx, &indicator.id).await {
                    Ok(d)
                        if known.version.as_deref()
                            == Some(self.series_version(&indicator.id, d).as_str()) =>
                    {
                        for id in ids.values() {
                            let dataset = stored[*id].dataset.clone();
                            out.insert(
                                (*id).to_string(),
                                Ok(FetchedSeries::unchanged(dataset, known.clone())),
                            );
                        }
                        continue;
                    }
                    Ok(_) => {}
                    Err(e @ (CrawlError::Auth(_) | CrawlError::RateLimited { .. })) => {
                        for id in ids.values() {
                            out.insert((*id).to_string(), Err(e.clone()));
                        }
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(indicator = %indicator.id, error = %e,
                            "World Bank lastupdated probe failed; full fetch");
                    }
                }
            }
            let data = match self.fetch_indicator(ctx, &indicator.id).await {
                Ok(data) => data,
                Err(e) => {
                    for id in ids.values() {
                        out.insert((*id).to_string(), Err(e.clone()));
                    }
                    continue;
                }
            };
            for (area, rows) in data.resolve(areas) {
                let Some(id) = ids.get(area.key.as_str()) else {
                    continue;
                };
                let series = match series_id(def, &indicator.id, &area.key) {
                    Ok((_, dataset)) => {
                        let metadata = self
                            .series_metadata(ctx, indicator, area, &rows, &mut attempted)
                            .await;
                        Ok(FetchedSeries {
                            metadata: Some(metadata),
                            points: rows
                                .iter()
                                .map(|r| FetchedPoint {
                                    date: r.date,
                                    value: Some(r.value.clone()),
                                    revision_date: data.last_updated,
                                    is_original_release: true,
                                })
                                .collect(),
                            dataset,
                            validators: Some(Validators {
                                version: Some(
                                    self.series_version(&indicator.id, data.last_updated),
                                ),
                                ..Validators::default()
                            }),
                        })
                    }
                    Err(e) => Err(e),
                };
                out.insert((*id).to_string(), series);
            }
        }
        Ok(out)
    }
}

/// The `wdi` definition from `datasets/world_bank.toml` (cached by [`reference::datasets`]).
fn wdi_def() -> Result<&'static DatasetDef, CrawlError> {
    reference::datasets(SourceId::WorldBank)?
        .iter()
        .find(|d| d.code == DATASET)
        .ok_or_else(|| {
            CrawlError::Permanent(format!(
                "{} defines no {DATASET} dataset",
                reference::datasets_file(SourceId::WorldBank).display()
            ))
        })
}

/// The canonical id and dataset of `indicator` for `area`.
fn series_id(
    def: &DatasetDef,
    indicator: &str,
    area: &str,
) -> Result<(String, SeriesDataset), CrawlError> {
    def.series([("indicator", indicator), ("area", area)])
}

/// `(indicator, area)` from `wdi/{indicator}.{area}`. Indicator codes contain dots and area keys
/// never do, so the area is everything after the last dot.
fn parse_id(external_id: &str) -> Option<(&str, &str)> {
    let rest = external_id.strip_prefix(DATASET)?.strip_prefix('/')?;
    let (indicator, area) = rest.rsplit_once('.')?;
    (!indicator.is_empty() && !area.is_empty()).then_some((indicator, area))
}

/// The indicator's page on data.worldbank.org, filtered to the row's area.
fn data_url(indicator: &str, rows: &[Row]) -> String {
    match rows.first() {
        Some(r) if !r.wb_id.is_empty() => {
            format!("{WEB_INDICATOR_URL}/{indicator}?locations={}", r.wb_id)
        }
        _ => format!("{WEB_INDICATOR_URL}/{indicator}"),
    }
}

/// Response metadata we use.
#[derive(Debug, Default, PartialEq)]
struct Meta {
    pages: Option<u64>,
    last_updated: Option<NaiveDate>,
}

/// Splits a `[meta, items]` response into its metadata and items; a `[{"message": ..}]` body
/// becomes an error via [`classify_world_bank_message`].
fn parse_list(what: &str, body: Value) -> Result<(Meta, Vec<Value>), CrawlError> {
    let Value::Array(mut parts) = body else {
        return Err(CrawlError::Parse(format!(
            "World Bank {what}: response is not an array"
        )));
    };
    if let Some(messages) = parts.first().and_then(|m| m.get("message")) {
        return Err(classify_world_bank_message(what, messages));
    }
    if parts.len() < 2 {
        return Err(CrawlError::Parse(format!(
            "World Bank {what}: expected [metadata, rows]"
        )));
    }
    let items = match parts.swap_remove(1) {
        Value::Null => Vec::new(),
        Value::Array(items) => items,
        other => {
            return Err(CrawlError::Parse(format!(
                "World Bank {what}: expected a row array, got {}",
                type_name(&other)
            )))
        }
    };
    let meta = parts.first();
    let pages = meta.and_then(|m| m.get("pages")).and_then(|p| {
        p.as_u64()
            .or_else(|| p.as_str().and_then(|s| s.trim().parse().ok()))
    });
    let last_updated = match meta.and_then(|m| m.get("lastupdated")) {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_str()
                .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
                .ok_or_else(|| {
                    CrawlError::Parse(format!("World Bank {what}: bad lastupdated {v}"))
                })?,
        ),
    };
    Ok((
        Meta {
            pages,
            last_updated,
        },
        items,
    ))
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Maps the World Bank's in-body error (`[{"message":[{"id":"120","key":"Invalid value","value":"..."}]}]`,
/// served with HTTP 200): invalid-value / unknown-indicator messages (ids `120`, `175`) are
/// `NotFound`, anything else `Permanent`.
pub fn classify_world_bank_message(what: &str, messages: &Value) -> CrawlError {
    let first = messages.as_array().and_then(|m| m.first());
    let field = |k: &str| {
        first
            .and_then(|m| m.get(k))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let (id, key, value) = (field("id"), field("key"), field("value"));
    let msg = format!("World Bank {what}: error {id} {key}: {value}");
    if matches!(id.as_str(), "120" | "175") || key.eq_ignore_ascii_case("invalid value") {
        CrawlError::NotFound(msg)
    } else {
        CrawlError::Permanent(msg)
    }
}

/// Observation frequency, from the form of the row's date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frequency {
    Annual,
    Quarterly,
    Monthly,
}

impl Frequency {
    fn label(self) -> &'static str {
        match self {
            Frequency::Annual => "Annual",
            Frequency::Quarterly => "Quarterly",
            Frequency::Monthly => "Monthly",
        }
    }
}

/// `2023` -> 2023-01-01, `2023Q2` -> 2023-04-01, `2023M05` -> 2023-05-01.
fn parse_period(s: &str) -> Option<(NaiveDate, Frequency)> {
    let s = s.trim();
    let year: i32 = s.get(..4)?.parse().ok()?;
    let rest = &s[4..];
    let (month, freq) = if rest.is_empty() {
        (1, Frequency::Annual)
    } else if let Some(q) = rest.strip_prefix('Q') {
        let q: u32 = q.parse().ok().filter(|q| (1..=4).contains(q))?;
        (3 * q - 2, Frequency::Quarterly)
    } else {
        let m = rest.strip_prefix('M')?;
        (m.parse().ok()?, Frequency::Monthly)
    };
    Some((NaiveDate::from_ymd_opt(year, month, 1)?, freq))
}

/// One observation with a value, and the area as the row names it.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// `countryiso3code` (ISO alpha-3, or the World Bank code of an aggregate); may be empty.
    iso3: String,
    /// `country.id` (ISO alpha-2, or a two-character aggregate id).
    wb_id: String,
    /// `country.value`, for warnings.
    name: String,
    /// `indicator.value` from this same row: the indicator's own name, straight from the data
    /// response. Used as a fallback title when `refresh_reference_data` hasn't named this
    /// indicator yet (a brand new database, or a fetch that races the first scheduled refresh),
    /// so a series still gets a real name instead of its bare id; `sourceNote` (the description)
    /// is still only ever fetched from `/indicator/{id}`, which this row doesn't carry.
    indicator_name: Option<String>,
    date: NaiveDate,
    frequency: Frequency,
    value: BigDecimal,
}

/// Wire format of a data row (only the fields we use).
#[derive(Debug, Deserialize)]
struct WbRow {
    #[serde(default)]
    indicator: IdValue,
    country: IdValue,
    #[serde(default, rename = "countryiso3code")]
    iso3: Option<String>,
    date: String,
    value: Option<serde_json::Number>,
}

#[derive(Debug, Default, Deserialize)]
struct IdValue {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

/// The rows with a value. A row whose date or value cannot be parsed is a `Parse` error.
fn parse_rows(what: &str, items: Vec<Value>) -> Result<Vec<Row>, CrawlError> {
    let raw: Vec<WbRow> = serde_json::from_value(Value::Array(items))
        .map_err(|e| CrawlError::Parse(format!("World Bank {what}: bad data rows: {e}")))?;
    let mut rows = Vec::with_capacity(raw.len());
    for r in raw {
        let Some(value) = r.value else { continue };
        let (date, frequency) = parse_period(&r.date).ok_or_else(|| {
            CrawlError::Parse(format!("World Bank {what}: bad date {:?}", r.date))
        })?;
        let value = BigDecimal::from_str(&value.to_string())
            .map_err(|e| CrawlError::Parse(format!("World Bank {what}: bad value {value}: {e}")))?;
        rows.push(Row {
            iso3: r.iso3.unwrap_or_default().trim().to_string(),
            wb_id: r.country.id.unwrap_or_default().trim().to_string(),
            name: r.country.value.unwrap_or_default(),
            indicator_name: r
                .indicator
                .value
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            date,
            frequency,
            value,
        });
    }
    Ok(rows)
}

/// One indicator's rows with values, grouped by the area code the rows use.
struct IndicatorData {
    indicator: String,
    last_updated: NaiveDate,
    /// `(iso3, wb_id)` -> rows, sorted by date.
    by_code: BTreeMap<(String, String), Vec<Row>>,
}

impl IndicatorData {
    fn group(indicator: &str, last_updated: NaiveDate, rows: Vec<Row>) -> Self {
        let mut by_code: BTreeMap<(String, String), Vec<Row>> = BTreeMap::new();
        for r in rows {
            by_code
                .entry((r.iso3.clone(), r.wb_id.clone()))
                .or_default()
                .push(r);
        }
        for rows in by_code.values_mut() {
            rows.sort_by_key(|r| r.date);
            // A repeated date keeps its last row.
            rows.reverse();
            rows.dedup_by_key(|r| r.date);
            rows.reverse();
        }
        Self {
            indicator: indicator.to_string(),
            last_updated,
            by_code,
        }
    }

    /// Rows per area of the country table, sorted by area key. Codes the table does not know are
    /// logged once and dropped. Two raw codes can resolve to the same area (a country whose
    /// `countryiso3code` is empty on some rows and set on others), so rows are merged rather than
    /// keeping only the first-seen code, with a repeated date keeping its last-merged row.
    fn resolve<'a>(
        &self,
        areas: &'a econ_graph_core::reference::Areas,
    ) -> Vec<(&'a Area, Vec<Row>)> {
        let mut out: BTreeMap<&str, (&Area, Vec<Row>)> = BTreeMap::new();
        let mut unknown = BTreeSet::new();
        let mut merged = BTreeSet::new();
        for ((iso3, wb_id), rows) in &self.by_code {
            let area = lookup(areas, iso3, wb_id);
            match area {
                Some(area) => match out.entry(area.key.as_str()) {
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert((area, rows.clone()));
                    }
                    std::collections::btree_map::Entry::Occupied(mut e) => {
                        merged.insert(area.key.as_str());
                        let combined = &mut e.get_mut().1;
                        combined.extend(rows.iter().cloned());
                        combined.sort_by_key(|r| r.date);
                        // A date shared between the merged codes keeps its later-merged row (the
                        // same "last wins" rule `group` uses within one raw code).
                        combined.reverse();
                        combined.dedup_by_key(|r| r.date);
                        combined.reverse();
                    }
                },
                None => {
                    let name = rows.first().map_or("", |r| r.name.as_str());
                    unknown.insert(format!("{iso3}/{wb_id} {name}"));
                }
            }
        }
        if !unknown.is_empty() {
            tracing::warn!(
                indicator = %self.indicator,
                count = unknown.len(),
                areas = ?unknown,
                "World Bank rows for areas not in the country table skipped"
            );
        }
        if !merged.is_empty() {
            tracing::info!(
                indicator = %self.indicator,
                areas = ?merged,
                "World Bank rows for these areas used more than one raw code (e.g. countryiso3code \
                 empty on some rows); merged into one series"
            );
        }
        out.into_values().collect()
    }
}

/// The area a row names: `countryiso3code` as a World Bank code or ISO alpha-3, or, only when a
/// row has no `countryiso3code`, `country.id` as ISO alpha-2 (aggregate ids are not ISO codes).
fn lookup<'a>(
    areas: &'a econ_graph_core::reference::Areas,
    iso3: &str,
    wb_id: &str,
) -> Option<&'a Area> {
    let by_iso3 = (!iso3.is_empty())
        .then(|| areas.by_wb_code(iso3).or_else(|| areas.by_iso3(iso3)))
        .flatten();
    by_iso3.or_else(|| {
        (iso3.is_empty() && wb_id.len() == 2)
            .then(|| areas.by_iso2(wb_id))
            .flatten()
    })
}

#[cfg(test)]
mod tests;
