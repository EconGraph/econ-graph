// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! A minimal adapter that exists only to exercise the test kit end to end, and to show the
//! adapter constructor convention and the contract macro in use.

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

const DEFAULT_BASE_URL: &str = "https://echo.invalid";

/// `GET {base}/series/{id}` -> `{"title": .., "points": [{"date": "YYYY-MM-DD", "value": "1.5" | null}]}`;
/// `GET {base}/catalog` -> `{"series": [{"id": .., "title": ..}]}`.
pub(crate) struct EchoAdapter {
    base_url: String,
}

impl EchoAdapter {
    pub(crate) fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }
}

impl Default for EchoAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[derive(Deserialize)]
struct SeriesBody {
    title: String,
    points: Vec<PointBody>,
}

#[derive(Deserialize)]
struct PointBody {
    date: NaiveDate,
    value: Option<String>,
}

#[derive(Deserialize)]
struct CatalogBody {
    series: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct CatalogEntry {
    id: String,
    title: String,
}

#[async_trait]
impl SourceAdapter for EchoAdapter {
    fn id(&self) -> SourceId {
        SourceId::Fred
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let url = format!("{}/catalog", self.base_url);
        let body: CatalogBody = ctx.http.get_json(self.id(), &url, &[]).await?;
        Ok(body
            .series
            .into_iter()
            .map(|e| DiscoveredSeries {
                data_url: Some(format!("{}/series/{}", self.base_url, e.id)),
                external_id: e.id,
                title: e.title,
                description: None,
                units: None,
                frequency: None,
            })
            .collect())
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let url = format!("{}/series/{external_id}", self.base_url);
        let body: SeriesBody = ctx.http.get_json(self.id(), &url, &[]).await?;
        let today = chrono::Utc::now().date_naive();
        let points = body
            .points
            .into_iter()
            .filter(|p| since.is_none_or(|s| p.date >= s))
            .map(|p| {
                let value = p
                    .value
                    .map(|v| {
                        BigDecimal::from_str(&v)
                            .map_err(|e| CrawlError::Parse(format!("value {v:?}: {e}")))
                    })
                    .transpose()?;
                Ok(FetchedPoint {
                    date: p.date,
                    value,
                    revision_date: today,
                    is_original_release: true,
                })
            })
            .collect::<Result<Vec<_>, CrawlError>>()?;
        Ok(FetchedSeries {
            metadata: Some(NewSeriesMetadataLite {
                title: body.title,
                ..Default::default()
            }),
            points,
        })
    }
}

#[test]
fn default_uses_real_url() {
    assert_eq!(EchoAdapter::default().base_url, DEFAULT_BASE_URL);
    assert_eq!(EchoAdapter::new("http://x/").base_url, "http://x");
}

mod contract {
    use super::EchoAdapter;
    use crate::testkit::{Reply, Route};

    const SERIES: &str = r#"{"title": "Echo GDP", "points": [
        {"date": "2024-01-01", "value": "1.5"},
        {"date": "2024-02-01", "value": null},
        {"date": "2024-03-01", "value": "-2"}
    ]}"#;

    crate::adapter_contract_tests! {
        adapter: |base_url: String| EchoAdapter::new(base_url),
        external_id: "GDP",
        route: Route::get("/series/GDP"),
        ok_reply: Reply::json_str(SERIES),
        expect_points: 3,
        discover: {
            route: Route::get("/catalog"),
            reply: Reply::json(serde_json::json!({"series": [
                {"id": "GDP", "title": "Gross domestic product"},
                {"id": "CPI", "title": "Consumer prices"}
            ]})),
            min_series: 2,
        },
    }
}

/// Exercises the optional `malformed_reply` and `setup` arms of the macro.
mod contract_with_options {
    use super::EchoAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: EchoAdapter::new,
        external_id: "X",
        route: Route::get("/series/X"),
        ok_reply: Reply::json(serde_json::json!({"title": "X", "points": []})),
        expect_points: 0,
        malformed_reply: Reply::json(serde_json::json!({"title": "X", "points": [{"date": "2024-01-01", "value": "abc"}]})),
        setup: |mock| {
            mock.mount(&Route::get("/unused"), Reply::ok()).await;
        },
    }
}
