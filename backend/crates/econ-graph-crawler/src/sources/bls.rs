// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Bureau of Labor Statistics (BLS) Public Data API v2 adapter.
//!
//! # Fetching
//!
//! `POST {base}/timeseries/data/` with
//! `{"seriesid": [id], "startyear": "YYYY", "endyear": "YYYY", "catalog": true, "registrationkey"?: key}`.
//! `registrationkey` is sent only when `ctx.keys.bls` is set.
//!
//! BLS caps the year span of one request (20 years with a registration key, 10 without), so a
//! fetch is split into windows of that size, newest first:
//! - `since = Some(d)`: from `d.year()` to the current year; points before `d` are dropped.
//! - `since = None`: the last [`HISTORY_YEARS`] years (one request with a key, two without).
//!
//! Once a window has returned data, an older window that returns none ends the fetch early
//! (the series did not exist that far back).
//!
//! # Errors
//!
//! BLS answers HTTP 200 even on failure, with `status` and `message` in the body:
//! - a message about the daily threshold / "exceeded" (request not processed) -> `RateLimited { retry_after: None }`
//! - "Series does not exist" (any status), or no data at all with `since = None` -> `NotFound`
//! - a not-processed message about an invalid/unregistered key -> `Auth`
//! - any other status than `REQUEST_SUCCEEDED` -> `Permanent`
//!
//! HTTP-level errors are mapped by [`HttpFetcher`](crate::HttpFetcher). The registration key is
//! scrubbed from every message this module builds.
//!
//! # Periods
//!
//! [`parse_period`] is the single period parser (it replaces `parse_bls_date` and
//! `convert_bls_period_to_date` from the old services). Dates are period *starts*:
//! `M01..M12` -> 1st of the month, `Q01..Q04` -> 1st of Jan/Apr/Jul/Oct, `S01`/`S02` -> Jan 1 /
//! Jul 1, `A01` -> Jan 1. `M13` (annual average of a monthly series) is skipped: it is derived
//! from the monthly values and would otherwise collide with `M01`. Unknown periods are skipped.
//!
//! BLS keeps no vintages, so every point has `revision_date = date` and
//! `is_original_release = true` (as the old services did); re-crawls overwrite in place.

use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::adapter::{
    CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite, SourceAdapter,
};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The real BLS Public Data API v2 root.
pub const DEFAULT_BASE_URL: &str = "https://api.bls.gov/publicAPI/v2";

/// Years fetched when no `since` is given.
pub const HISTORY_YEARS: i32 = 20;

/// Maximum years per request with a registration key.
const MAX_YEARS_WITH_KEY: i32 = 20;
/// Maximum years per request without a registration key.
const MAX_YEARS_WITHOUT_KEY: i32 = 10;

const STATUS_SUCCEEDED: &str = "REQUEST_SUCCEEDED";

/// BLS adapter. See the module docs for request shape, windowing and error mapping.
#[derive(Debug, Clone)]
pub struct BlsAdapter {
    base_url: String,
    /// Fixed "current year" for tests; `None` uses the clock.
    current_year: Option<i32>,
}

impl BlsAdapter {
    /// Talks to `base_url` (no trailing slash) instead of the real API. Used by tests.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            current_year: None,
        }
    }

    #[cfg(test)]
    fn with_current_year(mut self, year: i32) -> Self {
        self.current_year = Some(year);
        self
    }

    fn current_year(&self) -> i32 {
        self.current_year.unwrap_or_else(|| Utc::now().year())
    }
}

impl Default for BlsAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

// ---------------------------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
struct DataRequest<'a> {
    seriesid: [&'a str; 1],
    startyear: String,
    endyear: String,
    catalog: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    registrationkey: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct DataResponse {
    status: String,
    #[serde(default)]
    message: Vec<String>,
    #[serde(rename = "Results", default)]
    results: Option<DataResults>,
}

#[derive(Debug, Default, Deserialize)]
struct DataResults {
    #[serde(default)]
    series: Vec<SeriesBody>,
}

#[derive(Debug, Deserialize)]
struct SeriesBody {
    #[serde(default)]
    catalog: Option<Catalog>,
    #[serde(default)]
    data: Vec<DataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    #[serde(default)]
    series_title: Option<String>,
    #[serde(default)]
    seasonality: Option<String>,
    #[serde(default)]
    survey_name: Option<String>,
    #[serde(default)]
    measure_data_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DataPoint {
    year: String,
    period: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct SurveysResponse {
    status: String,
    #[serde(default)]
    message: Vec<String>,
    #[serde(rename = "Results")]
    results: SurveysResults,
}

#[derive(Debug, Deserialize)]
struct SurveysResults {
    #[serde(default)]
    survey: Vec<Survey>,
}

#[derive(Debug, Deserialize)]
struct Survey {
    survey_abbreviation: String,
    #[serde(default)]
    survey_name: String,
}

// ---------------------------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------------------------

/// Converts a BLS `year` + `period` code to the period's start date.
///
/// Returns `Ok(None)` for periods that are deliberately skipped (`M13` annual averages and
/// unknown codes) and `Err(Parse)` for an unparsable year.
pub fn parse_period(year: &str, period: &str) -> Result<Option<NaiveDate>, CrawlError> {
    let y: i32 = year
        .trim()
        .parse()
        .map_err(|e| CrawlError::Parse(format!("BLS: invalid year {year:?}: {e}")))?;
    let period = period.trim();
    let (kind, num) = match (period.get(..1), period.get(1..)) {
        (Some(k), Some(n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            (k, n.parse::<u32>().unwrap_or(0))
        }
        _ => return Ok(None),
    };
    let month = match (kind, num) {
        ("M", 1..=12) => num,
        ("Q", 1..=4) => (num - 1) * 3 + 1,
        ("S", 1..=2) => (num - 1) * 6 + 1,
        ("A", 1) => 1,
        // M13 = annual average; anything else is unknown.
        _ => return Ok(None),
    };
    NaiveDate::from_ymd_opt(y, month, 1)
        .map(Some)
        .ok_or_else(|| CrawlError::Parse(format!("BLS: invalid date {year} {period}")))
}

/// Parses a BLS value. `"-"`, empty, and footnote-only markers such as `"(NA)"` (anything with
/// no digit) mean "no value". Thousands separators are ignored.
fn parse_value(raw: &str) -> Result<Option<BigDecimal>, CrawlError> {
    let v = raw.trim();
    if !v.bytes().any(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    let cleaned = v.replace(',', "");
    BigDecimal::from_str(&cleaned)
        .map(Some)
        .map_err(|e| CrawlError::Parse(format!("BLS: invalid value {raw:?}: {e}")))
}

/// Infers the frequency from the first few period codes (ported from `determine_bls_frequency`).
fn determine_frequency<'a>(periods: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    let sample: Vec<&str> = periods.into_iter().take(5).collect();
    [
        ('M', "Monthly"),
        ('Q', "Quarterly"),
        ('S', "Semi-Annual"),
        ('A', "Annual"),
    ]
    .into_iter()
    .find(|(prefix, _)| sample.iter().any(|p| p.starts_with(*prefix)))
    .map(|(_, name)| name)
}

/// Splits `start..=end` into windows of at most `span` years, newest first.
fn year_windows(start: i32, end: i32, span: i32) -> Vec<(i32, i32)> {
    let span = span.max(1);
    let mut windows = Vec::new();
    let mut hi = end;
    while hi >= start {
        let lo = (hi - span + 1).max(start);
        windows.push((lo, hi));
        hi = lo - 1;
    }
    windows
}

/// Removes `secret` from `text`.
fn scrub(text: &str, secret: Option<&str>) -> String {
    match secret {
        Some(s) if !s.is_empty() => text.replace(s, "<redacted>"),
        _ => text.to_string(),
    }
}

/// Maps a BLS body `status` + `message` to an error, or `Ok` if the request succeeded.
fn check_status(
    status: &str,
    messages: &[String],
    series_id: &str,
    key: Option<&str>,
) -> Result<(), CrawlError> {
    let joined = scrub(&messages.join("; "), key);
    let lower = joined.to_ascii_lowercase();
    if lower.contains("series does not exist") {
        return Err(CrawlError::NotFound(format!(
            "BLS series {series_id}: {joined}"
        )));
    }
    if status == STATUS_SUCCEEDED {
        return Ok(());
    }
    if lower.contains("threshold") || lower.contains("exceeded") {
        return Err(CrawlError::RateLimited { retry_after: None });
    }
    let key_problem = [
        "invalid",
        "not valid",
        "expired",
        "not registered",
        "unregistered",
    ]
    .iter()
    .any(|w| lower.contains(w));
    if lower.contains("key") && key_problem {
        return Err(CrawlError::Auth(format!("BLS: {joined}")));
    }
    Err(CrawlError::Permanent(format!(
        "BLS {status} for series {series_id}: {joined}"
    )))
}

// ---------------------------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------------------------

impl BlsAdapter {
    async fn fetch_window(
        &self,
        ctx: &CrawlCtx,
        series_id: &str,
        (start, end): (i32, i32),
    ) -> Result<(Option<Catalog>, Vec<DataPoint>), CrawlError> {
        let key = ctx.keys.bls.as_deref();
        let url = format!("{}/timeseries/data/", self.base_url);
        let body = DataRequest {
            seriesid: [series_id],
            startyear: start.to_string(),
            endyear: end.to_string(),
            catalog: true,
            registrationkey: key,
        };
        debug!(series_id, start, end, "BLS: fetching window");
        let resp: DataResponse = ctx
            .http
            .post_json(SourceId::Bls, &url, &body)
            .await
            .map_err(|e| scrub_error(e, key))?;
        check_status(&resp.status, &resp.message, series_id, key)?;
        let series = resp.results.unwrap_or_default().series.into_iter().next();
        Ok(series.map(|s| (s.catalog, s.data)).unwrap_or_default())
    }
}

fn scrub_error(e: CrawlError, key: Option<&str>) -> CrawlError {
    match e {
        CrawlError::Transient(m) => CrawlError::Transient(scrub(&m, key)),
        CrawlError::NotFound(m) => CrawlError::NotFound(scrub(&m, key)),
        CrawlError::Auth(m) => CrawlError::Auth(scrub(&m, key)),
        CrawlError::Parse(m) => CrawlError::Parse(scrub(&m, key)),
        CrawlError::Permanent(m) => CrawlError::Permanent(scrub(&m, key)),
        e @ CrawlError::RateLimited { .. } => e,
    }
}

#[async_trait]
impl SourceAdapter for BlsAdapter {
    fn id(&self) -> SourceId {
        SourceId::Bls
    }

    /// `GET {base}/surveys`, then the known series for each survey offered (ported from
    /// `series_discovery/bls.rs`; the surveys endpoint lists surveys, not series).
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let url = format!("{}/surveys", self.base_url);
        let resp: SurveysResponse = ctx.http.get_json(self.id(), &url, &[]).await?;
        check_status(
            &resp.status,
            &resp.message,
            "(surveys)",
            ctx.keys.bls.as_deref(),
        )?;
        let mut out: Vec<DiscoveredSeries> = Vec::new();
        for survey in &resp.results.survey {
            debug!(
                survey = %survey.survey_abbreviation,
                name = %survey.survey_name,
                "BLS: survey"
            );
            for known in known_series_for_survey(&survey.survey_abbreviation) {
                if out.iter().any(|s| s.external_id == known.id) {
                    continue;
                }
                out.push(DiscoveredSeries {
                    external_id: known.id.to_string(),
                    title: known.title.to_string(),
                    description: Some(known.title.to_string()),
                    units: Some(known.units.to_string()),
                    frequency: Some(known.frequency.to_string()),
                    data_url: Some(format!("{}/timeseries/data/{}", self.base_url, known.id)),
                });
            }
        }
        Ok(out)
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let end = self.current_year();
        let start = match since {
            Some(d) => d.year().min(end),
            None => end - HISTORY_YEARS + 1,
        };
        let span = if ctx.keys.bls.is_some() {
            MAX_YEARS_WITH_KEY
        } else {
            MAX_YEARS_WITHOUT_KEY
        };

        let mut catalog: Option<Catalog> = None;
        let mut raw: Vec<DataPoint> = Vec::new();
        for window in year_windows(start, end, span) {
            let (cat, data) = self.fetch_window(ctx, external_id, window).await?;
            if catalog.is_none() {
                catalog = cat;
            }
            if data.is_empty() && !raw.is_empty() {
                // Older than the series' first observation: nothing further back.
                break;
            }
            raw.extend(data);
        }

        if raw.is_empty() && since.is_none() {
            return Err(CrawlError::NotFound(format!(
                "BLS series {external_id}: no data for {start}-{end}"
            )));
        }

        let frequency = determine_frequency(raw.iter().map(|p| p.period.as_str()));
        let mut points = Vec::with_capacity(raw.len());
        for p in &raw {
            let Some(date) = parse_period(&p.year, &p.period)? else {
                if p.period != "M13" {
                    warn!(series = external_id, period = %p.period, "BLS: skipping unknown period");
                }
                continue;
            };
            if since.is_some_and(|s| date < s) {
                continue;
            }
            points.push(FetchedPoint {
                date,
                value: parse_value(&p.value)?,
                revision_date: date,
                is_original_release: true,
            });
        }
        points.sort_by_key(|p| p.date);
        points.dedup_by_key(|p| p.date);

        let metadata = catalog.map(|c| NewSeriesMetadataLite {
            title: c
                .series_title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("BLS Series {external_id}")),
            description: c.survey_name,
            units: c.measure_data_type,
            frequency: frequency.map(str::to_string),
            seasonal_adjustment: c.seasonality,
        });

        Ok(FetchedSeries { metadata, points })
    }
}

struct KnownSeries {
    id: &'static str,
    title: &'static str,
    frequency: &'static str,
    units: &'static str,
}

/// Known series per survey (from the old `get_known_bls_series_by_survey`). The unemployment
/// rate `LNS14000000` belongs to the CPS survey (`LN`); the old table listed it under `LA`, so
/// both map to it.
fn known_series_for_survey(abbreviation: &str) -> &'static [KnownSeries] {
    const CU: &[KnownSeries] = &[
        KnownSeries {
            id: "CUUR0000SA0",
            title: "Consumer Price Index for All Urban Consumers: All Items in U.S. City Average",
            frequency: "Monthly",
            units: "Index 1982-1984=100",
        },
        KnownSeries {
            id: "CUUR0000SA0L1E",
            title: "Consumer Price Index for All Urban Consumers: All Items Less Food and Energy in U.S. City Average",
            frequency: "Monthly",
            units: "Index 1982-1984=100",
        },
    ];
    const CE: &[KnownSeries] = &[KnownSeries {
        id: "CES0000000001",
        title: "All Employees, Total Nonfarm",
        frequency: "Monthly",
        units: "Thousands of Persons",
    }];
    const LN: &[KnownSeries] = &[KnownSeries {
        id: "LNS14000000",
        title: "Unemployment Rate",
        frequency: "Monthly",
        units: "Percent",
    }];
    match abbreviation {
        "CU" => CU,
        "CE" => CE,
        "LN" | "LA" => LN,
        _ => &[],
    }
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};
    use serde_json::json;

    const MONTHLY: &str = include_str!("../../tests/fixtures/bls/cpi_monthly.json");
    const QUARTERLY: &str = include_str!("../../tests/fixtures/bls/eci_quarterly.json");
    const THRESHOLD: &str = include_str!("../../tests/fixtures/bls/not_processed_threshold.json");
    const NO_SERIES: &str = include_str!("../../tests/fixtures/bls/series_does_not_exist.json");

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn route() -> Route {
        Route::post("/timeseries/data/")
    }

    #[test]
    fn default_uses_real_url() {
        assert_eq!(BlsAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(BlsAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(BlsAdapter::default().id(), SourceId::Bls);
    }

    /// Merges the period tests of `legacy_crawler_service` (`parse_bls_date`) and
    /// `simple_crawler_service` (`convert_bls_period_to_date`), normalized to period starts.
    #[test]
    fn period_parser_table() {
        let cases: &[(&str, &str, Option<NaiveDate>)] = &[
            ("2023", "M01", Some(d(2023, 1, 1))),
            ("2023", "M02", Some(d(2023, 2, 1))),
            ("2023", "M06", Some(d(2023, 6, 1))),
            ("2023", "M12", Some(d(2023, 12, 1))),
            ("2024", "M01", Some(d(2024, 1, 1))),
            ("2023", "M13", None), // annual average: skipped
            ("2023", "M00", None),
            ("2023", "M14", None),
            ("2023", "Q01", Some(d(2023, 1, 1))),
            ("2024", "Q02", Some(d(2024, 4, 1))),
            ("2023", "Q03", Some(d(2023, 7, 1))),
            ("2023", "Q04", Some(d(2023, 10, 1))),
            ("2023", "Q05", None),
            ("2023", "S01", Some(d(2023, 1, 1))),
            ("2023", "S02", Some(d(2023, 7, 1))),
            ("2023", "S03", None),
            ("2023", "A01", Some(d(2023, 1, 1))),
            ("2024", "A01", Some(d(2024, 1, 1))),
            ("2023", "A02", None),
            ("2023", "", None),
            ("2023", "X01", None),
            ("2023", "Mxx", None),
            (" 2023 ", " M03 ", Some(d(2023, 3, 1))),
        ];
        for (year, period, want) in cases {
            assert_eq!(
                parse_period(year, period).unwrap(),
                *want,
                "{year:?} {period:?}"
            );
        }
        assert_eq!(parse_period("20x3", "M01").unwrap_err().kind(), "parse");
    }

    #[test]
    fn value_parsing() {
        assert_eq!(
            parse_value("312.332").unwrap(),
            BigDecimal::from_str("312.332").ok()
        );
        assert_eq!(
            parse_value(" -0.4 ").unwrap(),
            BigDecimal::from_str("-0.4").ok()
        );
        assert_eq!(
            parse_value("1,234.5").unwrap(),
            BigDecimal::from_str("1234.5").ok()
        );
        for missing in ["-", "", " ", "(NA)", "(X)", "*"] {
            assert_eq!(parse_value(missing).unwrap(), None, "{missing:?}");
        }
        assert_eq!(parse_value("12abc").unwrap_err().kind(), "parse");
    }

    #[test]
    fn frequency_detection() {
        assert_eq!(determine_frequency(["M03", "M02"]), Some("Monthly"));
        assert_eq!(determine_frequency(["M13", "M12"]), Some("Monthly"));
        assert_eq!(determine_frequency(["Q02"]), Some("Quarterly"));
        assert_eq!(determine_frequency(["S01"]), Some("Semi-Annual"));
        assert_eq!(determine_frequency(["A01"]), Some("Annual"));
        assert_eq!(determine_frequency(["Z9"]), None);
        assert_eq!(determine_frequency(Vec::<&str>::new()), None);
    }

    #[test]
    fn windows_newest_first() {
        assert_eq!(year_windows(2007, 2026, 20), vec![(2007, 2026)]);
        assert_eq!(
            year_windows(2007, 2026, 10),
            vec![(2017, 2026), (2007, 2016)]
        );
        assert_eq!(
            year_windows(2000, 2026, 20),
            vec![(2007, 2026), (2000, 2006)]
        );
        assert_eq!(year_windows(2026, 2026, 20), vec![(2026, 2026)]);
        assert!(year_windows(2027, 2026, 20).is_empty());
    }

    #[test]
    fn status_mapping() {
        let msgs = |m: &[&str]| m.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let k = Some("sekrit");
        assert!(check_status(STATUS_SUCCEEDED, &[], "X", k).is_ok());
        assert!(check_status(
            STATUS_SUCCEEDED,
            &msgs(&["No Data Available for Series X Year: 2004"]),
            "X",
            k
        )
        .is_ok());
        let cases: &[(&str, &str, &str)] = &[
            ("REQUEST_NOT_PROCESSED", "Request could not be serviced, as the daily threshold for total number of requests allocated to the user has been reached.", "rate_limited"),
            ("REQUEST_NOT_PROCESSED", "User sekrit has exceeded the number of requests allowed", "rate_limited"),
            ("REQUEST_SUCCEEDED", "Series does not exist for Series X", "not_found"),
            ("REQUEST_NOT_PROCESSED", "Series does not exist for Series X", "not_found"),
            ("REQUEST_NOT_PROCESSED", "The key: sekrit provided by the User is invalid.", "auth"),
            ("REQUEST_NOT_PROCESSED", "Invalid registration key", "auth"),
            ("REQUEST_NOT_PROCESSED", "Invalid parameters: startyear", "permanent"),
            ("REQUEST_FAILED", "", "permanent"),
        ];
        for (status, msg, kind) in cases {
            let err = check_status(status, &msgs(&[msg]), "X", k).unwrap_err();
            assert_eq!(err.kind(), *kind, "{status} {msg}");
            assert!(!err.to_string().contains("sekrit"), "{err}");
        }
        assert_eq!(
            check_status(
                "REQUEST_NOT_PROCESSED",
                &msgs(&["daily threshold reached"]),
                "X",
                k
            )
            .unwrap_err()
            .retry_after(),
            None
        );
    }

    #[tokio::test]
    async fn monthly_fixture_parses_points_and_catalog() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(MONTHLY)).await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let s = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap();
        let got: Vec<_> = s.points.iter().map(|p| (p.date, p.value.clone())).collect();
        let bd = |v: &str| BigDecimal::from_str(v).ok();
        assert_eq!(
            got,
            vec![
                (d(2023, 11, 1), None),
                (d(2023, 12, 1), bd("306.746")),
                (d(2024, 1, 1), bd("308.417")),
                (d(2024, 2, 1), bd("310.326")),
                (d(2024, 3, 1), bd("312.332")),
            ]
        );
        assert!(s
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));
        let m = s.metadata.unwrap();
        assert_eq!(
            m.title,
            "All items in U.S. city average, all urban consumers, not seasonally adjusted"
        );
        assert_eq!(m.frequency.as_deref(), Some("Monthly"));
        assert_eq!(m.units.as_deref(), Some("Index 1982-1984=100"));
        assert_eq!(
            m.seasonal_adjustment.as_deref(),
            Some("Not Seasonally Adjusted")
        );
        assert_eq!(
            m.description.as_deref(),
            Some("CPI for All Urban Consumers (CPI-U)")
        );
    }

    #[tokio::test]
    async fn quarterly_fixture_without_catalog() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(QUARTERLY)).await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let s = adapter
            .fetch_series(&test_ctx(), "CIU1010000000000A", None)
            .await
            .unwrap();
        let dates: Vec<_> = s.points.iter().map(|p| p.date).collect();
        assert_eq!(
            dates,
            vec![d(2023, 7, 1), d(2023, 10, 1), d(2024, 1, 1), d(2024, 4, 1)]
        );
        assert!(s.metadata.is_none());
    }

    #[tokio::test]
    async fn request_body_with_key() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &route().body_contains(json!({
                "seriesid": ["CUUR0000SA0"],
                "startyear": "2005",
                "endyear": "2024",
                "catalog": true,
                "registrationkey": TEST_API_KEY,
            })),
            Reply::json_str(MONTHLY),
            1,
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn no_key_omits_registrationkey_and_uses_10_year_windows() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(MONTHLY)).await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        adapter
            .fetch_series(&ctx, "CUUR0000SA0", None)
            .await
            .unwrap();
        let bodies: Vec<serde_json::Value> = mock
            .received_requests()
            .await
            .iter()
            .map(|r| serde_json::from_slice(&r.body).unwrap())
            .collect();
        let ranges: Vec<_> = bodies
            .iter()
            .map(|b| (b["startyear"].clone(), b["endyear"].clone()))
            .collect();
        assert_eq!(
            ranges,
            vec![
                (json!("2015"), json!("2024")),
                (json!("2005"), json!("2014")),
            ]
        );
        for b in &bodies {
            assert!(b.get("registrationkey").is_none(), "{b}");
            assert_eq!(b["seriesid"], json!(["CUUR0000SA0"]));
            assert_eq!(b["catalog"], json!(true));
        }
    }

    #[tokio::test]
    async fn since_windows_from_since_year_and_filters() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(MONTHLY)).await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2026);
        let s = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", Some(d(2000, 6, 15)))
            .await
            .unwrap();
        let ranges: Vec<_> = mock
            .received_requests()
            .await
            .iter()
            .map(|r| {
                let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
                (
                    b["startyear"].as_str().unwrap().to_string(),
                    b["endyear"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![
                ("2007".to_string(), "2026".to_string()),
                ("2000".to_string(), "2006".to_string()),
            ]
        );
        // Both windows served the same fixture; points are deduplicated by date.
        assert_eq!(s.points.len(), 5);

        // since inside the data: earlier points are dropped, one window only.
        mock.reset().await;
        mock.mount(&route(), Reply::json_str(MONTHLY)).await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let s = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", Some(d(2024, 2, 1)))
            .await
            .unwrap();
        assert_eq!(mock.received_requests().await.len(), 1);
        let dates: Vec<_> = s.points.iter().map(|p| p.date).collect();
        assert_eq!(dates, vec![d(2024, 2, 1), d(2024, 3, 1)]);
    }

    #[tokio::test]
    async fn older_empty_window_stops_early() {
        let mock = MockSource::start().await;
        let empty = json!({"status": "REQUEST_SUCCEEDED", "message": ["No Data Available for Series X Year: 1990"],
            "Results": {"series": [{"seriesID": "X", "data": []}]}});
        mock.mount_expect(
            &route().body_contains(json!({"startyear": "2007"})),
            Reply::json_str(MONTHLY),
            1,
        )
        .await;
        mock.mount_expect(
            &route().body_contains(json!({"startyear": "1987"})),
            Reply::json(empty),
            1,
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2026);
        let s = adapter
            .fetch_series(&test_ctx(), "X", Some(d(1950, 1, 1)))
            .await
            .unwrap();
        assert_eq!(s.points.len(), 5);
        // Windows 1967-1986 and 1950-1966 were never requested.
        assert_eq!(mock.received_requests().await.len(), 2);
    }

    #[tokio::test]
    async fn empty_data_is_not_found_without_since_ok_with_since() {
        let mock = MockSource::start().await;
        mock.mount(
            &route(),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [{"seriesID": "X", "data": []}]}})),
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let err = adapter
            .fetch_series(&test_ctx(), "X", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "not_found");
        let s = adapter
            .fetch_series(&test_ctx(), "X", Some(d(2024, 9, 1)))
            .await
            .unwrap();
        assert!(s.points.is_empty());
    }

    #[tokio::test]
    async fn not_processed_threshold_is_rate_limited() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(THRESHOLD)).await;
        let adapter = BlsAdapter::new(mock.base_url());
        let err = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap_err();
        assert_eq!(err, CrawlError::RateLimited { retry_after: None });
    }

    #[tokio::test]
    async fn series_does_not_exist_is_not_found() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(NO_SERIES)).await;
        let adapter = BlsAdapter::new(mock.base_url());
        let err = adapter
            .fetch_series(&test_ctx(), "NOPE0000000", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "not_found");
        assert!(err.to_string().contains("NOPE0000000"));
    }

    #[tokio::test]
    async fn invalid_key_is_auth_and_key_never_in_error() {
        let mock = MockSource::start().await;
        mock.mount(
            &route(),
            Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                "message": [format!("The key:{TEST_API_KEY} provided by the User is invalid.")],
                "Results": {}})),
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url());
        let err = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "auth");
        assert!(!err.to_string().contains(TEST_API_KEY), "{err}");
        assert!(!format!("{err:?}").contains(TEST_API_KEY), "{err:?}");

        // Other not-processed messages -> Permanent, also scrubbed.
        mock.reset().await;
        mock.mount(
            &route(),
            Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                "message": [format!("Invalid parameter startyear for {TEST_API_KEY}")],
                "Results": {}})),
        )
        .await;
        let err = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "permanent", "{err}");
        assert!(!format!("{err:?}").contains(TEST_API_KEY), "{err:?}");
    }

    #[tokio::test]
    async fn http_errors_never_contain_key() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::text("boom").with_status(400))
            .await;
        let adapter = BlsAdapter::new(mock.base_url());
        let err = adapter
            .fetch_series(&test_ctx(), "CUUR0000SA0", None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "permanent");
        assert!(!format!("{err:?}").contains(TEST_API_KEY), "{err:?}");
    }

    #[tokio::test]
    async fn discover_maps_surveys_to_known_series() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/surveys"),
            Reply::json_str(include_str!("../../tests/fixtures/bls/surveys.json")),
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url());
        let found = adapter.discover(&test_ctx()).await.unwrap();
        let ids: Vec<_> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "CES0000000001",
                "CUUR0000SA0",
                "CUUR0000SA0L1E",
                "LNS14000000"
            ]
        );
        assert_eq!(
            found[0].data_url.as_deref(),
            Some(format!("{}/timeseries/data/CES0000000001", mock.base_url()).as_str())
        );
        assert_eq!(found[0].frequency.as_deref(), Some("Monthly"));
    }

    #[tokio::test]
    async fn discover_not_processed_is_error() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/surveys"), Reply::json_str(THRESHOLD))
            .await;
        let err = BlsAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "rate_limited");
    }

    #[test]
    fn registered_in_default_registry() {
        assert!(crate::sources::default_registry()
            .get(SourceId::Bls)
            .is_some());
    }
}

#[cfg(test)]
mod contract {
    use super::BlsAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| BlsAdapter::new(base_url),
        external_id: "CUUR0000SA0",
        route: Route::post("/timeseries/data/"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/bls/cpi_monthly.json")),
        expect_points: 5,
        discover: {
            route: Route::get("/surveys"),
            reply: Reply::json_str(include_str!("../../tests/fixtures/bls/surveys.json")),
            min_series: 4,
        },
    }
}
