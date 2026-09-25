// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! BEA (Bureau of Economic Analysis) adapter — discovery only.
//!
//! Ported from `econ-graph-services/src/services/series_discovery/bea.rs`.
//!
//! # Discovery
//!
//! One request, `GET {base}/?UserID=KEY&method=GetDatasetList&ResultFormat=JSON`, parsed as
//! `{"BEAAPI":{"Results":{"Dataset":[{"DatasetName":..,"DatasetDescription":..}]}}}`. Each dataset
//! name is expanded to the old hard-coded series list ([`known_series`]: NIPA, FixedAssets, ITA,
//! RegionalData — also matched as `Regional`, the name the live dataset list uses). No
//! pagination, so no page cap; the single request is bounded by construction.
//!
//! The key (`ctx.keys.bea`, `BEA_API_KEY`) is required: without it discovery fails with `Auth`
//! before any request. The `UserID` query parameter is redacted by the
//! [`HttpFetcher`](crate::HttpFetcher) in logs and errors.
//!
//! # Errors
//!
//! BEA reports request errors as HTTP 200 with
//! `{"BEAAPI":{..,"Error":{"APIErrorCode":..,"APIErrorDescription":..}}}` (under `BEAAPI` or
//! `BEAAPI.Results`). [`classify_bea_error`] maps a description that mentions the `UserId` to
//! `Auth` and anything else to `Permanent`. HTTP-level errors use the fetcher's status mapping.
//!
//! # Fetching
//!
//! Not implemented: the old code never requested or parsed BEA data (`GetData`) responses.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The real BEA API root.
pub const DEFAULT_BASE_URL: &str = "https://apps.bea.gov/api/data";

/// BEA adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct BeaAdapter {
    base_url: String,
}

impl BeaAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn api_key<'a>(&self, ctx: &'a CrawlCtx) -> Result<&'a str, CrawlError> {
        ctx.keys
            .bea
            .as_deref()
            .ok_or_else(|| CrawlError::Auth("BEA_API_KEY not set".into()))
    }
}

impl Default for BeaAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for BeaAdapter {
    fn id(&self) -> SourceId {
        SourceId::Bea
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let key = self.api_key(ctx)?;
        let body: BeaEnvelope = ctx
            .http
            .get_json(
                SourceId::Bea,
                &format!("{}/", self.base_url),
                &[
                    ("UserID", key),
                    ("method", "GetDatasetList"),
                    ("ResultFormat", "JSON"),
                ],
            )
            .await?;
        let api = body.beaapi;
        if let Some(err) = api
            .error
            .as_ref()
            .or(api.results.as_ref().and_then(|r| r.error.as_ref()))
        {
            return Err(classify_bea_error(err));
        }
        let datasets = api
            .results
            .and_then(|r| r.dataset)
            .ok_or_else(|| CrawlError::Parse("BEA GetDatasetList: no Results.Dataset".into()))?;

        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for d in &datasets {
            tracing::debug!(dataset = %d.dataset_name, description = %d.dataset_description, "BEA dataset");
            for s in known_series(&d.dataset_name) {
                if seen.insert(s.external_id.clone()) {
                    found.push(s);
                }
            }
        }
        tracing::info!(
            datasets = datasets.len(),
            series = found.len(),
            "BEA discovery finished"
        );
        Ok(found)
    }

    /// Not implemented; see the module docs.
    async fn fetch_series(
        &self,
        _ctx: &CrawlCtx,
        _external_id: &str,
        _since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        Err(CrawlError::Permanent(
            "BEA fetch_series not implemented yet".into(),
        ))
    }
}

/// Maps BEA's in-body error: a description about the `UserId` (missing / invalid / inactive
/// key) is `Auth`, everything else `Permanent`. The message never contains the key itself.
pub fn classify_bea_error(err: &BeaError) -> CrawlError {
    let msg = format!(
        "BEA error {}: {}",
        err.code.as_deref().unwrap_or("?"),
        err.description.as_deref().unwrap_or("")
    );
    let desc = err
        .description
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    if desc.contains("userid") {
        CrawlError::Auth(msg)
    } else {
        CrawlError::Permanent(msg)
    }
}

/// The old hard-coded series per dataset.
fn known_series(dataset_name: &str) -> Vec<DiscoveredSeries> {
    let s =
        |id: &str, title: &str, description: &str, frequency: &str, units: &str| DiscoveredSeries {
            external_id: id.to_string(),
            title: title.to_string(),
            description: Some(description.to_string()),
            units: Some(units.to_string()),
            frequency: Some(frequency.to_string()),
            data_url: None,
        };
    match dataset_name {
        "NIPA" => vec![
            s(
                "NIPA_GDP_TOTAL",
                "Gross Domestic Product",
                "Total GDP from National Income and Product Accounts",
                "Quarterly",
                "Billions of Dollars",
            ),
            s(
                "NIPA_GDP_PC",
                "Gross Domestic Product Per Capita",
                "GDP per capita from National Income and Product Accounts",
                "Quarterly",
                "Dollars",
            ),
            s(
                "NIPA_PCE_TOTAL",
                "Personal Consumption Expenditures",
                "Total personal consumption expenditures",
                "Quarterly",
                "Billions of Dollars",
            ),
        ],
        "FixedAssets" => vec![s(
            "FA_NET_STOCK_TOTAL",
            "Net Stock of Fixed Assets",
            "Total net stock of fixed assets",
            "Annual",
            "Billions of Dollars",
        )],
        "ITA" => vec![
            s(
                "ITA_EXPORTS_TOTAL",
                "Total Exports of Goods and Services",
                "Total exports from International Transactions Accounts",
                "Quarterly",
                "Billions of Dollars",
            ),
            s(
                "ITA_IMPORTS_TOTAL",
                "Total Imports of Goods and Services",
                "Total imports from International Transactions Accounts",
                "Quarterly",
                "Billions of Dollars",
            ),
        ],
        "RegionalData" | "Regional" => vec![s(
            "REG_GDP_TOTAL",
            "Gross Domestic Product by State",
            "Total GDP by state",
            "Annual",
            "Millions of Dollars",
        )],
        _ => Vec::new(),
    }
}

// ---- Wire format ----

#[derive(Debug, Deserialize)]
struct BeaEnvelope {
    #[serde(rename = "BEAAPI")]
    beaapi: BeaApi,
}

#[derive(Debug, Deserialize)]
struct BeaApi {
    #[serde(rename = "Results", default)]
    results: Option<BeaResults>,
    #[serde(rename = "Error", default)]
    error: Option<BeaError>,
}

#[derive(Debug, Deserialize)]
struct BeaResults {
    #[serde(rename = "Dataset", default)]
    dataset: Option<Vec<BeaDataset>>,
    #[serde(rename = "Error", default)]
    error: Option<BeaError>,
}

#[derive(Debug, Deserialize)]
struct BeaDataset {
    #[serde(rename = "DatasetName")]
    dataset_name: String,
    #[serde(rename = "DatasetDescription", default)]
    dataset_description: String,
}

/// BEA's in-body error object.
#[derive(Debug, Clone, Deserialize)]
pub struct BeaError {
    /// `APIErrorCode`.
    #[serde(rename = "APIErrorCode", default)]
    pub code: Option<String>,
    /// `APIErrorDescription`.
    #[serde(rename = "APIErrorDescription", default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    const DATASETS: &str = include_str!("../../tests/fixtures/bea/dataset_list.json");
    const BAD_KEY: &str = include_str!("../../tests/fixtures/bea/error_invalid_userid.json");

    #[test]
    fn constructor_convention() {
        assert_eq!(BeaAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(BeaAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(BeaAdapter::default().id(), SourceId::Bea);
    }

    #[tokio::test]
    async fn discover_expands_known_datasets() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &Route::get("/")
                .query("UserID", TEST_API_KEY)
                .query("method", "GetDatasetList")
                .query("ResultFormat", "JSON"),
            Reply::json_str(DATASETS),
            1,
        )
        .await;
        let found = BeaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "NIPA_GDP_TOTAL",
                "NIPA_GDP_PC",
                "NIPA_PCE_TOTAL",
                "FA_NET_STOCK_TOTAL",
                "ITA_EXPORTS_TOTAL",
                "ITA_IMPORTS_TOTAL",
                "REG_GDP_TOTAL",
            ]
        );
        assert_eq!(found[0].units.as_deref(), Some("Billions of Dollars"));
        assert_eq!(found[0].frequency.as_deref(), Some("Quarterly"));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn missing_key_is_auth_error_without_requests() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json_str(DATASETS))
            .await;
        let mut ctx = test_ctx();
        ctx.keys.bea = None;
        let e = BeaAdapter::new(mock.base_url())
            .discover(&ctx)
            .await
            .unwrap_err();
        assert_eq!(e, CrawlError::Auth("BEA_API_KEY not set".into()));
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn in_body_errors_are_classified_and_key_redacted() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json_str(BAD_KEY)).await;
        let e = BeaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");
        assert!(!e.to_string().contains(TEST_API_KEY));

        let other = serde_json::json!({"BEAAPI": {"Error": {
            "APIErrorCode": "40", "APIErrorDescription": "The dataset requested is not valid."}}});
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json(other)).await;
        let e = BeaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");
    }

    #[tokio::test]
    async fn http_errors_map_by_status_and_redact_key() {
        for (reply, kind) in [
            (Reply::status(429).retry_after(1), "rate_limited"),
            (Reply::text("internal error").with_status(500), "transient"),
            (Reply::status(404), "not_found"),
            (
                Reply::json_str(format!("<html>{TEST_API_KEY}</html>")),
                "parse",
            ),
        ] {
            let mock = MockSource::start().await;
            mock.mount(&Route::get("/"), reply).await;
            let e = BeaAdapter::new(mock.base_url())
                .discover(&test_ctx())
                .await
                .unwrap_err();
            assert_eq!(e.kind(), kind, "{e}");
            assert!(!e.to_string().contains(TEST_API_KEY), "{e}");
        }
    }

    #[tokio::test]
    async fn fetch_series_is_not_implemented() {
        let mock = MockSource::start().await;
        let e = BeaAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "NIPA_GDP_TOTAL", None)
            .await
            .unwrap_err();
        assert_eq!(
            e,
            CrawlError::Permanent("BEA fetch_series not implemented yet".into())
        );
        assert!(mock.received_requests().await.is_empty());
    }
}

/// Discovery contract (fetch_series is not implemented, so the fetch half of
/// `adapter_contract_tests!` does not apply; these use the same testkit assertions).
#[cfg(test)]
mod contract {
    use super::BeaAdapter;
    use crate::testkit::contract::{assert_discover_ok, MALFORMED_JSON};
    use crate::testkit::{test_ctx, MockSource, Reply, Route};
    use crate::SourceAdapter;

    #[tokio::test]
    async fn contract_discover_ok() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/"),
            Reply::json_str(include_str!("../../tests/fixtures/bea/dataset_list.json")),
        )
        .await;
        assert_discover_ok(&BeaAdapter::new(mock.base_url()), &test_ctx(), &mock, 7).await;
    }

    #[tokio::test]
    async fn contract_discover_malformed_is_parse_error() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json_str(MALFORMED_JSON))
            .await;
        let e = BeaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse");
    }
}
