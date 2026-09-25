// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! IMF (International Monetary Fund) adapter — discovery only.
//!
//! Ported from `econ-graph-services/src/services/series_discovery/imf.rs`.
//!
//! # Discovery
//!
//! One request, `GET {base}/Dataflow` (SDMX-JSON, no key), parsed as
//! `{"Structure":{"Dataflows":{"Dataflow":[{"KeyFamilyRef":{"KeyFamilyID":..},"Name":[{"$":..}]}]}}}`
//! (the shape the old code deserialized; `Name` is also accepted as a single object and with
//! `#text` instead of `$`). Dataflows are filtered with the old keyword / id test
//! ([`is_economic_dataflow`]) and each matching dataset id is expanded to the old hard-coded
//! series list ([`known_series`]: IFS, BOP, GFS, WEO), de-duplicated by series id. There is no
//! pagination, so there is no page cap; the single request is bounded by construction.
//!
//! # Errors
//!
//! Plain HTTP status mapping by [`HttpFetcher`](crate::HttpFetcher) (the IMF service has no
//! in-body error convention the old code handled); a body that is not the SDMX-JSON structure
//! is `Parse`.
//!
//! # Fetching
//!
//! Not implemented: the old code never requested or parsed IMF data (`CompactData`) responses.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The IMF SDMX-JSON service root the old code used.
pub const DEFAULT_BASE_URL: &str = "http://dataservices.imf.org/REST/SDMX_JSON.svc";

/// IMF adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct ImfAdapter {
    base_url: String,
}

impl ImfAdapter {
    /// Talks to `base_url` (the service root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }
}

impl Default for ImfAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for ImfAdapter {
    fn id(&self) -> SourceId {
        SourceId::Imf
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let body: DataflowResponse = ctx
            .http
            .get_json(SourceId::Imf, &self.url("/Dataflow"), &[])
            .await?;
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        for flow in body
            .structure
            .dataflows
            .dataflow
            .iter()
            .filter(|f| is_economic_dataflow(f))
        {
            for s in known_series(&flow.key_family_ref.key_family_id) {
                if seen.insert(s.external_id.clone()) {
                    found.push(s);
                }
            }
        }
        tracing::info!(series = found.len(), "IMF discovery finished");
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
            "IMF fetch_series not implemented yet".into(),
        ))
    }
}

/// The old dataset filter: name keywords or id fragments.
fn is_economic_dataflow(flow: &Dataflow) -> bool {
    let name = flow
        .name
        .first()
        .map(|n| n.value.to_lowercase())
        .unwrap_or_default();
    let id = flow.key_family_ref.key_family_id.to_lowercase();
    const NAME_KEYWORDS: &[&str] = &[
        "financial statistics",
        "balance of payments",
        "government finance",
        "world economic outlook",
        "direction of trade",
        "international reserves",
        "exchange rates",
    ];
    const ID_FRAGMENTS: &[&str] = &["ifs", "bop", "gfs", "weo", "dot", "ir"];
    NAME_KEYWORDS.iter().any(|k| name.contains(k)) || ID_FRAGMENTS.iter().any(|f| id.contains(f))
}

/// The old hard-coded series per dataset.
fn known_series(dataset_id: &str) -> Vec<DiscoveredSeries> {
    let s =
        |id: &str, title: &str, description: &str, frequency: &str, units: &str| DiscoveredSeries {
            external_id: id.to_string(),
            title: title.to_string(),
            description: Some(description.to_string()),
            units: Some(units.to_string()),
            frequency: Some(frequency.to_string()),
            data_url: None,
        };
    match dataset_id.to_uppercase().as_str() {
        "IFS" => vec![
            s(
                "IFS_US_PCPI_IX",
                "Consumer Price Index - United States",
                "Consumer Price Index from International Financial Statistics",
                "Monthly",
                "Index",
            ),
            s(
                "IFS_US_LP_IX",
                "Labor Force Participation Rate - United States",
                "Labor force participation rate from International Financial Statistics",
                "Monthly",
                "Percent",
            ),
            s(
                "IFS_US_EREER_IX",
                "Real Effective Exchange Rate - United States",
                "Real effective exchange rate from International Financial Statistics",
                "Monthly",
                "Index",
            ),
        ],
        "BOP" => vec![
            s(
                "BOP_US_CA_BP6_USD",
                "Current Account Balance - United States",
                "Current account balance from Balance of Payments",
                "Quarterly",
                "US Dollars",
            ),
            s(
                "BOP_US_FA_BP6_USD",
                "Financial Account Balance - United States",
                "Financial account balance from Balance of Payments",
                "Quarterly",
                "US Dollars",
            ),
        ],
        "GFS" => vec![
            s(
                "GFS_US_GGR_G01_GDP_PT",
                "General Government Revenue - United States",
                "General government revenue as percentage of GDP from Government Finance Statistics",
                "Annual",
                "Percent of GDP",
            ),
            s(
                "GFS_US_GGX_G01_GDP_PT",
                "General Government Expenditure - United States",
                "General government expenditure as percentage of GDP from Government Finance Statistics",
                "Annual",
                "Percent of GDP",
            ),
        ],
        "WEO" => vec![
            s(
                "WEO_US_NGDP_RPCH",
                "Real GDP Growth - United States",
                "Real GDP growth rate from World Economic Outlook",
                "Annual",
                "Percent",
            ),
            s(
                "WEO_US_PCPIPCH",
                "Inflation Rate - United States",
                "Inflation rate from World Economic Outlook",
                "Annual",
                "Percent",
            ),
        ],
        _ => Vec::new(),
    }
}

// ---- Wire format (the old structs, slightly more tolerant) ----

#[derive(Debug, Deserialize)]
struct DataflowResponse {
    #[serde(rename = "Structure")]
    structure: Structure,
}

#[derive(Debug, Deserialize)]
struct Structure {
    #[serde(rename = "Dataflows")]
    dataflows: Dataflows,
}

#[derive(Debug, Deserialize)]
struct Dataflows {
    #[serde(rename = "Dataflow")]
    dataflow: Vec<Dataflow>,
}

#[derive(Debug, Deserialize)]
struct Dataflow {
    #[serde(rename = "Name", deserialize_with = "one_or_many")]
    name: Vec<Text>,
    #[serde(rename = "KeyFamilyRef")]
    key_family_ref: KeyFamilyRef,
}

#[derive(Debug, Deserialize)]
struct Text {
    #[serde(rename = "$", alias = "#text")]
    value: String,
}

#[derive(Debug, Deserialize)]
struct KeyFamilyRef {
    #[serde(rename = "KeyFamilyID")]
    key_family_id: String,
}

fn one_or_many<'de, D>(d: D) -> Result<Vec<Text>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<Text>),
        One(Text),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::Many(v) => v,
        OneOrMany::One(t) => vec![t],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route};

    const DATAFLOW: &str = include_str!("../../tests/fixtures/imf/dataflow.json");

    #[test]
    fn constructor_convention() {
        assert_eq!(ImfAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(ImfAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(ImfAdapter::default().id(), SourceId::Imf);
    }

    #[tokio::test]
    async fn discover_filters_datasets_and_expands_known_series() {
        let mock = MockSource::start().await;
        mock.mount_expect(&Route::get("/Dataflow"), Reply::json_str(DATAFLOW), 1)
            .await;
        let found = ImfAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        // IFS (x3), BOP (x2), WEO (x2, Name given as a single #text object); the duplicate IFS
        // flow adds nothing; PCPS is filtered out; DOT matches but has no known series.
        assert_eq!(
            ids,
            [
                "IFS_US_PCPI_IX",
                "IFS_US_LP_IX",
                "IFS_US_EREER_IX",
                "BOP_US_CA_BP6_USD",
                "BOP_US_FA_BP6_USD",
                "WEO_US_NGDP_RPCH",
                "WEO_US_PCPIPCH",
            ]
        );
        assert_eq!(found[0].title, "Consumer Price Index - United States");
        assert_eq!(found[0].frequency.as_deref(), Some("Monthly"));
        assert_eq!(found[0].units.as_deref(), Some("Index"));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn fetch_series_is_not_implemented() {
        let mock = MockSource::start().await;
        let e = ImfAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "IFS_US_PCPI_IX", None)
            .await
            .unwrap_err();
        assert_eq!(
            e,
            CrawlError::Permanent("IMF fetch_series not implemented yet".into())
        );
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn discover_errors_map_by_status() {
        for (reply, kind) in [
            (Reply::status(429).retry_after(1), "rate_limited"),
            (Reply::status(403), "auth"),
            (Reply::text("internal error").with_status(500), "transient"),
            (Reply::json_str("{\"Structure\": {}}"), "parse"),
            (Reply::status(404), "not_found"),
        ] {
            let mock = MockSource::start().await;
            mock.mount(&Route::get("/Dataflow"), reply).await;
            let e = ImfAdapter::new(mock.base_url())
                .discover(&test_ctx())
                .await
                .unwrap_err();
            assert_eq!(e.kind(), kind, "{e}");
        }
    }
}

/// Discovery contract (fetch_series is not implemented, so the fetch half of
/// `adapter_contract_tests!` does not apply; these use the same testkit assertions).
#[cfg(test)]
mod contract {
    use super::ImfAdapter;
    use crate::testkit::contract::{assert_discover_ok, MALFORMED_JSON};
    use crate::testkit::{test_ctx, MockSource, Reply, Route};
    use crate::SourceAdapter;

    #[tokio::test]
    async fn contract_discover_ok() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/Dataflow"),
            Reply::json_str(include_str!("../../tests/fixtures/imf/dataflow.json")),
        )
        .await;
        assert_discover_ok(&ImfAdapter::new(mock.base_url()), &test_ctx(), &mock, 5).await;
    }

    #[tokio::test]
    async fn contract_discover_malformed_is_parse_error() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/Dataflow"), Reply::json_str(MALFORMED_JSON))
            .await;
        let e = ImfAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse");
    }
}
