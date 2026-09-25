// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FRED (Federal Reserve Economic Data, St. Louis Fed) adapter.
//!
//! Endpoints used (all `GET`, all with `api_key` and `file_type=json`):
//! - `{base}/series?series_id=ID` — series metadata (`seriess[0]`).
//! - `{base}/series/observations?series_id=ID[&observation_start=YYYY-MM-DD]` — observations.
//! - `{base}/series/search?search_text=..&limit=..&offset=..` — discovery.
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

    async fn fetch_observations(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        series_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<Vec<FetchedPoint>, CrawlError> {
        let since = since.map(|d| d.format("%Y-%m-%d").to_string());
        let mut query = vec![
            ("series_id", series_id),
            ("api_key", api_key),
            ("file_type", "json"),
        ];
        if let Some(s) = &since {
            query.push(("observation_start", s.as_str()));
        }
        let body: ObservationsResponse = ctx
            .http
            .get_json(SourceId::Fred, &self.url("/series/observations"), &query)
            .await
            .map_err(classify_fred_error)?;
        let today = chrono::Utc::now().date_naive();
        body.observations
            .into_iter()
            .map(|o| parse_observation(series_id, o, today))
            .collect()
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

    /// Two requests: `/series` (metadata) then `/series/observations`.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let api_key = self.api_key(ctx)?;
        let metadata = self.fetch_metadata(ctx, api_key, external_id).await?;
        let points = self
            .fetch_observations(ctx, api_key, external_id, since)
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

fn parse_observation(
    series_id: &str,
    o: FredObservation,
    today: NaiveDate,
) -> Result<FetchedPoint, CrawlError> {
    let date = parse_date(series_id, "date", &o.date)?;
    let value = match o.value.trim() {
        MISSING_VALUE | "" => None,
        v => Some(BigDecimal::from_str(v).map_err(|e| {
            CrawlError::Parse(format!(
                "FRED {series_id}: invalid value {v:?} on {date}: {e}"
            ))
        })?),
    };
    // We request current values only (no realtime_start/realtime_end vintage window), so FRED
    // stamps every observation with today's realtime_start. Using that as the revision date would
    // store a fresh copy of the whole series on every crawl. Instead key each point by its own
    // date as the original release, so re-crawls update values in place (same as BLS).
    // Vintage history would need realtime_start=1776-07-04 and a separate revision model.
    let _ = (&o.realtime_start, today);
    let revision_date = date;
    let is_original_release = true;
    Ok(FetchedPoint {
        date,
        value,
        revision_date,
        is_original_release,
    })
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
    observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
struct FredObservation {
    date: String,
    value: String,
    #[serde(default)]
    realtime_start: Option<String>,
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

    #[tokio::test]
    async fn fetch_parses_metadata_points_and_missing_values() {
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

        assert_eq!(s.points.len(), 5);
        let p0 = &s.points[0];
        assert_eq!(p0.date, d("2025-04-01"));
        assert_eq!(p0.value, Some(BigDecimal::from_str("30485.729").unwrap()));
        // Current-values mode: each point is its own original release (idempotent re-crawls).
        assert_eq!(p0.revision_date, p0.date);
        assert!(p0.is_original_release);

        // "." is a missing observation: kept, with no value.
        assert_eq!(s.points[2].date, d("2025-10-01"));
        assert_eq!(s.points[2].value, None);

        assert!(s.points.iter().all(|p| p.revision_date == p.date && p.is_original_release));

        // Two requests, both with the key and file_type=json.
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 2);
        for r in &reqs {
            let q: Vec<(String, String)> = r.url.query_pairs().into_owned().collect();
            assert!(q.contains(&("api_key".into(), TEST_API_KEY.into())));
            assert!(q.contains(&("file_type".into(), "json".into())));
            assert!(!q.iter().any(|(k, _)| k == "observation_start"));
        }
    }

    #[test]
    fn observation_parsing_rules() {
        let today = d("2026-09-25");
        let obs = |date: &str, value: &str, rt: Option<&str>| FredObservation {
            date: date.into(),
            value: value.into(),
            realtime_start: rt.map(Into::into),
        };
        // realtime_start is ignored in current-values mode: revision date = observation date.
        let p =
            parse_observation("X", obs("2024-01-01", "1.5", Some("2024-01-09")), today).unwrap();
        assert!(p.is_original_release);
        assert_eq!(p.revision_date, d("2024-01-01"));
        let p = parse_observation("X", obs("2026-09-24", "-0.25", None), today).unwrap();
        assert_eq!(p.revision_date, d("2026-09-24"));
        assert!(p.is_original_release);
        assert_eq!(p.value, Some(BigDecimal::from_str("-0.25").unwrap()));
        // Missing marker.
        assert_eq!(
            parse_observation("X", obs("2024-01-01", ".", None), today)
                .unwrap()
                .value,
            None
        );
        // Garbage is a parse error, not silently dropped.
        for bad in [
            obs("2024-01-01", "n/a", None),
            obs("01/01/2024", "1", None),
        ] {
            let e = parse_observation("X", bad, today).unwrap_err();
            assert_eq!(e.kind(), "parse", "{e}");
        }
    }

    #[tokio::test]
    async fn since_is_sent_as_observation_start() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        mock.mount_expect(
            &Route::get("/series/observations")
                .query("series_id", "GDP")
                .query("observation_start", "2026-01-01"),
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
        expect_points: 5,
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
