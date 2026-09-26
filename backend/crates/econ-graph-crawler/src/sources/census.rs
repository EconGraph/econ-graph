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
//! - `GET {base}/timeseries/bds/variables.json` -> `{"variables": {NAME: {"label": ..}, ..}}`
//! - `GET {base}/timeseries/bds/geography.json` -> `{"fips": [{"name": .., "geoLevelDisplay": ..}, ..]}`
//!
//! Variables are filtered with the old keyword rules ([`is_economic_variable`]) and crossed with
//! every geography level, giving `CENSUS_BDS_{VARIABLE}_{geography name}` (annual, units "Count").
//! No pagination, so no page cap.
//!
//! # Fetching
//!
//! The old code parsed BDS data responses (a JSON array of string rows, header row first), so
//! fetching is implemented, for national series only (`CENSUS_BDS_{VARIABLE}_us`): other
//! geography levels have one value per area and year, which is not a single series, so they are
//! `Permanent`. The request is
//! `GET {base}/timeseries/bds?get={VARIABLE},YEAR&for=us:*[&key=KEY]`, i.e. all years (the old
//! comma-separated `YEAR=` list hit the API's "204 No Content" limitation for multi-year
//! queries); `since` is applied client-side by year. Each row becomes a point dated January 1 of
//! its `YEAR`, `revision_date = date`, `is_original_release = true`. As in the old parser, rows
//! of the wrong width or with an unparseable year are skipped, and an empty or non-numeric value
//! is a missing observation (`None`).
//!
//! # API key
//!
//! Optional. The Census API serves low volumes keylessly and the old code never sent a key, so
//! `ctx.keys.census` (`CENSUS_API_KEY`) is sent as `key=` only when set; it is never required.
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
use crate::source::SourceId;

/// The real Census Data API root.
pub const DEFAULT_BASE_URL: &str = "https://api.census.gov/data";

/// Dataset path (relative to the base URL).
const BDS_PATH: &str = "/timeseries/bds";

/// Prefix of every Census external id this adapter produces.
const ID_PREFIX: &str = "CENSUS_BDS_";

/// The only geography level [`CensusAdapter::fetch_series`] supports.
const NATIONAL_GEO: &str = "us";

/// SQL `LIKE` pattern matching the ids [`CensusAdapter::fetch_series`] accepts:
/// `CENSUS_BDS_{VARIABLE}_us`. Discovery also persists other geography levels, which the refresh
/// scheduler must not enqueue.
pub const FETCHABLE_ID_LIKE: &str = "CENSUS\\_BDS\\_%\\_us";

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

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn metadata_json(&self, ctx: &CrawlCtx, file: &str) -> Result<Value, CrawlError> {
        ctx.http
            .get_json(
                SourceId::Census,
                &self.url(&format!("{BDS_PATH}/{file}")),
                &[],
            )
            .await
            .map_err(classify_census_error)
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

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let variables = parse_variables(&self.metadata_json(ctx, "variables.json").await?)?;
        let geographies = parse_geographies(&self.metadata_json(ctx, "geography.json").await?)?;

        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for var in variables.iter().filter(|v| is_economic_variable(v)) {
            for geo in &geographies {
                let external_id = format!("{ID_PREFIX}{}_{}", var.name, geo.name);
                if !seen.insert(external_id.clone()) {
                    continue;
                }
                let geo_label = geo.level_display.as_deref().unwrap_or(&geo.name);
                found.push(DiscoveredSeries {
                    external_id,
                    title: format!("{} - {geo_label}", var.label),
                    description: Some(format!(
                        "Business Dynamics Statistics: {} for {geo_label}",
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

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let variable = national_variable(external_id)?;
        let get = format!("{variable},YEAR");
        let for_geo = format!("{NATIONAL_GEO}:*");
        let mut query = vec![("get", get.as_str()), ("for", for_geo.as_str())];
        if let Some(key) = ctx.keys.census.as_deref() {
            query.push(("key", key));
        }
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

/// `CENSUS_BDS_{VARIABLE}_us` -> `VARIABLE`.
fn national_variable(external_id: &str) -> Result<&str, CrawlError> {
    let rest = external_id.strip_prefix(ID_PREFIX).ok_or_else(|| {
        CrawlError::Permanent(format!(
            "Census: {external_id:?} is not a {ID_PREFIX}* series id"
        ))
    })?;
    let variable = rest
        .strip_suffix(NATIONAL_GEO)
        .and_then(|v| v.strip_suffix('_'))
        .ok_or_else(|| {
            CrawlError::Permanent(format!(
                "Census {external_id}: only national ({NATIONAL_GEO}) BDS series can be fetched"
            ))
        })?;
    if variable.is_empty()
        || !variable
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(CrawlError::Permanent(format!(
            "Census {external_id}: invalid variable name {variable:?}"
        )));
    }
    Ok(variable)
}

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
        if body.to_ascii_lowercase().contains("invalid key") {
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
    level_display: Option<String>,
}

fn parse_variables(body: &Value) -> Result<Vec<BdsVariable>, CrawlError> {
    let vars = body
        .get("variables")
        .and_then(Value::as_object)
        .ok_or_else(|| CrawlError::Parse("Census variables.json: no 'variables' object".into()))?;
    Ok(vars
        .iter()
        .filter_map(|(name, v)| {
            let obj = v.as_object()?;
            Some(BdsVariable {
                name: name.clone(),
                label: obj
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect())
}

fn parse_geographies(body: &Value) -> Result<Vec<BdsGeography>, CrawlError> {
    let fips = body
        .get("fips")
        .and_then(Value::as_array)
        .ok_or_else(|| CrawlError::Parse("Census geography.json: no 'fips' array".into()))?;
    Ok(fips
        .iter()
        .filter_map(|g| {
            let obj = g.as_object()?;
            let name = obj.get("name").and_then(Value::as_str)?.trim().to_string();
            (!name.is_empty()).then(|| BdsGeography {
                name,
                level_display: obj
                    .get("geoLevelDisplay")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
        })
        .collect())
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

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(CensusAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(CensusAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(CensusAdapter::default().id(), SourceId::Census);
    }

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
        let mut ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                "CENSUS_BDS_ESTAB_state",
                "CENSUS_BDS_ESTAB_us",
                "CENSUS_BDS_FIRM_state",
                "CENSUS_BDS_FIRM_us",
                "CENSUS_BDS_JOB_CREATION_state",
                "CENSUS_BDS_JOB_CREATION_us",
            ]
        );
        let estab_us = found
            .iter()
            .find(|s| s.external_id == "CENSUS_BDS_ESTAB_us")
            .unwrap();
        assert_eq!(estab_us.title, "Number of establishments - 010");
        assert_eq!(estab_us.units.as_deref(), Some("Count"));
        assert_eq!(estab_us.frequency.as_deref(), Some("Annual"));
        // Metadata endpoints never carry the key.
        for r in mock.received_requests().await {
            assert!(r.url.query().is_none(), "{}", r.url);
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
    async fn fetch_works_without_a_key_and_applies_since() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/timeseries/bds"), Reply::json_str(ESTAB_US))
            .await;
        let mut ctx = test_ctx();
        ctx.keys.census = None;
        let s = CensusAdapter::new(mock.base_url())
            .fetch_series(&ctx, "CENSUS_BDS_ESTAB_us", Some(d("2021-06-30")))
            .await
            .unwrap();
        let dates: Vec<NaiveDate> = s.points.iter().map(|p| p.date).collect();
        assert_eq!(dates, [d("2021-01-01"), d("2022-01-01")]);
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 1);
        assert!(!reqs[0].url.query_pairs().any(|(k, _)| k == "key"));
    }

    #[tokio::test]
    async fn fetch_rejects_non_national_and_foreign_ids_without_requests() {
        let mock = MockSource::start().await;
        let adapter = CensusAdapter::new(mock.base_url());
        for id in [
            "CENSUS_BDS_ESTAB_state",
            "GDP",
            "CENSUS_BDS__us",
            "CENSUS_BDS_ESTAB&x=1_us",
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
