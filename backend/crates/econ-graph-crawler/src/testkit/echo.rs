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
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::source::SourceId;

const DEFAULT_BASE_URL: &str = "https://echo.invalid";

/// `GET {base}/series/{id}` -> `{"title": .., "points": [{"date": "YYYY-MM-DD", "value": "1.5" | null}]}`;
/// `GET {base}/catalog` -> `{"series": [{"id": .., "title": ..}]}`.
pub(crate) struct EchoAdapter {
    base_url: String,
    /// Dataset codes returned by `datasets()`.
    declared: Vec<&'static str>,
    /// Dataset attached to every series, with the series id as the `id` dimension value when
    /// the dataset has an `id` key.
    dataset: Option<SeriesDataset>,
}

impl EchoAdapter {
    pub(crate) fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            declared: Vec::new(),
            dataset: None,
        }
    }

    /// Declares `declared` and attaches `dataset` to every series (for dataset contract tests).
    pub(crate) fn with_dataset(
        mut self,
        declared: Vec<&'static str>,
        dataset: SeriesDataset,
    ) -> Self {
        self.declared = declared;
        self.dataset = Some(dataset);
        self
    }

    fn dataset_for(&self, id: &str) -> Option<SeriesDataset> {
        let mut dataset = self.dataset.clone()?;
        if let Some(v) = dataset.dimensions.0.get_mut("id") {
            *v = id.to_string();
        }
        Some(dataset)
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

    fn datasets(&self) -> &[&str] {
        &self.declared
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let url = format!("{}/catalog", self.base_url);
        let body: CatalogBody = ctx.http.get_json(self.id(), &url, &[]).await?;
        Ok(body
            .series
            .into_iter()
            .map(|e| DiscoveredSeries {
                dataset: self.dataset_for(&e.id),
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
            dataset: self.dataset_for(external_id),
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

/// The contract's dataset checks: an undeclared dataset, wrong dimension keys or two series with
/// the same dimension values fail it.
mod dataset_contract {
    use super::EchoAdapter;
    use crate::dataset::{parse_dataset_file, DatasetCatalog, SeriesDataset};
    use crate::source::SourceId;
    use crate::testkit::contract::{
        assert_discover_ok, assert_fetch_ok, assert_series_datasets_in,
    };
    use crate::testkit::{test_ctx, MockSource, Reply, Route};
    use crate::SourceAdapter as _;

    const ECHO: &str = "[[dataset]]\ncode = \"echo\"\nname = \"Echo\"\n\n\
                        [[dataset.dimensions]]\nname = \"id\"\nlabel = \"Series\"\n";

    async fn mock() -> MockSource {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/catalog"),
            Reply::json(serde_json::json!({"series": [
                {"id": "GDP", "title": "Gross domestic product"},
                {"id": "CPI", "title": "Consumer prices"}
            ]})),
        )
        .await;
        mock.mount(
            &Route::get("/series/GDP"),
            Reply::json(serde_json::json!({"title": "GDP", "points": []})),
        )
        .await;
        mock
    }

    fn catalog() -> DatasetCatalog {
        let mut c = DatasetCatalog::empty();
        c.insert(SourceId::Fred, &["echo"], parse_dataset_file(ECHO).unwrap())
            .unwrap();
        c
    }

    fn series_of(adapter: &EchoAdapter, found: &[crate::DiscoveredSeries]) {
        assert_series_datasets_in(
            &catalog(),
            adapter,
            found
                .iter()
                .map(|s| (s.external_id.as_str(), s.dataset.as_ref())),
        );
    }

    #[tokio::test]
    #[should_panic(expected = "dataset echo is not declared by the adapter")]
    async fn discover_fails_on_undeclared_dataset() {
        let mock = mock().await;
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec![], SeriesDataset::new("echo", [("id", "")]));
        assert_discover_ok(&adapter, &test_ctx(), &mock, 1).await;
    }

    #[tokio::test]
    #[should_panic(expected = "dataset echo is not declared by the adapter")]
    async fn fetch_fails_on_undeclared_dataset() {
        let mock = mock().await;
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec![], SeriesDataset::new("echo", [("id", "")]));
        assert_fetch_ok(&adapter, &test_ctx(), &mock, "GDP", 0).await;
    }

    /// A declared dataset with no matching definition in the source's shipped file fails (the
    /// panic message also names that file; see `DatasetCatalog::insert`). The echo adapter shares
    /// `SourceId::Fred` with the FRED contract tests, so this reads the real `datasets/fred.toml`
    /// (which has `FRED`, not `echo`).
    #[tokio::test]
    #[should_panic(expected = "declares dataset echo, which has no definition in")]
    async fn declared_dataset_without_definition_fails() {
        let mock = mock().await;
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec!["echo"], SeriesDataset::new("echo", [("id", "")]));
        assert_discover_ok(&adapter, &test_ctx(), &mock, 1).await;
    }

    #[tokio::test]
    async fn declared_dataset_with_right_keys_passes() {
        let mock = mock().await;
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec!["echo"], SeriesDataset::new("echo", [("id", "")]));
        let found = adapter.discover(&test_ctx()).await.unwrap();
        let def = &parse_dataset_file(ECHO).unwrap()[0];
        let ids: Vec<String> = found
            .iter()
            .map(|s| {
                def.external_id(&s.dataset.as_ref().unwrap().dimensions)
                    .unwrap()
            })
            .collect();
        assert_eq!(ids, ["echo/GDP", "echo/CPI"]);
        // The source's own ids (GDP, CPI) are accepted as well as canonical ones.
        series_of(&adapter, &found);
    }

    #[tokio::test]
    #[should_panic(expected = "wrong dimension keys")]
    async fn wrong_dimension_keys_fail() {
        let mock = mock().await;
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec!["echo"], SeriesDataset::new("echo", [("series", "")]));
        let found = adapter.discover(&test_ctx()).await.unwrap();
        series_of(&adapter, &found);
    }

    #[tokio::test]
    #[should_panic(expected = "have the same dataset echo")]
    async fn two_series_with_the_same_dimensions_fail() {
        let mock = mock().await;
        // A fixed `key` dimension instead of `id`, so both series get the same values.
        let mut c = DatasetCatalog::empty();
        c.insert(
            SourceId::Fred,
            &["echo"],
            parse_dataset_file(&ECHO.replace("\"id\"", "\"key\"")).unwrap(),
        )
        .unwrap();
        let adapter = EchoAdapter::new(mock.base_url())
            .with_dataset(vec!["echo"], SeriesDataset::new("echo", [("key", "same")]));
        let found = adapter.discover(&test_ctx()).await.unwrap();
        assert_series_datasets_in(
            &c,
            &adapter,
            found
                .iter()
                .map(|s| (s.external_id.as_str(), s.dataset.as_ref())),
        );
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
