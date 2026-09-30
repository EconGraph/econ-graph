// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FRED (Federal Reserve Economic Data, St. Louis Fed) adapter.
//!
//! Endpoints used (all `GET`, all with `api_key` and `file_type=json`):
//! - `{base}/series?series_id=ID` — series metadata (`seriess[0]`); used both by discovery (one
//!   request per curated id) and by fetching.
//! - `{base}/series/observations?series_id=ID[&observation_start=YYYY-MM-DD]` — observations.
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
//! it is discovered.
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

    /// Looks up each of `ids`' live metadata, dropping copyright-restricted series (the
    /// [`is_copyright_restricted`] safety net) and building a [`DiscoveredSeries`] from the rest.
    /// A per-id lookup failure is logged and that id skipped, unless every id fails; `Auth` and
    /// `RateLimited` errors abort immediately and discard every id already found in this call
    /// (matching the old search-based `discover`'s behaviour: a retry starts over from the top of
    /// the list rather than resuming, so a rate limit partway through is more of a delay here,
    /// since the list is walked in full on every discovery run either way).
    async fn discover_ids(
        &self,
        ctx: &CrawlCtx,
        api_key: &str,
        ids: &[String],
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let mut found = Vec::with_capacity(ids.len());
        let mut restricted_count = 0usize;
        let mut skipped_count = 0usize;
        let mut last_err = None;
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
                        dataset: Some(fred_dataset()),
                    });
                }
                Err(e) => {
                    last_err = Some(skip_or_abort(e, id)?);
                    skipped_count += 1;
                }
            }
        }

        match (any_ok, last_err) {
            (false, Some(e)) => Err(e),
            _ => {
                tracing::info!(
                    series = found.len(),
                    dropped_restricted = restricted_count,
                    dropped_errors = skipped_count,
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
            dataset: Some(fred_dataset()),
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
    observations: Vec<FredObservation>,
}

#[derive(Debug, Deserialize)]
struct FredObservation {
    date: String,
    value: String,
    #[serde(default)]
    realtime_start: Option<String>,
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
        assert_eq!(s.dataset, Some(fred_dataset()));

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

        assert!(s
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));

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
        for bad in [obs("2024-01-01", "n/a", None), obs("01/01/2024", "1", None)] {
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
            let ds = s.dataset.as_ref().expect("every FRED series has a dataset");
            assert_eq!(ds.code, DATASET, "{}", s.external_id);
            assert!(ds.dimensions.0.is_empty(), "{}", s.external_id);
        }
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn discover_ids_drops_copyright_restricted_series_as_a_safety_net() {
        let mock = MockSource::start().await;
        let notes_field = "\"notes\": \"BEA Account Code: A191RC\\n\\nGross domestic product \
            (GDP), the featured measure of U.S. output, is the market value of the goods and \
            services produced by labor and property located in the United States.\"";
        let restricted = series_fixture("CBBTCUSD", "Coinbase Bitcoin").replace(
            notes_field,
            "\"notes\": \"Reproduction, retransmission, or other use is prohibited except \
                      with prior written permission.\"",
        );
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
        expect_points: 5,
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
