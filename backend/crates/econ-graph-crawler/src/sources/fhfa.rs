// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FHFA (Federal Housing Finance Agency) House Price Index adapter.
//!
//! Ported from `econ-graph-services/src/services/series_discovery/fhfa.rs`.
//!
//! # Discovery
//!
//! Makes no request, exactly like the old code: the catalog is the old static list — the
//! national index `USHPI`, one `{STATE}HPI` per state + DC (from the shared states file,
//! [`crate::reference::us_states`], read at runtime), and one `{METRO}HPI` per listed
//! metro area, all quarterly, units "Index (1991Q1 = 100)". Ids are de-duplicated first-seen, so
//! where a metro code collides with a state code (`LAHPI` = Louisiana, not Los Angeles; `SDHPI` =
//! South Dakota, not San Diego) the state wins, as the old `get_or_create` loop did.
//!
//! # Fetching
//!
//! The old code parsed data responses (`FhfaApiResponse`), so fetching is implemented against
//! that shape:
//! `GET {base}/v1/house-price-index/{national | state/{CODE} | metro/{CODE}}?page=N[&start_date=YYYY-MM-DD]`
//! returning `{"data":[{"year","quarter","hpi_value",..}],"meta":{"total_count","page","per_page"}}`.
//! The path is the discovered `data_url`. Pages are walked while `page * per_page < total_count`
//! (and the page was non-empty), at most [`MAX_PAGES`]. Each row becomes a point dated the first
//! day of its quarter, `revision_date = date`, `is_original_release = true`; `hpi_value` is converted via
//! its shortest decimal representation (e.g. `695.10` -> `695.1`). Metadata comes from the static catalog.
//!
//! **Unverified upstream.** The old code was never exercised against a live `api.fhfa.gov` (FHFA
//! publishes HPI as downloadable files); endpoint and response shape are carried over from the
//! old structs as-is.
//!
//! # Errors
//!
//! Unknown ids are `NotFound` without a request. HTTP errors use the fetcher's status mapping;
//! a row with an out-of-range quarter or a non-numeric `hpi_value` is `Parse`
//! ([`parse_row`]).

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
use crate::reference::us_states;
use crate::source::SourceId;

/// The API root the old code used.
pub const DEFAULT_BASE_URL: &str = "https://api.fhfa.gov";

/// Upper bound on pages fetched per series.
const MAX_PAGES: u32 = 20;

const FREQUENCY: &str = "Quarterly";
const UNITS: &str = "Index (1991Q1 = 100)";

const METROS: &[(&str, &str)] = &[
    ("NYC", "New York-Newark-Jersey City, NY-NJ-PA"),
    ("LA", "Los Angeles-Long Beach-Anaheim, CA"),
    ("CHI", "Chicago-Naperville-Elgin, IL-IN-WI"),
    ("DAL", "Dallas-Fort Worth-Arlington, TX"),
    ("HOU", "Houston-The Woodlands-Sugar Land, TX"),
    ("PHX", "Phoenix-Mesa-Chandler, AZ"),
    ("PHI", "Philadelphia-Camden-Wilmington, PA-NJ-DE-MD"),
    ("SAN", "San Antonio-New Braunfels, TX"),
    ("SD", "San Diego-Chula Vista-Carlsbad, CA"),
    ("AUS", "Austin-Round Rock-Georgetown, TX"),
    ("JAX", "Jacksonville, FL"),
    ("FTW", "Fort Worth-Arlington, TX"),
    ("COL", "Columbus, OH"),
    ("CHA", "Charlotte-Concord-Gastonia, NC-SC"),
    ("SF", "San Francisco-Oakland-Berkeley, CA"),
    ("IND", "Indianapolis-Carmel-Anderson, IN"),
    ("SEA", "Seattle-Tacoma-Bellevue, WA"),
    ("DEN", "Denver-Aurora-Lakewood, CO"),
    ("WAS", "Washington-Arlington-Alexandria, DC-VA-MD-WV"),
    ("BOS", "Boston-Cambridge-Newton, MA-NH"),
];

/// One catalog entry: external id, geography name, API path below `/v1/house-price-index/`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogEntry {
    external_id: String,
    name: String,
    path: String,
}

/// The catalog, de-duplicated first-seen (national, states, metros). The states come from the
/// shared states file ([`us_states`], read at runtime).
fn catalog() -> Result<Vec<CatalogEntry>, CrawlError> {
    let national = std::iter::once(CatalogEntry {
        external_id: "USHPI".into(),
        name: "U.S.".into(),
        path: "national".into(),
    });
    let states = us_states()?.iter().map(|s| CatalogEntry {
        external_id: format!("{}HPI", s.postal),
        name: s.name.clone(),
        path: format!("state/{}", s.postal),
    });
    let metros = METROS.iter().map(|(code, name)| CatalogEntry {
        external_id: format!("{code}HPI"),
        name: (*name).into(),
        path: format!("metro/{code}"),
    });
    let mut seen = HashSet::new();
    Ok(national
        .chain(states)
        .chain(metros)
        .filter(|e| seen.insert(e.external_id.clone()))
        .collect())
}

/// FHFA adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct FhfaAdapter {
    base_url: String,
}

impl FhfaAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn data_url(&self, entry: &CatalogEntry) -> String {
        format!("{}/v1/house-price-index/{}", self.base_url, entry.path)
    }
}

impl Default for FhfaAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for FhfaAdapter {
    fn id(&self) -> SourceId {
        SourceId::Fhfa
    }

    /// The static catalog; no request (see the module docs).
    async fn discover(&self, _ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        Ok(catalog()?
            .into_iter()
            .map(|e| DiscoveredSeries {
                data_url: Some(self.data_url(&e)),
                title: format!("{} House Price Index", e.name),
                description: Some(format!("Quarterly house price index for {}", e.name)),
                units: Some(UNITS.into()),
                frequency: Some(FREQUENCY.into()),
                external_id: e.external_id,
            })
            .collect())
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let entry = catalog()?
            .into_iter()
            .find(|e| e.external_id == external_id)
            .ok_or_else(|| CrawlError::NotFound(format!("FHFA: unknown series {external_id:?}")))?;
        let url = self.data_url(&entry);
        let start = since.map(|d| d.format("%Y-%m-%d").to_string());

        let mut seen = HashSet::new();
        let mut points = Vec::new();
        for page in 1..=MAX_PAGES {
            let page_s = page.to_string();
            let mut query = vec![("page", page_s.as_str())];
            if let Some(s) = &start {
                query.push(("start_date", s.as_str()));
            }
            let body: HpiResponse = ctx.http.get_json(SourceId::Fhfa, &url, &query).await?;
            let got = body.data.len();
            for row in body.data {
                let p = parse_row(external_id, row)?;
                if seen.insert(p.date) {
                    points.push(p);
                }
            }
            let fetched = u64::from(page) * u64::from(body.meta.per_page);
            if got == 0 || fetched >= u64::from(body.meta.total_count) {
                break;
            }
        }

        Ok(FetchedSeries {
            metadata: Some(NewSeriesMetadataLite {
                title: format!("{} House Price Index", entry.name),
                description: Some(format!("Quarterly house price index for {}", entry.name)),
                units: Some(UNITS.into()),
                frequency: Some(FREQUENCY.into()),
                seasonal_adjustment: None,
            }),
            points,
        })
    }
}

/// One HPI row -> a point on the first day of its quarter.
fn parse_row(external_id: &str, row: HpiRow) -> Result<FetchedPoint, CrawlError> {
    let month = match row.quarter {
        1..=4 => row.quarter * 3 - 2,
        q => {
            return Err(CrawlError::Parse(format!(
                "FHFA {external_id}: invalid quarter {q} in {}",
                row.year
            )))
        }
    };
    let date = NaiveDate::from_ymd_opt(row.year, month, 1).ok_or_else(|| {
        CrawlError::Parse(format!("FHFA {external_id}: invalid year {}", row.year))
    })?;
    let raw = row.hpi_value.to_string();
    let value = BigDecimal::from_str(&raw).map_err(|e| {
        CrawlError::Parse(format!(
            "FHFA {external_id}: invalid hpi_value {raw:?} on {date}: {e}"
        ))
    })?;
    Ok(FetchedPoint {
        date,
        value: Some(value),
        revision_date: date,
        is_original_release: true,
    })
}

// ---- Wire format (the old FhfaApiResponse / FhfaHpiData / FhfaMeta, fields we use) ----

#[derive(Debug, Deserialize)]
struct HpiResponse {
    data: Vec<HpiRow>,
    meta: HpiMeta,
}

#[derive(Debug, Deserialize)]
struct HpiRow {
    year: i32,
    quarter: u32,
    hpi_value: serde_json::Number,
}

#[derive(Debug, Deserialize)]
struct HpiMeta {
    total_count: u32,
    per_page: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route};

    const NATIONAL: &str = include_str!("../../tests/fixtures/fhfa/hpi_national.json");
    const PAGE1: &str = include_str!("../../tests/fixtures/fhfa/hpi_ca_page1.json");
    const PAGE2: &str = include_str!("../../tests/fixtures/fhfa/hpi_ca_page2.json");

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(FhfaAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(FhfaAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(FhfaAdapter::default().id(), SourceId::Fhfa);
    }

    #[tokio::test]
    async fn discover_is_the_static_catalog_without_requests() {
        let mock = MockSource::start().await;
        let found = FhfaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        assert!(mock.received_requests().await.is_empty());
        // 1 national + 51 states + 20 metros - 2 metro ids shadowed by states (LA, SD).
        assert_eq!(found.len(), 1 + 51 + 20 - 2);
        let ids: HashSet<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(ids.len(), found.len());
        for id in ["USHPI", "CAHPI", "NYCHPI", "DCHPI", "BOSHPI"] {
            assert!(ids.contains(id), "{id}");
        }
        let la = found.iter().find(|s| s.external_id == "LAHPI").unwrap();
        assert_eq!(la.title, "Louisiana House Price Index");
        let us = &found[0];
        assert_eq!(us.title, "U.S. House Price Index");
        assert_eq!(us.frequency.as_deref(), Some("Quarterly"));
        assert_eq!(us.units.as_deref(), Some("Index (1991Q1 = 100)"));
        assert_eq!(
            us.data_url,
            Some(format!("{}/v1/house-price-index/national", mock.base_url()))
        );
        assert!(found
            .iter()
            .all(|s| !s.external_id.is_empty() && !s.title.is_empty()));
    }

    #[tokio::test]
    async fn fetch_parses_quarters_exact_values_and_metadata() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/v1/house-price-index/national"),
            Reply::json_str(NATIONAL),
        )
        .await;
        let s = FhfaAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "USHPI", None)
            .await
            .unwrap();
        let m = s.metadata.unwrap();
        assert_eq!(m.title, "U.S. House Price Index");
        assert_eq!(m.frequency.as_deref(), Some("Quarterly"));
        let got: Vec<(NaiveDate, String)> = s
            .points
            .iter()
            .map(|p| (p.date, p.value.as_ref().unwrap().to_string()))
            .collect();
        assert_eq!(
            got,
            [
                (d("2024-10-01"), "688.27".to_string()),
                (d("2025-01-01"), "695.1".to_string()),
                (d("2025-04-01"), "701.33".to_string()),
            ]
        );
        assert!(s
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));
        let reqs = mock.received_requests().await;
        assert_eq!(reqs.len(), 1);
        assert!(!reqs[0].url.query_pairs().any(|(k, _)| k == "start_date"));
    }

    #[tokio::test]
    async fn fetch_paginates_and_sends_start_date() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/v1/house-price-index/state/CA")
                .query("page", "1")
                .query("start_date", "2024-01-01"),
            Reply::json_str(PAGE1),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/v1/house-price-index/state/CA")
                .query("page", "2")
                .query("start_date", "2024-01-01"),
            Reply::json_str(PAGE2),
            1,
        )
        .await;
        let s = FhfaAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CAHPI", Some(d("2024-01-01")))
            .await
            .unwrap();
        assert_eq!(s.points.len(), 3);
        assert_eq!(s.points[2].date, d("2024-07-01"));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn fetch_page_count_is_bounded() {
        let mock = MockSource::start().await;
        // Claims a huge total and keeps returning the same page.
        let endless = PAGE1.replace("\"total_count\": 3", "\"total_count\": 1000000");
        assert_ne!(endless, PAGE1);
        mock.mount(
            &Route::get("/v1/house-price-index/state/CA"),
            Reply::json_str(endless),
        )
        .await;
        let s = FhfaAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "CAHPI", None)
            .await
            .unwrap();
        assert_eq!(s.points.len(), 2, "repeated rows are de-duplicated");
        assert_eq!(mock.received_requests().await.len(), MAX_PAGES as usize);
    }

    #[tokio::test]
    async fn fetch_routes_by_catalog_and_rejects_unknown_ids() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/v1/house-price-index/metro/NYC"),
            Reply::json_str(NATIONAL),
        )
        .await;
        let adapter = FhfaAdapter::new(mock.base_url());
        adapter
            .fetch_series(&test_ctx(), "NYCHPI", None)
            .await
            .unwrap();
        let e = adapter
            .fetch_series(&test_ctx(), "XXHPI", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found");
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    #[test]
    fn row_parsing_rules() {
        let row = |q: u32, v: &str| HpiRow {
            year: 2024,
            quarter: q,
            hpi_value: serde_json::from_str(v).unwrap(),
        };
        assert_eq!(parse_row("X", row(4, "1")).unwrap().date, d("2024-10-01"));
        assert_eq!(parse_row("X", row(0, "1")).unwrap_err().kind(), "parse");
        assert_eq!(parse_row("X", row(5, "1")).unwrap_err().kind(), "parse");
        assert_eq!(
            parse_row("X", row(1, "350.5")).unwrap().value,
            Some(BigDecimal::from_str("350.5").unwrap())
        );
    }
}

/// Fetch contract. FHFA discovery makes no request (static catalog), so the `discover` section
/// (which asserts the mock was hit) does not apply; see `discover_is_the_static_catalog_without_requests`.
#[cfg(test)]
mod contract {
    use super::FhfaAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| FhfaAdapter::new(base_url),
        external_id: "USHPI",
        route: Route::get("/v1/house-price-index/national"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/fhfa/hpi_national.json")),
        expect_points: 3,
    }
}
