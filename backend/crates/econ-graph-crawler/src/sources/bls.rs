// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Bureau of Labor Statistics (BLS) Public Data API v2 adapter.
//!
//! # Discovery
//!
//! The series list is `bls_series.csv` in the crawler data directory
//! ([`crate::reference::bls_series`], read at runtime): headline CPI-U, CES payrolls and
//! earnings, CPS labor force measures, and LAUS state unemployment rates and labor force.
//! Discovery reads the file and makes no request, so it costs none of BLS's daily quota.
//!
//! # Fetching
//!
//! `POST {base}/timeseries/data/` with
//! `{"seriesid": [ids..], "startyear": "YYYY", "endyear": "YYYY", "catalog": true, "registrationkey"?: key}`.
//! `registrationkey` is sent only when `ctx.keys.bls` (`BLS_API_KEY`) is set.
//!
//! Every BLS series shares one [`batch_key`](SourceAdapter::batch_key), so the worker fetches up
//! to the policy's `max_batch` series in one request: [`MAX_SERIES_WITH_KEY`] (50) with a key,
//! [`MAX_SERIES_WITHOUT_KEY`] (25) without, BLS's per-request limits.
//! [`fetch_batch`](SourceAdapter::fetch_batch) also splits
//! a longer list into requests of that size, so a keyless context never sends 50.
//!
//! BLS caps the year span of one request (20 years with a registration key, 10 without), so a
//! fetch is split into windows of that size, newest first:
//! - `since = Some(d)`: from `d.year()` to the current year; points before `d` are dropped. In a
//!   batch, `since` is the earliest of the batch's series.
//! - `since = None`: the last [`HISTORY_YEARS`] years (one request with a key, two without).
//!
//! Once a window has returned data for a series, an older window that returns none for it drops
//! the series from later windows (it did not exist that far back); the fetch ends when no series
//! is left.
//!
//! # Request budget
//!
//! BLS allows 500 requests a day with a key and 25 without. For the shipped list (291 series,
//! all monthly), with full batches:
//! - With a key: 6 requests for the first 20-year fetch, and 6 for each weekly refresh
//!   (the 5-year revision lookback fits one window). Even one request per series (291) fits.
//! - Without a key: 12 batches of 25, so 24 requests for the first fetch (two 10-year windows)
//!   and 12 for each refresh. That leaves no headroom on the first day: a retry, a partial
//!   batch or a manual CLI fetch goes over, and the excess waits as `RateLimited`. Set a key.
//!
//! A batch's `since` is its earliest series', so one series with no stored points (a row added
//! later, or an id that keeps failing) turns its whole batch into a full 20-year fetch: two
//! requests per batch without a key instead of one (one with a key). An unrecognised
//! not-processed error retries the whole batch up to the policy's `max_retries` times.
//!
//! Discovery writes only `series_metadata`. Until the scheduler also fetches discovered series
//! (DATA-13, #228), the first fetch of a newly listed series needs `crawler enqueue --source BLS
//! --series <ids>`.
//!
//! # Errors
//!
//! BLS answers HTTP 200 even on failure, with `status` and `message` in the body:
//! - a message about the daily threshold / "exceeded" (request not processed) -> `RateLimited { retry_after: None }`
//!   for the whole batch
//! - "Series does not exist for Series X" -> `NotFound` for series X only; the rest of the batch
//!   is kept. On an older window, after X returned data from newer ones, it only marks the end
//!   of X's history and the data is kept
//! - no data at all with `since = None` -> `NotFound` for that series
//! - a not-processed message about an invalid/unregistered key -> `Auth` for the whole batch
//! - a not-processed response whose only messages are per-series ("does not exist", "No Data
//!   Available") -> `NotFound` for series named as not existing (or the end of their history, as
//!   above) and `Transient` for any it left out
//! - "invalid parameter" -> `Permanent` for the whole batch
//! - any other status than `REQUEST_SUCCEEDED` -> `Transient` for the whole batch, so the worker
//!   retries it up to `max_retries` instead of failing up to 50 series for good
//!
//! HTTP-level errors are mapped by [`HttpFetcher`](crate::HttpFetcher). The registration key is
//! scrubbed from every message this module builds.
//!
//! # Periods and footnotes
//!
//! [`parse_period`] is the single period parser (it replaces `parse_bls_date` and
//! `convert_bls_period_to_date` from the old services). Dates are period *starts*:
//! `M01..M12` -> 1st of the month, `Q01..Q04` -> 1st of Jan/Apr/Jul/Oct, `S01`/`S02` -> Jan 1 /
//! Jul 1, `A01` -> Jan 1. Annual averages (`M13`, `Q05`, `S03`) are skipped: they are derived
//! from the other periods and would otherwise collide with them. Unknown periods are skipped.
//!
//! Each observation's `footnotes` (`{code, text}`; `[{}]` means none) are parsed. A value such
//! as `"-"` with footnote `X` ("data unavailable") is stored as a missing value, and the
//! footnote text is logged. `P` (preliminary) values are stored like any other: BLS replaces
//! them on a later crawl. `data_points` has no column for footnotes yet, so they are not
//! stored.
//!
//! BLS keeps no vintages, so every point has `revision_date = date` and
//! `is_original_release = true` (as the old services did); re-crawls overwrite in place.
//!
//! # Datasets
//!
//! Each survey with a layout in [`SERIES_ID_LAYOUTS`] is one dataset, coded by its two-letter
//! series id prefix and defined in `data/datasets/bls.toml`. A series id is that prefix followed
//! by fixed-width fields, which [`series_dataset`] splits into the dataset's dimensions. Series
//! keep their BLS ids as external ids, since the API takes those. A series of any other survey,
//! or whose id does not fit its survey's layout, is written without a dataset.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::adapter::{
    ApiKeys, BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::reference::{bls_series, BlsSeries};
use crate::source::SourceId;

/// The real BLS Public Data API v2 root.
pub const DEFAULT_BASE_URL: &str = "https://api.bls.gov/publicAPI/v2";

/// Years fetched when no `since` is given.
pub const HISTORY_YEARS: i32 = 20;

/// Maximum years per request with a registration key.
const MAX_YEARS_WITH_KEY: i32 = 20;
/// Maximum years per request without a registration key.
const MAX_YEARS_WITHOUT_KEY: i32 = 10;

/// Maximum series per request with a registration key.
pub const MAX_SERIES_WITH_KEY: usize = 50;
/// Maximum series per request without a registration key.
pub const MAX_SERIES_WITHOUT_KEY: usize = 25;

/// The batch key every BLS series shares: any series can go in one request.
const BATCH_KEY: &str = "timeseries";

const STATUS_SUCCEEDED: &str = "REQUEST_SUCCEEDED";

/// Prefix of BLS's per-series "does not exist" message; the series id follows it.
const DOES_NOT_EXIST: &str = "series does not exist for series ";

/// Prefix of BLS's informational "no data" message; the series id follows it.
const NO_DATA_FOR_SERIES: &str = "no data available for series ";

/// Series id layouts of the surveys that are datasets: the two-letter prefix (also the dataset
/// code), then each dimension's name and width in id order. A width of 0 takes the rest of the
/// id and is only used last. See <https://www.bls.gov/help/hlpforma.htm>.
pub const SERIES_ID_LAYOUTS: &[(&str, &[(&str, usize)])] = &[
    // CPI-U: CUUR0000SA0.
    (
        "CU",
        &[
            ("seasonal", 1),
            ("periodicity", 1),
            ("area", 4),
            ("item", 0),
        ],
    ),
    // CES national: CES0000000001.
    ("CE", &[("seasonal", 1), ("industry", 8), ("data_type", 2)]),
    // CPS: LNS14000000.
    ("LN", &[("seasonal", 1), ("series_code", 8)]),
    // LAUS: LASST060000000000003.
    ("LA", &[("seasonal", 1), ("area", 15), ("measure", 2)]),
];

/// The codes in [`SERIES_ID_LAYOUTS`], for [`SourceAdapter::datasets`].
const DATASET_CODES: &[&str] = &["CU", "CE", "LN", "LA"];

/// BLS adapter. See the module docs for request shape, batching, windowing and error mapping.
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

/// The BLS policy: the built-in one with `max_batch` at BLS's per-request series limit, which
/// depends on whether a registration key is configured.
pub fn policy(keyed: bool) -> SourcePolicy {
    SourcePolicy {
        max_batch: max_series_per_request(keyed),
        ..SourcePolicy::default_for(SourceId::Bls)
    }
}

fn max_series_per_request(keyed: bool) -> usize {
    if keyed {
        MAX_SERIES_WITH_KEY
    } else {
        MAX_SERIES_WITHOUT_KEY
    }
}

// ---------------------------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------------------------

#[derive(Serialize)]
struct DataRequest<'a> {
    seriesid: &'a [&'a str],
    startyear: String,
    endyear: String,
    catalog: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    registrationkey: Option<&'a str>,
}

/// Deserializes a JSON array field that BLS may send as an explicit `null` (not just omit) as an
/// empty `Vec`. `#[serde(default)]` alone only covers a missing field, not an explicit `null`,
/// which would otherwise fail the whole batch with a `Parse` error.
fn null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct DataResponse {
    status: String,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    message: Vec<String>,
    #[serde(rename = "Results", default)]
    results: Option<DataResults>,
}

#[derive(Debug, Default, Deserialize)]
struct DataResults {
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    series: Vec<SeriesBody>,
}

#[derive(Debug, Deserialize)]
struct SeriesBody {
    #[serde(rename = "seriesID", default)]
    series_id: Option<String>,
    #[serde(default)]
    catalog: Option<Catalog>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
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
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    footnotes: Vec<Footnote>,
}

/// One observation footnote. BLS sends `[{}]` for "no footnotes", so both fields are optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
struct Footnote {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

impl DataPoint {
    /// Footnotes that carry a code or text (the `{}` placeholder dropped).
    fn footnotes(&self) -> impl Iterator<Item = &Footnote> {
        self.footnotes.iter().filter(|f| {
            f.code.as_deref().is_some_and(|c| !c.trim().is_empty())
                || f.text.as_deref().is_some_and(|t| !t.trim().is_empty())
        })
    }

    /// Whether BLS marked the value preliminary (footnote code `P`).
    fn is_preliminary(&self) -> bool {
        self.footnotes()
            .any(|f| f.code.as_deref().map(str::trim) == Some("P"))
    }

    /// The footnotes as `code: text` pairs, for logs.
    fn footnote_summary(&self) -> String {
        self.footnotes()
            .map(|f| {
                format!(
                    "{}: {}",
                    f.code.as_deref().unwrap_or("").trim(),
                    f.text.as_deref().unwrap_or("").trim()
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

// ---------------------------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------------------------

/// Converts a BLS `year` + `period` code to the period's start date.
///
/// Returns `Ok(None)` for periods that are deliberately skipped (annual averages `M13`, `Q05`,
/// `S03` and unknown codes) and `Err(Parse)` for an unparsable year.
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
        // M13, Q05, S03 = annual averages; anything else is unknown.
        _ => return Ok(None),
    };
    NaiveDate::from_ymd_opt(y, month, 1)
        .map(Some)
        .ok_or_else(|| CrawlError::Parse(format!("BLS: invalid date {year} {period}")))
}

/// Whether `period` is an annual average (`M13`, `Q05`, `S03`), skipped without a warning.
fn is_annual_average(period: &str) -> bool {
    matches!(period.trim(), "M13" | "Q05" | "S03")
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

/// The dataset and dimension values of BLS series `series_id`, split by its survey's layout in
/// [`SERIES_ID_LAYOUTS`]. `None` for a survey without a layout, an id whose length does not fit
/// the layout, or an id with characters other than ASCII letters and digits.
pub fn series_dataset(series_id: &str) -> Option<SeriesDataset> {
    if !series_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    let prefix = series_id.get(..2)?;
    let (code, layout) = SERIES_ID_LAYOUTS.iter().find(|(p, _)| *p == prefix)?;
    let mut rest = &series_id[2..];
    let mut dimensions = Vec::with_capacity(layout.len());
    for &(name, width) in *layout {
        let width = if width == 0 { rest.len() } else { width };
        if width == 0 || rest.len() < width {
            return None;
        }
        let (value, tail) = rest.split_at(width);
        dimensions.push((name, value));
        rest = tail;
    }
    rest.is_empty()
        .then(|| SeriesDataset::new(*code, dimensions))
}

/// Removes `secret` from `text`.
fn scrub(text: &str, secret: Option<&str>) -> String {
    match secret {
        Some(s) if !s.is_empty() => text.replace(s, "<redacted>"),
        _ => text.to_string(),
    }
}

/// Whether `message` is informational and about one series ("No Data Available for Series X
/// Year: Y"), not a reason to fail the request.
fn is_no_data_message(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .starts_with("no data available for series")
}

/// The series id named by a "Series does not exist for Series X" message, if `message` is one.
fn missing_series_id(message: &str) -> Option<&str> {
    let lower = message.to_ascii_lowercase();
    let at = lower.find(DOES_NOT_EXIST)? + DOES_NOT_EXIST.len();
    message
        .get(at..)?
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .find(|s| !s.is_empty())
}

/// The series id named by a "No Data Available for Series X Year: Y" message, if `message` is
/// one.
fn no_data_series_id(message: &str) -> Option<&str> {
    let lower = message.to_ascii_lowercase();
    let at = lower.find(NO_DATA_FOR_SERIES)? + NO_DATA_FOR_SERIES.len();
    message
        .get(at..)?
        .split(|c: char| c.is_whitespace() || c == ',' || c == '.')
        .find(|s| !s.is_empty())
}

/// Maps a BLS body `status` + `message` to a whole-request error, or `Ok` if the request was
/// processed. `ids` are the series in the request; errors name the first one and a count.
///
/// "Series does not exist for Series X" messages are per-series results, not request failures:
/// they are handled by the caller, and a response carrying only those (and informational "No
/// Data Available for Series X" messages) is `Ok` (the caller fails
/// the series it did not get back). The other messages decide, in this order: threshold ->
/// `RateLimited`, key problem -> `Auth`, a "series does not exist" that names no id in a
/// single-series request -> `NotFound`, "invalid parameter" -> `Permanent`, anything else ->
/// `Transient` (retried up to the policy's `max_retries`, so an unknown message never fails a
/// whole batch for good on the first try).
fn check_status(
    status: &str,
    messages: &[String],
    ids: &[&str],
    key: Option<&str>,
) -> Result<(), CrawlError> {
    let requested = match ids {
        [one] => one.to_string(),
        [first, rest @ ..] => format!("{first} and {} more", rest.len()),
        [] => "(none)".to_string(),
    };
    let other: Vec<&str> = messages
        .iter()
        .map(String::as_str)
        .filter(|m| missing_series_id(m).is_none() && !is_no_data_message(m))
        .collect();
    if status == STATUS_SUCCEEDED || (other.is_empty() && !messages.is_empty()) {
        return Ok(());
    }
    let joined = scrub(&other.join("; "), key);
    let lower = joined.to_ascii_lowercase();
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
    // Only a single-series request can pin an unnamed "does not exist" on its series.
    if lower.contains("series does not exist") && ids.len() == 1 {
        return Err(CrawlError::NotFound(format!(
            "BLS series {requested}: {joined}"
        )));
    }
    if lower.contains("invalid parameter") {
        return Err(CrawlError::Permanent(format!(
            "BLS {status} for series {requested}: {joined}"
        )));
    }
    // Unrecognised: retry (bounded by max_retries) rather than failing a whole batch for good.
    Err(CrawlError::Transient(format!(
        "BLS {status} for series {requested}: {joined}"
    )))
}

fn scrub_error(e: CrawlError, key: Option<&str>) -> CrawlError {
    match e {
        CrawlError::Transient(m) => CrawlError::Transient(scrub(&m, key)),
        CrawlError::NotFound(m) => CrawlError::NotFound(scrub(&m, key)),
        CrawlError::Auth(m) => CrawlError::Auth(scrub(&m, key)),
        CrawlError::Parse(m) => CrawlError::Parse(scrub(&m, key)),
        CrawlError::Permanent(m) => CrawlError::Permanent(scrub(&m, key)),
        CrawlError::Busy {
            retry_after,
            message,
        } => CrawlError::Busy {
            retry_after,
            message: scrub(&message, key),
        },
        e @ CrawlError::RateLimited { .. } => e,
    }
}

/// Seasonal adjustment as the id encodes it, for the surveys in the series list: the letter after
/// the two-letter survey prefix is `S` (adjusted) or `U` (not), as in `CUSR`/`CUUR`,
/// `CES`/`CEU`, `LNS`/`LNU` and `LASST`/`LAUST`.
fn seasonal_adjustment_from_id(external_id: &str) -> Option<&'static str> {
    match external_id.get(..3)? {
        "CUS" | "CES" | "LNS" | "LAS" => Some("Seasonally Adjusted"),
        "CUU" | "CEU" | "LNU" | "LAU" => Some("Not Seasonally Adjusted"),
        _ => None,
    }
}

/// The series list's row for `external_id`, if the list is readable and has it.
fn listed_series(external_id: &str) -> Option<&'static BlsSeries> {
    bls_series().ok()?.iter().find(|s| s.id == external_id)
}

/// Converts one series' raw observations to a [`FetchedSeries`], dropping points before `since`.
fn build_series(
    external_id: &str,
    catalog: Option<Catalog>,
    raw: &[DataPoint],
    since: Option<NaiveDate>,
) -> Result<FetchedSeries, CrawlError> {
    let frequency = determine_frequency(raw.iter().map(|p| p.period.as_str()));
    let mut points = Vec::with_capacity(raw.len());
    let mut preliminary = 0usize;
    for p in raw {
        let Some(date) = parse_period(&p.year, &p.period)? else {
            if !is_annual_average(&p.period) {
                warn!(series = external_id, period = %p.period, "BLS: skipping unknown period");
            }
            continue;
        };
        if since.is_some_and(|s| date < s) {
            continue;
        }
        let value = parse_value(&p.value)?;
        if value.is_none() {
            debug!(
                series = external_id,
                %date,
                raw = %p.value,
                footnotes = %p.footnote_summary(),
                "BLS: no value"
            );
        }
        if p.is_preliminary() {
            preliminary += 1;
        }
        points.push(FetchedPoint {
            date,
            value,
            revision_date: date,
            is_original_release: true,
        });
    }
    points.sort_by_key(|p| p.date);
    points.dedup_by_key(|p| p.date);
    if preliminary > 0 {
        debug!(series = external_id, preliminary, "BLS: preliminary values");
    }

    // A listed series keeps the list's title, units and frequency (so discovery and fetch agree,
    // with or without a key); BLS's catalog, sent only with a key, adds the survey name and
    // seasonality. An unlisted series uses the catalog alone.
    let row = listed_series(external_id);
    let metadata = match (row, catalog) {
        (Some(row), catalog) => {
            let (description, seasonality) =
                catalog.map_or((None, None), |c| (c.survey_name, c.seasonality));
            Some(NewSeriesMetadataLite {
                title: row.title.clone(),
                description,
                units: Some(row.units.clone()),
                frequency: Some(row.frequency.clone()),
                seasonal_adjustment: seasonality
                    .or_else(|| seasonal_adjustment_from_id(external_id).map(str::to_string)),
            })
        }
        (None, Some(c)) => Some(NewSeriesMetadataLite {
            title: c
                .series_title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("BLS Series {external_id}")),
            description: c.survey_name,
            units: c.measure_data_type,
            frequency: frequency.map(str::to_string),
            seasonal_adjustment: c.seasonality,
        }),
        (None, None) => None,
    };
    Ok(FetchedSeries {
        metadata,
        points,
        dataset: series_dataset(external_id),
    })
}

// ---------------------------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------------------------

/// What one request group has gathered so far for one series.
#[derive(Default)]
struct SeriesState {
    catalog: Option<Catalog>,
    raw: Vec<DataPoint>,
    /// The series' own failure (it does not exist, or a not-processed response left it out).
    failed: Option<CrawlError>,
    /// An older window returned nothing after a newer one had data: stop asking.
    done: bool,
}

impl SeriesState {
    /// A "does not exist" for an older window: the end of the series' history if newer windows
    /// returned data, otherwise the series' own `NotFound`.
    fn end_or_fail(&mut self, not_found: CrawlError) {
        if self.raw.is_empty() {
            self.failed = Some(not_found);
        } else {
            self.done = true;
        }
    }
}

impl BlsAdapter {
    /// One POST for `ids` over the years `start..=end`.
    async fn fetch_window(
        &self,
        ctx: &CrawlCtx,
        ids: &[&str],
        (start, end): (i32, i32),
    ) -> Result<DataResponse, CrawlError> {
        let key = ctx.keys.bls.as_deref();
        let url = format!("{}/timeseries/data/", self.base_url);
        let body = DataRequest {
            seriesid: ids,
            startyear: start.to_string(),
            endyear: end.to_string(),
            catalog: true,
            registrationkey: key,
        };
        debug!(series = ids.len(), start, end, "BLS: fetching window");
        let resp: DataResponse = ctx
            .http
            .post_json(SourceId::Bls, &url, &body)
            .await
            .map_err(|e| scrub_error(e, key))?;
        check_status(&resp.status, &resp.message, ids, key)?;
        Ok(resp)
    }

    /// Fetches `ids` (at most one request's worth) window by window, newest first.
    /// `Err` means a request failed and applies to every id.
    async fn fetch_group(
        &self,
        ctx: &CrawlCtx,
        ids: &[String],
        since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let keyed = ctx.keys.bls.is_some();
        let key = ctx.keys.bls.as_deref();
        let end = self.current_year();
        let start = match since {
            Some(d) => d.year().min(end),
            None => end - HISTORY_YEARS + 1,
        };
        let span = if keyed {
            MAX_YEARS_WITH_KEY
        } else {
            MAX_YEARS_WITHOUT_KEY
        };

        // Unique ids, in the order the worker asked for them.
        let mut seen = HashSet::new();
        let unique: Vec<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|id| seen.insert(*id))
            .collect();
        let mut state: HashMap<&str, SeriesState> = unique
            .iter()
            .map(|id| (*id, SeriesState::default()))
            .collect();
        for window in year_windows(start, end, span) {
            let active: Vec<&str> = unique
                .iter()
                .copied()
                .filter(|id| state[id].failed.is_none() && !state[id].done)
                .collect();
            if active.is_empty() {
                break;
            }
            let resp = match self.fetch_window(ctx, &active, window).await {
                Ok(resp) => resp,
                // A lone series left in a later window: its NotFound is its own, not the group's,
                // and if newer windows already returned data, this is just the end of its history.
                Err(e @ CrawlError::NotFound(_)) if active.len() == 1 => {
                    state
                        .get_mut(active[0])
                        .expect("active ids have state")
                        .end_or_fail(e);
                    continue;
                }
                Err(e) => return Err(e),
            };
            // Not processed, yet only per-series messages: the series BLS names don't exist, and
            // the ones it doesn't return were not served, so they fail (and are retried) rather
            // than completing with no data.
            let partial = resp.status != STATUS_SUCCEEDED;
            for m in &resp.message {
                if let Some(id) = missing_series_id(m) {
                    if let Some(s) = state.get_mut(id) {
                        s.end_or_fail(CrawlError::NotFound(format!(
                            "BLS series {id}: {}",
                            scrub(m, key)
                        )));
                    }
                }
            }
            let mut returned: HashMap<&str, SeriesBody> = HashMap::new();
            for body in resp.results.unwrap_or_default().series {
                let id = match body.series_id.as_deref() {
                    Some(id) => id.trim().to_string(),
                    // A single-series request may omit the id.
                    None if active.len() == 1 => active[0].to_string(),
                    None => {
                        warn!("BLS: series without seriesID in a batch response; ignored");
                        continue;
                    }
                };
                match active.iter().find(|a| **a == id) {
                    Some(a) => {
                        returned.insert(a, body);
                    }
                    None => warn!(series = %id, "BLS: unrequested series in response; ignored"),
                }
            }
            for id in &active {
                let s = state.get_mut(id).expect("active ids have state");
                if s.failed.is_some() || s.done {
                    continue;
                }
                let body = returned.remove(id);
                if body.is_none() && partial {
                    // A "no data" message naming this id, with data already collected from newer
                    // windows, is the end of its history, not a request failure: don't discard it.
                    let no_data = resp
                        .message
                        .iter()
                        .any(|m| no_data_series_id(m) == Some(*id));
                    if no_data && !s.raw.is_empty() {
                        s.done = true;
                    } else {
                        s.failed = Some(CrawlError::Transient(format!(
                            "BLS series {id}: not returned ({})",
                            resp.status
                        )));
                    }
                    continue;
                }
                let (catalog, data) = body.map(|b| (b.catalog, b.data)).unwrap_or_default();
                if s.catalog.is_none() {
                    s.catalog = catalog;
                }
                if data.is_empty() && !s.raw.is_empty() {
                    // Older than the series' first observation: nothing further back.
                    s.done = true;
                }
                s.raw.extend(data);
            }
        }

        let mut out = BatchFetch::with_capacity(ids.len());
        for (id, s) in state {
            let result = if let Some(e) = s.failed {
                Err(e)
            } else if s.raw.is_empty() && since.is_none() {
                Err(CrawlError::NotFound(format!(
                    "BLS series {id}: no data for {start}-{end}"
                )))
            } else {
                build_series(id, s.catalog, &s.raw, since)
            };
            out.insert(id.to_string(), result);
        }
        Ok(out)
    }
}

#[async_trait]
impl SourceAdapter for BlsAdapter {
    fn id(&self) -> SourceId {
        SourceId::Bls
    }

    /// The built-in BLS policy with `max_batch` 50 when `BLS_API_KEY` is set, 25 otherwise.
    ///
    /// Reads the key from the environment, as the worker does when it builds `ctx.keys`; if the
    /// two ever disagree, [`fetch_batch`](SourceAdapter::fetch_batch) still splits by the key in
    /// `ctx`, so no request exceeds BLS's limit.
    fn policy(&self) -> SourcePolicy {
        policy(ApiKeys::from_env().bls.is_some())
    }

    fn datasets(&self) -> &[&str] {
        DATASET_CODES
    }

    /// The series in `bls_series.csv`. No request is made.
    async fn discover(&self, _ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        Ok(bls_series()?
            .iter()
            .map(|s| DiscoveredSeries {
                external_id: s.id.clone(),
                title: s.title.clone(),
                description: None,
                units: Some(s.units.clone()),
                frequency: Some(s.frequency.clone()),
                data_url: Some(format!("{}/timeseries/data/{}", self.base_url, s.id)),
                dataset: series_dataset(&s.id),
            })
            .collect())
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let ids = [external_id.to_string()];
        self.fetch_group(ctx, &ids, since)
            .await?
            .remove(external_id)
            .unwrap_or_else(|| Err(CrawlError::NotFound(format!("BLS series {external_id}"))))
    }

    fn batch_key(&self, _external_id: &str) -> Option<String> {
        Some(BATCH_KEY.to_string())
    }

    /// Fetches `external_ids` in requests of at most 50 series (25 without a key). When the ids
    /// need several requests and one fails, that failure applies to its own series only, except
    /// `RateLimited` and `Auth`: those apply to every series not fetched yet, and no further
    /// request is sent (it would fail the same way and spend quota). On the first request they
    /// fail the whole call.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let size = max_series_per_request(ctx.keys.bls.is_some());
        if external_ids.len() <= size {
            return self.fetch_group(ctx, external_ids, since).await;
        }
        let mut out = BatchFetch::with_capacity(external_ids.len());
        for (i, chunk) in external_ids.chunks(size).enumerate() {
            match self.fetch_group(ctx, chunk, since).await {
                Ok(results) => out.extend(results),
                // Nothing fetched yet: fail the whole call, so the worker's breaker sees it.
                Err(e @ (CrawlError::RateLimited { .. } | CrawlError::Auth(_))) if i == 0 => {
                    return Err(e)
                }
                Err(e @ (CrawlError::RateLimited { .. } | CrawlError::Auth(_))) => {
                    let rest = &external_ids[i * size..];
                    out.extend(rest.iter().map(|id| (id.clone(), Err(e.clone()))));
                    break;
                }
                Err(e) => out.extend(chunk.iter().map(|id| (id.clone(), Err(e.clone())))),
            }
        }
        Ok(out)
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
    const BATCH: &str = include_str!("../../tests/fixtures/bls/batch_mixed.json");

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn route() -> Route {
        Route::post("/timeseries/data/")
    }

    fn ok_empty() -> serde_json::Value {
        json!({"status": "REQUEST_SUCCEEDED", "message": [], "Results": {"series": []}})
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
        assert!(check_status(STATUS_SUCCEEDED, &[], &["X"], k).is_ok());
        assert!(check_status(
            STATUS_SUCCEEDED,
            &msgs(&["No Data Available for Series X Year: 2004"]),
            &["X"],
            k
        )
        .is_ok());
        let cases: &[(&str, &str, &str)] = &[
            ("REQUEST_NOT_PROCESSED", "Request could not be serviced, as the daily threshold for total number of requests allocated to the user has been reached.", "rate_limited"),
            ("REQUEST_NOT_PROCESSED", "User sekrit has exceeded the number of requests allowed", "rate_limited"),
            ("REQUEST_NOT_PROCESSED", "Series does not exist", "not_found"),
            // Per-series messages never mask a request failure.
            ("REQUEST_NOT_PROCESSED", "Series does not exist for Series X|The daily threshold has been reached", "rate_limited"),
            ("REQUEST_NOT_PROCESSED", "No Data Available for Series Y Year: 2005|Invalid parameters", "permanent"),
            ("REQUEST_NOT_PROCESSED", "Series does not exist for Series X|The key: sekrit provided by the User is invalid.", "auth"),
            ("REQUEST_NOT_PROCESSED", "The key: sekrit provided by the User is invalid.", "auth"),
            ("REQUEST_NOT_PROCESSED", "Invalid registration key", "auth"),
            ("REQUEST_NOT_PROCESSED", "Invalid parameters: startyear", "permanent"),
            ("REQUEST_FAILED", "", "transient"),
            ("REQUEST_NOT_PROCESSED", "Something unexpected happened", "transient"),
        ];
        for (status, msg, kind) in cases {
            let parts: Vec<&str> = msg.split('|').collect();
            let err = check_status(status, &msgs(&parts), &["X"], k).unwrap_err();
            assert_eq!(err.kind(), *kind, "{status} {msg}");
            assert!(!err.to_string().contains("sekrit"), "{err}");
        }
        // An unnamed "does not exist" can't be pinned on one series of a batch: retry it.
        let err = check_status(
            "REQUEST_NOT_PROCESSED",
            &msgs(&["Series does not exist"]),
            &["A", "B", "C", "D"],
            k,
        )
        .unwrap_err();
        assert_eq!(err.kind(), "transient");
        assert!(err.to_string().contains("A and 3 more"), "{err}");
        // Per-series "does not exist" and "no data" messages are results, not request failures.
        for status in [STATUS_SUCCEEDED, "REQUEST_NOT_PROCESSED"] {
            assert!(check_status(
                status,
                &msgs(&[
                    "Series does not exist for Series X",
                    "No Data Available for Series Y Year: 2005"
                ]),
                &["X"],
                k
            )
            .is_ok());
        }
        assert_eq!(
            check_status(
                "REQUEST_NOT_PROCESSED",
                &msgs(&["daily threshold reached"]),
                &["X"],
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
        // Listed series: title and units from the list, survey and seasonality from the catalog.
        assert_eq!(
            m.title,
            "Consumer Price Index for All Urban Consumers: All Items in U.S. City Average (Not Seasonally Adjusted)"
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
        let ds = s.dataset.unwrap();
        assert_eq!(ds.code, "CU");
        assert_eq!(
            ds.dimensions.0,
            [
                ("area", "0000"),
                ("item", "SA0"),
                ("periodicity", "R"),
                ("seasonal", "U")
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<std::collections::BTreeMap<_, _>>()
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
        // ECI (CI) has no dataset layout yet.
        assert_eq!(s.dataset, None);

        // A listed series without a catalog (keyless) takes its metadata from the list.
        mock.reset().await;
        mock.mount(
            &route(),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [{"seriesID": "LNS14000000", "data": [
                    {"year": "2024", "period": "M01", "value": "3.7", "footnotes": [{}]}]}]}})),
        )
        .await;
        let s = adapter
            .fetch_series(&test_ctx(), "LNS14000000", None)
            .await
            .unwrap();
        let m = s.metadata.unwrap();
        assert_eq!(m.title, "Unemployment Rate (Seasonally Adjusted)");
        assert_eq!(m.units.as_deref(), Some("Percent"));
        assert_eq!(m.frequency.as_deref(), Some("Monthly"));
        assert_eq!(
            m.seasonal_adjustment.as_deref(),
            Some("Seasonally Adjusted")
        );
    }

    #[test]
    fn seasonal_adjustment_from_id_prefixes() {
        for (id, want) in [
            ("CUSR0000SA0", Some("Seasonally Adjusted")),
            ("CUUR0000SA0", Some("Not Seasonally Adjusted")),
            ("CES0000000001", Some("Seasonally Adjusted")),
            ("CEU0000000001", Some("Not Seasonally Adjusted")),
            ("LNS14000000", Some("Seasonally Adjusted")),
            ("LNU04000000", Some("Not Seasonally Adjusted")),
            ("LASST060000000000003", Some("Seasonally Adjusted")),
            ("LAUST060000000000003", Some("Not Seasonally Adjusted")),
            ("CIU1010000000000A", None),
            ("X", None),
        ] {
            assert_eq!(seasonal_adjustment_from_id(id), want, "{id}");
        }
    }

    /// An error on an older window fails the whole group, even after a newer window had data.
    #[tokio::test]
    async fn later_window_error_fails_the_group() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &route().body_contains(json!({"startyear": "2015"})),
            Reply::json_str(MONTHLY),
            1,
        )
        .await;
        mock.mount_expect(
            &route().body_contains(json!({"startyear": "2005"})),
            Reply::json_str(THRESHOLD),
            1,
        )
        .await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let ids = vec!["CUUR0000SA0".to_string()];
        let err = adapter.fetch_batch(&ctx, &ids, None).await.unwrap_err();
        assert_eq!(err, CrawlError::RateLimited { retry_after: None });
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
        let empty = json!({"status": "REQUEST_SUCCEEDED", "message": ["No Data Available for Series CUUR0000SA0 Year: 1990"],
            "Results": {"series": [{"seriesID": "CUUR0000SA0", "data": []}]}});
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
            .fetch_series(&test_ctx(), "CUUR0000SA0", Some(d(1950, 1, 1)))
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
    async fn discover_reads_series_file_without_requests() {
        let mock = MockSource::start().await;
        let adapter = BlsAdapter::new(mock.base_url());
        let found = adapter.discover(&test_ctx()).await.unwrap();
        assert!(found.len() >= 250, "{} series", found.len());
        assert!(mock.received_requests().await.is_empty());
        for id in [
            "CUUR0000SA0",
            "CUSR0000SA0L1E",
            "CUSR0000SAH1",
            "CES0000000001",
            "CES0500000003",
            "LNS14000000",
            "LNS11300000",
            "LNS12300000",
            "LASST060000000000003",
        ] {
            assert!(found.iter().any(|s| s.external_id == id), "{id} missing");
        }
        let cpi = found
            .iter()
            .find(|s| s.external_id == "CUUR0000SA0")
            .unwrap();
        assert_eq!(
            cpi.data_url.as_deref(),
            Some(format!("{}/timeseries/data/CUUR0000SA0", mock.base_url()).as_str())
        );
        assert_eq!(cpi.frequency.as_deref(), Some("Monthly"));
        assert_eq!(cpi.units.as_deref(), Some("Index 1982-1984=100"));
        let ds = cpi.dataset.as_ref().expect("CPI series has a dataset");
        assert_eq!(ds.code, "CU");
    }

    #[test]
    fn every_series_shares_one_batch_key() {
        let a = BlsAdapter::default();
        assert!(a.batch_key("CUUR0000SA0").is_some());
        assert_eq!(
            a.batch_key("CUUR0000SA0"),
            a.batch_key("LASST060000000000003")
        );
    }

    #[test]
    fn policy_batches_50_with_key_25_without() {
        assert_eq!(policy(true).max_batch, 50);
        assert_eq!(policy(false).max_batch, 25);
        let base = SourcePolicy::default_for(SourceId::Bls);
        assert_eq!(
            SourcePolicy {
                max_batch: base.max_batch,
                ..policy(true)
            },
            base
        );
    }

    /// Request budget stated in the module docs, for the shipped list.
    #[test]
    fn shipped_list_fits_daily_quotas() {
        let n = crate::reference::bls_series().unwrap().len();
        let requests = |per_request: usize, windows: usize| n.div_ceil(per_request) * windows;
        let keyed_first = requests(
            MAX_SERIES_WITH_KEY,
            year_windows(1, HISTORY_YEARS, MAX_YEARS_WITH_KEY).len(),
        );
        let keyless_first = requests(
            MAX_SERIES_WITHOUT_KEY,
            year_windows(1, HISTORY_YEARS, MAX_YEARS_WITHOUT_KEY).len(),
        );
        assert!(
            keyed_first <= 500 && n <= 500,
            "keyed: {keyed_first} requests, {n} series"
        );
        assert!(
            keyless_first <= 25,
            "keyless first fetch: {keyless_first} requests"
        );
        // A refresh (the policy's own revision lookback) is one window with or without a key.
        let lookback_years = (SourcePolicy::default_for(SourceId::Bls)
            .revision_lookback
            .as_secs()
            / (86_400 * 365)) as i32;
        let end = 2026;
        let start = end - lookback_years;
        for span in [MAX_YEARS_WITH_KEY, MAX_YEARS_WITHOUT_KEY] {
            assert_eq!(
                year_windows(start, end, span).len(),
                1,
                "span {span}, lookback {lookback_years}y"
            );
        }
    }

    #[test]
    fn missing_series_ids_from_messages() {
        assert_eq!(
            missing_series_id("Series does not exist for Series NOPE0000000"),
            Some("NOPE0000000")
        );
        assert_eq!(
            missing_series_id("series does not exist for series ABC1, check the id."),
            Some("ABC1")
        );
        assert_eq!(
            missing_series_id("No Data Available for Series X Year: 2004"),
            None
        );
        assert_eq!(missing_series_id("Series does not exist for Series "), None);
    }

    #[test]
    fn footnotes_parse() {
        let p: DataPoint =
            serde_json::from_value(json!({"year": "2024", "period": "Q02", "value": "4.1",
            "footnotes": [{"code": "P", "text": "preliminary"}, {}]}))
            .unwrap();
        assert!(p.is_preliminary());
        assert_eq!(p.footnote_summary(), "P: preliminary");
        let none: DataPoint = serde_json::from_value(json!({"year": "2024", "period": "M01",
            "value": "1", "footnotes": [{}]}))
        .unwrap();
        assert!(!none.is_preliminary());
        assert_eq!(none.footnote_summary(), "");
        let absent: DataPoint =
            serde_json::from_value(json!({"year": "2024", "period": "M01", "value": "1"})).unwrap();
        assert_eq!(absent.footnotes().count(), 0);
        // BLS may send an explicit `null` instead of omitting the field or sending `[{}]`.
        let null_footnotes: DataPoint = serde_json::from_value(
            json!({"year": "2024", "period": "M01", "value": "1", "footnotes": null}),
        )
        .unwrap();
        assert_eq!(null_footnotes.footnotes().count(), 0);
    }

    /// `null` for a whole-response array field (not just an omitted one) doesn't fail parsing.
    #[test]
    fn null_arrays_parse_as_empty() {
        let resp: DataResponse = serde_json::from_value(json!({
            "status": "REQUEST_SUCCEEDED",
            "message": null,
            "Results": {"series": null},
        }))
        .unwrap();
        assert!(resp.message.is_empty());
        assert!(resp.results.unwrap().series.is_empty());

        let body: SeriesBody = serde_json::from_value(json!({
            "seriesID": "X",
            "data": null,
        }))
        .unwrap();
        assert!(body.data.is_empty());
    }

    #[test]
    fn series_dataset_splits_fixed_width_ids() {
        let dims = |id: &str| {
            let ds = series_dataset(id).unwrap();
            (ds.code, ds.dimensions.0.into_iter().collect::<Vec<_>>())
        };
        let pairs = |v: &[(&str, &str)]| {
            v.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            dims("LASST060000000000003"),
            (
                "LA".to_string(),
                pairs(&[
                    ("area", "ST0600000000000"),
                    ("measure", "03"),
                    ("seasonal", "S")
                ])
            )
        );
        assert_eq!(
            dims("CUSR0000SEHA"),
            (
                "CU".to_string(),
                pairs(&[
                    ("area", "0000"),
                    ("item", "SEHA"),
                    ("periodicity", "R"),
                    ("seasonal", "S")
                ])
            )
        );
        assert_eq!(
            dims("CEU0500000003"),
            (
                "CE".to_string(),
                pairs(&[
                    ("data_type", "03"),
                    ("industry", "05000000"),
                    ("seasonal", "U")
                ])
            )
        );
        assert_eq!(
            dims("LNU04000000"),
            (
                "LN".to_string(),
                pairs(&[("seasonal", "U"), ("series_code", "04000000")])
            )
        );
        for bad in [
            "",
            "C",
            "CU",
            "CUUR0000",       // no item
            "CES000000000",   // one short
            "CES00000000011", // one long
            "LNS1400000",     // one short
            "LASST06000000000003",
            "CIU1010000000000A", // no layout for ECI
            "cuur0000SA0",       // prefixes are uppercase
            "CUUR 0000SA0",
            "CUUR0000SÄ0",
        ] {
            assert_eq!(series_dataset(bad), None, "{bad:?}");
        }
    }

    /// The layouts and `data/datasets/bls.toml` list the same datasets and dimensions, in the same
    /// order, so the id splits into exactly the declared keys.
    #[test]
    fn layouts_match_dataset_definitions() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/datasets/bls.toml");
        let defs =
            crate::dataset::parse_dataset_file(&std::fs::read_to_string(path).unwrap()).unwrap();
        let from_file: Vec<(&str, Vec<&str>)> = defs
            .iter()
            .map(|d| (d.code.as_str(), d.dimension_names().collect()))
            .collect();
        let from_layouts: Vec<(&str, Vec<&str>)> = SERIES_ID_LAYOUTS
            .iter()
            .map(|(code, layout)| (*code, layout.iter().map(|(name, _)| *name).collect()))
            .collect();
        assert_eq!(from_file, from_layouts);
        assert_eq!(
            DATASET_CODES,
            SERIES_ID_LAYOUTS
                .iter()
                .map(|(c, _)| *c)
                .collect::<Vec<_>>()
        );
        for ((code, layout), def) in SERIES_ID_LAYOUTS.iter().zip(&defs) {
            assert!(
                layout.iter().rev().skip(1).all(|(_, w)| *w > 0),
                "{code}: only the last dimension may take the rest"
            );
            // Labelled codes have their field's width, so a mistyped key cannot hide.
            for (&(name, width), dim) in layout.iter().zip(&def.dimensions) {
                if width > 0 {
                    for c in dim.codes.iter().flatten() {
                        assert_eq!(c.code.len(), width, "{code}.{name}: code {:?}", c.code);
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn one_request_carries_the_whole_batch() {
        let mock = MockSource::start().await;
        let ids: Vec<String> = ["CUUR0000SA0", "LNS14000000", "CES0000000001", "NOPE0000000"]
            .map(String::from)
            .to_vec();
        mock.mount_expect(
            &route().body_contains(json!({
                "seriesid": ids,
                "startyear": "2005",
                "endyear": "2024",
                "registrationkey": TEST_API_KEY,
            })),
            Reply::json_str(BATCH),
            1,
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let out = adapter.fetch_batch(&test_ctx(), &ids, None).await.unwrap();
        assert_eq!(out.len(), 4);

        let cpi = out["CUUR0000SA0"].as_ref().unwrap();
        assert_eq!(cpi.points.len(), 3);
        assert_eq!(
            cpi.metadata.as_ref().unwrap().units.as_deref(),
            Some("Index 1982-1984=100")
        );

        let unrate = out["LNS14000000"].as_ref().unwrap();
        let got: Vec<_> = unrate
            .points
            .iter()
            .map(|p| (p.date, p.value.clone()))
            .collect();
        let bd = |v: &str| BigDecimal::from_str(v).ok();
        assert_eq!(
            got,
            vec![(d(2024, 1, 1), bd("3.7")), (d(2024, 2, 1), bd("3.9"))]
        );

        // The "X" footnote's "-" is a missing value; the "P" value is kept.
        let ces = out["CES0000000001"].as_ref().unwrap();
        let got: Vec<_> = ces
            .points
            .iter()
            .map(|p| (p.date, p.value.clone()))
            .collect();
        assert_eq!(
            got,
            vec![(d(2024, 1, 1), None), (d(2024, 2, 1), bd("157808"))]
        );

        let missing = out["NOPE0000000"].as_ref().unwrap_err();
        assert_eq!(missing.kind(), "not_found");
        assert!(missing.to_string().contains("NOPE0000000"), "{missing}");
    }

    #[tokio::test]
    async fn batch_request_failure_fails_the_whole_batch() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json_str(THRESHOLD)).await;
        let adapter = BlsAdapter::new(mock.base_url());
        let ids = vec!["A1".to_string(), "B2".to_string()];
        let err = adapter
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap_err();
        assert_eq!(err, CrawlError::RateLimited { retry_after: None });
    }

    #[tokio::test]
    async fn keyed_batch_splits_into_requests_of_50() {
        let mock = MockSource::start().await;
        mock.mount(&route(), Reply::json(ok_empty())).await;
        let ids: Vec<String> = (0..60).map(|i| format!("S{i:02}")).collect();
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let out = adapter
            .fetch_batch(&test_ctx(), &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out.len(), 60);
        let sizes: Vec<usize> = mock
            .received_requests()
            .await
            .iter()
            .map(|r| {
                let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
                assert_eq!(b["registrationkey"], json!(TEST_API_KEY));
                b["seriesid"].as_array().unwrap().len()
            })
            .collect();
        assert_eq!(sizes, vec![50, 10]);
    }

    #[tokio::test]
    async fn keyless_batch_splits_into_requests_of_25() {
        let mock = MockSource::start().await;
        mock.mount(
            &route(),
            Reply::json(
                json!({"status": "REQUEST_SUCCEEDED", "message": [], "Results": {"series": []}}),
            ),
        )
        .await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let ids: Vec<String> = (0..30).map(|i| format!("S{i:02}")).collect();
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let out = adapter
            .fetch_batch(&ctx, &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out.len(), 30);
        assert!(out
            .values()
            .all(|r| r.as_ref().is_ok_and(|s| s.points.is_empty())));
        let sizes: Vec<usize> = mock
            .received_requests()
            .await
            .iter()
            .map(|r| {
                let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
                assert!(b.get("registrationkey").is_none(), "{b}");
                b["seriesid"].as_array().unwrap().len()
            })
            .collect();
        assert_eq!(sizes, vec![25, 5]);
    }

    #[tokio::test]
    async fn keyless_chunk_failure_applies_to_its_series_only() {
        let mock = MockSource::start().await;
        // The first request (25 series) fails permanently, the second succeeds.
        mock.mount_expect(
            &route().body_contains(
                json!({"seriesid": (0..25).map(|i| format!("S{i:02}")).collect::<Vec<_>>()}),
            ),
            Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                "message": ["Invalid parameters"], "Results": {}})),
            1,
        )
        .await;
        mock.mount(&route(), Reply::json(ok_empty())).await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let ids: Vec<String> = (0..27).map(|i| format!("S{i:02}")).collect();
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let out = adapter
            .fetch_batch(&ctx, &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out["S00"].as_ref().unwrap_err().kind(), "permanent");
        assert_eq!(out["S24"].as_ref().unwrap_err().kind(), "permanent");
        assert!(out["S25"].is_ok() && out["S26"].is_ok());
        assert_eq!(mock.received_requests().await.len(), 2);
    }

    /// A rate limit on one request applies to every series not fetched yet, without spending
    /// more quota on the rest.
    #[tokio::test]
    async fn keyless_rate_limit_stops_further_requests() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &route().body_contains(
                json!({"seriesid": (25..50).map(|i| format!("S{i:02}")).collect::<Vec<_>>()}),
            ),
            Reply::json_str(THRESHOLD),
            1,
        )
        .await;
        mock.mount(&route(), Reply::json(ok_empty())).await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let ids: Vec<String> = (0..60).map(|i| format!("S{i:02}")).collect();
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let out = adapter
            .fetch_batch(&ctx, &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out.len(), 60);
        assert!(out["S24"].is_ok());
        for id in ["S25", "S49", "S50", "S59"] {
            assert_eq!(
                out[id],
                Err(CrawlError::RateLimited { retry_after: None }),
                "{id}"
            );
        }
        assert_eq!(mock.received_requests().await.len(), 2);

        // On the first request, the rate limit fails the whole call.
        mock.reset().await;
        mock.mount(&route(), Reply::json_str(THRESHOLD)).await;
        let err = adapter
            .fetch_batch(&ctx, &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap_err();
        assert_eq!(err, CrawlError::RateLimited { retry_after: None });
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    /// An unnamed "does not exist" on a later window that carries one series affects only that
    /// series: it ends its history if newer windows returned data, and fails it as not found if
    /// not. The rest of the group keeps its data.
    #[tokio::test]
    async fn lone_series_not_found_in_later_window_ends_or_fails_only_it() {
        let series = |id: &str, year: &str| {
            json!({"seriesID": id, "data": [
            {"year": year, "period": "M01", "value": "1.0", "footnotes": [{}]}]})
        };
        let empty = |id: &str| json!({"seriesID": id, "data": []});
        // (B1 in the two newer windows, how B1 ends)
        for (b1_newer, want_points) in [(true, Some(2)), (false, None)] {
            let mock = MockSource::start().await;
            let b1 = |year: &str| {
                if b1_newer {
                    series("B1", year)
                } else {
                    empty("B1")
                }
            };
            mock.mount_expect(
                &route().body_contains(json!({"seriesid": ["A1", "B1"], "startyear": "2015"})),
                Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                    "Results": {"series": [series("A1", "2020"), b1("2020")]}})),
                1,
            )
            .await;
            mock.mount_expect(
                &route().body_contains(json!({"seriesid": ["A1", "B1"], "startyear": "2005"})),
                Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                    "Results": {"series": [empty("A1"), b1("2010")]}})),
                1,
            )
            .await;
            mock.mount_expect(
                &route().body_contains(json!({"seriesid": ["B1"], "startyear": "1995"})),
                Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                    "message": ["Series does not exist"], "Results": {}})),
                1,
            )
            .await;
            let mut ctx = test_ctx();
            ctx.keys.bls = None;
            let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
            let ids = vec!["A1".to_string(), "B1".to_string()];
            let out = adapter
                .fetch_batch(&ctx, &ids, Some(d(1995, 1, 1)))
                .await
                .unwrap();
            assert_eq!(out["A1"].as_ref().unwrap().points.len(), 1);
            match want_points {
                // Data from newer windows: the old window just ends its history.
                Some(n) => assert_eq!(out["B1"].as_ref().unwrap().points.len(), n),
                None => assert_eq!(out["B1"].as_ref().unwrap_err().kind(), "not_found"),
            }
        }
    }

    /// A named "does not exist" on an older window, after the series returned data, ends its
    /// history instead of failing it.
    #[tokio::test]
    async fn named_not_found_after_data_keeps_the_data() {
        // Not processed too: a series that already ended must not become transient.
        for status in [STATUS_SUCCEEDED, "REQUEST_NOT_PROCESSED"] {
            let mock = MockSource::start().await;
            mock.mount_expect(
                &route().body_contains(json!({"startyear": "2015"})),
                Reply::json_str(MONTHLY),
                1,
            )
            .await;
            mock.mount_expect(
                &route().body_contains(json!({"startyear": "2005"})),
                Reply::json(json!({"status": status,
                    "message": ["Series does not exist for Series CUUR0000SA0"],
                    "Results": {"series": []}})),
                1,
            )
            .await;
            let mut ctx = test_ctx();
            ctx.keys.bls = None;
            let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
            let s = adapter
                .fetch_series(&ctx, "CUUR0000SA0", None)
                .await
                .unwrap_or_else(|e| panic!("{status}: {e}"));
            assert_eq!(s.points.len(), 5, "{status}");
        }
    }

    /// Duplicate ids are requested once; a series the response leaves out is empty with `since`
    /// and not found without it.
    #[tokio::test]
    async fn duplicates_and_omitted_series() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["CUUR0000SA0", "GONE1"]})),
            Reply::json_str(MONTHLY),
            2,
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let ids = ["CUUR0000SA0", "GONE1", "CUUR0000SA0"]
            .map(String::from)
            .to_vec();
        let out = adapter
            .fetch_batch(&test_ctx(), &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out["CUUR0000SA0"].as_ref().unwrap().points.len(), 3);
        assert!(out["GONE1"].as_ref().unwrap().points.is_empty());

        let out = adapter.fetch_batch(&test_ctx(), &ids, None).await.unwrap();
        assert_eq!(out["GONE1"].as_ref().unwrap_err().kind(), "not_found");
    }

    /// A not-processed response whose only messages name missing series fails those as not
    /// found and the series it left out as transient, instead of completing them with no data.
    #[tokio::test]
    async fn not_processed_with_only_missing_series_fails_the_rest_transiently() {
        let mock = MockSource::start().await;
        mock.mount(
            &route(),
            Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                "message": ["Series does not exist for Series NOPE0000000"],
                "Results": {"series": []}})),
        )
        .await;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let ids = ["NOPE0000000", "CUUR0000SA0"].map(String::from).to_vec();
        let out = adapter
            .fetch_batch(&test_ctx(), &ids, Some(d(2024, 1, 1)))
            .await
            .unwrap();
        assert_eq!(out["NOPE0000000"].as_ref().unwrap_err().kind(), "not_found");
        assert_eq!(out["CUUR0000SA0"].as_ref().unwrap_err().kind(), "transient");
    }

    /// Series that run out of data in an older window drop out of later requests; the others
    /// keep going back.
    #[tokio::test]
    async fn batch_windows_drop_series_that_ran_out() {
        let mock = MockSource::start().await;
        let series = |id: &str, year: &str| {
            json!({"seriesID": id, "data": [
            {"year": year, "period": "M01", "value": "1.0", "footnotes": [{}]}]})
        };
        let empty = |id: &str| json!({"seriesID": id, "data": []});
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["OLD1", "NEW1"], "startyear": "2017"})),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [series("OLD1", "2024"), series("NEW1", "2024")]}})),
            1,
        )
        .await;
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["OLD1", "NEW1"], "startyear": "2007"})),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [series("OLD1", "2010"), empty("NEW1")]}})),
            1,
        )
        .await;
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["OLD1"], "startyear": "2000"})),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [series("OLD1", "2001")]}})),
            1,
        )
        .await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2026);
        let ids = vec!["OLD1".to_string(), "NEW1".to_string()];
        let out = adapter
            .fetch_batch(&ctx, &ids, Some(d(2000, 1, 1)))
            .await
            .unwrap();
        let dates = |id: &str| -> Vec<NaiveDate> {
            out[id]
                .as_ref()
                .unwrap()
                .points
                .iter()
                .map(|p| p.date)
                .collect()
        };
        assert_eq!(
            dates("OLD1"),
            vec![d(2001, 1, 1), d(2010, 1, 1), d(2024, 1, 1)]
        );
        assert_eq!(dates("NEW1"), vec![d(2024, 1, 1)]);
        assert_eq!(mock.received_requests().await.len(), 3);
    }

    /// A not-processed response that omits a series entirely (no entry in `Results.series`, not
    /// even an empty one), but names it in an informational "No Data Available" message, ends its
    /// history when it already has data instead of failing it transiently and losing that data.
    #[tokio::test]
    async fn omitted_series_named_no_data_after_data_keeps_the_data() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["OLD1", "NEW1"], "startyear": "2015"})),
            Reply::json(json!({"status": "REQUEST_SUCCEEDED", "message": [],
                "Results": {"series": [
                    {"seriesID": "OLD1", "data": [{"year": "2020", "period": "M01", "value": "1.0", "footnotes": [{}]}]},
                    {"seriesID": "NEW1", "data": [{"year": "2020", "period": "M01", "value": "2.0", "footnotes": [{}]}]},
                ]}})),
            1,
        )
        .await;
        mock.mount_expect(
            &route().body_contains(json!({"seriesid": ["OLD1", "NEW1"], "startyear": "2005"})),
            Reply::json(json!({"status": "REQUEST_NOT_PROCESSED",
                "message": ["No Data Available for Series OLD1 Year: 2010"],
                "Results": {"series": [
                    {"seriesID": "NEW1", "data": [{"year": "2010", "period": "M01", "value": "3.0", "footnotes": [{}]}]},
                ]}})),
            1,
        )
        .await;
        let mut ctx = test_ctx();
        ctx.keys.bls = None;
        let adapter = BlsAdapter::new(mock.base_url()).with_current_year(2024);
        let ids = vec!["OLD1".to_string(), "NEW1".to_string()];
        let out = adapter.fetch_batch(&ctx, &ids, None).await.unwrap();
        let dates = |id: &str| -> Vec<NaiveDate> {
            out[id]
                .as_ref()
                .unwrap()
                .points
                .iter()
                .map(|p| p.date)
                .collect()
        };
        // OLD1 keeps its window-1 data instead of losing it to a Transient failure.
        assert_eq!(dates("OLD1"), vec![d(2020, 1, 1)]);
        assert_eq!(dates("NEW1"), vec![d(2010, 1, 1), d(2020, 1, 1)]);
        assert_eq!(mock.received_requests().await.len(), 2);
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
    }
}
