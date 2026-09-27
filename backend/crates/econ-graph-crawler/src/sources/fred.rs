// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FRED (Federal Reserve Economic Data, St. Louis Fed) adapter.
//!
//! Endpoints used (all `GET`, all with `api_key` and `file_type=json`):
//! - `{base}/series?series_id=ID` — series metadata (`seriess[0]`).
//! - `{base}/series/observations?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=..&offset=..[&observation_start=YYYY-MM-DD]`
//!   — observations with their vintages (ALFRED), paged. See [Vintages](#vintages).
//! - `{base}/series/search?search_text=..&limit=..&offset=..` — discovery.
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

use std::collections::HashSet;
use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::adapter::{
    CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite, SourceAdapter,
};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The real FRED API root.
pub const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org/fred";

/// Human-facing series page, used for [`DiscoveredSeries::data_url`] (never requested).
const FRED_WEB_SERIES_URL: &str = "https://fred.stlouisfed.org/series";

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

/// Search terms walked by [`FredAdapter::discover`] (same list as the old series_discovery/fred.rs).
const SEARCH_TERMS: &[&str] = &[
    "GDP",
    "unemployment",
    "inflation",
    "interest rate",
    "employment",
    "consumer price",
    "producer price",
    "retail sales",
    "industrial production",
    "housing",
    "trade",
    "balance",
    "debt",
    "revenue",
    "expenditure",
];

/// Page size for search-term pages (FRED's maximum).
const SEARCH_PAGE_SIZE: usize = 1000;
/// Size of the single "most popular series" page requested first.
const POPULAR_PAGE_SIZE: usize = 100;
/// Upper bound on pages fetched per search term, so discovery stays bounded:
/// at most `1 + SEARCH_TERMS.len() * MAX_PAGES_PER_TERM` requests (= 76).
const MAX_PAGES_PER_TERM: usize = 5;

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

    /// One page of `/series/search`.
    async fn search_page(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        extra: &[(&str, &str)],
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, CrawlError> {
        let limit = limit.to_string();
        let offset = offset.to_string();
        let mut query: Vec<(&str, &str)> = vec![
            ("api_key", api_key),
            ("file_type", "json"),
            ("limit", &limit),
            ("offset", &offset),
        ];
        query.extend_from_slice(extra);
        ctx.http
            .get_json(SourceId::Fred, &self.url("/series/search"), &query)
            .await
            .map_err(classify_fred_error)
    }

    /// All pages of `/series/search?search_text=term`, up to [`MAX_PAGES_PER_TERM`].
    async fn search_term(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        term: &str,
    ) -> Result<Vec<FredSeriesInfo>, CrawlError> {
        let mut out = Vec::new();
        let mut offset = 0;
        for _ in 0..MAX_PAGES_PER_TERM {
            let page = self
                .search_page(
                    ctx,
                    api_key,
                    &[("search_text", term)],
                    SEARCH_PAGE_SIZE,
                    offset,
                )
                .await?;
            let got = page.seriess.len();
            out.extend(page.seriess);
            offset += got;
            if got == 0 || offset >= page.count {
                break;
            }
        }
        Ok(out)
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

    /// Walks the most popular series (one page of [`POPULAR_PAGE_SIZE`]) and then each of
    /// [`SEARCH_TERMS`] (up to [`MAX_PAGES_PER_TERM`] pages of [`SEARCH_PAGE_SIZE`]), de-duplicated
    /// by series id in first-seen order.
    ///
    /// `Auth` and `RateLimited` errors abort discovery; other per-query failures are logged and
    /// that query skipped (as the old implementation did), unless every query failed.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let api_key = self.api_key(ctx)?;
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        let mut last_err = None;
        let mut any_ok = false;

        let mut add = |infos: Vec<FredSeriesInfo>| {
            for info in infos {
                if seen.insert(info.id.clone()) {
                    found.push(to_discovered(info));
                }
            }
        };

        let popular = self
            .search_page(
                ctx,
                api_key,
                &[
                    ("search_text", "*"),
                    ("order_by", "popularity"),
                    ("sort_order", "desc"),
                ],
                POPULAR_PAGE_SIZE,
                0,
            )
            .await;
        match popular {
            Ok(page) => {
                any_ok = true;
                add(page.seriess);
            }
            Err(e) => last_err = Some(skip_or_abort(e, "popular series")?),
        }

        for term in SEARCH_TERMS {
            match self.search_term(ctx, api_key, term).await {
                Ok(infos) => {
                    any_ok = true;
                    add(infos);
                }
                Err(e) => last_err = Some(skip_or_abort(e, term)?),
            }
        }

        match (any_ok, last_err) {
            (false, Some(e)) => Err(e),
            _ => {
                tracing::info!(series = found.len(), "FRED discovery finished");
                Ok(found)
            }
        }
    }

    /// `/series` (metadata), then every vintage of the observations on or after `since` from
    /// `/series/observations` (one request per [`OBSERVATIONS_PAGE_SIZE`] rows).
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
        let points = self
            .fetch_observations(ctx, api_key, external_id, since, known_vintage)
            .await?;
        Ok(FetchedSeries {
            metadata: Some(metadata),
            points,
        })
    }
}

/// Returns `err` for aborting errors (auth, rate limiting); otherwise logs it and hands it back
/// to be remembered as the last failure.
fn skip_or_abort(err: CrawlError, what: &str) -> Result<CrawlError, CrawlError> {
    match err {
        CrawlError::Auth(_) | CrawlError::RateLimited { .. } => Err(err),
        e => {
            tracing::warn!(query = what, error = %e, "FRED search failed; skipping");
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

fn to_discovered(info: FredSeriesInfo) -> DiscoveredSeries {
    DiscoveredSeries {
        data_url: Some(format!("{FRED_WEB_SERIES_URL}/{}", info.id)),
        external_id: info.id,
        title: info.title,
        description: non_empty(info.notes),
        units: non_empty(info.units),
        frequency: non_empty(info.frequency),
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

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    count: usize,
    seriess: Vec<FredSeriesInfo>,
}

#[derive(Debug, Deserialize)]
struct FredSeriesInfo {
    id: String,
    title: String,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    frequency: Option<String>,
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
    const SEARCH_P1: &str = include_str!("../../tests/fixtures/fred/series_search_page1.json");
    const SEARCH_P2: &str = include_str!("../../tests/fixtures/fred/series_search_page2.json");
    const SEARCH_EMPTY: &str = include_str!("../../tests/fixtures/fred/series_search_empty.json");

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

    /// FRED answers an unknown `series_id` with HTTP 400 and a JSON error body.
    ///
    /// Ignored until `HttpFetcher` quotes the (redacted) response body in status errors: today
    /// `CrawlError::from_status` only sees the status, so this 400 is indistinguishable from any
    /// other 400 and comes back `Permanent`. `classify_fred_error` is ready for it (see
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

    #[tokio::test]
    async fn discover_paginates_dedupes_and_walks_all_terms() {
        let mock = MockSource::start().await;
        // GDP: two pages (count 3; 2 + 1).
        mock.mount_expect(
            &Route::get("/series/search")
                .query("search_text", "GDP")
                .query("offset", "0"),
            Reply::json_str(SEARCH_P1),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/series/search")
                .query("search_text", "GDP")
                .query("offset", "2"),
            Reply::json_str(SEARCH_P2),
            1,
        )
        .await;
        // Popular page returns page 1 again: duplicates must be dropped.
        mock.mount_expect(
            &Route::get("/series/search")
                .query("search_text", "*")
                .query("order_by", "popularity")
                .query("limit", "100"),
            Reply::json_str(SEARCH_P1),
            1,
        )
        .await;
        // One term fails with a 400: skipped, not fatal.
        mock.mount(
            &Route::get("/series/search").query("search_text", "housing"),
            Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        )
        .await;
        mock.mount(&Route::get("/series/search"), Reply::json_str(SEARCH_EMPTY))
            .await;

        let found = FredAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(ids, ["GDP", "GDPC1", "A191RL1Q225SBEA"]);
        assert_eq!(found[0].title, "Gross Domestic Product");
        assert_eq!(found[0].units.as_deref(), Some("Billions of Dollars"));
        assert_eq!(found[0].frequency.as_deref(), Some("Quarterly"));
        assert_eq!(
            found[0].description.as_deref(),
            Some("BEA Account Code: A191RC")
        );
        assert_eq!(
            found[0].data_url.as_deref(),
            Some("https://fred.stlouisfed.org/series/GDP")
        );
        assert_eq!(found[1].description, None);
        assert_eq!(found[2].description, None, "empty notes -> None");

        // 1 popular + 2 GDP pages + 1 page for each of the other 14 terms.
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 1 + 2 + (SEARCH_TERMS.len() - 1));
        for r in &reqs {
            let q: Vec<(String, String)> = r.url.query_pairs().into_owned().collect();
            assert!(q.contains(&("api_key".into(), TEST_API_KEY.into())));
            assert!(q.contains(&("file_type".into(), "json".into())));
        }
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn discover_page_count_is_bounded() {
        let mock = MockSource::start().await;
        // Claims a huge count with 2 items per page, forever.
        let endless = SEARCH_P1.replace("\"count\": 3", "\"count\": 1000000");
        mock.mount(&Route::get("/series/search"), Reply::json_str(endless))
            .await;
        let found = FredAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(
            mock.received_requests().await.len(),
            1 + SEARCH_TERMS.len() * MAX_PAGES_PER_TERM
        );
    }

    #[tokio::test]
    async fn discover_aborts_on_rate_limit_and_fails_if_everything_fails() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series/search"),
            Reply::status(429).retry_after(60),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "rate_limited");
        assert_eq!(mock.received_requests().await.len(), 1);

        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series/search"),
            Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        )
        .await;
        let e = FredAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "permanent");
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
            route: Route::get("/series/search"),
            reply: Reply::json_str(include_str!("../../tests/fixtures/fred/series_search_page1.json")),
            min_series: 2,
        },
    }
}
