// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! U.S. Census Bureau adapter: Business Dynamics Statistics (BDS) time series.
//!
//! Ported from `econ-graph-services/src/services/series_discovery/census/mod.rs`.
//!
//! # Discovery
//!
//! Two requests, both required (as in the old code):
//! - `GET {base}/timeseries/bds/variables.json?key=KEY` -> `{"variables": {NAME: {"label": ..}, ..}}`
//! - `GET {base}/timeseries/bds/geography.json?key=KEY` -> `{"fips": [{"name": .., "geoLevelDisplay": ..}, ..]}`
//!
//! Variables are filtered with the old keyword rules ([`is_economic_variable`]) and crossed with
//! the geography levels that give single series (annual, units "Count"):
//! - `us`: one national series, `CENSUS_BDS_{VARIABLE}_us`;
//! - `state`: one series per state and DC, `CENSUS_BDS_{VARIABLE}_state_{FIPS}` (two-digit state
//!   FIPS code, e.g. `_state_06` for California), from the shared states file
//!   ([`crate::reference::us_states`], read at runtime).
//!
//! Only levels that `geography.json` lists are used. Finer levels (county, metro area) have
//! thousands of areas and are skipped. No pagination, so no page cap.
//!
//! # Fetching
//!
//! The old code parsed BDS data responses (a JSON array of string rows, header row first), so
//! fetching is implemented for the ids discovery produces: national (`CENSUS_BDS_{VARIABLE}_us`)
//! and per-state (`CENSUS_BDS_{VARIABLE}_state_{FIPS}`). Any other id, including the bare
//! `_state` ids older discovery runs persisted (one value per state and year, not a single
//! series), is `Permanent`. The request is
//! `GET {base}/timeseries/bds?get={VARIABLE},YEAR&for={us:*|state:FIPS}&key=KEY`, i.e. all
//! years (the old
//! comma-separated `YEAR=` list hit the API's "204 No Content" limitation for multi-year
//! queries); `since` is applied client-side by year. Each row becomes a point dated January 1 of
//! its `YEAR`, `revision_date = date`, `is_original_release = true`. As in the old parser, rows
//! of the wrong width or with an unparseable year are skipped, and an empty or non-numeric value
//! is a missing observation (`None`).
//!
//! # API key
//!
//! Required. The Census Data API rejects requests without a key ("A valid key must be included
//! with each data API request", checked live on 2026-09-26), so `ctx.keys.census`
//! (`CENSUS_API_KEY`) is sent as `key=` on every request, the metadata files included. Without
//! it, discovery and fetching fail with `Auth` ("CENSUS_API_KEY not set") before any request, as
//! for FRED and BEA; `Auth` is never retried. Keys are free at
//! <https://api.census.gov/data/key_signup.html>.
//!
//! # Errors
//!
//! [`classify_census_error`] refines the fetcher's status mapping: HTTP 400 whose body says
//! `unknown variable` is `NotFound`. Census answers an invalid key with a 200 HTML page saying
//! "Invalid Key", which is `Auth`; any other non-JSON body is `Parse`; an empty body (HTTP 204)
//! is `NotFound`.

use std::collections::HashSet;
use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate};
use serde_json::Value;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::reference::us_states;
use crate::source::SourceId;

/// The real Census Data API root.
pub const DEFAULT_BASE_URL: &str = "https://api.census.gov/data";

/// Dataset path (relative to the base URL).
const BDS_PATH: &str = "/timeseries/bds";

/// Prefix of every Census external id this adapter produces.
const ID_PREFIX: &str = "CENSUS_BDS_";

/// National geography level.
const NATIONAL_GEO: &str = "us";

/// The error when `CENSUS_API_KEY` is missing.
const MISSING_KEY: &str = "CENSUS_API_KEY not set: the Census Data API rejects requests without \
                           a key (free at https://api.census.gov/data/key_signup.html)";

/// State geography level. Its series ids end in `_state_{FIPS}`.
const STATE_GEO: &str = "state";

/// PostgreSQL regular expression matching exactly the ids [`CensusAdapter::fetch_series`] accepts:
/// `CENSUS_BDS_{VARIABLE}_us` and `CENSUS_BDS_{VARIABLE}_state_{FIPS}` for the FIPS codes in
/// [`us_states`]. Older discovery runs also persisted other geography levels (e.g. bare `_state`),
/// which the refresh scheduler must not enqueue.
pub fn fetchable_id_regex() -> Result<String, CrawlError> {
    let fips: Vec<&str> = us_states()?.iter().map(|s| s.fips.as_str()).collect();
    Ok(format!(
        "^CENSUS_BDS_[A-Za-z0-9_]+_({NATIONAL_GEO}|{STATE_GEO}_({}))$",
        fips.join("|")
    ))
}

/// Census adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct CensusAdapter {
    base_url: String,
}

impl CensusAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    /// `path` (starting with `/`) under the API root.
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// The API key, required on every request. Missing -> `Auth` before any request is made.
    fn api_key<'a>(&self, ctx: &'a CrawlCtx) -> Result<&'a str, CrawlError> {
        ctx.keys
            .census
            .as_deref()
            .ok_or_else(|| CrawlError::Auth(MISSING_KEY.into()))
    }

    /// A BDS metadata file (`variables.json`, `geography.json`). Fetched as text, not
    /// `get_json`, on purpose: a rejected key comes back as a 200 HTML "Invalid Key" page, which
    /// must be `Auth`; any other non-JSON body is `Parse`.
    async fn metadata_json(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        file: &str,
    ) -> Result<Value, CrawlError> {
        let url = self.url(&format!("{BDS_PATH}/{file}"));
        let body = ctx
            .http
            .get_text(SourceId::Census, &url, &[("key", key)])
            .await
            .map_err(classify_census_error)?;
        serde_json::from_str(&body).map_err(|e| {
            let err = if is_invalid_key_page(&body) {
                CrawlError::Auth(format!("Census {file}: API key rejected (Invalid Key)"))
            } else {
                CrawlError::Parse(format!("Census {file}: response is not JSON: {e}"))
            };
            // `get_text` only counts the HTTP request as successful; this classification is
            // ours, so we record it ourselves (get_json's decode() would do this for us).
            ctx.http.record_response_error(SourceId::Census, &url, &err);
            err
        })
    }
}

impl Default for CensusAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for CensusAdapter {
    fn id(&self) -> SourceId {
        SourceId::Census
    }

    /// Discovery crosses the dataset's full variable and geography metadata, and fails rather
    /// than returning part of it (a malformed `variables.json`/`geography.json` entry fails the
    /// parse, per [`parse_variables`]/[`parse_geographies`], instead of being silently dropped
    /// from a still-successful discovery).
    fn discovery_is_complete(&self) -> bool {
        true
    }

    /// This adapter only ever produces `CENSUS_BDS_*` ids; the Census data source also holds ACS
    /// rows seeded by the initial migration, which retirement must not touch.
    fn retirement_scope_prefix(&self) -> Option<&str> {
        Some(ID_PREFIX)
    }

    /// One series per economic variable for the nation and for each state and DC, limited to the
    /// levels `geography.json` lists. See the module docs.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let key = self.api_key(ctx)?;
        let variables = parse_variables(&self.metadata_json(ctx, key, "variables.json").await?)?;
        let geographies =
            parse_geographies(&self.metadata_json(ctx, key, "geography.json").await?)?;

        // (id suffix, area name) for every single series the listed geography levels give.
        let mut areas: Vec<(String, String)> = Vec::new();
        let mut skipped = Vec::new();
        for geo in &geographies {
            match geo.name.as_str() {
                NATIONAL_GEO => areas.push((NATIONAL_GEO.into(), "United States".into())),
                STATE_GEO => areas.extend(
                    us_states()?
                        .iter()
                        .map(|s| (format!("{STATE_GEO}_{}", s.fips), s.name.clone())),
                ),
                other => skipped.push(other.to_string()),
            }
        }
        if !skipped.is_empty() {
            tracing::debug!(
                ?skipped,
                "Census BDS: skipping geography levels without single series"
            );
        }

        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for var in variables.iter().filter(|v| is_economic_variable(v)) {
            for (suffix, area) in &areas {
                let external_id = format!("{ID_PREFIX}{}_{suffix}", var.name);
                if !seen.insert(external_id.clone()) {
                    continue;
                }
                found.push(DiscoveredSeries {
                    external_id,
                    title: format!("{} - {area}", var.label),
                    description: Some(format!(
                        "Business Dynamics Statistics: {} for {area}",
                        var.label
                    )),
                    units: Some("Count".into()),
                    frequency: Some("Annual".into()),
                    data_url: None,
                });
            }
        }
        tracing::info!(series = found.len(), "Census BDS discovery finished");
        Ok(found)
    }

    /// All years of a national or per-state series, filtered to `since`'s year and later.
    /// A missing key fails as `Auth` (checked first) and an unsupported id as `Permanent`, both
    /// without a request.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let key = self.api_key(ctx)?;
        let (variable, area) = parse_series_id(external_id)?;
        let get = format!("{variable},YEAR");
        let for_geo = match area {
            Area::National => format!("{NATIONAL_GEO}:*"),
            Area::State(fips) => format!("{STATE_GEO}:{fips}"),
        };
        let query = [
            ("get", get.as_str()),
            ("for", for_geo.as_str()),
            ("key", key),
        ];
        let body = ctx
            .http
            .get_text(SourceId::Census, &self.url(BDS_PATH), &query)
            .await
            .map_err(classify_census_error)?;
        let mut points = parse_bds_rows(external_id, variable, &body)?;
        if let Some(since) = since {
            points.retain(|p| p.date.year() >= since.year());
        }
        Ok(FetchedSeries {
            metadata: None,
            points,
        })
    }
}

/// Refines a fetcher error with Census conventions: HTTP 400 "unknown variable" -> `NotFound`.
pub fn classify_census_error(err: CrawlError) -> CrawlError {
    match err {
        CrawlError::Permanent(msg)
            if msg.starts_with("HTTP 400")
                && msg.to_ascii_lowercase().contains("unknown variable") =>
        {
            CrawlError::NotFound(msg)
        }
        e => e,
    }
}

/// Census answers a rejected key with a 200 HTML page saying "Invalid Key".
fn is_invalid_key_page(body: &str) -> bool {
    body.to_ascii_lowercase().contains("invalid key")
}

/// The area a fetchable series covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Area<'a> {
    National,
    /// Two-digit state FIPS code, one of [`us_states`].
    State(&'a str),
}

/// `CENSUS_BDS_{VARIABLE}_us` -> (`VARIABLE`, national);
/// `CENSUS_BDS_{VARIABLE}_state_{FIPS}` -> (`VARIABLE`, that state).
fn parse_series_id(external_id: &str) -> Result<(&str, Area<'_>), CrawlError> {
    let rest = external_id.strip_prefix(ID_PREFIX).ok_or_else(|| {
        CrawlError::Permanent(format!(
            "Census: {external_id:?} is not a {ID_PREFIX}* series id"
        ))
    })?;
    let unsupported = || {
        CrawlError::Permanent(format!(
            "Census {external_id}: only national (_{NATIONAL_GEO}) and per-state \
             (_{STATE_GEO}_{{FIPS}}) BDS series can be fetched"
        ))
    };
    let (variable, area) = if let Some(v) = rest.strip_suffix(NATIONAL_GEO) {
        (v.strip_suffix('_').ok_or_else(unsupported)?, Area::National)
    } else {
        let (v, fips) = rest.rsplit_once('_').ok_or_else(unsupported)?;
        let v = v
            .strip_suffix(STATE_GEO)
            .and_then(|v| v.strip_suffix('_'))
            .ok_or_else(unsupported)?;
        if !us_states()?.iter().any(|s| s.fips == fips) {
            return Err(CrawlError::Permanent(format!(
                "Census {external_id}: {fips:?} is not a BDS state FIPS code"
            )));
        }
        (v, Area::State(fips))
    };
    if variable.is_empty()
        || !variable
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(CrawlError::Permanent(format!(
            "Census {external_id}: invalid variable name {variable:?}"
        )));
    }
    Ok((variable, area))
}

/// A response cell as text; `null` is empty.
fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Parses a BDS data response (`[[header..], [row..], ..]`) into points for `variable`.
fn parse_bds_rows(
    external_id: &str,
    variable: &str,
    body: &str,
) -> Result<Vec<FetchedPoint>, CrawlError> {
    if body.trim().is_empty() {
        return Err(CrawlError::NotFound(format!(
            "Census {external_id}: no data (empty response)"
        )));
    }
    let rows: Vec<Vec<Value>> = serde_json::from_str(body).map_err(|e| {
        if is_invalid_key_page(body) {
            CrawlError::Auth(format!(
                "Census {external_id}: API key rejected (Invalid Key)"
            ))
        } else {
            CrawlError::Parse(format!(
                "Census {external_id}: response is not a JSON row array: {e}"
            ))
        }
    })?;
    let Some((header, data)) = rows.split_first() else {
        return Err(CrawlError::Parse(format!(
            "Census {external_id}: empty row array"
        )));
    };
    let header: Vec<String> = header.iter().map(cell).collect();
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                CrawlError::Parse(format!(
                    "Census {external_id}: no {name} column in {header:?}"
                ))
            })
    };
    let year_idx = column("YEAR")?;
    let value_idx = column(variable)?;

    let mut seen = HashSet::new();
    let mut points = Vec::new();
    for row in data {
        if row.len() != header.len() {
            continue;
        }
        let Some(date) = cell(&row[year_idx])
            .trim()
            .parse::<i32>()
            .ok()
            .and_then(|y| NaiveDate::from_ymd_opt(y, 1, 1))
        else {
            continue;
        };
        if !seen.insert(date) {
            continue;
        }
        let raw = cell(&row[value_idx]);
        let value = BigDecimal::from_str(raw.trim()).ok();
        points.push(FetchedPoint {
            date,
            value,
            revision_date: date,
            is_original_release: true,
        });
    }
    Ok(points)
}

/// A BDS variable from `variables.json`.
#[derive(Debug, Clone, PartialEq)]
struct BdsVariable {
    name: String,
    label: String,
}

/// A BDS geography level from `geography.json`.
#[derive(Debug, Clone, PartialEq)]
struct BdsGeography {
    name: String,
}

/// The variables in `variables.json`. Discovery feeds a complete-catalog retirement
/// ([`CensusAdapter::discovery_is_complete`]), so a malformed entry fails the whole parse instead
/// of being silently dropped: a partial catalog must never look like a smaller real one.
fn parse_variables(body: &Value) -> Result<Vec<BdsVariable>, CrawlError> {
    let vars = body
        .get("variables")
        .and_then(Value::as_object)
        .ok_or_else(|| CrawlError::Parse("Census variables.json: no 'variables' object".into()))?;
    vars.iter()
        .map(|(name, v)| {
            let obj = v.as_object().ok_or_else(|| {
                CrawlError::Parse(format!("Census variables.json: {name:?} is not an object"))
            })?;
            Ok(BdsVariable {
                name: name.clone(),
                label: obj
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

/// The geography levels in `geography.json`. An entry with an empty (or absent-after-trim) name
/// is deliberately skipped, as the old code did; anything else malformed (not an object, or a
/// `name` field that isn't a string) fails the whole parse, for the reason [`parse_variables`]
/// gives.
fn parse_geographies(body: &Value) -> Result<Vec<BdsGeography>, CrawlError> {
    let fips = body
        .get("fips")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlError::Parse("Census geography.json: no 'fips' array".into()))?;
    let mut out = Vec::with_capacity(fips.len());
    for (i, g) in fips.iter().enumerate() {
        let obj = g.as_object().ok_or_else(|| {
            CrawlError::Parse(format!("Census geography.json: fips[{i}] is not an object"))
        })?;
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CrawlError::Parse(format!(
                    "Census geography.json: fips[{i}] has no string 'name'"
                ))
            })?
            .trim()
            .to_string();
        if !name.is_empty() {
            out.push(BdsGeography { name });
        }
    }
    Ok(out)
}

/// The old `filter_economic_indicators` rules.
fn is_economic_variable(var: &BdsVariable) -> bool {
    const EXCLUDE: &[&str] = &[
        "for", "in", "year", "time", "geo", "state", "county", "metro", "cbsa", "nation",
    ];
    const KEYWORDS: &[&str] = &[
        "estab",
        "firm",
        "job",
        "emp",
        "creation",
        "destruction",
        "net",
        "reallocation",
        "birth",
        "death",
        "entry",
        "exit",
        "rate",
        "employment",
        "establishment",
    ];
    let name = var.name.to_lowercase();
    let label = var.label.to_lowercase();
    if name.is_empty() || label.is_empty() || EXCLUDE.iter().any(|x| name.contains(x)) {
        return false;
    }
    KEYWORDS
        .iter()
        .any(|k| name.contains(k) || label.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    const VARIABLES: &str = include_str!("../../tests/fixtures/census/variables.json");
    const GEOGRAPHY: &str = include_str!("../../tests/fixtures/census/geography.json");
    const ESTAB_US: &str = include_str!("../../tests/fixtures/census/bds_estab_us.json");
    const ESTAB_STATE_06: &str =
        include_str!("../../tests/fixtures/census/bds_estab_state_06.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(CensusAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(CensusAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(CensusAdapter::default().id(), SourceId::Census);
    }

    /// Regression for the review finding on #219: a malformed (but non-empty) `variables.json`
    /// must fail discovery, not silently drop the bad entries and let a partial catalog trigger
    /// [`SourceAdapter::retirement_scope_prefix`]-scoped retirement of the ids it left out.
    #[test]
    fn parse_variables_rejects_non_object_entry() {
        let body = serde_json::json!({
            "variables": {
                "ESTAB": {"label": "Number of establishments"},
                "BROKEN": "not an object",
            }
        });
        let err = parse_variables(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parse_variables_rejects_missing_variables_object() {
        let body = serde_json::json!({"nope": {}});
        let err = parse_variables(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parse_variables_accepts_well_formed_entries() {
        let body = serde_json::json!({
            "variables": {"ESTAB": {"label": "Number of establishments"}}
        });
        let vars = parse_variables(&body).unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "ESTAB");
        assert_eq!(vars[0].label, "Number of establishments");
    }

    /// Same reasoning as `parse_variables_rejects_non_object_entry`, for `geography.json`.
    #[test]
    fn parse_geographies_rejects_non_object_entry() {
        let body = serde_json::json!({"fips": [{"name": "us"}, "not an object"]});
        let err = parse_geographies(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parse_geographies_rejects_non_string_name() {
        let body = serde_json::json!({"fips": [{"name": 123}]});
        let err = parse_geographies(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parse_geographies_rejects_missing_name_field() {
        let body = serde_json::json!({"fips": [{"geoLevelDisplay": "us"}]});
        let err = parse_geographies(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    #[test]
    fn parse_geographies_rejects_missing_fips_array() {
        let body = serde_json::json!({"nope": []});
        let err = parse_geographies(&body).unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
    }

    /// An empty (or whitespace-only) name is deliberately skipped, not an error: this is
    /// unchanged behavior from before the malformed-entry hardening above.
    #[test]
    fn parse_geographies_skips_blank_names_without_erroring() {
        let body = serde_json::json!({
            "fips": [{"name": "us"}, {"name": "   "}, {"name": ""}]
        });
        let geos = parse_geographies(&body).unwrap();
        assert_eq!(geos.len(), 1);
        assert_eq!(geos[0].name, "us");
    }

    /// Every economic variable gets a national series and one per state and DC.
    #[tokio::test]
    async fn discover_crosses_economic_variables_with_geographies() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/timeseries/bds/variables.json"),
            Reply::json_str(VARIABLES),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/timeseries/bds/geography.json"),
            Reply::json_str(GEOGRAPHY),
            1,
        )
        .await;
        let found = CensusAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        // 3 economic variables x (national + 50 states + DC).
        let states = us_states().unwrap();
        assert_eq!(states.len(), 51);
        assert_eq!(found.len(), 3 * (1 + states.len()));
        let ids: HashSet<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        for var in ["ESTAB", "FIRM", "JOB_CREATION"] {
            assert!(
                ids.contains(format!("CENSUS_BDS_{var}_us").as_str()),
                "{var}"
            );
            for s in states {
                let id = format!("CENSUS_BDS_{var}_state_{}", s.fips);
                assert!(ids.contains(id.as_str()), "{id}");
            }
        }
        // No bare per-level ids: those are not single series.
        assert!(!ids.contains("CENSUS_BDS_ESTAB_state"));
        let by_id = |id: &str| found.iter().find(|s| s.external_id == id).unwrap();
        let estab_us = by_id("CENSUS_BDS_ESTAB_us");
        assert_eq!(estab_us.title, "Number of establishments - United States");
        assert_eq!(estab_us.units.as_deref(), Some("Count"));
        assert_eq!(estab_us.frequency.as_deref(), Some("Annual"));
        let estab_ca = by_id("CENSUS_BDS_ESTAB_state_06");
        assert_eq!(estab_ca.title, "Number of establishments - California");
        assert_eq!(
            estab_ca.description.as_deref(),
            Some("Business Dynamics Statistics: Number of establishments for California")
        );
        // Metadata requests carry the key too.
        for r in mock.received_requests().await {
            let pairs: Vec<(String, String)> = r.url.query_pairs().into_owned().collect();
            assert_eq!(
                pairs,
                [("key".to_string(), TEST_API_KEY.to_string())],
                "{}",
                r.url
            );
        }
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn discover_needs_both_metadata_files() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds/variables.json"),
            Reply::json_str(VARIABLES),
        )
        .await;
        let e = CensusAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found");

        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds/variables.json"),
            Reply::json(serde_json::json!({"nope": 1})),
        )
        .await;
        let e = CensusAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse");
    }

    #[tokio::test]
    async fn fetch_parses_rows_and_sends_key() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/timeseries/bds")
                .query("get", "ESTAB,YEAR")
                .query("for", "us:*")
                .query("key", TEST_API_KEY),
            Reply::json_str(ESTAB_US),
            1,
        )
        .await;
        let s = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_ESTAB_us", None)
            .await
            .unwrap();
        assert_eq!(s.metadata, None);
        let got: Vec<(NaiveDate, Option<String>)> = s
            .points
            .iter()
            .map(|p| (p.date, p.value.as_ref().map(ToString::to_string)))
            .collect();
        assert_eq!(
            got,
            [
                (d("2019-01-01"), Some("7106316".into())),
                (d("2020-01-01"), Some("7179420".into())),
                (d("2021-01-01"), None),
                (d("2022-01-01"), Some("7324017".into())),
            ]
        );
        assert!(s
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn fetch_applies_since() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/timeseries/bds"), Reply::json_str(ESTAB_US))
            .await;
        let s = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_ESTAB_us", Some(d("2021-06-30")))
            .await
            .unwrap();
        let dates: Vec<NaiveDate> = s.points.iter().map(|p| p.date).collect();
        assert_eq!(dates, [d("2021-01-01"), d("2022-01-01")]);
    }

    /// The Census API rejects keyless requests, so a missing key fails before any request.
    #[tokio::test]
    async fn missing_key_is_auth_error_without_requests() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/timeseries/bds"), Reply::json_str(ESTAB_US))
            .await;
        let mut ctx = test_ctx();
        ctx.keys.census = None;
        let adapter = CensusAdapter::new(mock.base_url());

        let e = adapter
            .fetch_series(&ctx, "CENSUS_BDS_ESTAB_us", None)
            .await
            .unwrap_err();
        assert_eq!(e, CrawlError::Auth(MISSING_KEY.into()));
        assert!(!e.is_retryable());
        // The key is checked before the id, so an unsupported id also reports the missing key.
        let e = adapter
            .fetch_series(&ctx, "CENSUS_BDS_ESTAB_county", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");
        assert!(e.to_string().contains("CENSUS_API_KEY not set"), "{e}");
        let e = adapter.discover(&ctx).await.unwrap_err();
        assert_eq!(e, CrawlError::Auth(MISSING_KEY.into()));
        assert!(mock.received_requests().await.is_empty());
    }

    /// County and metro-area levels are skipped; only the listed levels produce series.
    #[tokio::test]
    async fn discover_skips_levels_without_single_series() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds/variables.json"),
            Reply::json_str(VARIABLES),
        )
        .await;
        mock.mount(
            &Route::get("/timeseries/bds/geography.json"),
            Reply::json(serde_json::json!({"fips": [
                {"name": "us", "geoLevelDisplay": "010"},
                {"name": "county", "geoLevelDisplay": "050"},
                {"name": "metropolitan statistical area/micropolitan statistical area"},
            ]})),
        )
        .await;
        let found = CensusAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        let mut ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                "CENSUS_BDS_ESTAB_us",
                "CENSUS_BDS_FIRM_us",
                "CENSUS_BDS_JOB_CREATION_us",
            ]
        );
    }

    /// A per-state id requests `for=state:{FIPS}` and parses its rows.
    #[tokio::test]
    async fn fetch_state_series_queries_that_state() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/timeseries/bds")
                .query("get", "ESTAB,YEAR")
                .query("for", "state:06")
                .query("key", TEST_API_KEY),
            Reply::json_str(ESTAB_STATE_06),
            1,
        )
        .await;
        let s = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_ESTAB_state_06", None)
            .await
            .unwrap();
        let got: Vec<(NaiveDate, Option<String>)> = s
            .points
            .iter()
            .map(|p| (p.date, p.value.as_ref().map(ToString::to_string)))
            .collect();
        assert_eq!(
            got,
            [
                (d("2021-01-01"), Some("823456".into())),
                (d("2022-01-01"), Some("841234".into())),
            ]
        );
        mock.server().verify().await;
    }

    /// Ids split into variable and area, including variables whose name ends in `_state`.
    #[test]
    fn series_id_parsing() {
        assert_eq!(
            parse_series_id("CENSUS_BDS_ESTAB_us").unwrap(),
            ("ESTAB", Area::National)
        );
        assert_eq!(
            parse_series_id("CENSUS_BDS_JOB_CREATION_state_56").unwrap(),
            ("JOB_CREATION", Area::State("56"))
        );
        // A variable whose own name ends in "_state" still needs a FIPS suffix.
        assert_eq!(
            parse_series_id("CENSUS_BDS_X_state_state_01").unwrap(),
            ("X_state", Area::State("01"))
        );
        // fetchable_id_regex is checked against Postgres in the scheduler's DB tests.
        for s in us_states().unwrap() {
            let id = format!("CENSUS_BDS_ESTAB_state_{}", s.fips);
            assert!(parse_series_id(&id).is_ok(), "{id}");
        }
    }

    /// Bare levels, unknown FIPS codes, malformed and foreign ids fail before any request.
    #[tokio::test]
    async fn fetch_rejects_unsupported_and_foreign_ids_without_requests() {
        let mock = MockSource::start().await;
        let adapter = CensusAdapter::new(mock.base_url());
        for id in [
            "CENSUS_BDS_ESTAB_state",
            "CENSUS_BDS_ESTAB_county",
            "CENSUS_BDS_ESTAB_state_03",
            "CENSUS_BDS_ESTAB_state_6",
            "CENSUS_BDS_ESTAB_state_06x",
            "CENSUS_BDS__state_06",
            "GDP",
            "CENSUS_BDS__us",
            "CENSUS_BDS_ESTAB&x=1_us",
            "CENSUS_BDS_ESTAB&x=1_state_06",
        ] {
            let e = adapter
                .fetch_series(&test_ctx(), id, None)
                .await
                .unwrap_err();
            assert_eq!(e.kind(), "permanent", "{id}: {e}");
        }
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn census_specific_errors() {
        // Unknown variable: HTTP 400 with a plain-text body.
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds"),
            Reply::text("error: unknown variable 'NOPE'").with_status(400),
        )
        .await;
        let e = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_NOPE_us", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");
        assert!(!e.to_string().contains(TEST_API_KEY), "{e}");

        // Invalid key: 200 with an HTML page.
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds"),
            Reply::text("<html><body><h1>Invalid Key</h1><p>A valid key must be included with each data API request.</p></body></html>"),
        )
        .await;
        let e = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_ESTAB_us", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");

        // Invalid key on a metadata file: discovery fails with Auth, not Parse.
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/timeseries/bds/variables.json"),
            Reply::text("<html><body><h1>Invalid Key</h1></body></html>"),
        )
        .await;
        let e = CensusAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");

        // 204 No Content.
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/timeseries/bds"), Reply::status(204))
            .await;
        let e = CensusAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CENSUS_BDS_ESTAB_us", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");

        // Other 400s stay permanent.
        let e = classify_census_error(CrawlError::Permanent("HTTP 400: error: bad for".into()));
        assert_eq!(e.kind(), "permanent");
    }

    #[test]
    fn row_parsing_rules() {
        let body = r#"[["YEAR","ESTAB","us"],["2020","5","1"],["bad","6","1"],["2021","7"],["2020","8","1"],["2022",null,"1"],["2023","(D)","1"]]"#;
        let pts = parse_bds_rows("X", "ESTAB", body).unwrap();
        let got: Vec<(i32, Option<String>)> = pts
            .iter()
            .map(|p| (p.date.year(), p.value.as_ref().map(ToString::to_string)))
            .collect();
        // Short row and bad year skipped, duplicate year keeps the first, null / non-numeric -> None.
        assert_eq!(got, [(2020, Some("5".into())), (2022, None), (2023, None)]);
        assert_eq!(
            parse_bds_rows("X", "FIRM", body).unwrap_err().kind(),
            "parse"
        );
        assert_eq!(
            parse_bds_rows("X", "ESTAB", "[]").unwrap_err().kind(),
            "parse"
        );
        assert!(parse_bds_rows("X", "ESTAB", r#"[["ESTAB","YEAR","us"]]"#)
            .unwrap()
            .is_empty());
    }
}

#[cfg(test)]
mod contract {
    use super::CensusAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| CensusAdapter::new(base_url),
        external_id: "CENSUS_BDS_ESTAB_us",
        route: Route::get("/timeseries/bds").query("get", "ESTAB,YEAR").query("for", "us:*"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/census/bds_estab_us.json")),
        expect_points: 4,
        setup: |mock| {
            mock.mount(
                &Route::get("/timeseries/bds/geography.json"),
                Reply::json_str(include_str!("../../tests/fixtures/census/geography.json")),
            )
            .await;
        },
        discover: {
            route: Route::get("/timeseries/bds/variables.json"),
            reply: Reply::json_str(include_str!("../../tests/fixtures/census/variables.json")),
            min_series: 6,
        },
    }
}
