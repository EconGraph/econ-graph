// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FRED (Federal Reserve Economic Data, St. Louis Fed) adapter.
//!
//! Endpoints used (all `GET`, all with `api_key` and `file_type=json`):
//! - `{base}/release/series?release_id=ID` and `/category/series?category_id=ID` — catalog pages.
//! - `{base}/series?series_id=ID` — metadata for include overrides and fetching.
//! - `{base}/series/observations?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=..&offset=..[&observation_start=YYYY-MM-DD]`
//!   — observations with their vintages (ALFRED), paged. See [Vintages](#vintages).
//! - `{base}/series/vintagedates?series_id=ID&realtime_start=..&realtime_end=..&limit=..&offset=..`
//!   — the vintage dates in a fetch's real-time window, paged, to plan the observations
//!   requests. See [Vintage-date limit](#vintage-date-limit).
//!
//! # Discovery
//!
//! `discover()` walks complete release/category catalogs selected by
//! `data/catalog_selection/fred.toml`. Rules match live titles, id prefixes and frequency;
//! a small include/exclude override list covers headline exceptions. Both the number of
//! catalog rows inspected and the number of selected series are bounded before any observation
//! work is enqueued. An empty catalog, failed page, changing count, truncated page or duplicate
//! id fails the entire discovery, so a temporary upstream problem cannot look like retirement.
//!
//! Restricted series remain in the catalog metadata but are hidden by the lifecycle writer;
//! `(DISCONTINUED)` titles retain their histories and carry a badge. Fetching any series still
//! repeats the copyright-notes check before requesting observations. Pagination follows FRED's
//! documented `series_id` ascending order with a pinned real-time date.
//!
//! # Vintages
//!
//! Observations are requested with a real-time window, so FRED returns one row per vintage: the
//! value of `date` as published from `realtime_start` until `realtime_end`. Each row becomes a
//! point with `revision_date = realtime_start`, and a date's earliest row is its original release.
//!
//! - **First fetch**: the window starts at [`EARLIEST_REALTIME_START`], so every vintage FRED
//!   has is returned. The first vintage is the earliest FRED knows (ALFRED history starts in the
//!   1990s for most series), not necessarily the first publication ever.
//! - **Incremental fetch** ([`SourceAdapter::fetch_series_incremental`] with a known vintage
//!   `K`, the newest stored `revision_date`): the window starts the day before `K`. FRED returns
//!   every value in effect on that day (with `realtime_start` clamped to it) plus every later
//!   vintage. Rows starting before `K` are already stored and are dropped. Vintage `K` itself is
//!   asked for again, because FRED dates vintages by day and may have added to or corrected it
//!   after the last crawl; unchanged rows are no-ops in the upsert. A date's earliest row is its
//!   original release whether or not it was dropped, so full and incremental fetches mark the
//!   same rows. No `observation_start` is sent, so revisions to old dates (annual and benchmark
//!   revisions) are picked up.
//! - **Paging**: FRED returns at most [`OBSERVATIONS_PAGE_SIZE`] rows per request. Pages are
//!   requested with `offset` until `count` rows have arrived (at most [`MAX_OBSERVATION_PAGES`]
//!   per real-time window).
//!
//! # Vintage-date limit
//!
//! One `/series/observations` request may cover at most [`FRED_DOCUMENTED_VINTAGE_DATE_LIMIT`]
//! vintage dates; FRED rejects a real-time window holding more with an HTTP 400 (daily series
//! such as DFF or DGS10 have over 5,000). So every fetch first lists the vintage dates of its
//! real-time window (`/series/vintagedates`, one request for any series with up to 10,000) and
//! requests the observations as consecutive windows ([`RealtimeWindow`]) of at most
//! [`FRED_MAX_VINTAGE_DATES_PER_REQUEST`] vintage dates each, half the documented limit (the
//! margin is explained there). Each window is paged as above:
//!
//! - **Splitting**: the vintage dates are cut into groups of at most the default. The first
//!   window starts where the whole one did; each later one starts the day before its first
//!   vintage and ends the day before the next window's first vintage; the last ends at
//!   `9999-12-31`. A window with no more vintage dates than the default is the whole window,
//!   so most series take one observations request, the same one as without splitting.
//! - **No gaps or duplicates**: a row is kept only by the window whose range, from its first
//!   vintage to its end, holds the row's `realtime_start`. Those ranges partition the real-time
//!   axis, so every row FRED has is kept exactly once, whatever FRED does at the edges.
//! - **Original releases**: starting each window the day before its first vintage makes FRED
//!   also return every value already in effect then, clamped to that day, exactly as for an
//!   incremental fetch. Those rows aren't kept, but take part in choosing each date's earliest
//!   row, so a date first published in an earlier window has no original release in a later
//!   one, and the points match a single-window fetch.
//! - **Fallback**: if FRED still answers a planned window with the vintage-limit 400
//!   ([`is_too_many_vintage_dates`]), for instance because the limit was lowered, the window is
//!   split in half (down to a single vintage) and retried, with a warning logged. No other 400
//!   is retried.
//! - **Politeness**: every request, the vintage-date listing included, goes through the shared
//!   [`HttpFetcher`](crate::HttpFetcher), so windows are requested one after another under
//!   FRED's rate limit, with its User-Agent and retries.
//!
//! # Re-crawls
//!
//! Series are refreshed more often than most of them change (a monthly series weekly), and an
//! incremental fetch still returns every date's value in effect the day before the known
//! vintage, so an unchanged series would re-read and re-upsert its whole history. `/series`, which
//! every fetch requests for metadata anyway, gives `last_updated`, which FRED moves whenever the
//! series' data changes, revisions included. It is stored, prefixed with [`PARSE_VERSION`], as the
//! series' validator `version`, and
//! an incremental fetch whose `last_updated` matches the stored one skips `/series/vintagedates` and
//! `/series/observations`: the series is marked crawled with the new metadata and keeps its points
//! ([`FetchedSeries::unchanged`]). A first fetch (no known vintage), a series that stored no
//! `last_updated`, a response without one, or a failure reading the stored value fetches the
//! observations as before. QA verifies against the live API that `last_updated` moves on a
//! revision.
//!
//! FRED reports errors as JSON `{"error_code": 400, "error_message": "Bad Request.  ..."}`
//! with the same HTTP status. An unknown `series_id` is HTTP **400** (not 404) with the message
//! `"Bad Request.  The series does not exist."`; that is mapped to [`CrawlError::NotFound`].
//! The vintage-limit 400 splits the request (see [Vintage-date limit](#vintage-date-limit)),
//! and every other 400 stays [`CrawlError::Permanent`]. Recognising a message requires the
//! [`HttpFetcher`](crate::HttpFetcher) error to quote the (redacted) response body; see
//! [`classify_fred_error`].
//!
//! Every series belongs to the one dimensionless dataset [`DATASET`] (defined in
//! `data/datasets/fred.toml`) and keeps its FRED series id as its external id.

use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::adapter::{
    CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::http::Validators;
use crate::lifecycle::CatalogLifecycle;
use crate::persist;
use crate::reference::data_dir;
use crate::source::SourceId;

/// The real FRED API root.
pub const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org/fred";

/// Human-facing series page, used for [`DiscoveredSeries::data_url`] (never requested).
const FRED_WEB_SERIES_URL: &str = "https://fred.stlouisfed.org/series";

/// Declarative catalog selection in the runtime reference-data directory.
pub const CATALOG_RULES_FILE: &str = "catalog_selection/fred.toml";
const CATALOG_PAGE_SIZE: usize = 1000;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FredCatalogRules {
    max_series: usize,
    /// Bound the rows inspected, independently of how many match the selection predicates.
    max_catalog_series: usize,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    rules: Vec<FredCatalogRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FredCatalogRule {
    #[serde(default)]
    release_id: Option<u32>,
    #[serde(default)]
    category_id: Option<u32>,
    #[serde(default)]
    title_contains: Option<String>,
    #[serde(default)]
    id_prefix: Option<String>,
    #[serde(default)]
    frequencies: Vec<String>,
}

impl FredCatalogRules {
    fn parse(text: &str) -> Result<Self, CrawlError> {
        let rules: Self = toml::from_str(text)
            .map_err(|e| CrawlError::Permanent(format!("FRED catalog rules: {e}")))?;
        if rules.max_series == 0
            || rules.max_series > 10_000
            || rules.max_catalog_series == 0
            || rules.max_catalog_series > 100_000
            || rules.rules.is_empty()
            || rules.rules.len() > 16
            || rules.include.len() > 32
            || rules.exclude.len() > 32
        {
            return Err(CrawlError::Permanent(
                "FRED catalog rules: positive limits required (at most 10000 selected, \
                 100000 catalog rows, 16 rules and 32 include/exclude ids)"
                    .into(),
            ));
        }
        for rule in &rules.rules {
            if rule.release_id.is_some() == rule.category_id.is_some()
                || rule.release_id == Some(0)
                || rule
                    .title_contains
                    .as_ref()
                    .is_some_and(|s| s.trim().is_empty())
                || rule.id_prefix.as_ref().is_some_and(|s| s.trim().is_empty())
                || rule.frequencies.iter().any(|f| {
                    !matches!(
                        f.as_str(),
                        "Daily"
                            | "Weekly"
                            | "Biweekly"
                            | "Monthly"
                            | "Quarterly"
                            | "Semiannual"
                            | "Annual"
                    )
                })
            {
                return Err(CrawlError::Permanent(
                    "FRED catalog rule: specify exactly one release_id or category_id, \
                     nonempty predicates and documented FRED frequencies"
                        .into(),
                ));
            }
        }
        for id in rules.include.iter().chain(&rules.exclude) {
            validate_catalog_id(id)?;
        }
        Ok(rules)
    }

    fn fingerprint(&self) -> String {
        // Serializing the parsed rules keeps whitespace and comments out of the identity.
        let bytes = serde_json::to_vec(self).expect("catalog rule serialization is infallible");
        format!(
            "fred-catalog-v1:{}",
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }
}

impl FredCatalogRule {
    fn matches(&self, series: &CatalogSeries) -> bool {
        self.title_contains.as_ref().is_none_or(|needle| {
            series
                .metadata
                .title
                .to_lowercase()
                .contains(&needle.to_lowercase())
        }) && self
            .id_prefix
            .as_ref()
            .is_none_or(|prefix| series.id.starts_with(prefix))
            && (self.frequencies.is_empty()
                || series
                    .metadata
                    .frequency
                    .as_ref()
                    .is_some_and(|f| self.frequencies.contains(f)))
    }
}

fn validate_catalog_id(id: &str) -> Result<(), CrawlError> {
    if id.is_empty() || id.len() > 255 || id.contains(char::is_whitespace) || id.contains('/') {
        return Err(CrawlError::Parse(
            "FRED catalog contains an invalid series id".into(),
        ));
    }
    Ok(())
}

fn catalog_volume_error(what: &str, count: usize, limit: usize) -> CrawlError {
    econ_graph_metrics::crawler::CRAWLER_CATALOG_METRICS
        .guard_rejections_total
        .with_label_values(&["FRED", "volume"])
        .inc();
    tracing::warn!(
        count,
        limit,
        what,
        "FRED catalog volume guard stopped discovery"
    );
    CrawlError::Permanent(format!(
        "FRED catalog volume guard: {what} {count} exceeds {limit}; \
        review selection rules before retrying"
    ))
}

/// Code of the dataset every FRED series belongs to. It has no dimensions: a FRED series id is
/// an opaque key, not a combination of dimension values.
pub const DATASET: &str = "FRED";

/// How this adapter reads observations, prefixed to the stored `last_updated` (see
/// [Re-crawls](self#re-crawls)). Bump it whenever a parsing change should rewrite already-stored
/// series instead of waiting for FRED to update each one.
pub const PARSE_VERSION: &str = "fred-1";

/// FRED's marker for a missing observation.
const MISSING_VALUE: &str = ".";

/// Start of the real-time window for a full vintage fetch: FRED's documented earliest
/// `realtime_start`, meaning "every vintage".
pub const EARLIEST_REALTIME_START: &str = "1776-07-04";

/// Rows per `/series/observations` page (FRED's maximum `limit`).
const OBSERVATIONS_PAGE_SIZE: usize = 100_000;

/// Upper bound on observation pages per real-time window, so a response that keeps claiming
/// more rows can't page forever (10 million rows; daily series with every vintage stay far
/// below this).
const MAX_OBSERVATION_PAGES: usize = 100;

/// FRED's limit on the vintage dates one `/series/observations` request may cover, for JSON.
/// From the `vintage_dates` parameter of
/// <https://fred.stlouisfed.org/docs/api/fred/series_observations.html>: "Your query will be
/// limited to a specific number of vintage dates depending on the series_id, file_type and
/// output_type specified", with "any output_type or other series: 1000 csv, 1000 xlsx, 2000
/// json, 2000 xml" (the smaller limits listed there are for output_type=2, which this adapter
/// doesn't use). FRED applies it to a `realtime_start`..`realtime_end` window too, answering
/// HTTP 400 `"Bad Request.  This exceeds the maximum number of vintage dates allowed (2000)."`
/// (see [`is_too_many_vintage_dates`]).
pub const FRED_DOCUMENTED_VINTAGE_DATE_LIMIT: usize = 2000;

/// Vintage dates each `/series/observations` request is planned to cover: half of
/// [`FRED_DOCUMENTED_VINTAGE_DATE_LIMIT`]. The margin is deliberately wide, because what FRED
/// counts against its limit isn't documented exactly: every window after the first starts the
/// day before its first vintage, which can itself be a vintage date or be counted as one for
/// the values clamped to it; the count comes from `/series/vintagedates`, which leaves out
/// release dates on which the series didn't change; and 1000 is also FRED's own limit for the
/// other formats, so a JSON limit lowered to match them still isn't hit. The cost is a few
/// more requests for the daily series that need splitting at all (DFF's ~5,150 vintages take 6
/// windows instead of 3); a series with fewer vintages than this is still one request.
pub const FRED_MAX_VINTAGE_DATES_PER_REQUEST: usize = 1000;

/// Dates per `/series/vintagedates` page (FRED's maximum `limit`, per
/// <https://fred.stlouisfed.org/docs/api/fred/series_vintagedates.html>).
const VINTAGE_DATES_PAGE_SIZE: usize = 10_000;

/// Upper bound on `/series/vintagedates` pages per listing (100,000 dates, far beyond one per
/// day since ALFRED began).
const MAX_VINTAGE_DATE_PAGES: usize = 10;

/// Lower-cased substrings of a series' `notes` known to mark it as copyright-restricted, e.g. the
/// Coinbase `CBBTCUSD` family ("reproduction ... is prohibited except with prior written
/// permission") or S&P/Case-Shiller ("Copyright, 2026, Standard & Poor's ... Reprinted with
/// permission"). A series whose notes contain one of these known phrases is dropped by
/// [`is_copyright_restricted`] rather than fetched and published; wording outside this list is
/// not caught (a stopgap: `notes` is what discovery returns, and there's no adapter-visible tag
/// to key on instead, since checking FRED's own `copyrighted` tag would need a live call against
/// the real API, which the crawler's network policy blocks in this environment). Revisit with a
/// tag-based check once that access exists (ECO-201).
///
/// This is deliberately broad, and over-drops on purpose: `copyright` and `reprinted with
/// permission` also catch FRED's "citation required" third-party series (e.g. OECD, whose notes
/// read "Copyright, 2026, OECD. Reprinted with permission."), which could legally be republished
/// with a citation, not only its "pre-approval required" series (Coinbase, ICE, S&P/Case-Shiller,
/// NAR). A note merely mentioning copyright without an actual restriction would be dropped too.
/// ECO-201's tag-based check, once live API access exists, can recover the citation-required
/// group and any other false positive; until then, losing a legally-reproducible series from
/// discovery is the safer failure than publishing a restricted one.
const COPYRIGHT_RESTRICTION_MARKERS: &[&str] = &[
    "prior written permission",
    "may not be reproduced",
    "reproduction, retransmission, or other use is prohibited",
    "all rights reserved",
    "copyright",
    "reprinted with permission",
    "used with permission",
];

/// Whether `notes` reads as a copyright/reproduction restriction (see
/// [`COPYRIGHT_RESTRICTION_MARKERS`]). Whitespace (including line breaks) is normalised first, so
/// a marker split across a line break in the source text still matches.
fn is_copyright_restricted(notes: Option<&str>) -> bool {
    let Some(notes) = notes else { return false };
    let lower = notes
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    COPYRIGHT_RESTRICTION_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

/// FRED adapter. See the module docs for endpoints and error mapping.
#[derive(Debug, Clone)]
pub struct FredAdapter {
    base_url: String,
    /// Explicit override ids whose last complete catalog confirmed NotFound.
    skipped_ids: Arc<Mutex<Vec<String>>>,
    /// Facts and rule fingerprint from the last fully validated catalog only.
    catalog: Arc<Mutex<Option<(String, CatalogLifecycle)>>>,
}

impl FredAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            skipped_ids: Arc::default(),
            catalog: Arc::default(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn api_key<'a>(&self, ctx: &'a CrawlCtx) -> Result<&'a str, CrawlError> {
        ctx.keys
            .fred
            .as_deref()
            .ok_or_else(|| CrawlError::Auth("FRED_API_KEY not set".into()))
    }

    /// Returns a complete, validated selection or nothing. No observations are requested here.
    async fn discover_catalog(
        &self,
        ctx: &CrawlCtx,
        rules: &FredCatalogRules,
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let api_key = self.api_key(ctx)?;
        let mut selected = BTreeMap::new();
        let mut scanned = 0;
        // Pin the realtime date for every page of this catalog walk. Count/order validation
        // below also refuses common forms of a catalog changing during pagination.
        let realtime = chrono::Utc::now().date_naive().to_string();
        for rule in &rules.rules {
            let rows = self
                .catalog_rows(ctx, api_key, rule, rules, &realtime, scanned)
                .await?;
            scanned += rows.len();
            for row in rows {
                if !rule.matches(&row) || rules.exclude.contains(&row.id) {
                    continue;
                }
                if selected
                    .get(&row.id)
                    .is_some_and(|previous| previous != &row.metadata)
                {
                    return Err(CrawlError::Transient(format!(
                        "FRED catalog returned conflicting metadata for {}",
                        row.id
                    )));
                }
                selected.insert(row.id.clone(), row.metadata);
                if selected.len() > rules.max_series {
                    return Err(catalog_volume_error(
                        "selected series",
                        selected.len(),
                        rules.max_series,
                    ));
                }
            }
        }
        let mut facts = CatalogLifecycle::default();
        for id in &rules.include {
            if rules.exclude.contains(id) || selected.contains_key(id) {
                continue;
            }
            match self.fetch_metadata(ctx, api_key, id).await {
                Ok((meta, _)) => {
                    selected.insert(
                        id.clone(),
                        FredSeries {
                            title: meta.title,
                            notes: meta.description,
                            units: meta.units,
                            frequency: meta.frequency,
                            seasonal_adjustment: meta.seasonal_adjustment,
                            last_updated: None,
                        },
                    );
                    if selected.len() > rules.max_series {
                        return Err(catalog_volume_error(
                            "selected series",
                            selected.len(),
                            rules.max_series,
                        ));
                    }
                }
                Err(CrawlError::NotFound(_)) => facts.confirmed_missing_ids.push(id.clone()),
                // Parse/permanent errors are not evidence of removal. Never publish a partial
                // override list as a successful catalog, even if other entries succeeded.
                Err(error) => return Err(error),
            }
        }
        if selected.is_empty() {
            return Err(CrawlError::Transient(
                "FRED catalog selection is empty; refusing to infer removals".into(),
            ));
        }
        let mut found = Vec::with_capacity(selected.len());
        for (id, metadata) in selected {
            if is_copyright_restricted(metadata.notes.as_deref()) {
                facts.restricted_ids.push(id.clone());
            } else if metadata
                .title
                .to_ascii_uppercase()
                .contains("(DISCONTINUED)")
            {
                facts.discontinued_ids.push(id.clone());
            }
            found.push(DiscoveredSeries {
                external_id: id.clone(),
                title: metadata.title,
                description: non_empty(metadata.notes),
                units: non_empty(metadata.units),
                frequency: non_empty(metadata.frequency),
                data_url: Some(format!("{FRED_WEB_SERIES_URL}/{id}")),
                dataset: fred_dataset(),
            });
        }
        *self
            .skipped_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = facts.confirmed_missing_ids.clone();
        *self.catalog.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((rules.fingerprint(), facts));
        Ok(found)
    }

    async fn catalog_rows(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        rule: &FredCatalogRule,
        limits: &FredCatalogRules,
        realtime: &str,
        already_scanned: usize,
    ) -> Result<Vec<CatalogSeries>, CrawlError> {
        let (path, parameter, catalog_id) = if let Some(id) = rule.release_id {
            ("/release/series", "release_id", id)
        } else {
            (
                "/category/series",
                "category_id",
                rule.category_id.expect("validated rule"),
            )
        };
        let catalog_id = catalog_id.to_string();
        let page_size = CATALOG_PAGE_SIZE.to_string();
        let mut rows = Vec::new();
        let mut expected_count = None;
        let mut last_id: Option<String> = None;
        loop {
            let offset = rows.len().to_string();
            let query = [
                (parameter, catalog_id.as_str()),
                ("api_key", api_key),
                ("file_type", "json"),
                ("limit", page_size.as_str()),
                ("offset", offset.as_str()),
                ("order_by", "series_id"),
                ("sort_order", "asc"),
                ("realtime_start", realtime),
                ("realtime_end", realtime),
            ];
            let page: CatalogPage = ctx
                .http
                .get_json(SourceId::Fred, &self.url(path), &query)
                .await
                .map_err(classify_fred_error)?;
            if page.count.saturating_add(already_scanned) > limits.max_catalog_series {
                return Err(catalog_volume_error(
                    "catalog rows",
                    page.count.saturating_add(already_scanned),
                    limits.max_catalog_series,
                ));
            }
            if page.offset != rows.len()
                || page.limit != CATALOG_PAGE_SIZE
                || expected_count.is_some_and(|count| page.count != count)
                || page.seriess.len()
                    != (page.count.saturating_sub(rows.len())).min(CATALOG_PAGE_SIZE)
            {
                return Err(CrawlError::Transient(format!(
                    "FRED {parameter}={catalog_id}: incomplete or changing catalog pagination"
                )));
            }
            expected_count = Some(page.count);
            for row in page.seriess {
                validate_catalog_id(&row.id)?;
                if row.metadata.title.trim().is_empty()
                    || row
                        .metadata
                        .frequency
                        .as_ref()
                        .is_none_or(|frequency| frequency.trim().is_empty())
                {
                    return Err(CrawlError::Parse(
                        "FRED catalog row lacks a title or frequency; refusing a partial catalog"
                            .into(),
                    ));
                }
                if last_id.as_ref().is_some_and(|last| row.id <= *last) {
                    return Err(CrawlError::Transient(format!(
                        "FRED {parameter}={catalog_id}: repeated or out-of-order catalog series"
                    )));
                }
                last_id = Some(row.id.clone());
                rows.push(row);
            }
            if rows.len() == page.count {
                break;
            }
        }
        if rows.is_empty() {
            return Err(CrawlError::Transient(format!(
                "FRED {parameter}={catalog_id}: empty catalog; refusing to infer removals"
            )));
        }
        Ok(rows)
    }

    /// The series' metadata and its `last_updated` (`None` if FRED sent none).
    async fn fetch_metadata(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
    ) -> Result<(NewSeriesMetadataLite, Option<String>), CrawlError> {
        let body: SeriesResponse = ctx
            .http
            .get_json(
                SourceId::Fred,
                &self.url("/series"),
                &[
                    ("series_id", series_id),
                    ("api_key", api_key),
                    ("file_type", "json"),
                ],
            )
            .await
            .map_err(classify_fred_error)?;
        let s = body.seriess.into_iter().next().ok_or_else(|| {
            CrawlError::NotFound(format!("FRED series {series_id}: no metadata returned"))
        })?;
        Ok((
            NewSeriesMetadataLite {
                title: s.title,
                description: non_empty(s.notes),
                units: non_empty(s.units),
                frequency: non_empty(s.frequency),
                seasonal_adjustment: non_empty(s.seasonal_adjustment),
            },
            non_empty(s.last_updated),
        ))
    }

    /// Every observation vintage of `series_id`, paged and, when FRED's per-request vintage
    /// limit needs it, split into real-time windows (see [Vintages](self#vintages) and
    /// [Vintage-date limit](self#vintage-date-limit)).
    ///
    /// `observation_start` limits the observation dates. `known_vintage` starts the real-time
    /// window there and drops rows already stored; `None` asks for every vintage. The window's
    /// vintage dates are listed first, and it is requested in windows of at most
    /// [`FRED_MAX_VINTAGE_DATES_PER_REQUEST`] of them.
    async fn fetch_observations(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        observation_start: Option<NaiveDate>,
        known_vintage: Option<NaiveDate>,
    ) -> Result<Vec<FetchedPoint>, CrawlError> {
        // The day before the known vintage, so vintage `known_vintage` itself is re-read.
        let start = known_vintage
            .and_then(|k| k.pred_opt())
            .unwrap_or_else(earliest_realtime_start);
        let whole = RealtimeWindow {
            start,
            end: latest_realtime_end(),
            keep_from: known_vintage,
            vintages: None,
        };
        let vintages = self
            .fetch_vintage_dates(ctx, api_key, series_id, whole.start, whole.end)
            .await?;
        let mut pending = whole.split(&vintages, FRED_MAX_VINTAGE_DATES_PER_REQUEST);
        pending.reverse(); // used as a stack, earliest window on top

        let mut rows = Vec::new();
        while let Some(window) = pending.pop() {
            match self
                .fetch_window(ctx, api_key, series_id, observation_start, &window)
                .await
            {
                Ok(window_rows) => rows.extend(window_rows),
                Err(err) if is_too_many_vintage_dates(&err) => {
                    let parts = self
                        .split_rejected(ctx, api_key, series_id, &window)
                        .await?;
                    if parts.len() < 2 {
                        return Err(err);
                    }
                    // Planned windows stay well under FRED's documented limit, so this means
                    // FRED counts or limits vintage dates differently than documented.
                    tracing::warn!(
                        series_id,
                        vintages = window.vintages.as_ref().map(Vec::len),
                        windows = parts.len(),
                        realtime_start = %window.start,
                        realtime_end = %window.end,
                        "FRED rejected a real-time window for too many vintage dates; \
                         splitting it further"
                    );
                    pending.extend(parts.into_iter().rev());
                }
                Err(err) => return Err(classify_fred_error(err)),
            }
        }
        Ok(to_points(rows))
    }

    /// The windows to retry `window` as after FRED rejected it for too many vintage dates: at
    /// most [`FRED_MAX_VINTAGE_DATES_PER_REQUEST`] vintages each, and at most half of
    /// `window`'s, so a window that FRED counts differently from `/series/vintagedates` still
    /// shrinks on every retry. Fewer than two windows means it can't be split any further.
    async fn split_rejected(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        window: &RealtimeWindow,
    ) -> Result<Vec<RealtimeWindow>, CrawlError> {
        let vintages = match &window.vintages {
            Some(v) => v.clone(),
            None => {
                self.fetch_vintage_dates(ctx, api_key, series_id, window.start, window.end)
                    .await?
            }
        };
        if vintages.len() < 2 {
            return Ok(vec![]);
        }
        let per_window = FRED_MAX_VINTAGE_DATES_PER_REQUEST.min(vintages.len().div_ceil(2));
        Ok(window.split(&vintages, per_window))
    }

    /// One real-time window's observation rows, paged with `limit`/`offset`. Each row's `keep`
    /// says whether this window is the one that reads it (see [`RealtimeWindow::keeps`]).
    /// Errors are returned unclassified, so the caller can recognise the vintage-limit 400.
    async fn fetch_window(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        observation_start: Option<NaiveDate>,
        window: &RealtimeWindow,
    ) -> Result<Vec<VintageRow>, CrawlError> {
        let observation_start = observation_start.map(|d| d.format("%Y-%m-%d").to_string());
        let realtime_start = window.start.format("%Y-%m-%d").to_string();
        let realtime_end = window.end.format("%Y-%m-%d").to_string();
        let limit = OBSERVATIONS_PAGE_SIZE.to_string();
        let mut rows = Vec::new();
        for _ in 0..MAX_OBSERVATION_PAGES {
            let offset = rows.len().to_string();
            let mut query = vec![
                ("series_id", series_id),
                ("api_key", api_key),
                ("file_type", "json"),
                ("realtime_start", realtime_start.as_str()),
                ("realtime_end", realtime_end.as_str()),
                ("limit", limit.as_str()),
                ("offset", offset.as_str()),
            ];
            if let Some(s) = &observation_start {
                query.push(("observation_start", s.as_str()));
            }
            let page: ObservationsResponse = ctx
                .http
                .get_json(SourceId::Fred, &self.url("/series/observations"), &query)
                .await?;
            let got = page.observations.len();
            for o in page.observations {
                let mut row = parse_row(series_id, o)?;
                row.keep = window.keeps(row.realtime_start);
                rows.push(row);
            }
            if got == 0 || rows.len() >= page.count {
                return Ok(rows);
            }
        }
        Err(CrawlError::Permanent(format!(
            "FRED {series_id}: more than {MAX_OBSERVATION_PAGES} pages of \
             {OBSERVATIONS_PAGE_SIZE} observations"
        )))
    }

    /// The series' vintage dates from `start` to `end` (`/series/vintagedates`, paged), sorted
    /// and deduplicated.
    async fn fetch_vintage_dates(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<NaiveDate>, CrawlError> {
        let realtime_start = start.format("%Y-%m-%d").to_string();
        let realtime_end = end.format("%Y-%m-%d").to_string();
        let limit = VINTAGE_DATES_PAGE_SIZE.to_string();
        let mut dates = Vec::new();
        let mut received = 0;
        for _ in 0..MAX_VINTAGE_DATE_PAGES {
            let offset = received.to_string();
            let page: VintageDatesResponse = ctx
                .http
                .get_json(
                    SourceId::Fred,
                    &self.url("/series/vintagedates"),
                    &[
                        ("series_id", series_id),
                        ("api_key", api_key),
                        ("file_type", "json"),
                        ("realtime_start", realtime_start.as_str()),
                        ("realtime_end", realtime_end.as_str()),
                        ("limit", limit.as_str()),
                        ("offset", offset.as_str()),
                    ],
                )
                .await
                .map_err(classify_fred_error)?;
            let got = page.vintage_dates.len();
            received += got;
            for s in &page.vintage_dates {
                dates.push(parse_date(series_id, "vintage date", s)?);
            }
            if got == 0 || received >= page.count {
                dates.retain(|d| (start..=end).contains(d));
                dates.sort_unstable();
                dates.dedup();
                return Ok(dates);
            }
        }
        Err(CrawlError::Permanent(format!(
            "FRED {series_id}: more than {MAX_VINTAGE_DATE_PAGES} pages of \
             {VINTAGE_DATES_PAGE_SIZE} vintage dates"
        )))
    }
}

impl Default for FredAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for FredAdapter {
    fn id(&self) -> SourceId {
        SourceId::Fred
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// Walks the complete configured catalog rules, under both scan and selection limits.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let path = data_dir().join(CATALOG_RULES_FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| CrawlError::Permanent(format!("reading {}: {e}", path.display())))?;
        let rules = FredCatalogRules::parse(&text)?;
        self.discover_catalog(ctx, &rules).await
    }

    fn discovery_is_complete(&self) -> bool {
        true
    }

    fn catalog_scope(&self) -> Option<String> {
        self.catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|(scope, _)| scope.clone())
    }

    fn catalog_lifecycle(&self) -> CatalogLifecycle {
        self.catalog
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|(_, facts)| facts.clone())
            .unwrap_or_default()
    }

    /// `/series` (metadata), then every vintage of the observations on or after `since` from
    /// `/series/observations` (one request per [`OBSERVATIONS_PAGE_SIZE`] rows per real-time
    /// window; see [Vintage-date limit](self#vintage-date-limit)). Refuses with
    /// [`CrawlError::Restricted`] if the series' notes are copyright-restricted (see
    /// [`is_copyright_restricted`]), whether or not it was reached via discovery.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        self.fetch(ctx, external_id, since, None).await
    }

    /// Explicit override ids whose lookup confirmed NotFound in the last complete catalog.
    fn curated_missing(&self) -> Vec<String> {
        self.skipped_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn tracks_vintages(&self) -> bool {
        true
    }

    /// With a `known_vintage`, asks only for vintages after it, for every observation date
    /// (`since` is ignored, so revisions to old dates arrive). Without one, as
    /// [`fetch_series`](SourceAdapter::fetch_series).
    async fn fetch_series_incremental(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
        known_vintage: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let since = if known_vintage.is_some() { None } else { since };
        self.fetch(ctx, external_id, since, known_vintage).await
    }
}

impl FredAdapter {
    async fn fetch(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
        known_vintage: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let api_key = self.api_key(ctx)?;
        let (metadata, last_updated) = self.fetch_metadata(ctx, api_key, external_id).await?;
        if is_copyright_restricted(metadata.description.as_deref()) {
            tracing::warn!(
                series_id = external_id,
                "refusing to fetch copyright-restricted series"
            );
            return Err(CrawlError::Restricted(format!(
                "FRED {external_id}: copyright-restricted, refusing to fetch"
            )));
        }
        let validators = last_updated.map(|v| Validators {
            version: Some(format!("{PARSE_VERSION}:{v}")),
            ..Validators::default()
        });
        if known_vintage.is_some()
            && validators.is_some()
            && stored_validators(ctx, external_id).await == validators
        {
            return Ok(FetchedSeries {
                metadata: Some(metadata),
                ..FetchedSeries::unchanged(fred_dataset(), validators.unwrap_or_default())
            });
        }
        let points = self
            .fetch_observations(ctx, api_key, external_id, since, known_vintage)
            .await?;
        Ok(FetchedSeries {
            metadata: Some(metadata),
            points,
            dataset: fred_dataset(),
            validators,
        })
    }
}

/// The validators `external_id` stored with its last fetch; `None` if it stored none or they
/// can't be read (the fetch is then a full one).
async fn stored_validators(ctx: &CrawlCtx, external_id: &str) -> Option<Validators> {
    match persist::stored_fetch_state(&ctx.pool, SourceId::Fred, &[external_id.to_string()]).await {
        Ok(mut stored) => stored.remove(external_id)?.validators,
        Err(error) => {
            tracing::warn!(series_id = external_id, %error,
                "FRED: reading stored validators failed; fetching observations");
            None
        }
    }
}

/// Refines an [`HttpFetcher`](crate::HttpFetcher) error with FRED's error conventions:
/// an HTTP 400 whose message says the series does not exist becomes `NotFound`; everything
/// else is returned unchanged (so other 400s stay `Permanent`).
///
/// FRED's body for this case is `{"error_code":400,"error_message":"Bad Request.  The series does not exist."}`,
/// so this only fires when the error message quotes the response body.
pub fn classify_fred_error(err: CrawlError) -> CrawlError {
    match err {
        CrawlError::Permanent(msg)
            if msg.starts_with("HTTP 400")
                && msg.to_ascii_lowercase().contains("series does not exist") =>
        {
            CrawlError::NotFound(msg)
        }
        e => e,
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn parse_date(series_id: &str, field: &str, s: &str) -> Result<NaiveDate, CrawlError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|e| CrawlError::Parse(format!("FRED {series_id}: invalid {field} {s:?}: {e}")))
}

/// One `/series/observations` row: the value of `date` published on `realtime_start`.
#[derive(Debug, Clone, PartialEq)]
struct VintageRow {
    date: NaiveDate,
    realtime_start: NaiveDate,
    value: Option<BigDecimal>,
    /// Whether this row becomes a point. A row that isn't kept was already stored (before the
    /// known vintage) or is read by an earlier window; it still counts when choosing each
    /// date's original release.
    keep: bool,
}

/// A real-time window of `/series/observations` requests (`realtime_start..=realtime_end`).
///
/// A series' history is read as consecutive windows whose kept rows partition the real-time
/// axis: a row is kept by the one window whose `keep_from..=end` holds its `realtime_start`.
/// Every window after the first starts the day before its `keep_from`, its first vintage, so
/// FRED also returns the values in effect on that day (with `realtime_start` clamped to the
/// window start); those are not kept, but show the date was already published, so a later row
/// isn't mistaken for an original release. This is the same mechanism as an incremental fetch.
#[derive(Debug, Clone, PartialEq)]
struct RealtimeWindow {
    start: NaiveDate,
    end: NaiveDate,
    /// The earliest `realtime_start` kept; `None` keeps everything from `start`.
    keep_from: Option<NaiveDate>,
    /// The vintage dates this window covers, when known (from `/series/vintagedates`).
    vintages: Option<Vec<NaiveDate>>,
}

impl RealtimeWindow {
    /// Whether this window is the one that turns a row starting on `realtime_start` into a point.
    fn keeps(&self, realtime_start: NaiveDate) -> bool {
        self.keep_from.is_none_or(|k| realtime_start >= k) && realtime_start <= self.end
    }

    /// This window as consecutive windows of at most `per_window` of `vintages` (the vintage
    /// dates inside it, sorted) each. The first keeps this window's start and `keep_from`; each
    /// later one starts the day before its first vintage and keeps from that vintage; each
    /// ends the day before the next one's first vintage, and the last at this window's end.
    /// No vintages, or no more than `per_window`, is this window unchanged.
    fn split(&self, vintages: &[NaiveDate], per_window: usize) -> Vec<RealtimeWindow> {
        let groups: Vec<&[NaiveDate]> = vintages.chunks(per_window.max(1)).collect();
        if groups.len() <= 1 {
            return vec![RealtimeWindow {
                vintages: Some(vintages.to_vec()),
                ..self.clone()
            }];
        }
        groups
            .iter()
            .enumerate()
            .map(|(i, group)| {
                let first = group[0];
                let (start, keep_from) = if i == 0 {
                    (self.start, self.keep_from)
                } else {
                    (first.pred_opt().unwrap_or(first), Some(first))
                };
                let end = groups
                    .get(i + 1)
                    .and_then(|next| next[0].pred_opt())
                    .unwrap_or(self.end);
                RealtimeWindow {
                    start,
                    end,
                    keep_from,
                    vintages: Some(group.to_vec()),
                }
            })
            .collect()
    }
}

/// Whether `err` is FRED rejecting a request for covering too many vintage dates, the one HTTP
/// 400 that is answered by splitting the request rather than failing the series. FRED's body
/// reads `"Bad Request.  This exceeds the maximum number of vintage dates allowed (2000)."`;
/// only the stable part of it is matched, so a changed limit in the text still matches.
fn is_too_many_vintage_dates(err: &CrawlError) -> bool {
    matches!(err, CrawlError::Permanent(msg)
        if msg.starts_with("HTTP 400")
            && msg.to_ascii_lowercase().contains("maximum number of vintage dates"))
}

/// [`EARLIEST_REALTIME_START`] as a date.
fn earliest_realtime_start() -> NaiveDate {
    NaiveDate::from_ymd_opt(1776, 7, 4).expect("valid date")
}

/// End of every fetch's real-time window: FRED's documented latest `realtime_end`, meaning
/// "still current".
fn latest_realtime_end() -> NaiveDate {
    NaiveDate::from_ymd_opt(9999, 12, 31).expect("valid date")
}

fn parse_row(series_id: &str, o: FredObservation) -> Result<VintageRow, CrawlError> {
    let date = parse_date(series_id, "date", &o.date)?;
    let realtime_start = parse_date(series_id, "realtime_start", &o.realtime_start)?;
    let value = match o.value.trim() {
        MISSING_VALUE | "" => None,
        v => Some(BigDecimal::from_str(v).map_err(|e| {
            CrawlError::Parse(format!(
                "FRED {series_id}: invalid value {v:?} on {date}: {e}"
            ))
        })?),
    };
    Ok(VintageRow {
        date,
        realtime_start,
        value,
        keep: true,
    })
}

/// Turns vintage rows into points: each kept row becomes one, with `revision_date =
/// realtime_start`, and is the original release if no row of its date (kept or not) starts
/// earlier. Rows not kept were stored before the known vintage or belong to another real-time
/// window; when a date's earliest row is one of them, none of its kept rows is an original
/// release. See [Vintages](self#vintages).
///
/// `is_original_release` is part of the `data_points` upsert key, so a full, an incremental
/// and a windowed fetch must mark the same rows, or one vintage would be stored twice. They
/// do: the earliest row is chosen among all rows, before anything is dropped. Kept rows are
/// unique by `(date, realtime_start)`, since windows keep disjoint ranges of `realtime_start`.
fn to_points(mut rows: Vec<VintageRow>) -> Vec<FetchedPoint> {
    rows.sort_by_key(|r| (r.date, r.realtime_start, !r.keep));
    let mut points = Vec::with_capacity(rows.len());
    let mut earliest: Option<(NaiveDate, NaiveDate)> = None;
    for r in rows {
        if earliest.is_none_or(|(date, _)| date != r.date) {
            earliest = Some((r.date, r.realtime_start));
        }
        if !r.keep {
            continue;
        }
        points.push(FetchedPoint {
            date: r.date,
            value: r.value,
            revision_date: r.realtime_start,
            is_original_release: earliest == Some((r.date, r.realtime_start)),
        });
    }
    points
}

/// The dataset of every FRED series: [`DATASET`], with no dimension values.
fn fred_dataset() -> SeriesDataset {
    SeriesDataset {
        code: DATASET.into(),
        ..SeriesDataset::default()
    }
}

// ---- Wire formats (only the fields we use; FRED sends many more) ----

#[derive(Debug, Deserialize)]
struct SeriesResponse {
    seriess: Vec<FredSeries>,
}

#[derive(Debug, Deserialize)]
struct CatalogPage {
    count: usize,
    offset: usize,
    limit: usize,
    seriess: Vec<CatalogSeries>,
}

#[derive(Debug, Deserialize)]
struct CatalogSeries {
    id: String,
    #[serde(flatten)]
    metadata: FredSeries,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct FredSeries {
    title: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    frequency: Option<String>,
    #[serde(default)]
    seasonal_adjustment: Option<String>,
    /// When FRED last changed the series (e.g. `2026-08-28 07:54:02-05`), observations and
    /// revisions included. See [Re-crawls](self#re-crawls).
    #[serde(default)]
    last_updated: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ObservationsResponse {
    /// Total rows across all pages. Required: without it, paging can't tell a full page from the
    /// last one.
    count: usize,
    observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
struct VintageDatesResponse {
    /// Total dates across all pages.
    count: usize,
    vintage_dates: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct FredObservation {
    date: String,
    value: String,
    realtime_start: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    fn catalog_rules(max_series: usize, max_catalog: usize) -> FredCatalogRules {
        FredCatalogRules::parse(&format!(
            "max_series={max_series}\nmax_catalog_series={max_catalog}\n\
             [[rules]]\nrelease_id=53\ntitle_contains='domestic product'\nfrequencies=['Quarterly']"
        ))
        .unwrap()
    }

    fn catalog_row(id: &str, title: &str) -> serde_json::Value {
        serde_json::json!({"id":id,"title":title,"frequency":"Quarterly","units":"Dollars"})
    }

    fn catalog_page(count: usize, offset: usize, rows: Vec<serde_json::Value>) -> Reply {
        Reply::json_str(
            serde_json::json!({
                "count":count,"offset":offset,"limit":CATALOG_PAGE_SIZE,"seriess":rows
            })
            .to_string(),
        )
    }

    #[tokio::test]
    async fn catalog_automatically_selects_new_ids_and_reports_retired_overrides() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/release/series").query("release_id", "53"),
            catalog_page(
                3,
                0,
                vec![
                    catalog_row("GDPNEW", "Gross Domestic Product"),
                    catalog_row("OTHER", "An unrelated series"),
                    catalog_row("REALGDPNEW", "Real Gross Domestic Product"),
                ],
            ),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "OLDGDP"),
            Reply::json_str(ERR_NO_SERIES).with_status(400),
        )
        .await;
        let mut rules = catalog_rules(10, 10);
        rules.include = vec!["OLDGDP".into()];
        let adapter = FredAdapter::new(mock.base_url());
        let found = adapter.discover_catalog(&test_ctx(), &rules).await.unwrap();
        assert_eq!(
            found
                .iter()
                .map(|s| s.external_id.as_str())
                .collect::<Vec<_>>(),
            ["GDPNEW", "REALGDPNEW"]
        );
        assert_eq!(
            adapter.catalog_lifecycle().confirmed_missing_ids,
            ["OLDGDP"]
        );
        assert!(adapter.discovery_is_complete());
        assert_eq!(adapter.catalog_scope().unwrap(), rules.fingerprint());
        assert_eq!(
            mock.received_requests().await.len(),
            2,
            "catalog discovery must not fetch any observations"
        );
    }

    #[tokio::test]
    async fn catalog_picks_up_a_replacement_without_changing_rules_or_merging_histories() {
        let mock = MockSource::start().await;
        let route = Route::get("/release/series");
        mock.mount(
            &route,
            catalog_page(1, 0, vec![catalog_row("OLDGDP", "Gross Domestic Product")]),
        )
        .await;
        let adapter = FredAdapter::new(mock.base_url());
        let rules = catalog_rules(10, 10);
        let old = adapter.discover_catalog(&test_ctx(), &rules).await.unwrap();
        let scope = adapter.catalog_scope();
        mock.server().reset().await;
        mock.mount(
            &route,
            catalog_page(1, 0, vec![catalog_row("NEWGDP", "Gross Domestic Product")]),
        )
        .await;
        let new = adapter.discover_catalog(&test_ctx(), &rules).await.unwrap();
        assert_eq!(old[0].external_id, "OLDGDP");
        assert_eq!(new[0].external_id, "NEWGDP");
        assert_eq!(
            scope,
            adapter.catalog_scope(),
            "upstream changes do not change selection identity"
        );
        assert!(
            new.iter().all(|s| s.external_id != "OLDGDP"),
            "absence is evidence for the conservative lifecycle writer, not an id rewrite"
        );
    }

    #[tokio::test]
    async fn catalog_retains_restricted_and_discontinued_metadata_with_explicit_facts() {
        let mock = MockSource::start().await;
        let mut restricted = catalog_row("GDP1", "Gross Domestic Product");
        restricted["notes"] = "All rights reserved.".into();
        mock.mount(
            &Route::get("/release/series"),
            catalog_page(
                2,
                0,
                vec![
                    restricted,
                    catalog_row("GDP2", "Gross Domestic Product (DISCONTINUED)"),
                ],
            ),
        )
        .await;
        let adapter = FredAdapter::new(mock.base_url());
        let found = adapter
            .discover_catalog(&test_ctx(), &catalog_rules(10, 10))
            .await
            .unwrap();
        assert_eq!(found.len(), 2, "metadata is kept internally, never deleted");
        let facts = adapter.catalog_lifecycle();
        assert_eq!(facts.restricted_ids, ["GDP1"]);
        assert_eq!(facts.discontinued_ids, ["GDP2"]);
    }

    #[tokio::test]
    async fn catalog_volume_guards_stop_before_overrides_or_observations() {
        for (max_series, max_catalog, expected) in
            [(1, 10, "selected series"), (10, 1, "catalog rows")]
        {
            let mock = MockSource::start().await;
            mock.mount(
                &Route::get("/release/series"),
                catalog_page(
                    2,
                    0,
                    vec![
                        catalog_row("GDP1", "Gross Domestic Product"),
                        catalog_row("GDP2", "Real Gross Domestic Product"),
                    ],
                ),
            )
            .await;
            let mut rules = catalog_rules(max_series, max_catalog);
            rules.include = vec!["UNRATE".into()];
            let adapter = FredAdapter::new(mock.base_url());
            let error = adapter
                .discover_catalog(&test_ctx(), &rules)
                .await
                .unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
            assert!(adapter.catalog_scope().is_none());
            assert_eq!(mock.received_requests().await.len(), 1);
        }
    }

    #[tokio::test]
    async fn catalog_validates_every_page_and_rejects_drift_without_publishing_partial_state() {
        for scenario in ["valid", "count_change", "truncated", "duplicate"] {
            let mock = MockSource::start().await;
            let first = (0..CATALOG_PAGE_SIZE)
                .map(|i| catalog_row(&format!("GDP{i:04}"), "Gross Domestic Product"))
                .collect();
            mock.mount(
                &Route::get("/release/series").query("offset", "0"),
                catalog_page(1001, 0, first),
            )
            .await;
            let count = if scenario == "count_change" {
                1002
            } else {
                1001
            };
            let last = if scenario == "truncated" {
                vec![]
            } else {
                vec![catalog_row(
                    if scenario == "duplicate" {
                        "GDP0000"
                    } else {
                        "GDP1000"
                    },
                    "Gross Domestic Product",
                )]
            };
            mock.mount(
                &Route::get("/release/series").query("offset", "1000"),
                catalog_page(count, 1000, last),
            )
            .await;
            let adapter = FredAdapter::new(mock.base_url());
            let result = adapter
                .discover_catalog(&test_ctx(), &catalog_rules(2000, 3000))
                .await;
            if scenario == "valid" {
                assert_eq!(result.unwrap().len(), 1001);
            } else {
                assert!(result.is_err(), "{scenario} must fail the entire snapshot");
                assert!(adapter.catalog_scope().is_none());
            }
            assert_eq!(mock.received_requests().await.len(), 2);
        }
    }

    #[tokio::test]
    async fn category_catalog_and_exclusions_use_the_same_complete_selection_path() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/category/series").query("category_id", "125"),
            catalog_page(
                2,
                0,
                vec![catalog_row("A", "Income"), catalog_row("B", "Income")],
            ),
        )
        .await;
        let rules = FredCatalogRules::parse(
            "max_series=10\nmax_catalog_series=10\nexclude=['B']\n\
            [[rules]]\ncategory_id=125\nfrequencies=['Quarterly']",
        )
        .unwrap();
        let found = FredAdapter::new(mock.base_url())
            .discover_catalog(&test_ctx(), &rules)
            .await
            .unwrap();
        assert_eq!(
            found
                .iter()
                .map(|s| s.external_id.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
    }

    #[test]
    fn catalog_scope_ignores_comments_and_rules_validate_before_any_request() {
        let text = "max_series=10\nmax_catalog_series=20\n[[rules]]\nrelease_id=53";
        assert_eq!(
            FredCatalogRules::parse(text).unwrap().fingerprint(),
            FredCatalogRules::parse(&format!("# comment\n{text}\n"))
                .unwrap()
                .fingerprint()
        );
        for suffix in [
            "\ncategory_id=125",
            "\nfrequencies=['Wrong']",
            "\nid_prefix=''",
            "\nmisspelled=true",
        ] {
            assert!(FredCatalogRules::parse(&format!("{text}{suffix}")).is_err());
        }
    }

    const SERIES_GDP: &str = include_str!("../../tests/fixtures/fred/series_gdp.json");
    const OBS_GDP: &str = include_str!("../../tests/fixtures/fred/observations_gdp.json");
    const OBS_GDP_SINCE: &str =
        include_str!("../../tests/fixtures/fred/observations_gdp_since_2026-04-28.json");
    const ERR_NO_SERIES: &str =
        include_str!("../../tests/fixtures/fred/error_series_does_not_exist.json");
    const ERR_BAD_VARIABLE: &str =
        include_str!("../../tests/fixtures/fred/error_bad_variable.json");
    const VINTAGES_GDP: &str = include_str!("../../tests/fixtures/fred/vintagedates_gdp.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    /// Serves [`VINTAGES_GDP`] for every `/series/vintagedates` request.
    async fn mount_vintages(mock: &MockSource) {
        mock.mount(
            &Route::get("/series/vintagedates"),
            Reply::json_str(VINTAGES_GDP),
        )
        .await;
    }

    async fn mock_gdp() -> MockSource {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
        )
        .await;
        mount_vintages(&mock).await;
        mock.mount(
            &Route::get("/series/observations").query("series_id", "GDP"),
            Reply::json_str(OBS_GDP),
        )
        .await;
        mock
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(FredAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(FredAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(FredAdapter::default().id(), SourceId::Fred);
    }

    fn query_of(r: &wiremock::Request) -> Vec<(String, String)> {
        r.url.query_pairs().into_owned().collect()
    }

    fn param<'a>(q: &'a [(String, String)], key: &str) -> Option<&'a str> {
        q.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    fn point(date: &str, value: Option<&str>, revision: &str, original: bool) -> FetchedPoint {
        FetchedPoint {
            date: d(date),
            value: value.map(|v| BigDecimal::from_str(v).unwrap()),
            revision_date: d(revision),
            is_original_release: original,
        }
    }

    #[tokio::test]
    async fn fetch_parses_metadata_and_every_vintage() {
        let mock = mock_gdp().await;
        let ctx = test_ctx();
        let s = FredAdapter::new(mock.base_url())
            .fetch_series(&ctx, "GDP", None)
            .await
            .unwrap();

        let m = s.metadata.unwrap();
        assert_eq!(m.title, "Gross Domestic Product");
        assert_eq!(m.units.as_deref(), Some("Billions of Dollars"));
        assert_eq!(m.frequency.as_deref(), Some("Quarterly"));
        assert_eq!(
            m.seasonal_adjustment.as_deref(),
            Some("Seasonally Adjusted Annual Rate")
        );
        assert!(m.description.unwrap().starts_with("BEA Account Code"));
        assert_eq!(s.dataset, fred_dataset());

        // One point per vintage, revision_date = realtime_start, the earliest vintage of each
        // date is its original release, and "." is kept as a missing value.
        assert_eq!(
            s.points,
            vec![
                point("2025-04-01", Some("30331.117"), "2025-07-30", true),
                point("2025-04-01", Some("30353.902"), "2025-08-28", false),
                point("2025-04-01", Some("30485.729"), "2025-09-25", false),
                point("2025-07-01", None, "2025-10-30", true),
                point("2025-07-01", Some("31095.089"), "2025-12-23", false),
                point("2026-01-01", Some("31722.514"), "2026-04-29", true),
            ]
        );

        // Metadata, the vintage dates, then one observations request (the series has far fewer
        // vintages than one request may cover), all with the key and file_type=json; both
        // the vintage dates and the observations cover every vintage.
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 3);
        for r in &reqs {
            let q = query_of(r);
            assert_eq!(param(&q, "api_key"), Some(TEST_API_KEY));
            assert_eq!(param(&q, "file_type"), Some("json"));
        }
        let q = query_of(&reqs[1]);
        assert_eq!(reqs[1].url.path(), "/series/vintagedates");
        assert_eq!(param(&q, "realtime_start"), Some(EARLIEST_REALTIME_START));
        assert_eq!(param(&q, "realtime_end"), Some("9999-12-31"));
        assert_eq!(param(&q, "limit"), Some("10000"));
        assert_eq!(param(&q, "offset"), Some("0"));
        let q = query_of(&reqs[2]);
        assert_eq!(reqs[2].url.path(), "/series/observations");
        assert_eq!(param(&q, "realtime_start"), Some(EARLIEST_REALTIME_START));
        assert_eq!(param(&q, "realtime_end"), Some("9999-12-31"));
        assert_eq!(param(&q, "limit"), Some("100000"));
        assert_eq!(param(&q, "offset"), Some("0"));
        assert_eq!(param(&q, "observation_start"), None);
    }

    /// A manually triggered crawl (triggerCrawl) must not pull in a series copyright-restriction
    /// would otherwise exclude from discovery: `fetch_series` checks `notes` itself and refuses
    /// before fetching observations, so the restricted series is never persisted.
    #[tokio::test]
    async fn fetch_series_refuses_a_copyright_restricted_series() {
        let mock = MockSource::start().await;
        let restricted = restricted_fixture("CBBTCUSD", "Coinbase Bitcoin");
        mock.mount(
            &Route::get("/series").query("series_id", "CBBTCUSD"),
            Reply::json_str(restricted),
        )
        .await;
        // No /series/observations route mounted: if the adapter fetched observations anyway,
        // the request would fail with a connection/404 error instead of this one, failing the
        // assertion below.

        let err = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CBBTCUSD", None)
            .await
            .unwrap_err();
        assert_eq!(
            err,
            CrawlError::Restricted(
                "FRED CBBTCUSD: copyright-restricted, refusing to fetch".to_string()
            )
        );
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 1, "must not fetch observations");
    }

    #[tokio::test]
    async fn incremental_fetch_asks_only_for_vintages_from_the_known_one() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        // Known vintage 2026-04-29: the window starts the day before, so that vintage is re-read.
        mock.mount_expect(
            &Route::get("/series/observations")
                .query("series_id", "GDP")
                .query("realtime_start", "2026-04-28")
                .query("realtime_end", "9999-12-31"),
            Reply::json_str(OBS_GDP_SINCE),
            1,
        )
        .await;
        let s = FredAdapter::new(mock.base_url())
            .fetch_series_incremental(
                &test_ctx(),
                "GDP",
                Some(d("2026-01-01")),
                Some(d("2026-04-29")),
            )
            .await
            .unwrap();
        // Rows in effect before the known vintage are already stored. The revision to an old
        // quarter arrives although it predates `since`. January's first vintage (the known one)
        // is re-read, with a same-day correction, and is still its original release; its
        // revision isn't. April, first published after the known vintage, is an original release.
        assert_eq!(
            s.points,
            vec![
                point("2025-04-01", Some("30502.3"), "2026-05-28", false),
                point("2026-01-01", Some("31725.0"), "2026-04-29", true),
                point("2026-01-01", Some("31688.25"), "2026-05-28", false),
                point("2026-04-01", Some("32101.6"), "2026-05-28", true),
            ]
        );
        mock.server().verify().await;
        // `since` is ignored with a known vintage, so revisions to old dates arrive.
        let reqs = mock.received_requests().await;
        assert!(reqs
            .iter()
            .all(|r| param(&query_of(r), "observation_start").is_none()));
    }

    #[tokio::test]
    async fn incremental_fetch_without_known_vintage_is_a_full_fetch() {
        let mock = mock_gdp().await;
        let s = FredAdapter::new(mock.base_url())
            .fetch_series_incremental(&test_ctx(), "GDP", None, None)
            .await
            .unwrap();
        assert_eq!(s.points.len(), 6);
        let reqs = mock.received_requests().await;
        assert_eq!(reqs[2].url.path(), "/series/observations");
        assert_eq!(
            param(&query_of(&reqs[2]), "realtime_start"),
            Some(EARLIEST_REALTIME_START)
        );
        assert!(FredAdapter::default().tracks_vintages());
    }

    #[tokio::test]
    async fn an_unchanged_last_updated_skips_the_observations() {
        use crate::dataset::DatasetCatalog;
        use crate::persist::stable_id_tests::{database_url, FreshDb};

        let Some(admin_url) = database_url() else {
            return;
        };
        let db = FreshDb::create(&admin_url, "econgraph_fred_validators").await;
        let mut catalog = DatasetCatalog::empty();
        catalog.load_adapter(&FredAdapter::default()).unwrap();
        persist::sync_datasets(&db.pool, &catalog).await.unwrap();
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let mock = mock_gdp().await;
        let adapter = FredAdapter::new(mock.base_url());
        let observation_requests = || async {
            mock.received_requests()
                .await
                .iter()
                .filter(|r| r.url.path() == "/series/observations")
                .count()
        };

        // A first fetch stores `last_updated` with the points.
        let first = adapter
            .fetch_series_incremental(&ctx, "GDP", None, None)
            .await
            .unwrap();
        let version = Validators {
            version: Some(format!("{PARSE_VERSION}:2026-08-28 07:54:02-05")),
            ..Validators::default()
        };
        assert_eq!(first.validators.as_ref(), Some(&version));
        persist::persist_series(&db.pool, SourceId::Fred, "GDP", &first)
            .await
            .unwrap();
        assert_eq!(observation_requests().await, 1);

        // Unchanged: metadata is refreshed, observations aren't requested.
        let known = Some(d("2026-04-29"));
        let again = adapter
            .fetch_series_incremental(&ctx, "GDP", None, known)
            .await
            .unwrap();
        assert_eq!(
            again,
            FetchedSeries {
                metadata: first.metadata.clone(),
                ..FetchedSeries::unchanged(fred_dataset(), version)
            }
        );
        assert_eq!(observation_requests().await, 1);

        // Without a known vintage (no stored points) the observations are always fetched.
        adapter
            .fetch_series_incremental(&ctx, "GDP", None, None)
            .await
            .unwrap();
        assert_eq!(observation_requests().await, 2);

        // A series that became copyright-restricted is refused even though its stored
        // `last_updated` still matches.
        mock.reset().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json_str(SERIES_GDP.replace(
                "BEA Account Code: A191RC",
                "Copyright, 2026, All rights reserved.",
            )),
        )
        .await;
        let refused = adapter
            .fetch_series_incremental(&ctx, "GDP", None, known)
            .await
            .unwrap_err();
        assert!(matches!(refused, CrawlError::Restricted(_)), "{refused:?}");

        // A new `last_updated` fetches the observations again.
        mock.reset().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json_str(SERIES_GDP.replace("2026-08-28 07:54:02-05", "2026-09-25 07:52:02-05")),
        )
        .await;
        mount_vintages(&mock).await;
        mock.mount(
            &Route::get("/series/observations"),
            Reply::json_str(OBS_GDP_SINCE),
        )
        .await;
        let changed = adapter
            .fetch_series_incremental(&ctx, "GDP", None, known)
            .await
            .unwrap();
        assert!(!changed.points.is_empty());
        assert_eq!(
            changed.validators.and_then(|v| v.version).as_deref(),
            Some(format!("{PARSE_VERSION}:2026-09-25 07:52:02-05").as_str())
        );

        db.drop().await;
    }

    #[test]
    fn to_points_marks_original_releases_and_drops_known_vintages() {
        let row = |date: &str, rt: &str, v: &str| VintageRow {
            date: d(date),
            realtime_start: d(rt),
            value: Some(BigDecimal::from_str(v).unwrap()),
            keep: true,
        };
        // Rows as one window keeping from `keep_from` would tag them.
        let keeping = |rows: &[VintageRow], keep_from: Option<&str>| -> Vec<VintageRow> {
            let window = RealtimeWindow {
                start: earliest_realtime_start(),
                end: latest_realtime_end(),
                keep_from: keep_from.map(d),
                vintages: None,
            };
            rows.iter()
                .map(|r| VintageRow {
                    keep: window.keeps(r.realtime_start),
                    ..r.clone()
                })
                .collect()
        };
        // Out of order on purpose; FRED's order isn't relied on.
        let rows = vec![
            row("2024-02-01", "2024-03-10", "2"),
            row("2024-01-01", "2024-03-10", "1.1"),
            row("2024-01-01", "2024-02-10", "1"),
        ];
        assert_eq!(
            to_points(rows.clone()),
            vec![
                point("2024-01-01", Some("1"), "2024-02-10", true),
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Known vintage 2024-02-20: January was published before it, so its later row is a
        // revision. The dropped row starts before the known vintage here (FRED not clamping).
        assert_eq!(
            to_points(keeping(&rows, Some("2024-02-20"))),
            vec![
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Rows of the known vintage itself are kept (it may have changed since the last crawl),
        // and an earliest row keeps its original-release mark.
        assert_eq!(
            to_points(keeping(&rows, Some("2024-03-10"))),
            vec![
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Everything already known: nothing to write.
        assert_eq!(to_points(keeping(&rows, Some("2024-03-11"))), vec![]);

        // The same row returned by two windows (FRED not clamping), once kept and once not, in
        // either order: the kept copy is still the original release.
        for kept_first in [true, false] {
            let mut copies = vec![
                VintageRow {
                    keep: kept_first,
                    ..row("2024-01-01", "2024-02-10", "1")
                },
                VintageRow {
                    keep: !kept_first,
                    ..row("2024-01-01", "2024-02-10", "1")
                },
            ];
            copies.push(row("2024-01-01", "2024-03-10", "1.1"));
            assert_eq!(
                to_points(copies),
                vec![
                    point("2024-01-01", Some("1"), "2024-02-10", true),
                    point("2024-01-01", Some("1.1"), "2024-03-10", false),
                ]
            );
        }
    }

    #[test]
    fn windows_split_at_vintage_boundaries() {
        let whole = RealtimeWindow {
            start: earliest_realtime_start(),
            end: latest_realtime_end(),
            keep_from: None,
            vintages: None,
        };
        let vintages = [
            d("2020-01-01"),
            d("2020-01-02"),
            d("2020-02-01"),
            d("2020-03-01"),
            d("2020-03-02"),
        ];
        let w = whole.split(&vintages, 2);
        let summary: Vec<_> = w
            .iter()
            .map(|w| (w.start, w.end, w.keep_from, w.vintages.clone().unwrap()))
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    d("1776-07-04"),
                    d("2020-01-31"),
                    None,
                    vec![d("2020-01-01"), d("2020-01-02")]
                ),
                // Starts the day before its first vintage, keeps from that vintage.
                (
                    d("2020-01-31"),
                    d("2020-03-01"),
                    Some(d("2020-02-01")),
                    vec![d("2020-02-01"), d("2020-03-01")]
                ),
                // Its start, the day before its first vintage, is the previous window's last
                // vintage: a vintage date on the boundary.
                (
                    d("2020-03-01"),
                    d("9999-12-31"),
                    Some(d("2020-03-02")),
                    vec![d("2020-03-02")]
                ),
            ]
        );
        // Every realtime_start is kept by exactly one window, the boundary days included.
        let mut day = d("2019-12-30");
        while day <= d("2020-03-05") {
            assert_eq!(w.iter().filter(|w| w.keeps(day)).count(), 1, "{day}");
            day = day.succ_opt().unwrap();
        }
        assert_eq!(w.iter().filter(|w| w.keeps(d("1776-07-04"))).count(), 1);
        assert_eq!(w.iter().filter(|w| w.keeps(d("9999-12-31"))).count(), 1);

        // At most `per_window` vintages: the window is unchanged, its vintages recorded.
        let one = whole.split(&vintages, 5);
        assert_eq!(one.len(), 1);
        assert_eq!(
            (one[0].start, one[0].end, one[0].keep_from),
            (whole.start, whole.end, None)
        );
        assert_eq!(whole.split(&[], 5).len(), 1);

        // Splitting an incremental window keeps its own start and keep_from first.
        let incremental = RealtimeWindow {
            start: d("2020-01-01"),
            end: latest_realtime_end(),
            keep_from: Some(d("2020-01-02")),
            vintages: None,
        };
        let w = incremental.split(&vintages, 3);
        assert_eq!(
            (w[0].start, w[0].end, w[0].keep_from),
            (d("2020-01-01"), d("2020-02-29"), Some(d("2020-01-02")))
        );
        assert_eq!(
            (w[1].start, w[1].keep_from),
            (d("2020-02-29"), Some(d("2020-03-01")))
        );
    }

    #[test]
    fn recognises_only_the_vintage_limit_400() {
        let too_many = CrawlError::Permanent(
            "HTTP 400: FRED GET http://x/series/observations?series_id=DFF&api_key=REDACTED: \
             {\"error_code\":400,\"error_message\":\"Bad Request.  This exceeds the maximum \
             number of vintage dates allowed (2000).\"}"
                .into(),
        );
        assert!(is_too_many_vintage_dates(&too_many));
        // It is still an ordinary permanent error to everything else.
        assert_eq!(classify_fred_error(too_many).kind(), "permanent");
        for other in [
            CrawlError::Permanent(format!("HTTP 400: FRED GET http://x: {ERR_BAD_VARIABLE}")),
            CrawlError::Permanent(format!("HTTP 400: FRED GET http://x: {ERR_NO_SERIES}")),
            CrawlError::Permanent("HTTP 400: FRED GET http://x".into()),
            // Same words, not a 400.
            CrawlError::Transient("maximum number of vintage dates".into()),
            CrawlError::Permanent("HTTP 403: maximum number of vintage dates".into()),
        ] {
            assert!(!is_too_many_vintage_dates(&other), "{other:?}");
        }
    }

    #[test]
    fn row_parsing_rules() {
        let obs = |date: &str, value: &str, rt: &str| FredObservation {
            date: date.into(),
            value: value.into(),
            realtime_start: rt.into(),
        };
        let r = parse_row("X", obs("2024-01-01", "-0.25", "2024-01-09")).unwrap();
        assert_eq!(r.date, d("2024-01-01"));
        assert_eq!(r.realtime_start, d("2024-01-09"));
        assert_eq!(r.value, Some(BigDecimal::from_str("-0.25").unwrap()));
        // Missing marker.
        assert_eq!(
            parse_row("X", obs("2024-01-01", ".", "2024-01-09"))
                .unwrap()
                .value,
            None
        );
        // Garbage is a parse error, not silently dropped.
        for bad in [
            obs("2024-01-01", "n/a", "2024-01-09"),
            obs("01/01/2024", "1", "2024-01-09"),
            obs("2024-01-01", "1", "soon"),
        ] {
            let e = parse_row("X", bad).unwrap_err();
            assert_eq!(e.kind(), "parse", "{e}");
        }
        // A row without realtime_start is a parse error too (FRED always sends it).
        let e = serde_json::from_str::<ObservationsResponse>(
            r#"{"count":1,"observations":[{"date":"2024-01-01","value":"1"}]}"#,
        )
        .unwrap_err();
        assert!(e.to_string().contains("realtime_start"), "{e}");
        // So is a response without `count`: paging couldn't tell a full page from the last one.
        let e = serde_json::from_str::<ObservationsResponse>(r#"{"observations":[]}"#).unwrap_err();
        assert!(e.to_string().contains("count"), "{e}");
    }

    #[tokio::test]
    async fn since_is_sent_as_observation_start() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        mock.mount_expect(
            &Route::get("/series/observations")
                .query("series_id", "GDP")
                .query("observation_start", "2026-01-01")
                .query("realtime_start", EARLIEST_REALTIME_START),
            Reply::json_str(OBS_GDP),
            1,
        )
        .await;
        FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", Some(d("2026-01-01")))
            .await
            .unwrap();
        mock.server().verify().await;
    }

    /// Observation rows for dates `from..from+n` (days from 2000-01-01), one vintage each.
    fn obs_page(count: usize, from: usize, n: usize) -> Reply {
        let start = d("2000-01-01");
        let rows: Vec<_> = (from..from + n)
            .map(|i| {
                let date = start + chrono::Duration::days(i as i64);
                serde_json::json!({
                    "realtime_start": "2001-01-01",
                    "realtime_end": "9999-12-31",
                    "date": date.to_string(),
                    "value": i.to_string(),
                })
            })
            .collect();
        Reply::json(serde_json::json!({"count": count, "observations": rows}))
    }

    #[tokio::test]
    async fn observations_are_paged_past_the_row_limit() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        // 5 rows, served 2 per page (as if FRED's limit were 2).
        for (offset, n) in [(0, 2), (2, 2), (4, 1)] {
            mock.mount_expect(
                &Route::get("/series/observations").query("offset", offset.to_string()),
                obs_page(5, offset, n),
                1,
            )
            .await;
        }
        let s = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap();
        let values: Vec<String> = s
            .points
            .iter()
            .map(|p| p.value.as_ref().unwrap().to_string())
            .collect();
        assert_eq!(values, ["0", "1", "2", "3", "4"]);
        assert!(s.points.iter().all(|p| p.is_original_release));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn observation_paging_is_bounded() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        // Claims far more rows than it ever sends, one row per page.
        mock.mount(
            &Route::get("/series/observations"),
            obs_page(usize::MAX, 0, 1),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");
        assert_eq!(
            mock.received_requests().await.len(),
            2 + MAX_OBSERVATION_PAGES
        );
    }

    #[tokio::test]
    async fn empty_page_ends_paging() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        mock.mount(&Route::get("/series/observations"), obs_page(10, 0, 0))
            .await;
        let s = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap();
        assert!(s.points.is_empty());
        assert_eq!(mock.received_requests().await.len(), 3);
    }

    #[tokio::test]
    async fn missing_key_is_auth_error_without_requests() {
        let mock = mock_gdp().await;
        let mut ctx = test_ctx();
        ctx.keys.fred = None;
        let adapter = FredAdapter::new(mock.base_url());

        let e = adapter.fetch_series(&ctx, "GDP", None).await.unwrap_err();
        assert_eq!(e, CrawlError::Auth("FRED_API_KEY not set".into()));
        let e = adapter.discover(&ctx).await.unwrap_err();
        assert_eq!(e.kind(), "auth");
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn api_key_is_redacted_from_errors() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap_err();
        let msg = e.to_string();
        assert!(!msg.contains(TEST_API_KEY), "{msg}");
        assert!(msg.contains("api_key=REDACTED"), "{msg}");

        // Also through a parse error on the observations call.
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mount_vintages(&mock).await;
        mock.mount(
            &Route::get("/series/observations"),
            Reply::json_str(format!("<html>bad key {TEST_API_KEY}</html>")),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse");
        assert!(!e.to_string().contains(TEST_API_KEY), "{e}");
    }

    #[tokio::test]
    async fn other_400_is_permanent() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    /// FRED answers an unknown `series_id` with HTTP 400 and a JSON error body; `HttpFetcher`
    /// quotes the (redacted) response body in its error, so `classify_fred_error` recognises the
    /// "series does not exist" message and turns this into `NotFound` (see
    /// `classify_recognises_series_does_not_exist`).
    #[tokio::test]
    async fn unknown_series_400_is_not_found() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/series").query("series_id", "NOPE"),
            Reply::json_str(ERR_NO_SERIES).with_status(400),
            1,
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "NOPE", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");
        assert!(!e.to_string().contains(TEST_API_KEY));
        // No observations request after the metadata said the series doesn't exist.
        mock.server().verify().await;
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    #[test]
    fn classify_recognises_series_does_not_exist() {
        let not_found = classify_fred_error(CrawlError::Permanent(
            "HTTP 400: FRED GET http://x/series?series_id=NOPE&api_key=REDACTED: \
             {\"error_code\":400,\"error_message\":\"Bad Request.  The series does not exist.\"}"
                .into(),
        ));
        assert_eq!(not_found.kind(), "not_found");
        let other = classify_fred_error(CrawlError::Permanent(
            "HTTP 400: FRED GET http://x/series: Bad Request.  Variable api_key is not registered."
                .into(),
        ));
        assert_eq!(other.kind(), "permanent");
        let bare = classify_fred_error(CrawlError::Permanent("HTTP 400: FRED GET http://x".into()));
        assert_eq!(bare.kind(), "permanent");
        let t = CrawlError::Transient("series does not exist".into());
        assert_eq!(classify_fred_error(t.clone()), t);
    }

    #[tokio::test]
    async fn empty_seriess_is_not_found() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json(serde_json::json!({"realtime_start": "2026-09-25", "seriess": []})),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found");
    }

    /// A second series fixture (derived from [`SERIES_GDP`]) for tests that need more than one
    /// distinct curated id.
    fn series_fixture(id: &str, title: &str) -> String {
        SERIES_GDP
            .replace("\"id\": \"GDP\"", &format!("\"id\": \"{id}\""))
            .replace(
                "\"title\": \"Gross Domestic Product\"",
                &format!("\"title\": \"{title}\""),
            )
    }

    /// [`series_fixture`] with `notes` replaced by copyright-restricted wording (the Coinbase
    /// `CBBTCUSD` family's actual FRED notes), for tests of [`is_copyright_restricted`] callers.
    fn restricted_fixture(id: &str, title: &str) -> String {
        let notes_field = "\"notes\": \"BEA Account Code: A191RC\\n\\nGross domestic product \
            (GDP), the featured measure of U.S. output, is the market value of the goods and \
            services produced by labor and property located in the United States.\"";
        series_fixture(id, title).replace(
            notes_field,
            "\"notes\": \"Reproduction, retransmission, or other use is prohibited except \
                      with prior written permission.\"",
        )
    }

    #[test]
    fn shipped_catalog_rules_are_valid_and_bounded() {
        let text = std::fs::read_to_string(data_dir().join(CATALOG_RULES_FILE)).unwrap();
        let rules = FredCatalogRules::parse(&text).unwrap();
        assert!(!rules.rules.is_empty());
        assert!(rules.max_series <= 1000);
        assert!(rules.include.len() <= 32);
    }

    #[test]
    fn markers_are_lower_case() {
        assert!(COPYRIGHT_RESTRICTION_MARKERS
            .iter()
            .all(|m| *m == m.to_lowercase()));
    }

    #[test]
    fn is_copyright_restricted_matches_known_wording() {
        assert!(is_copyright_restricted(Some(
            "Reproduction of this data by third parties is prohibited except with prior \
             written permission from Coinbase."
        )));
        assert!(is_copyright_restricted(Some(
            "Reproduction of this information in any form is prohibited except with the \
             prior\nwritten permission of ICE Data Indices, LLC.",
        )));
        assert!(is_copyright_restricted(Some("ALL RIGHTS RESERVED.")));
        assert!(is_copyright_restricted(Some(
            "Copyright, 2026, Standard & Poor's Financial Services LLC. Reprinted with permission."
        )));
        assert!(is_copyright_restricted(Some(
            "Data used with permission of the National Association of Realtors."
        )));
        assert!(is_copyright_restricted(Some(
            "Reproduction, retransmission, or other use is prohibited without written consent."
        )));
        assert!(
            is_copyright_restricted(Some("All  rights\treserved.")),
            "whitespace normalized"
        );
        assert!(!is_copyright_restricted(Some("BEA Account Code: A191RC")));
        assert!(!is_copyright_restricted(None));
    }
}

#[cfg(test)]
mod vintage_tests;

#[cfg(test)]
mod contract {
    use super::FredAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| FredAdapter::new(base_url),
        external_id: "GDP",
        route: Route::get("/series/observations").query("series_id", "GDP"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/fred/observations_gdp.json")),
        expect_points: 6,
        setup: |mock| {
            mock.mount(
                &Route::get("/series").query("series_id", "GDP"),
                Reply::json_str(include_str!("../../tests/fixtures/fred/series_gdp.json")),
            )
            .await;
            mock.mount(
                &Route::get("/series/vintagedates").query("series_id", "GDP"),
                Reply::json_str(include_str!("../../tests/fixtures/fred/vintagedates_gdp.json")),
            )
            .await;
        },
        discover: {
            route: Route::get("/series"),
            reply: Reply::json_str(include_str!("../../tests/fixtures/fred/series_gdp.json")),
            min_series: 100,
        },
    }
}
