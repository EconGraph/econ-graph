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

/// Seeds `adapter`'s in-process indicator name cache, as `refresh_reference_data` would have,
/// for tests that check a built title without exercising the fetch itself.
fn with_indicator_names(adapter: &WorldBankAdapter, entries: &[(&str, &str)]) {
    let mut cache = adapter.indicator_meta.lock().unwrap();
    for (id, name) in entries {
        cache.insert(
            (*id).to_string(),
            IndicatorMeta {
                name: (*name).to_string(),
                description: None,
            },
        );
    }
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
    for bad in [
        "NY.GDP.PCAP.CD",
        "wdi/USA",
        "wdi/.USA",
        "wdi/X.",
        "other/X.USA",
        "wdiX/A.B",
    ] {
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
    assert_eq!(
        parse_period("2023"),
        Some((date(2023, 1), Frequency::Annual))
    );
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

/// The dataset file declares the `indicator` dimension with no inline codes: names and
/// descriptions come from the crawl (`refresh_reference_data`), not this file.
#[test]
fn dataset_file_declares_indicator_dimension_with_no_shipped_codes() {
    let def = wdi_def().unwrap();
    let indicator_dim = def
        .dimensions
        .iter()
        .find(|d| d.name == "indicator")
        .unwrap();
    assert!(indicator_dim.codes.is_none());
    assert!(!reference::wdi_indicators().unwrap().is_empty());
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
        points
            .iter()
            .map(|r| (r.date, &r.value))
            .collect::<Vec<_>>(),
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
    let adapter = WorldBankAdapter::new(mock.base_url());
    with_indicator_names(
        &adapter,
        &[("NY.GDP.PCAP.CD", "GDP per capita (current US$)")],
    );
    let out = adapter
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
    let ds = &usa.dataset;
    assert_eq!(ds.code, DATASET);
    assert_eq!(
        serde_json::to_value(&ds.dimensions).unwrap(),
        serde_json::json!({"indicator": "NY.GDP.PCAP.CD", "area": "USA"})
    );

    let wld = out["wdi/NY.GDP.PCAP.CD.WLD"].as_ref().unwrap();
    assert_eq!(
        wld.metadata.as_ref().unwrap().title,
        "GDP per capita (current US$): World"
    );
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
    assert_eq!(mock.received_requests().await.len() as u64, MAX_PAGES);
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
    assert_eq!(
        (meta.pages, meta.last_updated, items.len()),
        (Some(3), None, 0)
    );
    for bad in [
        serde_json::json!({"a": 1}),
        serde_json::json!([{"pages": 1}]),
        serde_json::json!([{"pages": 1}, {"a": 1}]),
        serde_json::json!([{"lastupdated": "July"}, []]),
    ] {
        assert_eq!(
            parse_list("x", bad.clone()).unwrap_err().kind(),
            "parse",
            "{bad}"
        );
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
    let adapter = WorldBankAdapter::new(mock.base_url());
    with_indicator_names(
        &adapter,
        &[
            ("NY.GDP.PCAP.CD", "GDP per capita (current US$)"),
            ("SP.POP.TOTL", "Population, total"),
            ("FP.CPI.TOTL.ZG", "Inflation, consumer prices (annual %)"),
        ],
    );
    let found = adapter.discover(&test_ctx()).await.unwrap();
    mock.server().verify().await;

    let indicators = reference::wdi_indicators().unwrap();
    // One request per indicator, nothing else.
    assert_eq!(mock.received_requests().await.len(), indicators.len());

    let keys = |ind: &str| -> Vec<String> {
        found
            .iter()
            .filter_map(|s| {
                let d = &s.dataset.dimensions.0;
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
        .map(|s| position(&s.dataset.dimensions.0["indicator"]))
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

/// `code_lists` carries one `CodeList` per listed indicator; refreshing one fetches that
/// indicator's name and `sourceNote` and merges them into the `wdi` dataset's `indicator`
/// dimension codes, caching the response's `ETag` for next time. The in-process cache is not
/// touched by this alone (that is `refresh_reference_data`'s job, via `seed_cache_from_db`).
/// Mirrors the BLS adapter's `refresh_code_file_merges_labels_and_caches_etag`.
#[tokio::test]
async fn code_list_merges_name_and_caches_etag() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db = crate::persist::stable_id_tests::FreshDb::create(
        &admin_url,
        "econgraph_wb_code_list_merge",
    )
    .await;
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();

    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/indicator/NY.GDP.PCAP.CD").query("format", "json"),
        Reply::json(serde_json::json!([
            {"page": 1, "pages": 1, "per_page": 50, "total": 1},
            [{
                "id": "NY.GDP.PCAP.CD",
                "name": "GDP per capita (current US$)",
                "sourceNote": "GDP per capita is gross domestic product divided by midyear population.",
            }]
        ]))
        .header("ETag", "\"v1\""),
    )
    .await;

    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    let lists = adapter.code_lists(&ApiKeys::default());
    let list = lists
        .iter()
        .find(|l| l.url.contains("NY.GDP.PCAP.CD"))
        .unwrap();
    assert!(
        reference_file::refresh_code_list(&ctx, SourceId::WorldBank, list)
            .await
            .unwrap()
    );

    let url = format!("{}/indicator/NY.GDP.PCAP.CD?format=json", mock.base_url());
    assert_eq!(
        crate::persist::reference_file_etag(&db.pool, SourceId::WorldBank, &url)
            .await
            .unwrap(),
        Some("\"v1\"".to_string())
    );
    let codes = crate::persist::dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
    )
    .await
    .unwrap();
    let code = codes.get("NY.GDP.PCAP.CD").unwrap();
    assert_eq!(code.label, "GDP per capita (current US$)");
    assert_eq!(
        code.description.as_deref(),
        Some("GDP per capita is gross domestic product divided by midyear population.")
    );

    // `seed_cache_from_db` is what actually populates the in-process cache from what just merged.
    adapter.seed_cache_from_db(&ctx).await;
    assert_eq!(
        adapter
            .indicator_meta
            .lock()
            .unwrap()
            .get("NY.GDP.PCAP.CD")
            .unwrap()
            .name,
        "GDP per capita (current US$)"
    );

    // A second refresh with the same ETag short-circuits (no body): the DB codes from the first
    // refresh are untouched, not cleared. The 304 only fires for the stored ETag, so this also
    // proves it was actually sent as `If-None-Match`.
    mock.server().reset().await;
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::method("GET"))
                .and(wiremock::matchers::path("/indicator/NY.GDP.PCAP.CD"))
                .and(wiremock::matchers::header("If-None-Match", "\"v1\""))
                .respond_with(wiremock::ResponseTemplate::new(304)),
        )
        .await;
    assert!(
        !reference_file::refresh_code_list(&ctx, SourceId::WorldBank, list)
            .await
            .unwrap()
    );
    let codes = crate::persist::dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
    )
    .await
    .unwrap();
    assert_eq!(
        codes["NY.GDP.PCAP.CD"].label,
        "GDP per capita (current US$)"
    );
    db.drop().await;
}

/// Without `sync_datasets` having run yet, there is no `wdi` dataset row to merge into: refreshing
/// the code list returns `Ok(false)` and stores no `ETag`, so the same indicator is fetched again
/// next time instead of being wrongly treated as done.
#[tokio::test]
async fn code_list_does_not_cache_when_the_dataset_is_not_synced() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db = crate::persist::stable_id_tests::FreshDb::create(
        &admin_url,
        "econgraph_wb_refresh_not_synced",
    )
    .await;

    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/indicator/NY.GDP.PCAP.CD").query("format", "json"),
        Reply::json(serde_json::json!([
            {"page": 1, "pages": 1, "per_page": 50, "total": 1},
            [{
                "id": "NY.GDP.PCAP.CD",
                "name": "GDP per capita (current US$)",
                "sourceNote": "GDP per capita is gross domestic product divided by midyear population.",
            }]
        ]))
        .header("ETag", "\"v1\""),
    )
    .await;

    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    let lists = adapter.code_lists(&ApiKeys::default());
    let list = lists
        .iter()
        .find(|l| l.url.contains("NY.GDP.PCAP.CD"))
        .unwrap();
    assert!(
        !reference_file::refresh_code_list(&ctx, SourceId::WorldBank, list)
            .await
            .unwrap()
    );

    assert!(adapter.indicator_meta.lock().unwrap().is_empty());
    let url = format!("{}/indicator/NY.GDP.PCAP.CD?format=json", mock.base_url());
    assert_eq!(
        crate::persist::reference_file_etag(&db.pool, SourceId::WorldBank, &url)
            .await
            .unwrap(),
        None
    );
    db.drop().await;
}

/// A process that restarts (or a fetch job that runs before this process's first discovery) has
/// an empty in-process cache even though the DB still holds a label from an earlier process's
/// refresh. `discover` and `fetch_batch` must seed the cache from the DB before building series
/// metadata, rather than falling back to the bare indicator id.
#[tokio::test]
async fn cold_cache_is_seeded_from_the_db_before_building_titles() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db =
        crate::persist::stable_id_tests::FreshDb::create(&admin_url, "econgraph_wb_cold_cache")
            .await;
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();
    // Simulate an earlier process's successful refresh: the DB already has the label, with no
    // entry yet in this fresh adapter's in-process cache.
    crate::persist::merge_dataset_dimension_code_entries(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
        &[Code::new("NY.GDP.PCAP.CD", "GDP per capita (current US$)")],
    )
    .await
    .unwrap();

    let mock = MockSource::start().await;
    mock.mount_expect(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP), 1)
        .await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    assert!(adapter.indicator_meta.lock().unwrap().is_empty());

    let out = adapter
        .fetch_batch(&ctx, &ids(&["wdi/NY.GDP.PCAP.CD.USA"]), None)
        .await
        .unwrap();
    mock.server().verify().await;

    assert_eq!(
        out["wdi/NY.GDP.PCAP.CD.USA"]
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .title,
        "GDP per capita (current US$): United States"
    );
    db.drop().await;
}

/// A cache already holding one indicator (as an earlier call in this process would leave it)
/// must not stop a later call from seeding a *different* indicator's name and description from
/// the DB: `seed_cache_from_db` runs unconditionally, not only when the whole cache is empty, so
/// an indicator this process hasn't fetched itself still gets its merged `sourceNote` instead of
/// the row-carried fallback (which has no description).
#[tokio::test]
async fn partial_cache_still_seeds_db_description_for_a_different_indicator() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db =
        crate::persist::stable_id_tests::FreshDb::create(&admin_url, "econgraph_wb_partial_cache")
            .await;
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();
    // Both indicators already merged into the DB, as an earlier `refresh_reference_data` would
    // have left them.
    crate::persist::merge_dataset_dimension_code_entries(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
        &[Code::new("NY.GDP.PCAP.CD", "GDP per capita (current US$)")],
    )
    .await
    .unwrap();
    let mut population_code = Code::new("SP.POP.TOTL", "Population, total (World Bank)");
    population_code.description =
        Some("Total population is based on the de facto definition.".to_string());
    crate::persist::merge_dataset_dimension_code_entries(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
        &[population_code],
    )
    .await
    .unwrap();

    let mock = MockSource::start().await;
    mock.mount_expect(&route("SP.POP.TOTL"), Reply::json_str(POPULATION), 1)
        .await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    // Simulate an earlier `fetch_batch` call in this same process having already cached
    // NY.GDP.PCAP.CD (but never SP.POP.TOTL): the cache is non-empty, which must not short-circuit
    // `seed_cache_from_db` the way `is_empty()` used to.
    with_indicator_names(
        &adapter,
        &[("NY.GDP.PCAP.CD", "GDP per capita (current US$)")],
    );
    assert!(!adapter.indicator_meta.lock().unwrap().is_empty());
    assert!(!adapter
        .indicator_meta
        .lock()
        .unwrap()
        .contains_key("SP.POP.TOTL"));

    let out = adapter
        .fetch_batch(&ctx, &ids(&["wdi/SP.POP.TOTL.USA"]), None)
        .await
        .unwrap();
    mock.server().verify().await;

    let metadata = out["wdi/SP.POP.TOTL.USA"]
        .as_ref()
        .unwrap()
        .metadata
        .as_ref()
        .unwrap();
    assert_eq!(
        metadata.title,
        "Population, total (World Bank): United States"
    );
    assert_eq!(
        metadata.description.as_deref(),
        Some("Total population is based on the de facto definition.")
    );
    db.drop().await;
}

/// With no cache entry and nothing in the DB (a brand new database that hasn't run
/// `refresh_reference_data` yet, as in the release e2e seed), `series_metadata` still names the
/// series from the data response's own `indicator.value`, not the bare id.
#[tokio::test]
async fn row_carried_indicator_name_names_a_series_before_any_reference_refresh() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db = crate::persist::stable_id_tests::FreshDb::create(
        &admin_url,
        "econgraph_wb_row_carried_name",
    )
    .await;
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();

    let mock = MockSource::start().await;
    mock.mount_expect(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP), 1)
        .await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    assert!(adapter.indicator_meta.lock().unwrap().is_empty());

    let out = adapter
        .fetch_batch(&ctx, &ids(&["wdi/NY.GDP.PCAP.CD.USA"]), None)
        .await
        .unwrap();

    assert_eq!(
        out["wdi/NY.GDP.PCAP.CD.USA"]
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .title,
        "GDP per capita (current US$): United States"
    );
    // Also merged into the dataset's own indicator codes, not just this process's cache, so a
    // fresh database gets a usable indicator picker from the first fetch alone.
    let codes = crate::persist::dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
    )
    .await
    .unwrap();
    assert_eq!(
        codes["NY.GDP.PCAP.CD"].label,
        "GDP per capita (current US$)"
    );
    db.drop().await;
}

/// `refresh_reference_data` (`reference_file::refresh_code_lists` under the hood) stops at once on
/// `Auth`/`RateLimited` (more requests would only hit a source that just refused us), but attempts
/// every listed indicator regardless of an earlier one's `NotFound`/`Parse`/etc. failure,
/// aggregating those into one error. An indicator that did merge before a stop is still reflected
/// in the in-process cache afterwards, since `seed_cache_from_db` runs unconditionally even when
/// the refresh as a whole failed.
#[tokio::test]
async fn refresh_reference_data_stops_on_auth_and_aggregates_other_failures() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let indicators = reference::wdi_indicators().unwrap();

    let db = crate::persist::stable_id_tests::FreshDb::create(
        &admin_url,
        "econgraph_wb_refresh_reference_data_auth",
    )
    .await;
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get(format!("/indicator/{}", indicators[0].id)).query("format", "json"),
        Reply::json(serde_json::json!([
            {"page": 1, "pages": 1, "per_page": 50, "total": 1},
            [{"id": indicators[0].id, "name": "First indicator"}]
        ])),
    )
    .await;
    mock.mount(
        &Route::get(format!("/indicator/{}", indicators[1].id)),
        Reply::status(403),
    )
    .await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    let e = adapter.refresh_reference_data(&ctx).await.unwrap_err();
    // `/country` (not mounted: 404) is skipped, the first indicator's URL succeeds, the second
    // 403s (`Auth`): the refresh stops there rather than requesting any other indicator, but the
    // first merge is still reflected in the in-process cache afterwards.
    assert_eq!(mock.received_requests().await.len(), 3);
    assert_eq!(e.kind(), "auth");
    assert_eq!(
        adapter
            .indicator_meta
            .lock()
            .unwrap()
            .get(&indicators[0].id)
            .unwrap()
            .name,
        "First indicator"
    );
    db.drop().await;

    let db = crate::persist::stable_id_tests::FreshDb::create(
        &admin_url,
        "econgraph_wb_refresh_reference_data_skip",
    )
    .await;
    let mock = MockSource::start().await;
    mount_invalid_fallback(&mock).await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    // `/country` and every indicator answer "Invalid value" (NotFound, not Auth/RateLimited):
    // all attempted, aggregated into one error.
    let e = adapter.refresh_reference_data(&ctx).await.unwrap_err();
    assert_eq!(e.kind(), "transient");
    assert_eq!(mock.received_requests().await.len(), indicators.len() + 1);
    assert!(adapter.indicator_meta.lock().unwrap().is_empty());
    db.drop().await;
}

/// A `/country` response in the API's shape: the economies and aggregates it lists, by `id`.
fn country_list(pages: u64, entries: &[(&str, &str)]) -> String {
    let items: Vec<Value> = entries
        .iter()
        .map(|(id, name)| {
            serde_json::json!({
                "id": id,
                "iso2Code": "",
                "name": name,
                "region": {"id": "NA", "iso2code": "NA", "value": "Aggregates"},
                "incomeLevel": {"id": "NA", "iso2code": "NA", "value": "Aggregates"},
            })
        })
        .collect();
    serde_json::json!([
        {"page": 1, "pages": pages, "per_page": "1000", "total": entries.len()},
        items
    ])
    .to_string()
}

/// `/country` names the aggregates the country table carries, keyed as the table keys them;
/// economies, aggregates the table lacks and nameless entries are skipped. A list naming none of
/// them, or one with more pages, is a parse error, as is an API error message.
#[test]
fn aggregate_names_come_from_the_country_list() {
    let body = country_list(
        1,
        &[
            ("EMU", "Euro area"),
            ("WLD", " World "),
            ("USA", "United States"),
            ("EAS", "East Asia & Pacific"),
            ("HIC", ""),
            ("EMU", "Euro area again"),
        ],
    );
    let codes = parse_aggregate_names(&body).unwrap();
    let pairs: Vec<(&str, &str)> = codes
        .iter()
        .map(|c| (c.code.as_str(), c.label.as_str()))
        .collect();
    assert_eq!(pairs, [("EMU", "Euro area"), ("WLD", "World")]);

    for (body, needle) in [
        (country_list(1, &[("USA", "United States")]), "none of"),
        (country_list(2, &[("WLD", "World")]), "2 pages"),
        (INVALID.to_string(), ""),
        ("{".to_string(), ""),
    ] {
        let e = parse_aggregate_names(&body).unwrap_err();
        assert!(
            matches!(e, CrawlError::Parse(_) | CrawlError::NotFound(_)),
            "{e:?}"
        );
        assert!(e.to_string().contains(needle), "{e}");
    }
}

/// The `/country` code list merges the World Bank's aggregate names into the `wdi` area
/// dimension, which keeps its shared code list; they survive the next `sync_datasets`, and the
/// next refresh of an unchanged list is a 304.
#[tokio::test]
async fn country_list_names_aggregates_on_the_area_dimension() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db =
        crate::persist::stable_id_tests::FreshDb::create(&admin_url, "econgraph_wb_country_list")
            .await;
    let catalog = wdi_catalog();
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();

    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/country")
            .query("format", "json")
            .query("per_page", COUNTRY_PER_PAGE),
        Reply::text(country_list(
            1,
            &[
                ("EMU", "Euro area"),
                ("WLD", "World"),
                ("USA", "United States"),
            ],
        ))
        .header("ETag", "\"c1\""),
    )
    .await;
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let adapter = WorldBankAdapter::new(mock.base_url());
    let lists = adapter.code_lists(&ApiKeys::default());
    let list = lists
        .iter()
        .find(|l| l.dimension == AREA_DIMENSION)
        .unwrap();
    assert!(
        reference_file::refresh_code_list(&ctx, SourceId::WorldBank, list)
            .await
            .unwrap()
    );

    let area_dimension = || async {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::datasets::dsl;
        let mut conn = db.pool.get().await.unwrap();
        let dims: econ_graph_core::models::DatasetComponents = dsl::datasets
            .filter(dsl::code.eq(DATASET))
            .select(dsl::dimensions)
            .first(&mut conn)
            .await
            .unwrap();
        dims.0
            .into_iter()
            .find(|d| d.name == AREA_DIMENSION)
            .unwrap()
    };
    let check = |area: econ_graph_core::models::DatasetComponent| {
        assert_eq!(area.codelist.as_deref(), Some("countries"));
        let labels: Vec<(String, String)> = area
            .codes
            .unwrap_or_default()
            .into_iter()
            .map(|c| (c.code, c.label))
            .collect();
        assert_eq!(
            labels,
            [
                ("EMU".to_string(), "Euro area".to_string()),
                ("WLD".to_string(), "World".to_string())
            ]
        );
    };
    check(area_dimension().await);
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();
    check(area_dimension().await);

    mock.server().reset().await;
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::method("GET"))
                .and(wiremock::matchers::path("/country"))
                .and(wiremock::matchers::header("If-None-Match", "\"c1\""))
                .respond_with(wiremock::ResponseTemplate::new(304)),
        )
        .await;
    assert!(
        !reference_file::refresh_code_list(&ctx, SourceId::WorldBank, list)
            .await
            .unwrap()
    );
    check(area_dimension().await);
    db.drop().await;
}

/// A recorded seed of the `/country` list, applied to a new database before the catalog is
/// synced, stores the aggregate names on the area dimension with its code list kept, and the
/// first sync keeps both.
#[tokio::test]
async fn seeded_country_list_keeps_the_code_list() {
    let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
        return;
    };
    let db =
        crate::persist::stable_id_tests::FreshDb::create(&admin_url, "econgraph_wb_country_seed")
            .await;
    let catalog = wdi_catalog();
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get("/country"),
        Reply::text(country_list(1, &[("EMU", "Euro area")])).header("ETag", "\"c1\""),
    )
    .await;
    let adapter = WorldBankAdapter::new(mock.base_url());
    let lists: Vec<CodeList> = adapter
        .code_lists(&ApiKeys::default())
        .into_iter()
        .filter(|l| l.dimension == AREA_DIMENSION)
        .collect();
    let download = reference_file::download_seed_entries(
        &test_ctx().http,
        SourceId::WorldBank,
        &lists,
        &catalog,
    )
    .await
    .unwrap();
    assert!(download.failures.is_empty());
    let (up, _down) = reference_file::seed_migration_sql(
        SourceId::WorldBank,
        chrono::Utc::now(),
        &download.entries,
    )
    .unwrap();
    {
        use diesel_async::SimpleAsyncConnection;
        db.pool
            .get()
            .await
            .unwrap()
            .batch_execute(&format!("BEGIN;\n{up}\nCOMMIT;"))
            .await
            .unwrap();
    }
    let area_dimension = || async {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::datasets::dsl;
        let mut conn = db.pool.get().await.unwrap();
        let dims: econ_graph_core::models::DatasetComponents = dsl::datasets
            .filter(dsl::code.eq(DATASET))
            .select(dsl::dimensions)
            .first(&mut conn)
            .await
            .unwrap();
        dims.0
            .into_iter()
            .find(|d| d.name == AREA_DIMENSION)
            .unwrap()
    };
    let check = |area: econ_graph_core::models::DatasetComponent| {
        assert_eq!(area.codelist.as_deref(), Some("countries"));
        assert_eq!(area.code("EMU").unwrap().label, "Euro area");
    };
    // The seed created the dataset row: the dimension already has its code list.
    check(area_dimension().await);
    crate::persist::sync_datasets(&db.pool, &catalog)
        .await
        .unwrap();
    check(area_dimension().await);
    let row = crate::persist::url_validators(&db.pool, SourceId::WorldBank, &lists[0].url)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.etag.as_deref(), Some("\"c1\""));
    db.drop().await;
}

/// The `wdi` dataset catalog from the shipped dataset file.
fn wdi_catalog() -> crate::dataset::DatasetCatalog {
    let mut catalog = crate::dataset::DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &[DATASET],
            crate::dataset::parse_dataset_file(
                &std::fs::read_to_string(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("data/datasets/world_bank.toml"),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    catalog
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

/// Every request not matched by an earlier mount answers as an indicator with no rows, updated
/// `2026-07-01` (the fixtures' `lastupdated`).
async fn mount_empty_fallback(mock: &MockSource) {
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::method("GET")).respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    br#"[{"page": 1, "pages": 1, "lastupdated": "2026-07-01"}, null]"#.to_vec(),
                    "application/json",
                ),
            ),
        )
        .await;
}

async fn validator_db(name: &str) -> Option<crate::persist::stable_id_tests::FreshDb> {
    use crate::dataset::DatasetCatalog;
    use crate::persist::stable_id_tests::{database_url, FreshDb};
    let admin_url = database_url()?;
    let db = FreshDb::create(&admin_url, name).await;
    let mut catalog = DatasetCatalog::empty();
    catalog.load_adapter(&WorldBankAdapter::default()).unwrap();
    persist::sync_datasets(&db.pool, &catalog).await.unwrap();
    Some(db)
}

fn full_requests(reqs: &[wiremock::Request]) -> usize {
    reqs.iter()
        .filter(|r| {
            r.url
                .query_pairs()
                .any(|(k, v)| k == "per_page" && v == PER_PAGE)
        })
        .count()
}

#[tokio::test]
async fn an_unchanged_indicator_is_not_refetched() {
    let Some(db) = validator_db("econgraph_wb_fetch_validators").await else {
        return;
    };
    let mock = MockSource::start().await;
    mock.mount(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP))
        .await;
    mount_empty_fallback(&mock).await;
    let adapter = WorldBankAdapter::new(mock.base_url());
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();
    let requested = ids(&["wdi/NY.GDP.PCAP.CD.USA", "wdi/NY.GDP.PCAP.CD.DEU"]);

    let first = adapter.fetch_batch(&ctx, &requested, None).await.unwrap();
    let lastupdated = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
    let version = Validators {
        version: Some(adapter.series_version("NY.GDP.PCAP.CD", lastupdated)),
        ..Validators::default()
    };
    assert!(version
        .version
        .as_deref()
        .unwrap()
        .starts_with(&format!("{PARSE_VERSION}:2026-07-01:")));
    for id in &requested {
        let f = first[id].as_ref().unwrap();
        assert_eq!(f.validators.as_ref(), Some(&version));
        persist::persist_series(&db.pool, SourceId::WorldBank, id, f)
            .await
            .unwrap();
    }
    assert_eq!(full_requests(&mock.received_requests().await), 1);

    let again = adapter.fetch_batch(&ctx, &requested, None).await.unwrap();
    for id in &requested {
        assert_eq!(
            again[id].as_ref().unwrap(),
            &FetchedSeries::unchanged(first[id].as_ref().unwrap().dataset.clone(), version.clone()),
            "{id}"
        );
    }
    let reqs = mock.received_requests().await;
    assert_eq!(
        full_requests(&reqs),
        1,
        "the probe alone answers an unchanged indicator"
    );
    assert_eq!(reqs.len(), 2);

    // A series never fetched makes the indicator a full fetch.
    let mixed = ids(&["wdi/NY.GDP.PCAP.CD.USA", "wdi/NY.GDP.PCAP.CD.WLD"]);
    let out = adapter.fetch_batch(&ctx, &mixed, None).await.unwrap();
    assert!(mixed
        .iter()
        .all(|id| !out[id].as_ref().unwrap().points.is_empty()));
    assert_eq!(full_requests(&mock.received_requests().await), 2);

    // A newer lastupdated makes it a full fetch too.
    let mut conn = db.pool.get().await.unwrap();
    diesel_async::RunQueryDsl::execute(
        diesel::sql_query("UPDATE series_fetch_validators SET version = 'wb-1:2026-01-01'"),
        &mut conn,
    )
    .await
    .unwrap();
    drop(conn);
    let out = adapter.fetch_batch(&ctx, &requested, None).await.unwrap();
    assert!(!out["wdi/NY.GDP.PCAP.CD.USA"]
        .as_ref()
        .unwrap()
        .points
        .is_empty());
    assert_eq!(full_requests(&mock.received_requests().await), 3);

    // A renamed indicator rewrites its series' titles though its data hasn't moved.
    for id in &requested {
        persist::persist_series(&db.pool, SourceId::WorldBank, id, out[id].as_ref().unwrap())
            .await
            .unwrap();
    }
    let out = adapter.fetch_batch(&ctx, &requested, None).await.unwrap();
    assert!(out["wdi/NY.GDP.PCAP.CD.USA"]
        .as_ref()
        .unwrap()
        .points
        .is_empty());
    assert_eq!(full_requests(&mock.received_requests().await), 3);
    persist::merge_dataset_dimension_code_entries(
        &db.pool,
        SourceId::WorldBank,
        DATASET,
        INDICATOR_DIMENSION,
        &[Code::new("NY.GDP.PCAP.CD", "GDP per capita, renamed")],
    )
    .await
    .unwrap();
    let out = adapter.fetch_batch(&ctx, &requested, None).await.unwrap();
    let usa = out["wdi/NY.GDP.PCAP.CD.USA"].as_ref().unwrap();
    assert!(!usa.points.is_empty());
    assert!(
        usa.metadata
            .as_ref()
            .unwrap()
            .title
            .starts_with("GDP per capita, renamed"),
        "{:?}",
        usa.metadata
    );
    assert_eq!(full_requests(&mock.received_requests().await), 4);

    db.drop().await;
}

#[tokio::test]
async fn discovery_is_unchanged_until_an_indicator_updates() {
    let Some(db) = validator_db("econgraph_wb_discovery_validators").await else {
        return;
    };
    let mock = MockSource::start().await;
    mock.mount(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP))
        .await;
    mount_empty_fallback(&mock).await;
    let adapter = WorldBankAdapter::new(mock.base_url());
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();

    let Discovery::Changed {
        found,
        validator: Some((key, validators)),
    } = adapter.discover_if_changed(&ctx).await.unwrap()
    else {
        panic!("first discovery must list every indicator")
    };
    assert!(!found.is_empty());
    assert_eq!(key, mock.url("/country/all/indicator"));
    let version = validators.version.clone().unwrap();
    assert!(
        version.starts_with(&format!("{PARSE_VERSION}|")),
        "{version}"
    );
    assert!(
        version.contains(&format!("NY.GDP.PCAP.CD={PARSE_VERSION}:2026-07-01:")),
        "{version}"
    );
    let mut conn = db.pool.get().await.unwrap();
    persist::set_url_validators_conn(&mut conn, SourceId::WorldBank, &key, &validators)
        .await
        .unwrap();
    let before = full_requests(&mock.received_requests().await);
    assert_eq!(
        adapter.discover_if_changed(&ctx).await.unwrap(),
        Discovery::Unchanged
    );
    assert_eq!(full_requests(&mock.received_requests().await), before);

    // Any indicator with a different lastupdated means a full discovery.
    let stale = Validators {
        version: Some(version.replace(
            &format!("NY.GDP.PCAP.CD={PARSE_VERSION}:2026-07-01"),
            &format!("NY.GDP.PCAP.CD={PARSE_VERSION}:2026-01-01"),
        )),
        ..Validators::default()
    };
    persist::set_url_validators_conn(&mut conn, SourceId::WorldBank, &key, &stale)
        .await
        .unwrap();
    drop(conn);
    assert!(matches!(
        adapter.discover_if_changed(&ctx).await.unwrap(),
        Discovery::Changed {
            validator: Some(_),
            ..
        }
    ));

    db.drop().await;
}

#[tokio::test]
async fn a_discovery_that_skipped_an_indicator_stores_no_version() {
    let mock = MockSource::start().await;
    mock.mount(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP))
        .await;
    // Probes (per_page=1) would succeed, but with nothing stored there are none; every other full
    // request is an "Invalid value" error.
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::query_param("per_page", "1")).respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    br#"[{"page": 1, "pages": 1, "lastupdated": "2026-07-01"}, null]"#.to_vec(),
                    "application/json",
                ),
            ),
        )
        .await;
    mount_invalid_fallback(&mock).await;
    let Discovery::Changed { found, validator } = WorldBankAdapter::new(mock.base_url())
        .discover_if_changed(&test_ctx())
        .await
        .unwrap()
    else {
        panic!("nothing stored, so not Unchanged")
    };
    assert!(!found.is_empty());
    assert_eq!(validator, None);
}

/// An incomplete discovery keeps the version stored by the last complete one, and that can't
/// hide the indicator it skipped: the stored version still differs from the probes until a
/// discovery reads every indicator, so each later discovery runs in full again rather than
/// returning `Unchanged`.
#[tokio::test]
async fn an_incomplete_discovery_is_retried_in_full() {
    let Some(db) = validator_db("econgraph_wb_incomplete_discovery").await else {
        return;
    };
    let mock = MockSource::start().await;
    mock.mount(&route("NY.GDP.PCAP.CD"), Reply::json_str(GDP_PCAP))
        .await;
    // Every probe answers; every other full request is an "Invalid value" error.
    mock.server()
        .register(
            wiremock::Mock::given(wiremock::matchers::query_param("per_page", "1")).respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    br#"[{"page": 1, "pages": 1, "lastupdated": "2026-07-01"}, null]"#.to_vec(),
                    "application/json",
                ),
            ),
        )
        .await;
    mount_invalid_fallback(&mock).await;
    let adapter = WorldBankAdapter::new(mock.base_url());
    let mut ctx = test_ctx();
    ctx.pool = db.pool.clone();

    // The last complete discovery saw an older NY.GDP.PCAP.CD.
    let current = adapter
        .catalog_version(&ctx, reference::wdi_indicators().unwrap())
        .await
        .unwrap()
        .unwrap();
    let older = current.replace(
        &format!("NY.GDP.PCAP.CD={PARSE_VERSION}:2026-07-01"),
        &format!("NY.GDP.PCAP.CD={PARSE_VERSION}:2026-01-01"),
    );
    assert_ne!(older, current);
    persist::set_url_validators(
        &db.pool,
        SourceId::WorldBank,
        &adapter.catalog_key(),
        &Validators {
            version: Some(older),
            ..Validators::default()
        },
    )
    .await
    .unwrap();

    let full = |reqs: &[wiremock::Request]| {
        reqs.iter()
            .filter(|r| {
                r.url.path().ends_with("/NY.GDP.PCAP.CD")
                    && r.url
                        .query_pairs()
                        .any(|(k, v)| k == "per_page" && v == PER_PAGE)
            })
            .count()
    };
    for run in 1..=2 {
        let Discovery::Changed { found, validator } =
            adapter.discover_if_changed(&ctx).await.unwrap()
        else {
            panic!("run {run}: an indicator moved since the stored version")
        };
        assert!(!found.is_empty(), "run {run}");
        assert_eq!(
            validator, None,
            "run {run}: incomplete, so nothing to store"
        );
        assert_eq!(full(&mock.received_requests().await), run);
    }

    db.drop().await;
}

#[test]
fn shared_validators_need_one_stored_version() {
    let state = |version: Option<&str>| StoredFetchState {
        dataset: SeriesDataset::default(),
        validators: Some(Validators {
            version: version.map(str::to_owned),
            ..Validators::default()
        }),
    };
    let stored = HashMap::from([
        ("a".to_string(), state(Some("2026-07-01"))),
        ("b".to_string(), state(Some("2026-07-01"))),
        ("c".to_string(), state(Some("2026-01-01"))),
        ("d".to_string(), state(None)),
    ]);
    assert!(shared_validators(["a", "b"], &stored).is_some());
    assert_eq!(shared_validators(["a", "c"], &stored), None);
    assert_eq!(shared_validators(["a", "d"], &stored), None);
    assert_eq!(shared_validators(["a", "missing"], &stored), None);
}
