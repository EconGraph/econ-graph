// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use std::str::FromStr;

use super::*;
use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

const INFO_EXPORTS: &str = include_str!("../../../tests/fixtures/wto/indicators_its_mtv_ax.json");
const DATA_EXPORTS: &str = include_str!("../../../tests/fixtures/wto/data_its_mtv_ax.json");
const INFO_TARIFF: &str = include_str!("../../../tests/fixtures/wto/indicators_tp_a_0010.json");
const DATA_TARIFF: &str = include_str!("../../../tests/fixtures/wto/data_tp_a_0010.json");

fn info_route(indicator: &str) -> Route {
    Route::get("/indicators")
        .query("i", indicator)
        .query("lang", "1")
        .query(KEY_PARAM, TEST_API_KEY)
}

/// The `/data` request for a listed indicator, with every parameter the adapter must send.
fn data_route(indicator: &str) -> Route {
    data_route_for(indicator, "all")
}

/// [`data_route`] for the economies `r` names.
fn data_route_for(indicator: &str, r: &str) -> Route {
    let listed = reference::wto_indicators()
        .unwrap()
        .iter()
        .find(|i| i.id == indicator)
        .unwrap();
    let mut route = Route::get("/data")
        .query("i", indicator)
        .query("r", r)
        .query("ps", "all")
        .query("fmt", "json")
        .query("mode", "full")
        .query("head", "M")
        .query("lang", "1")
        .query("max", "1000000")
        .query(KEY_PARAM, TEST_API_KEY);
    if !listed.product.is_empty() {
        route = route.query("pc", listed.product.clone());
    }
    if !listed.partner.is_empty() {
        route = route.query("p", listed.partner.clone());
    }
    route
}

/// Mounts the exports and tariff fixtures; every other `/indicators` request answers `[]`.
async fn mount_fixtures(mock: &MockSource) {
    mock.mount(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS))
        .await;
    mock.mount(&data_route("ITS_MTV_AX"), Reply::json_str(DATA_EXPORTS))
        .await;
    mock.mount(&info_route("TP_A_0010"), Reply::json_str(INFO_TARIFF))
        .await;
    mock.mount(&data_route("TP_A_0010"), Reply::json_str(DATA_TARIFF))
        .await;
    mock.mount(&Route::get("/indicators"), Reply::json_str("[]"))
        .await;
}

fn date(y: i32, m: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, 1).unwrap()
}

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn exports() -> &'static WtoIndicator {
    reference::wto_indicators()
        .unwrap()
        .iter()
        .find(|i| i.id == "ITS_MTV_AX")
        .unwrap()
}

#[test]
fn constructor_policy_and_datasets() {
    assert_eq!(WtoAdapter::default().base_url, DEFAULT_BASE_URL);
    assert_eq!(WtoAdapter::new("http://x/").base_url, "http://x");
    let a = WtoAdapter::default();
    assert_eq!(a.id(), SourceId::Wto);
    assert_eq!(a.datasets(), [DATASET]);
    let p = a.policy();
    assert_eq!(p.max_batch, MAX_BATCH);
    assert!(p.needs_api_key);
    assert_eq!(p.requests_per_second, 1.0);
    assert_eq!(p.max_concurrency, 1);
    assert!(a.discovery_is_complete());
    assert!(areas().unwrap().all().len() < MAX_BATCH);
}

#[test]
fn ids_and_batch_keys() {
    assert_eq!(
        parse_id("timeseries/ITS_MTV_AX.USA"),
        Some(("ITS_MTV_AX", "USA"))
    );
    for bad in [
        "ITS_MTV_AX",
        "timeseries/USA",
        "timeseries/.USA",
        "timeseries/X.",
        "wdi/X.USA",
    ] {
        assert_eq!(parse_id(bad), None, "{bad}");
    }
    let a = WtoAdapter::default();
    assert_eq!(
        a.batch_key("timeseries/TP_A_0010.JPN").as_deref(),
        Some("TP_A_0010")
    );
    assert_eq!(a.batch_key("ITS_MTV_AX"), None);
    let (id, ds) = series_id(dataset_def().unwrap(), "ITS_MTV_AX", "USA").unwrap();
    assert_eq!(id, "timeseries/ITS_MTV_AX.USA");
    assert_eq!(parse_id(&id), Some(("ITS_MTV_AX", "USA")));
    assert_eq!(ds.dimensions.0["area"], "USA");
    assert_eq!(ds.dimensions.0["indicator"], "ITS_MTV_AX");
}

#[test]
fn periods() {
    assert_eq!(parse_period("2023", Some("A")), Some(date(2023, 1)));
    assert_eq!(parse_period("2023", None), Some(date(2023, 1)));
    assert_eq!(parse_period(" 2023 ", Some("")), Some(date(2023, 1)));
    assert_eq!(parse_period("2023", Some("Q3")), Some(date(2023, 7)));
    assert_eq!(parse_period("2023", Some("M05")), Some(date(2023, 5)));
    assert_eq!(parse_period("2023", Some("S2")), Some(date(2023, 7)));
    for (y, p) in [
        ("", Some("A")),
        ("23", Some("A")),
        ("2023", Some("Q5")),
        ("2023", Some("M13")),
        ("2023", Some("M5")),
        ("2023", Some("S3")),
        ("2023", Some("W01")),
    ] {
        assert_eq!(parse_period(y, p), None, "{y:?} {p:?}");
    }
}

#[test]
fn economy_codes_and_lookup() {
    assert_eq!(economy_code("4"), "004");
    assert_eq!(economy_code("0"), "000");
    assert_eq!(economy_code("840"), "840");
    assert_eq!(economy_code("EUT"), "EUT");
    assert_eq!(economy_code(""), "");
    let areas = areas().unwrap();
    assert_eq!(lookup(areas, "840").unwrap().key, "USA");
    assert_eq!(lookup(areas, "004").unwrap().key, "AFG");
    assert_eq!(lookup(areas, "000").unwrap().key, "WLD");
    assert_eq!(lookup(areas, "918").unwrap().key, "EUU");
    assert!(lookup(areas, "975").is_none());
    assert!(lookup(areas, "EUT").is_none());
    assert!(lookup(areas, "").is_none());
}

#[test]
fn parses_the_data_fixture() {
    let rows = parse_rows("t", DATA_EXPORTS).unwrap();
    // 11 rows, one with a null value.
    assert_eq!(rows.len(), 10);
    let afg = rows.iter().find(|r| r.economy == "004").unwrap();
    assert_eq!(afg.date, date(2024, 1));
    assert_eq!(afg.value, dec("1900"));
    assert_eq!(afg.product, "TO");
    assert_eq!(afg.partner, "000");
    let usa_2023 = rows
        .iter()
        .find(|r| r.economy == "840" && r.date == date(2023, 1))
        .unwrap();
    assert_eq!(usa_2023.value, dec("2010000.5"));
    assert_eq!(usa_2023.economy_name, "United States of America");
    assert_eq!(usa_2023.product_name, "Total merchandise");
    assert_eq!(usa_2023.partner_name, "World");
}

#[test]
fn parse_rows_edge_cases() {
    // A 204 has no body.
    assert!(parse_rows("t", "").unwrap().is_empty());
    assert!(parse_rows("t", r#"{"Dataset": []}"#).unwrap().is_empty());
    // The schema's camelCase spelling reads the same.
    let rows = parse_rows(
        "t",
        r#"{"dataset": [{"reportingEconomyCode": "392", "periodCode": "Q2", "year": 2024, "value": 1.25}]}"#,
    )
    .unwrap();
    assert_eq!(rows[0].economy, "392");
    assert_eq!(rows[0].date, date(2024, 4));
    assert_eq!(rows[0].value, dec("1.25"));
    // Values keep every digit.
    let rows = parse_rows(
        "t",
        r#"{"Dataset": [{"ReportingEconomyCode": "840", "Year": 2024, "Value": 123456789012345678.25}]}"#,
    )
    .unwrap();
    assert_eq!(rows[0].value, dec("123456789012345678.25"));
    // An empty string value is no observation.
    assert!(parse_rows(
        "t",
        r#"{"Dataset": [{"ReportingEconomyCode": "840", "Year": 2024, "Value": ""}]}"#
    )
    .unwrap()
    .is_empty());
    for (body, needle) in [
        ("[]", "no Dataset"),
        ("{}", "no Dataset"),
        (r#"{"Dataset": null}"#, "no Dataset"),
        ("not json", "invalid response"),
        (
            r#"{"Dataset": [{"ReportingEconomyCode": "840", "Year": 2024, "PeriodCode": "X", "Value": 1}]}"#,
            "bad period",
        ),
        (
            r#"{"Dataset": [{"ReportingEconomyCode": "840", "Value": 1}]}"#,
            "bad period",
        ),
        (
            r#"{"Dataset": [{"ReportingEconomyCode": "840", "Year": 2024, "Value": "abc"}]}"#,
            "bad value",
        ),
        (
            r#"{"Dataset": [{"ReportingEconomyCode": [1], "Year": 2024, "Value": 1}]}"#,
            "bad data row",
        ),
    ] {
        let e = parse_rows("t", body).unwrap_err();
        assert!(matches!(e, CrawlError::Parse(_)), "{body}: {e:?}");
        assert!(e.to_string().contains(needle), "{body}: {e}");
    }
}

#[test]
fn resolve_filters_groups_and_skips_unknown_economies() {
    let rows = parse_rows("t", DATA_EXPORTS).unwrap();
    let resolved = resolve(exports(), rows, areas().unwrap()).unwrap();
    let keys: Vec<&str> = resolved.iter().map(|(a, _)| a.key.as_str()).collect();
    assert_eq!(keys, ["AFG", "CHN", "EUU", "USA", "WLD"]);
    let usa = &resolved.iter().find(|(a, _)| a.key == "USA").unwrap().1;
    // The AG row is another product; 2021 is null.
    assert_eq!(
        usa.points,
        BTreeMap::from([
            (date(2022, 1), dec("2000000")),
            (date(2023, 1), dec("2010000.5")),
            (date(2024, 1), dec("2020000")),
        ])
    );
    let wld = &resolved.iter().find(|(a, _)| a.key == "WLD").unwrap().1;
    assert_eq!(wld.economy_name, "World");
}

#[test]
fn resolve_rejects_two_values_for_one_period() {
    let body = r#"{"Dataset": [
        {"ReportingEconomyCode": "840", "Year": 2024, "ProductOrSectorCode": "X1", "Value": 1},
        {"ReportingEconomyCode": "840", "Year": 2024, "ProductOrSectorCode": "X2", "Value": 2}
    ]}"#;
    let unpinned = WtoIndicator {
        id: "T".into(),
        product: String::new(),
        partner: String::new(),
    };
    let e = resolve(&unpinned, parse_rows("t", body).unwrap(), areas().unwrap()).unwrap_err();
    assert!(matches!(e, CrawlError::Parse(_)), "{e:?}");
    assert!(e.to_string().contains("wto_indicators.csv"), "{e}");
    // Pinning the product keeps one of them; a repeated identical value is fine.
    let pinned = WtoIndicator {
        product: "X2".into(),
        ..unpinned.clone()
    };
    let r = resolve(&pinned, parse_rows("t", body).unwrap(), areas().unwrap()).unwrap();
    assert_eq!(r[0].1.points[&date(2024, 1)], dec("2"));
    let same = r#"{"Dataset": [
        {"ReportingEconomyCode": "840", "Year": 2024, "Value": 1},
        {"ReportingEconomyCode": "840", "Year": 2024, "Value": 1.0}
    ]}"#;
    assert!(resolve(&unpinned, parse_rows("t", same).unwrap(), areas().unwrap()).is_ok());
}

#[test]
fn quota_exhaustion_is_rate_limited() {
    let e = classify(CrawlError::Auth(
        "HTTP 403: WTO GET ...: Out of call volume quota. Quota will be replenished in 00:10:00."
            .into(),
    ));
    let secs = |s| Some(std::time::Duration::from_secs(s));
    assert_eq!(
        e,
        CrawlError::RateLimited {
            retry_after: secs(600)
        }
    );
    // Days, hours, minutes and seconds.
    let e = classify(CrawlError::Auth(
        "HTTP 403: Out of call volume quota. Quota will be replenished in 1.02:03:04.".into(),
    ));
    assert_eq!(
        e,
        CrawlError::RateLimited {
            retry_after: secs(((24 + 2) * 60 + 3) * 60 + 4)
        }
    );
    // No usable time: wait an hour. Almost replenished: at least a minute.
    for message in [
        "HTTP 403: Out of call volume quota.",
        "HTTP 403: Out of call volume quota. Quota will be replenished in soon.",
        "HTTP 403: Out of call volume quota. Quota will be replenished in 00:99:00.",
    ] {
        assert_eq!(
            classify(CrawlError::Auth(message.into())),
            CrawlError::RateLimited {
                retry_after: secs(3600)
            },
            "{message}"
        );
    }
    assert_eq!(
        classify(CrawlError::Auth(
            "HTTP 403: Out of call volume quota. Quota will be replenished in 00:00:05.".into()
        )),
        CrawlError::RateLimited {
            retry_after: secs(60)
        }
    );
    let e = classify(CrawlError::Auth("HTTP 401: Access denied".into()));
    assert!(matches!(e, CrawlError::Auth(_)));
}

/// A `/data` body that echoes the key (here in a row value) never puts it into a parse error,
/// which the worker logs and stores with the job.
#[tokio::test]
async fn parse_errors_from_the_data_body_never_carry_the_key() {
    let mock = MockSource::start().await;
    mock.mount(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS))
        .await;
    let body = format!(
        r#"{{"Dataset": [{{"ReportingEconomyCode": "840", "Year": 2024, "PeriodCode": "A", "Value": "{TEST_API_KEY}"}}]}}"#
    );
    mock.mount(&data_route_for("ITS_MTV_AX", "840"), Reply::json_str(body))
        .await;
    let e = WtoAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "timeseries/ITS_MTV_AX.USA", None)
        .await
        .unwrap_err();
    assert!(matches!(e, CrawlError::Parse(_)), "{e:?}");
    assert!(e.to_string().contains("bad value"), "{e}");
    assert!(!e.to_string().contains(TEST_API_KEY), "{e}");
    assert!(e.to_string().contains("REDACTED"), "{e}");
    // A row the parser rejects outright quotes nothing from it either.
    let e = redact_key(
        parse_rows(
            "t",
            &format!(r#"{{"Dataset": [{{"Year": ["{TEST_API_KEY}"]}}]}}"#),
        )
        .unwrap_err(),
        TEST_API_KEY,
    );
    assert!(!e.to_string().contains(TEST_API_KEY), "{e}");
}

#[test]
fn economy_codes_round_trip_to_areas() {
    let areas = areas().unwrap();
    for (key, code) in [
        ("USA", "840"),
        ("AFG", "004"),
        ("WLD", "000"),
        ("EUU", "918"),
    ] {
        let area = areas.by_key(key).unwrap();
        assert_eq!(wto_economy_code(area).as_deref(), Some(code), "{key}");
        assert_eq!(lookup(areas, code).unwrap().key, key);
    }
    // An aggregate the WTO adapter doesn't map has no code.
    assert_eq!(wto_economy_code(areas.by_key("EMU").unwrap()), None);
}

/// Serbia, Montenegro and Sudan have WTO codes that are not their ISO codes; historical entities
/// map to no area.
#[test]
fn wto_specific_country_codes() {
    let areas = areas().unwrap();
    for (code, key) in [("892", "SRB"), ("893", "MNE"), ("736", "SDN")] {
        assert_eq!(
            lookup(areas, code).map(|a| a.key.as_str()),
            Some(key),
            "{code}"
        );
        assert_eq!(
            wto_economy_code(areas.by_key(key).unwrap()).as_deref(),
            Some(code),
            "{key}"
        );
    }
    for historical in ["280", "891", "810", "530"] {
        assert!(lookup(areas, historical).is_none(), "{historical}");
    }
    // The rows resolve: a Serbian row becomes a SRB series.
    let body = r#"{"Dataset": [
        {"ReportingEconomyCode": "892", "Year": 2024, "Value": 1},
        {"ReportingEconomyCode": "891", "Year": 2000, "Value": 2}
    ]}"#;
    let unpinned = WtoIndicator {
        id: "T".into(),
        product: String::new(),
        partner: String::new(),
    };
    let resolved = resolve(&unpinned, parse_rows("t", body).unwrap(), areas).unwrap();
    let keys: Vec<&str> = resolved.iter().map(|(a, _)| a.key.as_str()).collect();
    assert_eq!(keys, ["SRB"]);
}

/// An indicator the WTO lists but answers with no data (a 204, or an empty `Dataset`) is skipped
/// and reported, so the worker retires none of its series.
#[tokio::test]
async fn a_listed_indicator_without_data_is_curated_missing() {
    for reply in [Reply::status(204), Reply::json_str(r#"{"Dataset": []}"#)] {
        let mock = MockSource::start().await;
        mock.mount(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS))
            .await;
        mock.mount(&data_route("ITS_MTV_AX"), reply).await;
        mock.mount(&info_route("TP_A_0010"), Reply::json_str(INFO_TARIFF))
            .await;
        mock.mount(&data_route("TP_A_0010"), Reply::json_str(DATA_TARIFF))
            .await;
        mock.mount(&Route::get("/indicators"), Reply::json_str("[]"))
            .await;
        let a = WtoAdapter::new(mock.base_url());
        let found = a.discover(&test_ctx()).await.unwrap();
        assert!(found
            .iter()
            .all(|s| s.external_id.starts_with("timeseries/TP_A_0010.")));
        assert!(!found.is_empty());
        assert!(a.curated_missing().contains(&"ITS_MTV_AX".to_string()));
    }
}

#[test]
fn metadata_comes_from_the_indicator_and_names_the_selection() {
    let info: Vec<IndicatorInfo> = serde_json::from_str(INFO_EXPORTS).unwrap();
    let rows = parse_rows("t", DATA_EXPORTS).unwrap();
    let resolved = resolve(exports(), rows, areas().unwrap()).unwrap();
    let (usa, series) = resolved.iter().find(|(a, _)| a.key == "USA").unwrap();
    let meta = series_metadata(&info[0], exports(), usa, series);
    assert_eq!(
        meta.title,
        "Merchandise exports by product group - annual: United States"
    );
    assert_eq!(meta.units.as_deref(), Some("Million US dollar"));
    assert_eq!(meta.frequency.as_deref(), Some("Annual"));
    let d = meta.description.unwrap();
    assert!(
        d.starts_with("Value of merchandise exports by product group"),
        "{d}"
    );
    assert!(d.contains("Product/sector: Total merchandise (TO)."), "{d}");
    assert!(d.contains("Partner: World (000)."), "{d}");
    assert!(
        d.ends_with("Source: World Trade Organization, WTO Stats."),
        "{d}"
    );
    // An aggregate is named by the WTO.
    let (eu, series) = resolved.iter().find(|(a, _)| a.key == "EUU").unwrap();
    let meta = series_metadata(&info[0], exports(), eu, series);
    assert!(meta.title.ends_with(": European Union"), "{}", meta.title);
    // No description: the name stands in.
    let tariff: Vec<IndicatorInfo> = serde_json::from_str(INFO_TARIFF).unwrap();
    let meta = series_metadata(&tariff[0], exports(), usa, series);
    assert!(meta
        .description
        .unwrap()
        .starts_with("MFN - Simple average ad valorem duty. Reporting economy"),);
}

#[tokio::test]
async fn without_a_key_nothing_is_requested() {
    let mock = MockSource::start().await;
    let mut ctx = test_ctx();
    ctx.keys.wto = None;
    let a = WtoAdapter::new(mock.base_url());
    let e = a.discover(&ctx).await.unwrap_err();
    assert!(matches!(e, CrawlError::Auth(_)), "{e:?}");
    assert!(e.to_string().contains("WTO_API_KEY not set"), "{e}");
    let e = a
        .fetch_series(&ctx, "timeseries/ITS_MTV_AX.USA", None)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("WTO_API_KEY not set"), "{e}");
    assert!(mock.received_requests().await.is_empty());
}

#[tokio::test]
async fn discover_lists_economies_with_values_and_reports_unlisted_indicators() {
    let mock = MockSource::start().await;
    mount_fixtures(&mock).await;
    let a = WtoAdapter::new(mock.base_url());
    let found = a.discover(&test_ctx()).await.unwrap();
    let ids: Vec<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "timeseries/ITS_MTV_AX.AFG",
            "timeseries/ITS_MTV_AX.CHN",
            "timeseries/ITS_MTV_AX.EUU",
            "timeseries/ITS_MTV_AX.USA",
            "timeseries/ITS_MTV_AX.WLD",
            "timeseries/TP_A_0010.EUU",
            "timeseries/TP_A_0010.JPN",
            "timeseries/TP_A_0010.USA",
        ]
    );
    let usa = found
        .iter()
        .find(|s| s.external_id == "timeseries/TP_A_0010.USA")
        .unwrap();
    assert_eq!(
        usa.title,
        "MFN - Simple average ad valorem duty: United States"
    );
    assert_eq!(usa.units.as_deref(), Some("Percent"));
    assert_eq!(usa.frequency.as_deref(), Some("Annual"));
    assert_eq!(usa.data_url.as_deref(), Some(WEB_URL));
    // The other listed indicators answered `[]`: skipped, and reported so nothing is retired.
    let mut missing = a.curated_missing();
    missing.sort();
    assert_eq!(
        missing,
        ["ITS_CS_AM6", "ITS_CS_AX6", "ITS_MTV_AM", "TP_A_0030"]
    );
    // No data request for an indicator the WTO doesn't list.
    for r in mock.received_requests().await {
        if r.url.path() == "/data" {
            let i = r
                .url
                .query_pairs()
                .find(|(k, _)| k == "i")
                .map(|(_, v)| v.into_owned());
            assert!(
                matches!(i.as_deref(), Some("ITS_MTV_AX" | "TP_A_0010")),
                "{i:?}"
            );
        }
    }
}

#[tokio::test]
async fn discover_fails_on_a_failed_request() {
    let mock = MockSource::start().await;
    mock.mount(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS))
        .await;
    mock.mount(&data_route("ITS_MTV_AX"), Reply::status(500))
        .await;
    let e = WtoAdapter::new(mock.base_url())
        .discover(&test_ctx())
        .await
        .unwrap_err();
    assert!(matches!(e, CrawlError::Transient(_)), "{e:?}");
}

#[tokio::test]
async fn an_indicator_code_the_api_rejects_is_unlisted() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/indicators"),
        Reply::json_str(r#"{"title": "One or more validation errors occurred.", "status": 400}"#)
            .with_status(400),
    )
    .await;
    let a = WtoAdapter::new(mock.base_url());
    assert!(a.discover(&test_ctx()).await.unwrap().is_empty());
    assert_eq!(
        a.curated_missing().len(),
        reference::wto_indicators().unwrap().len()
    );
}

#[tokio::test]
async fn fetch_batch_serves_every_requested_economy_from_one_request() {
    let mock = MockSource::start().await;
    // Once for the batch of five ids (every economy), once for the lone `fetch_series` at the
    // end (France only, by its WTO code).
    mock.mount_expect(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS), 2)
        .await;
    mock.mount_expect(&data_route("ITS_MTV_AX"), Reply::json_str(DATA_EXPORTS), 1)
        .await;
    mock.mount_expect(
        &data_route_for("ITS_MTV_AX", "250"),
        Reply::json_str(DATA_EXPORTS),
        1,
    )
    .await;
    let a = WtoAdapter::new(mock.base_url());
    let requested = ids(&[
        "timeseries/ITS_MTV_AX.USA",
        "timeseries/ITS_MTV_AX.WLD",
        "timeseries/ITS_MTV_AX.FRA",
        "timeseries/NOT_LISTED.USA",
        "wdi/NY.GDP.PCAP.CD.USA",
    ]);
    let out = a.fetch_batch(&test_ctx(), &requested, None).await.unwrap();
    let usa = out["timeseries/ITS_MTV_AX.USA"].as_ref().unwrap();
    assert_eq!(
        usa.points
            .iter()
            .map(|p| (p.date, p.value.clone().unwrap()))
            .collect::<Vec<_>>(),
        [
            (date(2022, 1), dec("2000000")),
            (date(2023, 1), dec("2010000.5")),
            (date(2024, 1), dec("2020000")),
        ]
    );
    for p in &usa.points {
        assert_eq!(p.revision_date, p.date);
        assert!(p.is_original_release);
    }
    let meta = usa.metadata.as_ref().unwrap();
    assert_eq!(meta.units.as_deref(), Some("Million US dollar"));
    assert_eq!(usa.dataset.dimensions.0["indicator"], "ITS_MTV_AX");
    assert!(usa.validators.is_none());
    assert_eq!(
        out["timeseries/ITS_MTV_AX.WLD"]
            .as_ref()
            .unwrap()
            .points
            .len(),
        1
    );
    // France has no rows: left out, so it fails as NotFound.
    assert!(!out.contains_key("timeseries/ITS_MTV_AX.FRA"));
    for id in ["timeseries/NOT_LISTED.USA", "wdi/NY.GDP.PCAP.CD.USA"] {
        assert!(matches!(out[id], Err(CrawlError::NotFound(_))), "{id}");
    }
    let e = a
        .fetch_series(&test_ctx(), "timeseries/ITS_MTV_AX.FRA", None)
        .await
        .unwrap_err();
    assert!(matches!(e, CrawlError::NotFound(_)), "{e:?}");
}

#[tokio::test]
async fn no_content_is_no_series() {
    let mock = MockSource::start().await;
    mock.mount(&info_route("TP_A_0010"), Reply::json_str(INFO_TARIFF))
        .await;
    // A lone series asks for its own economy (the United States, 840), not every one.
    mock.mount(&data_route_for("TP_A_0010", "840"), Reply::status(204))
        .await;
    let e = WtoAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "timeseries/TP_A_0010.USA", None)
        .await
        .unwrap_err();
    assert!(matches!(e, CrawlError::NotFound(_)), "{e:?}");
}

#[tokio::test]
async fn request_failures_fail_the_indicator_and_never_show_the_key() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/indicators"),
        Reply::json_str(format!(
            r#"{{"statusCode": 401, "message": "Access denied due to invalid subscription key {TEST_API_KEY}."}}"#
        ))
        .with_status(401),
    )
    .await;
    let a = WtoAdapter::new(mock.base_url());
    let out = a
        .fetch_batch(&test_ctx(), &ids(&["timeseries/ITS_MTV_AX.USA"]), None)
        .await
        .unwrap();
    let e = out["timeseries/ITS_MTV_AX.USA"].clone().unwrap_err();
    assert!(matches!(e, CrawlError::Auth(_)), "{e:?}");
    assert!(!e.to_string().contains(TEST_API_KEY), "{e}");

    mock.reset().await;
    mock.mount(
        &Route::get("/indicators"),
        Reply::json_str(r#"{"statusCode": 403, "message": "Out of call volume quota. Quota will be replenished in 00:59:00."}"#)
            .with_status(403),
    )
    .await;
    let e = a
        .fetch_series(&test_ctx(), "timeseries/ITS_MTV_AX.USA", None)
        .await
        .unwrap_err();
    assert_eq!(
        e,
        CrawlError::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(59 * 60))
        }
    );
}

mod contract {
    use super::{data_route, info_route, DATA_EXPORTS, INFO_EXPORTS};
    use crate::sources::wto::WtoAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| WtoAdapter::new(base_url),
        external_id: "timeseries/ITS_MTV_AX.USA",
        route: Route::get("/data"),
        ok_reply: Reply::json_str(DATA_EXPORTS),
        expect_points: 3,
        setup: |m| {
            m.mount(&info_route("ITS_MTV_AX"), Reply::json_str(INFO_EXPORTS)).await;
            m.mount(&Route::get("/indicators"), Reply::json_str("[]")).await;
        },
        discover: {
            route: data_route("ITS_MTV_AX"),
            reply: Reply::json_str(DATA_EXPORTS),
            min_series: 5,
        },
    }
}
