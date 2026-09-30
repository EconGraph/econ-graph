// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use super::*;
use crate::testkit::{test_ctx, MockSource, Reply, Route};

const GDP_PCAP: &str = include_str!("../../../tests/fixtures/world_bank/gdp_per_capita.json");
const POPULATION: &str = include_str!("../../../tests/fixtures/world_bank/population.json");
const INFLATION: &str = include_str!("../../../tests/fixtures/world_bank/inflation.json");
const INVALID: &str = include_str!("../../../tests/fixtures/world_bank/error_invalid_value.json");

/// The recorded-shape fixtures by indicator.
const FIXTURES: [(&str, &str); 3] = [
    ("NY.GDP.PCAP.CD", GDP_PCAP),
    ("SP.POP.TOTL", POPULATION),
    ("FP.CPI.TOTL.ZG", INFLATION),
];

fn route(indicator: &str) -> Route {
    Route::get(format!("/country/all/indicator/{indicator}"))
        .query("format", "json")
        .query("per_page", "20000")
        .query("page", "1")
}

fn date(y: i32, m: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, 1).unwrap()
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Every other request answers with the API's in-body "Invalid value" error.
async fn mount_invalid_fallback(mock: &MockSource) {
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::method("GET")).respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_raw(INVALID.as_bytes().to_vec(), "application/json"),
            ),
        )
        .await;
}

#[test]
fn constructor_policy_and_datasets() {
    assert_eq!(WorldBankAdapter::default().base_url, DEFAULT_BASE_URL);
    assert_eq!(WorldBankAdapter::new("http://x/").base_url, "http://x");
    let a = WorldBankAdapter::default();
    assert_eq!(a.id(), SourceId::WorldBank);
    assert_eq!(a.datasets(), [DATASET]);
    assert_eq!(a.policy().max_batch, MAX_BATCH);
    assert_eq!(
        a.policy().requests_per_second,
        SourcePolicy::default_for(SourceId::WorldBank).requests_per_second
    );
    // Every area of an indicator fits one batch.
    assert!(areas().unwrap().all().len() < MAX_BATCH);
}

#[test]
fn ids_and_batch_keys() {
    assert_eq!(
        parse_id("wdi/NY.GDP.PCAP.CD.USA"),
        Some(("NY.GDP.PCAP.CD", "USA"))
    );
    for bad in ["NY.GDP.PCAP.CD", "wdi/USA", "wdi/.USA", "wdi/X.", "other/X.USA", "wdiX/A.B"] {
        assert_eq!(parse_id(bad), None, "{bad}");
    }
    let a = WorldBankAdapter::default();
    assert_eq!(
        a.batch_key("wdi/SP.POP.TOTL.EMU").as_deref(),
        Some("SP.POP.TOTL")
    );
    assert_eq!(a.batch_key("NY.GDP.MKTP.CD"), None);
    // Ids come from the dataset definition, in declared order.
    let (id, ds) = series_id(wdi_def().unwrap(), "SP.POP.TOTL", "FRA").unwrap();
    assert_eq!(id, "wdi/SP.POP.TOTL.FRA");
    assert_eq!(parse_id(&id), Some(("SP.POP.TOTL", "FRA")));
    assert_eq!(ds.dimensions.0["area"], "FRA");
}

#[test]
fn periods() {
    assert_eq!(parse_period("2023"), Some((date(2023, 1), Frequency::Annual)));
    assert_eq!(
        parse_period("2023Q2"),
        Some((date(2023, 4), Frequency::Quarterly))
    );
    assert_eq!(
        parse_period("2023M05"),
        Some((date(2023, 5), Frequency::Monthly))
    );
    for bad in ["", "23", "2023Q5", "2023M13", "2023-01", "abcd"] {
        assert_eq!(parse_period(bad), None, "{bad}");
    }
}

/// The indicator list and the dataset file's indicator codes agree.
#[test]
fn dataset_file_lists_every_indicator() {
    let def = wdi_def().unwrap();
    let codes = def
        .dimensions
        .iter()
        .find(|d| d.name == "indicator")
        .unwrap()
        .codes
        .as_ref()
        .unwrap();
    let list = reference::wdi_indicators().unwrap();
    let from_list: BTreeMap<&str, &str> = list
        .iter()
        .map(|i| (i.id.as_str(), i.name.as_str()))
        .collect();
    let from_file: BTreeMap<&str, &str> = codes
        .iter()
        .map(|c| (c.code.as_str(), c.label.as_str()))
        .collect();
    assert_eq!(from_list, from_file);
    assert_eq!(
        def.dimension_names().collect::<Vec<_>>(),
        ["indicator", "area"]
    );
    assert_eq!(def.dimensions[1].codelist.as_deref(), Some("countries"));
}

#[test]
fn area_lookup() {
    let a = areas().unwrap();
    assert_eq!(lookup(a, "USA", "US").unwrap().key, "USA");
    assert_eq!(lookup(a, "EMU", "XC").unwrap().key, "EMU");
    assert_eq!(lookup(a, "", "DE").unwrap().key, "DEU");
    assert_eq!(lookup(a, "XKX", "XK").unwrap().key, "XKX");
    // Unknown aggregates are not retried as ISO alpha-2.
    assert!(lookup(a, "AFE", "ZH").is_none());
    assert!(lookup(a, "CHI", "JG").is_none());
}

/// A country whose `countryiso3code` is empty on some rows and set on others (both real cases in
/// the wild) must not lose the rows under the code that resolves second: they merge into one
/// series instead of the second raw code silently overwriting nothing (`Entry::Vacant` only fires
/// once per area).
#[test]
fn resolve_merges_rows_that_reach_the_same_area_by_two_raw_codes() {
    let rows = parse_rows(
        "x",
        vec![
            // countryiso3code set: resolves via by_wb_code/by_iso3 first (sorts before "").
            serde_json::json!({"country": {"id": "US", "value": "United States"},
                "countryiso3code": "USA", "date": "2023", "value": 3}),
            // countryiso3code empty on this row: resolves via ISO2 fallback, same area.
            serde_json::json!({"country": {"id": "US", "value": "United States"},
                "countryiso3code": "", "date": "2022", "value": 2}),
            // A shared date: the row from the code processed later wins (matches `group`'s rule).
            serde_json::json!({"country": {"id": "US", "value": "United States"},
                "countryiso3code": "", "date": "2021", "value": 1}),
            serde_json::json!({"country": {"id": "US", "value": "United States"},
                "countryiso3code": "USA", "date": "2021", "value": 99}),
        ],
    )
    .unwrap();
    assert_eq!(rows.len(), 4);
    let data = IndicatorData::group("SP.POP.TOTL", date(2026, 7), rows);
    // Two distinct raw codes before merging.
    assert_eq!(data.by_code.len(), 2);
    let resolved = data.resolve(areas().unwrap());
    assert_eq!(resolved.len(), 1, "the two codes merge into one area");
    let (area, points) = &resolved[0];
    assert_eq!(area.key, "USA");
    assert_eq!(
        points.iter().map(|r| (r.date, &r.value)).collect::<Vec<_>>(),
        [
            (date(2021, 1), &BigDecimal::from(99)),
            (date(2022, 1), &BigDecimal::from(2)),
            (date(2023, 1), &BigDecimal::from(3)),
        ]
    );
}

#[tokio::test]
async fn fetch_batch_one_request_for_every_area() {
    let mock = MockSource::start().await;
    mock.mount_expect(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP), 1)
        .await;
    let requested = ids(&[
        "wdi/NY.GDP.PCAP.CD.USA",
        "wdi/NY.GDP.PCAP.CD.DEU",
        "wdi/NY.GDP.PCAP.CD.WLD",
        // All null: no series.
        "wdi/NY.GDP.PCAP.CD.ASM",
        // Not in the country table.
        "wdi/NY.GDP.PCAP.CD.CHI",
        // Not a listed indicator, and not a wdi id: no request.
        "wdi/XX.NOPE.USA",
        "NY.GDP.PCAP.CD",
    ]);
    let out = WorldBankAdapter::new(mock.base_url())
        .fetch_batch(&test_ctx(), &requested, Some(date(2023, 1)))
        .await
        .unwrap();
    mock.server().verify().await;

    let usa = out["wdi/NY.GDP.PCAP.CD.USA"].as_ref().unwrap();
    // `since` is ignored: the whole history comes in the one request.
    assert_eq!(
        usa.points.iter().map(|p| p.date).collect::<Vec<_>>(),
        [date(2021, 1), date(2022, 1), date(2023, 1)]
    );
    assert_eq!(
        usa.points[2].value,
        Some(BigDecimal::from_str("82769.4").unwrap())
    );
    let last_updated = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    assert!(usa
        .points
        .iter()
        .all(|p| p.revision_date == last_updated && p.is_original_release));
    let meta = usa.metadata.as_ref().unwrap();
    assert_eq!(meta.title, "GDP per capita (current US$): United States");
    assert_eq!(meta.units.as_deref(), Some("current US$"));
    assert_eq!(meta.frequency.as_deref(), Some("Annual"));
    let ds = usa.dataset.as_ref().unwrap();
    assert_eq!(ds.code, DATASET);
    assert_eq!(
        serde_json::to_value(&ds.dimensions).unwrap(),
        serde_json::json!({"indicator": "NY.GDP.PCAP.CD", "area": "USA"})
    );

    let wld = out["wdi/NY.GDP.PCAP.CD.WLD"].as_ref().unwrap();
    assert_eq!(wld.metadata.as_ref().unwrap().title, "GDP per capita (current US$): World");
    assert!(out["wdi/NY.GDP.PCAP.CD.DEU"].is_ok());
    // No values, or an area outside the table: left out, so the worker fails them as NotFound.
    assert!(!out.contains_key("wdi/NY.GDP.PCAP.CD.ASM"));
    assert!(!out.contains_key("wdi/NY.GDP.PCAP.CD.CHI"));
    for id in ["wdi/XX.NOPE.USA", "NY.GDP.PCAP.CD"] {
        assert_eq!(out[id].as_ref().unwrap_err().kind(), "not_found", "{id}");
    }
    assert_eq!(out.len(), 5);
}

#[tokio::test]
async fn fetch_series_keeps_its_area_and_drops_nulls() {
    let mock = MockSource::start().await;
    mock.mount(&route("FP.CPI.TOTL.ZG"), Reply::json_str(INFLATION))
        .await;
    let a = WorldBankAdapter::new(mock.base_url());
    let ind = a
        .fetch_series(&test_ctx(), "wdi/FP.CPI.TOTL.ZG.IND", None)
        .await
        .unwrap();
    // 2023 is null.
    assert_eq!(
        ind.points.iter().map(|p| p.date).collect::<Vec<_>>(),
        [date(2021, 1), date(2022, 1)]
    );
    let e = a
        .fetch_series(&test_ctx(), "wdi/FP.CPI.TOTL.ZG.ASM", None)
        .await
        .unwrap_err();
    assert_eq!(e.kind(), "not_found");
}

#[tokio::test]
async fn fetch_batch_fails_every_id_of_a_failed_request() {
    for (reply, kind) in [
        (Reply::status(429).retry_after(60), "rate_limited"),
        (Reply::json_str(INVALID), "not_found"),
        (
            Reply::json(serde_json::json!([{"page": 1, "pages": 1}, []])),
            "parse",
        ),
    ] {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/country/all/indicator/SP.POP.TOTL"), reply)
            .await;
        let requested = ids(&["wdi/SP.POP.TOTL.USA", "wdi/SP.POP.TOTL.FRA"]);
        let out = WorldBankAdapter::new(mock.base_url())
            .fetch_batch(&test_ctx(), &requested, None)
            .await
            .unwrap();
        for id in &requested {
            assert_eq!(out[id].as_ref().unwrap_err().kind(), kind, "{id}");
        }
        assert_eq!(mock.received_requests().await.len(), 1);
    }
}

#[tokio::test]
async fn follows_pages_and_takes_lastupdated_from_the_first() {
    let mock = MockSource::start().await;
    let row = |iso3: &str, wb: &str, year: &str, v: f64| {
        serde_json::json!({
            "indicator": {"id": "SP.POP.TOTL", "value": "Population, total"},
            "country": {"id": wb, "value": iso3},
            "countryiso3code": iso3, "date": year, "value": v,
            "unit": "", "obs_status": "", "decimal": 0
        })
    };
    mock.mount_expect(
        &route("SP.POP.TOTL"),
        Reply::json(serde_json::json!([
            {"page": 1, "pages": 2, "per_page": 2, "total": 3, "lastupdated": "2026-07-01"},
            [row("USA", "US", "2023", 3.0), row("USA", "US", "2022", 2.0)]
        ])),
        1,
    )
    .await;
    mock.mount_expect(
        &Route::get("/country/all/indicator/SP.POP.TOTL").query("page", "2"),
        Reply::json(serde_json::json!([
            {"page": 2, "pages": "2", "per_page": 2, "total": 3, "lastupdated": "2026-07-02"},
            [row("FRA", "FR", "2023", 1.0)]
        ])),
        1,
    )
    .await;
    let out = WorldBankAdapter::new(mock.base_url())
        .fetch_batch(
            &test_ctx(),
            &ids(&["wdi/SP.POP.TOTL.USA", "wdi/SP.POP.TOTL.FRA"]),
            None,
        )
        .await
        .unwrap();
    mock.server().verify().await;
    let usa = out["wdi/SP.POP.TOTL.USA"].as_ref().unwrap();
    let fra = out["wdi/SP.POP.TOTL.FRA"].as_ref().unwrap();
    assert_eq!((usa.points.len(), fra.points.len()), (2, 1));
    assert_eq!(fra.points[0].revision_date, date(2026, 7));
}

#[tokio::test]
async fn page_count_is_bounded() {
    let mock = MockSource::start().await;
    let endless = GDP_PCAP.replace("\"pages\": 1,", "\"pages\": 1000000,");
    assert_ne!(endless, GDP_PCAP);
    mock.mount(
        &Route::get("/country/all/indicator/NY.GDP.PCAP.CD"),
        Reply::json_str(endless),
    )
    .await;
    WorldBankAdapter::new(mock.base_url())
        .fetch_series(&test_ctx(), "wdi/NY.GDP.PCAP.CD.USA", None)
        .await
        .unwrap();
    assert_eq!(
        mock.received_requests().await.len() as u64,
        MAX_PAGES
    );
}

#[test]
fn response_shapes() {
    let (meta, items) = parse_list("x", serde_json::from_str(GDP_PCAP).unwrap()).unwrap();
    assert_eq!(
        meta,
        Meta {
            pages: Some(1),
            last_updated: Some(date(2026, 7)),
        }
    );
    assert_eq!(items.len(), 39);
    // `null` rows are an empty page; a missing `lastupdated` is only an error once needed.
    let (meta, items) = parse_list("x", serde_json::json!([{"pages": "3"}, null])).unwrap();
    assert_eq!((meta.pages, meta.last_updated, items.len()), (Some(3), None, 0));
    for bad in [
        serde_json::json!({"a": 1}),
        serde_json::json!([{"pages": 1}]),
        serde_json::json!([{"pages": 1}, {"a": 1}]),
        serde_json::json!([{"lastupdated": "July"}, []]),
    ] {
        assert_eq!(parse_list("x", bad.clone()).unwrap_err().kind(), "parse", "{bad}");
    }
    let e = parse_list("x", serde_json::from_str(INVALID).unwrap()).unwrap_err();
    assert_eq!(e.kind(), "not_found", "{e}");
    let other = classify_world_bank_message(
        "x",
        &serde_json::json!([{"id": "999", "key": "Something", "value": "else"}]),
    );
    assert_eq!(other.kind(), "permanent");
    let row = |date: &str, value: Value| {
        serde_json::json!({"country": {"id": "US", "value": "United States"},
            "countryiso3code": "USA", "date": date, "value": value})
    };
    assert_eq!(
        parse_rows("x", vec![row("20x3", serde_json::json!(1))])
            .unwrap_err()
            .kind(),
        "parse"
    );
    // Large and exponent-formatted numbers keep their value.
    let rows = parse_rows(
        "x",
        vec![
            row("2023", serde_json::json!(2.7360935e13)),
            row("2022", Value::Null),
        ],
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value, BigDecimal::from(27_360_935_000_000i64));
}

#[tokio::test]
async fn discover_lists_areas_with_values_indicator_by_indicator() {
    let mock = MockSource::start().await;
    for (indicator, body) in FIXTURES {
        mock.mount_expect(&route(indicator), Reply::json_str(body), 1)
            .await;
    }
    mount_invalid_fallback(&mock).await;
    let found = WorldBankAdapter::new(mock.base_url())
        .discover(&test_ctx())
        .await
        .unwrap();
    mock.server().verify().await;

    let indicators = reference::wdi_indicators().unwrap();
    // One request per indicator, nothing else.
    assert_eq!(mock.received_requests().await.len(), indicators.len());

    let keys = |ind: &str| -> Vec<String> {
        found
            .iter()
            .filter_map(|s| {
                let d = &s.dataset.as_ref()?.dimensions.0;
                (d["indicator"] == ind).then(|| d["area"].clone())
            })
            .collect()
    };
    let with_values = [
        "BRA", "CHN", "DEU", "EMU", "FRA", "GBR", "IND", "JPN", "USA", "WLD",
    ];
    // American Samoa has no GDP or inflation values, but has population.
    assert_eq!(keys("NY.GDP.PCAP.CD"), with_values);
    assert_eq!(keys("FP.CPI.TOTL.ZG"), with_values);
    let mut pop = vec!["ASM"];
    pop.extend(with_values);
    assert_eq!(keys("SP.POP.TOTL"), pop);
    assert_eq!(found.len(), 31);

    // Indicator by indicator, in file order.
    let position = |id: &str| indicators.iter().position(|i| i.id == id).unwrap();
    let order: Vec<usize> = found
        .iter()
        .map(|s| position(&s.dataset.as_ref().unwrap().dimensions.0["indicator"]))
        .collect();
    assert!(order.windows(2).all(|w| w[0] <= w[1]), "{order:?}");

    let usa = found
        .iter()
        .find(|s| s.external_id == "wdi/SP.POP.TOTL.USA")
        .unwrap();
    assert_eq!(usa.title, "Population, total: United States");
    assert_eq!(usa.units.as_deref(), Some("persons"));
    assert_eq!(usa.frequency.as_deref(), Some("Annual"));
    assert_eq!(
        usa.data_url.as_deref(),
        Some("https://data.worldbank.org/indicator/SP.POP.TOTL?locations=US")
    );
    let emu = found
        .iter()
        .find(|s| s.external_id == "wdi/SP.POP.TOTL.EMU")
        .unwrap();
    assert_eq!(emu.title, "Population, total: Euro area");
}

#[tokio::test]
async fn discover_aborts_on_rate_limit_and_auth() {
    for (reply, kind) in [
        (Reply::status(429).retry_after(60), "rate_limited"),
        (Reply::status(403), "auth"),
    ] {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/country/all/indicator/NY.GDP.MKTP.CD"), reply)
            .await;
        let e = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), kind);
        // The first indicator in the file; nothing after it.
        assert_eq!(mock.received_requests().await.len(), 1);
    }
}

#[tokio::test]
async fn discover_fails_when_every_indicator_fails() {
    let mock = MockSource::start().await;
    mount_invalid_fallback(&mock).await;
    let e = WorldBankAdapter::new(mock.base_url())
        .discover(&test_ctx())
        .await
        .unwrap_err();
    assert_eq!(e.kind(), "not_found");
}

mod contract {
    use super::{route, GDP_PCAP};
    use crate::sources::world_bank::WorldBankAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| WorldBankAdapter::new(base_url),
        external_id: "wdi/NY.GDP.PCAP.CD.USA",
        route: Route::get("/country/all/indicator/NY.GDP.PCAP.CD"),
        ok_reply: Reply::json_str(GDP_PCAP),
        expect_points: 3,
        discover: {
            route: route("NY.GDP.PCAP.CD"),
            reply: Reply::json_str(GDP_PCAP),
            min_series: 10,
        },
    }
}
