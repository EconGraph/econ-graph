// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FRED (Federal Reserve Economic Data, St. Louis Fed) adapter.
//!
//! Endpoints used (all `GET`, all with `api_key` and `file_type=json`):
//! - `{base}/series?series_id=ID` — series metadata (`seriess[0]`); used both by discovery (one
//!   request per curated id) and by fetching.
//! - `{base}/series/observations?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=..&offset=..[&observation_start=YYYY-MM-DD]`
//!   — observations with their vintages (ALFRED), paged. See [Vintages](#vintages).
//!
//! # Discovery
//!
//! `discover()` looks up the live metadata for each id in [`crate::reference::fred_series`] (a
//! curated headline list read from `data/fred_series.csv`), rather than walking
//! `/series/search`. A search-term crawl has no bound on how many series it turns up (one
//! "most popular" page plus several search terms, each paged to FRED's limit, can find tens of
//! thousands of ids on the first run), and #214's per-series ALFRED vintage walk makes fetching
//! all of them too slow to finish a first crawl in reasonable time. The curated list keeps
//! discovery's own request count equal to its size (under two hundred, not unbounded), while still
//! checking each series' live notes against [`is_copyright_restricted`] as a safety net before
//! it is discovered. [`FredAdapter::fetch`] repeats the same check, so a series id fetched
//! directly (a manual `triggerCrawl`, or a scheduled refresh of an already-stored series) without
//! going through discovery is refused too, rather than only kept out of the curated list.
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
//!   requested with `offset` until `count` rows have arrived (at most [`MAX_OBSERVATION_PAGES`]).
//!
//! FRED reports errors as JSON `{"error_code": 400, "error_message": "Bad Request.  ..."}`
//! with the same HTTP status. An unknown `series_id` is HTTP **400** (not 404) with the message
//! `"Bad Request.  The series does not exist."`; that is mapped to [`CrawlError::NotFound`], and
//! every other 400 stays [`CrawlError::Permanent`]. Recognising the message requires the
//! [`HttpFetcher`](crate::HttpFetcher) error to quote the (redacted) response body; see
//! [`classify_fred_error`].
//!
//! Every series belongs to the one dimensionless dataset [`DATASET`] (defined in
//! `data/datasets/fred.toml`) and keeps its FRED series id as its external id.

use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::adapter::{
    CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::reference::fred_series;
use crate::source::SourceId;

/// The real FRED API root.
pub const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org/fred";

/// Human-facing series page, used for [`DiscoveredSeries::data_url`] (never requested).
const FRED_WEB_SERIES_URL: &str = "https://fred.stlouisfed.org/series";

/// Code of the dataset every FRED series belongs to. It has no dimensions: a FRED series id is
/// an opaque key, not a combination of dimension values.
pub const DATASET: &str = "FRED";

/// FRED's marker for a missing observation.
const MISSING_VALUE: &str = ".";

/// Start of the real-time window for a full vintage fetch: FRED's documented earliest
/// `realtime_start`, meaning "every vintage".
pub const EARLIEST_REALTIME_START: &str = "1776-07-04";

/// End of the real-time window: FRED's documented latest `realtime_end`, meaning "still current".
const LATEST_REALTIME_END: &str = "9999-12-31";

/// Rows per `/series/observations` page (FRED's maximum `limit`).
const OBSERVATIONS_PAGE_SIZE: usize = 100_000;

/// Upper bound on observation pages per series, so a response that keeps claiming more rows
/// can't page forever (10 million rows; daily series with every vintage stay far below this).
const MAX_OBSERVATION_PAGES: usize = 100;

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
}

impl FredAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
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

    async fn fetch_metadata(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
    ) -> Result<NewSeriesMetadataLite, CrawlError> {
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
        Ok(NewSeriesMetadataLite {
            title: s.title,
            description: non_empty(s.notes),
            units: non_empty(s.units),
            frequency: non_empty(s.frequency),
            seasonal_adjustment: non_empty(s.seasonal_adjustment),
        })
    }

    /// Every observation vintage of `series_id`, paged (see [Vintages](self#vintages)).
    ///
    /// `observation_start` limits the observation dates. `known_vintage` starts the real-time
    /// window there and drops rows already stored; `None` asks for every vintage.
    async fn fetch_observations(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        observation_start: Option<NaiveDate>,
        known_vintage: Option<NaiveDate>,
    ) -> Result<Vec<FetchedPoint>, CrawlError> {
        let observation_start = observation_start.map(|d| d.format("%Y-%m-%d").to_string());
        // The day before the known vintage, so vintage `known_vintage` itself is re-read.
        let realtime_start = known_vintage.and_then(|k| k.pred_opt()).map_or_else(
            || EARLIEST_REALTIME_START.to_string(),
            |d| d.format("%Y-%m-%d").to_string(),
        );
        let limit = OBSERVATIONS_PAGE_SIZE.to_string();
        let mut rows = Vec::new();
        for _ in 0..MAX_OBSERVATION_PAGES {
            let offset = rows.len().to_string();
            let mut query = vec![
                ("series_id", series_id),
                ("api_key", api_key),
                ("file_type", "json"),
                ("realtime_start", realtime_start.as_str()),
                ("realtime_end", LATEST_REALTIME_END),
                ("limit", limit.as_str()),
                ("offset", offset.as_str()),
            ];
            if let Some(s) = &observation_start {
                query.push(("observation_start", s.as_str()));
            }
            let page: ObservationsResponse = ctx
                .http
                .get_json(SourceId::Fred, &self.url("/series/observations"), &query)
                .await
                .map_err(classify_fred_error)?;
            let got = page.observations.len();
            for o in page.observations {
                rows.push(parse_row(series_id, o)?);
            }
            if got == 0 || rows.len() >= page.count {
                return Ok(to_points(rows, known_vintage));
            }
        }
        Err(CrawlError::Permanent(format!(
            "FRED {series_id}: more than {MAX_OBSERVATION_PAGES} pages of \
             {OBSERVATIONS_PAGE_SIZE} observations"
        )))
    }

    /// Looks up each of `ids`' live metadata, dropping copyright-restricted series (the
    /// [`is_copyright_restricted`] safety net) and building a [`DiscoveredSeries`] from the rest.
    /// A per-id lookup failure is logged and that id skipped, unless every id fails; `Auth` and
    /// `RateLimited` errors abort immediately and discard every id already found in this call
    /// (matching the old search-based `discover`'s behaviour: a retry starts over from the top of
    /// the list rather than resuming, so a rate limit partway through is more of a delay here,
    /// since the list is walked in full on every discovery run either way).
    ///
    /// A `Transient` skip (a timeout or 5xx) means the list is incomplete for a reason that
    /// should clear up on retry, so it fails the whole call instead of quietly persisting a
    /// partial discovery: otherwise a blip partway through the list would look, from the finished
    /// count alone, like those ids simply don't exist. `NotFound`/`Parse`/`Permanent` skips (a bad
    /// id, a malformed response) won't resolve on retry, so they stay skip-and-continue as before.
    async fn discover_ids(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        ids: &[String],
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let mut found = Vec::with_capacity(ids.len());
        let mut restricted_count = 0usize;
        let mut skipped_ids = Vec::new();
        let mut last_err = None;
        let mut first_transient = None;
        let mut any_ok = false;

        for id in ids {
            match self.fetch_metadata(ctx, api_key, id).await {
                Ok(meta) => {
                    any_ok = true;
                    if is_copyright_restricted(meta.description.as_deref()) {
                        tracing::info!(id, "dropping copyright-restricted series");
                        restricted_count += 1;
                        continue;
                    }
                    found.push(DiscoveredSeries {
                        external_id: id.clone(),
                        title: meta.title,
                        description: meta.description,
                        units: meta.units,
                        frequency: meta.frequency,
                        data_url: Some(format!("{FRED_WEB_SERIES_URL}/{id}")),
                        dataset: fred_dataset(),
                    });
                }
                Err(e) => {
                    let e = skip_or_abort(e, id)?;
                    if first_transient.is_none() && matches!(e, CrawlError::Transient(_)) {
                        first_transient = Some(e.clone());
                    }
                    skipped_ids.push(id.as_str());
                    last_err = Some(e);
                }
            }
        }

        if !skipped_ids.is_empty() {
            let sample: Vec<&str> = skipped_ids.iter().take(5).copied().collect();
            tracing::warn!(
                skipped = skipped_ids.len(),
                sample = ?sample,
                "FRED discovery: some curated ids failed lookup"
            );
        }

        match (any_ok, last_err, first_transient) {
            (false, Some(e), _) => Err(e),
            (_, _, Some(transient)) => Err(transient),
            _ => {
                tracing::info!(
                    series = found.len(),
                    dropped_restricted = restricted_count,
                    dropped_errors = skipped_ids.len(),
                    "FRED discovery finished"
                );
                Ok(found)
            }
        }
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

    /// Looks up the curated list from [`crate::reference::fred_series`]. See the module docs.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let api_key = self.api_key(ctx)?;
        let ids = fred_series()?;
        self.discover_ids(ctx, api_key, ids).await
    }

    /// `/series` (metadata), then every vintage of the observations on or after `since` from
    /// `/series/observations` (one request per [`OBSERVATIONS_PAGE_SIZE`] rows). Refuses with
    /// [`CrawlError::Permanent`] if the series' notes are copyright-restricted (see
    /// [`is_copyright_restricted`]), whether or not it was reached via discovery.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        self.fetch(ctx, external_id, since, None).await
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
        let metadata = self.fetch_metadata(ctx, api_key, external_id).await?;
        if is_copyright_restricted(metadata.description.as_deref()) {
            tracing::warn!(
                series_id = external_id,
                "refusing to fetch copyright-restricted series"
            );
            return Err(CrawlError::Permanent(format!(
                "FRED {external_id}: copyright-restricted, refusing to fetch"
            )));
        }
        let points = self
            .fetch_observations(ctx, api_key, external_id, since, known_vintage)
            .await?;
        Ok(FetchedSeries {
            metadata: Some(metadata),
            points,
            dataset: fred_dataset(),
        })
    }
}

/// Returns `err` for aborting errors (auth, rate limiting); otherwise logs it and hands it back
/// to be remembered as the last failure.
fn skip_or_abort(err: CrawlError, what: &str) -> Result<CrawlError, CrawlError> {
    match err {
        CrawlError::Auth(_) | CrawlError::RateLimited { .. } => Err(err),
        e => {
            tracing::warn!(series_id = what, error = %e, "FRED series lookup failed; skipping");
            Ok(e)
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
    })
}

/// Turns vintage rows into points: `revision_date = realtime_start`, and each date's earliest
/// row is its original release. With `known_vintage`, rows starting before it are dropped as
/// already stored; when a date's earliest row is dropped, none of its remaining rows is an
/// original release. See [Vintages](self#vintages).
///
/// `is_original_release` is part of the `data_points` upsert key, so a full and an incremental
/// fetch must mark the same rows, or one vintage would be stored twice. They do: the earliest
/// row is chosen before anything is dropped.
fn to_points(mut rows: Vec<VintageRow>, known_vintage: Option<NaiveDate>) -> Vec<FetchedPoint> {
    rows.sort_by_key(|r| (r.date, r.realtime_start));
    let mut points = Vec::with_capacity(rows.len());
    let mut prev_date = None;
    for r in rows {
        let earliest = prev_date != Some(r.date);
        prev_date = Some(r.date);
        if known_vintage.is_some_and(|k| r.realtime_start < k) {
            continue;
        }
        points.push(FetchedPoint {
            date: r.date,
            value: r.value,
            revision_date: r.realtime_start,
            is_original_release: earliest,
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
}

#[derive(Debug, Deserialize)]
struct ObservationsResponse {
    /// Total rows across all pages. Required: without it, paging can't tell a full page from the
    /// last one.
    count: usize,
    observations: Vec<FredObservation>,
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

    const SERIES_GDP: &str = include_str!("../../tests/fixtures/fred/series_gdp.json");
    const OBS_GDP: &str = include_str!("../../tests/fixtures/fred/observations_gdp.json");
    const OBS_GDP_SINCE: &str =
        include_str!("../../tests/fixtures/fred/observations_gdp_since_2026-04-28.json");
    const ERR_NO_SERIES: &str =
        include_str!("../../tests/fixtures/fred/error_series_does_not_exist.json");
    const ERR_BAD_VARIABLE: &str =
        include_str!("../../tests/fixtures/fred/error_bad_variable.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    async fn mock_gdp() -> MockSource {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
        )
        .await;
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

        // Two requests, both with the key and file_type=json; observations ask for every vintage.
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 2);
        for r in &reqs {
            let q = query_of(r);
            assert_eq!(param(&q, "api_key"), Some(TEST_API_KEY));
            assert_eq!(param(&q, "file_type"), Some("json"));
        }
        let q = query_of(&reqs[1]);
        assert_eq!(reqs[1].url.path(), "/series/observations");
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
            CrawlError::Permanent(
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
        assert_eq!(
            param(&query_of(&reqs[1]), "realtime_start"),
            Some(EARLIEST_REALTIME_START)
        );
        assert!(FredAdapter::default().tracks_vintages());
    }

    #[test]
    fn to_points_marks_original_releases_and_drops_known_vintages() {
        let row = |date: &str, rt: &str, v: &str| VintageRow {
            date: d(date),
            realtime_start: d(rt),
            value: Some(BigDecimal::from_str(v).unwrap()),
        };
        // Out of order on purpose; FRED's order isn't relied on.
        let rows = vec![
            row("2024-02-01", "2024-03-10", "2"),
            row("2024-01-01", "2024-03-10", "1.1"),
            row("2024-01-01", "2024-02-10", "1"),
        ];
        assert_eq!(
            to_points(rows.clone(), None),
            vec![
                point("2024-01-01", Some("1"), "2024-02-10", true),
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Known vintage 2024-02-20: January was published before it, so its later row is a
        // revision. The dropped row starts before the known vintage here (FRED not clamping).
        assert_eq!(
            to_points(rows.clone(), Some(d("2024-02-20"))),
            vec![
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Rows of the known vintage itself are kept (it may have changed since the last crawl),
        // and an earliest row keeps its original-release mark.
        assert_eq!(
            to_points(rows.clone(), Some(d("2024-03-10"))),
            vec![
                point("2024-01-01", Some("1.1"), "2024-03-10", false),
                point("2024-02-01", Some("2"), "2024-03-10", true),
            ]
        );
        // Everything already known: nothing to write.
        assert_eq!(to_points(rows, Some(d("2024-03-11"))), vec![]);
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
            1 + MAX_OBSERVATION_PAGES
        );
    }

    #[tokio::test]
    async fn empty_page_ends_paging() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mock.mount(&Route::get("/series/observations"), obs_page(10, 0, 0))
            .await;
        let s = FredAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "GDP", None)
            .await
            .unwrap();
        assert!(s.points.is_empty());
        assert_eq!(mock.received_requests().await.len(), 2);
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

    #[tokio::test]
    async fn discover_ids_looks_up_each_curated_id() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/series").query("series_id", "GDPC1"),
            Reply::json_str(series_fixture("GDPC1", "Real Gross Domestic Product")),
            1,
        )
        .await;

        let ctx = test_ctx();
        let ids = ["GDP".to_string(), "GDPC1".to_string()];
        let found = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap();
        let got: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(got, ["GDP", "GDPC1"]);
        assert_eq!(found[0].title, "Gross Domestic Product");
        assert_eq!(found[1].title, "Real Gross Domestic Product");
        assert_eq!(found[0].units.as_deref(), Some("Billions of Dollars"));
        assert_eq!(found[0].frequency.as_deref(), Some("Quarterly"));
        assert!(found[0]
            .description
            .as_deref()
            .unwrap()
            .starts_with("BEA Account Code: A191RC"));
        assert_eq!(
            found[0].data_url.as_deref(),
            Some("https://fred.stlouisfed.org/series/GDP")
        );
        for s in &found {
            let ds = &s.dataset;
            assert_eq!(ds.code, DATASET, "{}", s.external_id);
            assert!(ds.dimensions.0.is_empty(), "{}", s.external_id);
        }
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn discover_ids_drops_copyright_restricted_series_as_a_safety_net() {
        let mock = MockSource::start().await;
        let restricted = restricted_fixture("CBBTCUSD", "Coinbase Bitcoin");
        mock.mount(
            &Route::get("/series").query("series_id", "CBBTCUSD"),
            Reply::json_str(restricted),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDPC1"),
            Reply::json_str(series_fixture("GDPC1", "Real Gross Domestic Product")),
        )
        .await;

        let ctx = test_ctx();
        let ids = ["CBBTCUSD".to_string(), "GDPC1".to_string()];
        let found = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap();
        let got: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(got, ["GDPC1"], "restricted series must be dropped");
    }

    #[tokio::test]
    async fn discover_ids_skips_a_failing_id_but_keeps_going() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series").query("series_id", "NOPE"),
            Reply::json_str(ERR_NO_SERIES).with_status(400),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
        )
        .await;

        let ctx = test_ctx();
        let ids = ["NOPE".to_string(), "GDP".to_string()];
        let found = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap();
        let got: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(got, ["GDP"]);
    }

    #[tokio::test]
    async fn discover_ids_aborts_on_rate_limit_and_fails_if_everything_fails() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::status(429).retry_after(60))
            .await;
        let ctx = test_ctx();
        let ids = ["GDP".to_string(), "GDPC1".to_string()];
        let e = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "rate_limited");
        // Aborted on the first id: the second was never requested.
        assert_eq!(mock.received_requests().await.len(), 1);

        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series"),
            Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "permanent");
        assert_eq!(
            mock.received_requests().await.len(),
            2,
            "both ids fail, so both are tried before giving up"
        );
    }

    /// A rate limit partway through the list aborts immediately, without trying the remaining
    /// ids, even though earlier ones already succeeded (a bug that skipped instead of aborting
    /// once `any_ok` was true would still request the third id).
    #[tokio::test]
    async fn discover_ids_aborts_mid_list_on_rate_limit() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDPC1"),
            Reply::status(429).retry_after(60),
        )
        .await;
        let ctx = test_ctx();
        let ids = ["GDP".to_string(), "GDPC1".to_string(), "UNRATE".to_string()];
        let e = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "rate_limited");
        assert_eq!(
            mock.received_requests().await.len(),
            2,
            "the third id must never be requested"
        );
    }

    /// A `Transient` skip (e.g. a 500) fails discovery even though every other id in the list
    /// succeeded, instead of quietly persisting a partial list: a blip partway through should
    /// retry, not look like the failed id simply doesn't exist. Unlike a rate limit or auth
    /// failure, a transient error doesn't abort mid-list: the rest of the ids are still tried
    /// before the call fails.
    #[tokio::test]
    async fn discover_ids_fails_on_a_transient_skip_even_if_others_succeeded() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDP"),
            Reply::json_str(SERIES_GDP),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "GDPC1"),
            Reply::status(500),
        )
        .await;
        mock.mount(
            &Route::get("/series").query("series_id", "UNRATE"),
            Reply::json_str(series_fixture("UNRATE", "Unemployment Rate")),
        )
        .await;
        let ctx = test_ctx();
        let ids = ["GDP".to_string(), "GDPC1".to_string(), "UNRATE".to_string()];
        let e = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
        assert_eq!(
            mock.received_requests().await.len(),
            5,
            "GDPC1 costs 3 requests (HttpFetcher's in-process retries on a 5xx); \
             a transient skip doesn't abort mid-list, unlike rate-limit/auth, so UNRATE is \
             still tried"
        );
    }

    /// A missing/invalid key (`Auth`) aborts discovery the same way a rate limit does.
    #[tokio::test]
    async fn discover_ids_aborts_on_auth_error() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::status(401)).await;
        let ctx = test_ctx();
        let ids = ["GDP".to_string(), "GDPC1".to_string()];
        let e = FredAdapter::new(mock.base_url())
            .discover_ids(&ctx, TEST_API_KEY, &ids)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth");
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    /// `discover()` reads the shipped curated list ([`crate::reference::fred_series`]) and looks
    /// up every one of its ids; none of the shipped ids' fixture notes are restricted, so all of
    /// them come back.
    #[tokio::test]
    async fn discover_reads_curated_list_from_reference_data() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        let found = FredAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        let want = crate::reference::fred_series().unwrap();
        assert_eq!(found.len(), want.len());
        let got: std::collections::HashSet<&str> =
            found.iter().map(|s| s.external_id.as_str()).collect();
        for id in want {
            assert!(got.contains(id.as_str()), "{id} missing from discover()");
        }
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
        },
        discover: {
            route: Route::get("/series"),
            reply: Reply::json_str(include_str!("../../tests/fixtures/fred/series_gdp.json")),
            min_series: 100,
        },
    }
}
