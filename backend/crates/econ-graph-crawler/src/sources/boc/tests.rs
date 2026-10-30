// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Bank of Canada Valet adapter tests. The fixtures in `tests/fixtures/boc/` are recorded Valet
//! responses (see the README there); the bodies built inline are synthetic.

use std::str::FromStr;

use serde_json::json;

use super::*;
use crate::testkit::contract::{assert_discover_ok, assert_series_datasets};
use crate::testkit::{test_ctx, MockSource, Reply, Route};

const SERIES_V39079: &str = include_str!("../../../tests/fixtures/boc/series_V39079.json");
const SERIES_FXUSDCAD: &str = include_str!("../../../tests/fixtures/boc/series_FXUSDCAD.json");
const SERIES_V41552801: &str = include_str!("../../../tests/fixtures/boc/series_V41552801.json");
const OBS_FXUSDCAD: &str = include_str!("../../../tests/fixtures/boc/observations_FXUSDCAD.json");
const OBS_V41552801: &str = include_str!("../../../tests/fixtures/boc/observations_V41552801.json");
const NOT_FOUND: &str = include_str!("../../../tests/fixtures/boc/error_series_not_found.json");

fn names(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| (*s).to_string()).collect()
}

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

async fn mount_series(mock: &MockSource, name: &str, body: &str) {
    mock.mount(
        &Route::get(format!("/series/{name}/json")),
        Reply::json_str(body),
    )
    .await;
}

/// Discovery takes every field from Valet: the label as the title, `metadata.unit` and
/// `metadata.frequency`, and for a series with empty metadata no units and its `dateFormat`.
#[tokio::test]
async fn discovery_takes_metadata_from_valet() {
    let mock = MockSource::start().await;
    mount_series(&mock, "V39079", SERIES_V39079).await;
    mount_series(&mock, "FXUSDCAD", SERIES_FXUSDCAD).await;
    mount_series(&mock, "V41552801", SERIES_V41552801).await;
    let adapter = BocAdapter::new(mock.base_url());
    let found = adapter
        .discover_names(&test_ctx(), &names(&["V39079", "FXUSDCAD", "V41552801"]))
        .await
        .unwrap();
    assert_eq!(found.len(), 3);

    let rate = &found[0];
    assert_eq!(rate.external_id, "V39079");
    assert_eq!(rate.title, "Target for the overnight rate (business daily)");
    assert!(rate
        .description
        .as_deref()
        .unwrap()
        .starts_with("Also called the policy interest rate"));
    assert_eq!(rate.units.as_deref(), Some("Percent"));
    assert_eq!(rate.frequency.as_deref(), Some("Business daily"));
    assert_eq!(
        rate.data_url.as_deref(),
        Some("https://www.bankofcanada.ca/valet/series/V39079")
    );

    let fx = &found[1];
    assert_eq!(fx.title, "USD/CAD");
    assert_eq!(fx.units.as_deref(), Some("Canadian dollars per US dollar"));

    let m2 = &found[2];
    assert_eq!(m2.title, "M2++ (gross) (Seasonally adjusted)");
    assert_eq!(m2.description.as_deref(), Some("M2++ (gross)"));
    assert_eq!(m2.units, None);
    assert_eq!(m2.frequency.as_deref(), Some("Monthly"));

    assert_series_datasets(
        &adapter,
        found.iter().map(|s| (s.external_id.as_str(), &s.dataset)),
    );
    assert!(adapter.curated_missing().is_empty());
}

/// A curated name Valet doesn't know is skipped and reported; the rest are still discovered.
#[tokio::test]
async fn discovery_skips_and_reports_unknown_names() {
    let mock = MockSource::start().await;
    mount_series(&mock, "V39079", SERIES_V39079).await;
    mock.mount(
        &Route::get("/series/NOPE123/json"),
        Reply::json_str(NOT_FOUND).with_status(404),
    )
    .await;
    // Details for another series than the one asked for.
    mount_series(&mock, "FXEURCAD", SERIES_FXUSDCAD).await;
    let adapter = BocAdapter::new(mock.base_url());
    let found = adapter
        .discover_names(&test_ctx(), &names(&["NOPE123", "V39079", "FXEURCAD"]))
        .await
        .unwrap();
    let ids: Vec<_> = found.iter().map(|s| s.external_id.as_str()).collect();
    assert_eq!(ids, ["V39079"]);
    assert_eq!(adapter.curated_missing(), ["NOPE123", "FXEURCAD"]);
}

/// A server error fails discovery (retried by the queue) instead of returning a partial catalog
/// the worker would retire series against; rate limits abort at once.
#[tokio::test]
async fn discovery_fails_on_transient_and_rate_limit() {
    let mock = MockSource::start().await;
    mount_series(&mock, "V39079", SERIES_V39079).await;
    mock.mount(
        &Route::get("/series/FXUSDCAD/json"),
        Reply::text("boom").with_status(500),
    )
    .await;
    let adapter = BocAdapter::new(mock.base_url());
    let err = adapter
        .discover_names(&test_ctx(), &names(&["V39079", "FXUSDCAD"]))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "transient", "{err}");

    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/V39079/json"),
        Reply::status(429).retry_after(1),
    )
    .await;
    mount_series(&mock, "FXUSDCAD", SERIES_FXUSDCAD).await;
    let adapter = BocAdapter::new(mock.base_url());
    let err = adapter
        .discover_names(&test_ctx(), &names(&["V39079", "FXUSDCAD"]))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "rate_limited", "{err}");
    // Aborted before the second lookup (the first may have been retried in-process).
    assert!(mock
        .received_requests()
        .await
        .iter()
        .all(|r| r.url.path() == "/series/V39079/json"));
}

/// Every name failing fails the discovery with the last error.
#[tokio::test]
async fn discovery_with_no_series_found_fails() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/series/NOPE123/json"),
        Reply::json_str(NOT_FOUND).with_status(404),
    )
    .await;
    let err = BocAdapter::new(mock.base_url())
        .discover_names(&test_ctx(), &names(&["NOPE123"]))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "not_found", "{err}");
}

/// `discover()` looks up every name in the shipped list, one request each.
#[tokio::test]
async fn discover_reads_the_shipped_list() {
    let mock = MockSource::start().await;
    let want = crate::reference::boc_series().unwrap();
    for name in want {
        let body = json!({
            "seriesDetails": {
                "name": name,
                "label": format!("label {name}"),
                "description": "synthetic",
                "dateFormat": "daily",
                "metadata": {"frequency": "Business daily", "unit": "Percent"},
            }
        });
        mock.mount(
            &Route::get(format!("/series/{name}/json")),
            Reply::json(body),
        )
        .await;
    }
    let adapter = BocAdapter::new(mock.base_url());
    let found = assert_discover_ok(&adapter, &test_ctx(), &mock, want.len()).await;
    let ids: Vec<_> = found.iter().map(|s| s.external_id.clone()).collect();
    assert_eq!(ids, want);
    assert_eq!(mock.received_requests().await.len(), want.len());
    assert!(adapter.discovery_is_complete());
}

/// A fetch keeps Valet's value text exactly, dates each point by `d`, and carries the series'
/// metadata from the same response.
#[tokio::test]
async fn fetch_parses_recorded_exchange_rates() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/observations/FXUSDCAD/json"),
        Reply::json_str(OBS_FXUSDCAD),
    )
    .await;
    let fetched = BocAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "FXUSDCAD", None)
        .await
        .unwrap();
    let meta = fetched.metadata.unwrap();
    assert_eq!(meta.title, "USD/CAD");
    assert_eq!(
        meta.units.as_deref(),
        Some("Canadian dollars per US dollar")
    );
    assert_eq!(meta.frequency.as_deref(), Some("Business daily"));
    assert_eq!(fetched.points.len(), 9);
    let first = &fetched.points[0];
    assert_eq!(first.date, d("2026-10-19"));
    assert_eq!(first.value, Some(dec("1.4097")));
    let last = fetched.points.last().unwrap();
    assert_eq!(last.date, d("2026-10-29"));
    assert_eq!(last.value, Some(dec("1.3985")));
    assert!(fetched
        .points
        .iter()
        .all(|p| p.revision_date == p.date && p.is_original_release));
    assert_eq!(fetched.dataset, valet_dataset());
    assert_eq!(fetched.validators, None);
}

/// A monthly aggregate without Valet metadata: no units, frequency from `dateFormat`.
#[tokio::test]
async fn fetch_parses_recorded_monthly_aggregate() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/observations/V41552801/json"),
        Reply::json_str(OBS_V41552801),
    )
    .await;
    let fetched = BocAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "V41552801", None)
        .await
        .unwrap();
    let meta = fetched.metadata.unwrap();
    assert_eq!(meta.units, None);
    assert_eq!(meta.frequency.as_deref(), Some("Monthly"));
    let dates: Vec<_> = fetched.points.iter().map(|p| p.date).collect();
    assert_eq!(
        dates,
        [
            "2026-03-01",
            "2026-04-01",
            "2026-05-01",
            "2026-06-01",
            "2026-07-01",
            "2026-08-01"
        ]
        .map(d)
    );
    assert_eq!(fetched.points[5].value, Some(dec("5346672")));
}

/// `since` is sent as `start_date`; without it no date parameter is sent.
#[tokio::test]
async fn fetch_sends_since_as_start_date() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/observations/FXUSDCAD/json").query("start_date", "2025-10-30"),
        Reply::json_str(OBS_FXUSDCAD),
    )
    .await;
    let adapter = BocAdapter::new(mock.base_url());
    adapter
        .fetch_series(&test_ctx(), "FXUSDCAD", Some(d("2025-10-30")))
        .await
        .unwrap();

    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/observations/FXUSDCAD/json"),
        Reply::json_str(OBS_FXUSDCAD),
    )
    .await;
    BocAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "FXUSDCAD", None)
        .await
        .unwrap();
    let requests = mock.received_requests().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.query(), None);
}

fn observations(rows: serde_json::Value) -> serde_json::Value {
    json!({
        "seriesDetail": {
            "X1": {"label": "X", "description": "synthetic", "dateFormat": "daily",
                   "metadata": {"unit": "Percent", "frequency": "Business daily"}}
        },
        "observations": rows,
    })
}

async fn fetch_body(body: serde_json::Value) -> Result<FetchedSeries, CrawlError> {
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/observations/X1/json"), Reply::json(body))
        .await;
    BocAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "X1", None)
        .await
}

/// An observation without the series' cell, or with a null or empty `v`, is a missing value.
#[tokio::test]
async fn missing_values_are_stored_as_none() {
    let fetched = fetch_body(observations(json!([
        {"d": "2026-01-01", "X1": {"v": "1.50"}},
        {"d": "2026-01-02"},
        {"d": "2026-01-03", "X1": {}},
        {"d": "2026-01-04", "X1": {"v": ""}},
        {"d": "2026-01-05", "X1": {"v": null}},
    ])))
    .await
    .unwrap();
    let values: Vec<_> = fetched.points.iter().map(|p| p.value.clone()).collect();
    assert_eq!(values, [Some(dec("1.50")), None, None, None, None]);
    // The written scale is kept.
    assert_eq!(
        fetched.points[0].value.as_ref().unwrap().to_string(),
        "1.50"
    );
}

/// Malformed observations are `Parse` errors, never guessed at.
#[tokio::test]
async fn malformed_observations_are_parse_errors() {
    for rows in [
        json!([{"X1": {"v": "1"}}]),
        json!([{"d": "2026Q1", "X1": {"v": "1"}}]),
        json!([{"d": "2026-01-01", "X1": {"v": 1.5}}]),
        json!([{"d": "2026-01-01", "X1": {"v": "1,5"}}]),
    ] {
        let err = fetch_body(observations(rows.clone())).await.unwrap_err();
        assert_eq!(err.kind(), "parse", "{rows}: {err}");
    }
    // No details for the requested series, or no label.
    let err = fetch_body(json!({"seriesDetail": {}, "observations": []}))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "parse", "{err}");
    let err = fetch_body(json!({"seriesDetail": {"X1": {"label": " "}}, "observations": []}))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "parse", "{err}");
}

/// A name that isn't a Valet series name is `NotFound` without any request.
#[tokio::test]
async fn invalid_names_are_not_requested() {
    let mock = MockSource::start().await;
    let adapter = BocAdapter::new(mock.base_url());
    for name in ["", "../lists/series", "FX USD", "A?b=c"] {
        let err = adapter
            .fetch_series(&test_ctx(), name, None)
            .await
            .unwrap_err();
        assert_eq!(err.kind(), "not_found", "{name:?}: {err}");
    }
    let found = adapter
        .discover_names(&test_ctx(), &names(&["V39079", "bad/name"]))
        .await;
    assert!(found.is_err());
    assert!(mock.received_requests().await.len() <= 1);
}

#[test]
fn capitalises_date_formats() {
    assert_eq!(capitalise("monthly"), "Monthly");
    assert_eq!(capitalise("daily"), "Daily");
    assert_eq!(capitalise(""), "");
}
