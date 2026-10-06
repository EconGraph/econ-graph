// Database tests for cross-sections.
//
// These need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database (CI runs them against its Postgres service). They run the embedded migrations
// once, never drop the schema, and give every test its own data source.
//
// The seeded dataset is `wdi` with dimensions `indicator` (GDP, POP) and `area` (the
// countries code list, naming the aggregate WLD "World"), over BRA, DEU, JPN, USA and WLD:
//
// | GDP  | 2021                  | 2022 | 2023                    |
// |------|-----------------------|------|-------------------------|
// | USA  |                       | 100  | 105, revised to 110     |
// | DEU  |                       | 50   | null                    |
// | JPN  | 30, revised to 35 (same revision date) | | |
// | BRA  |                       |      | 20, revised to null    |
// | WLD  |                       |      | 500                     |
//
// POP has a 2023 value for every area.

use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use diesel::connection::InstrumentationEvent;
use diesel_async::AsyncConnection;

use super::*;
use econ_graph_core::database::{create_pool, run_migrations};
use econ_graph_core::models::dataset::{
    Code, ComponentType, DatasetComponent, NewDataset, SeriesDimensions,
};
use econ_graph_core::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
use econ_graph_core::reference::{self, AreaKind};
use econ_graph_core::schema::data_points;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

fn test_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        let url = std::env::var("DATABASE_URL")
            .expect("set TEST_DATABASE_URL (or DATABASE_URL naming a *test* database)");
        let db_name = url
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('?')
            .next()
            .unwrap_or("");
        assert!(
            db_name.contains("test"),
            "refusing to run cross-section tests against database {db_name:?}"
        );
        url
    })
}

async fn migrated_url() -> String {
    let url = test_url();
    MIGRATED
        .get_or_init(|| async {
            run_migrations(&url).await.expect("migrations failed");
        })
        .await;
    url
}

async fn connect() -> AsyncPgConnection {
    let url = migrated_url().await;
    AsyncPgConnection::establish(&url)
        .await
        .expect("connect to test database")
}

fn areas() -> &'static Areas {
    reference::areas().expect("countries reference file")
}

const AREAS: [&str; 5] = ["USA", "DEU", "JPN", "BRA", "WLD"];

fn d(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
}

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

/// (indicator, area, date, value, revision_date, is_original_release)
type SeedPoint = (
    &'static str,
    &'static str,
    NaiveDate,
    Option<&'static str>,
    NaiveDate,
    bool,
);

/// The seeded dataset and the id of each (indicator, area) series.
struct Seeded {
    dataset_id: Uuid,
    series: BTreeMap<(String, String), Uuid>,
}

impl Seeded {
    fn series_id(&self, indicator: &str, area: &str) -> Uuid {
        self.series[&(indicator.to_string(), area.to_string())]
    }
}

async fn seed() -> Seeded {
    let pool = create_pool(&migrated_url().await).await.expect("pool");
    let source = DataSource::create(
        &pool,
        NewDataSource {
            name: format!("Cross-section Test Source {}", Uuid::new_v4()),
            description: None,
            base_url: "https://cross-section.example.com/api".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: None,
            api_key_name: None,
        },
    )
    .await
    .expect("create data source");

    let mut indicator = DatasetComponent::new("indicator", "Indicator", ComponentType::String);
    indicator.codes = Some(vec![
        Code::new("GDP", "GDP per capita"),
        Code::new("POP", "Population"),
    ]);
    let mut area = DatasetComponent::new("area", "Area", ComponentType::String);
    area.codelist = Some(COUNTRIES_CODELIST.to_string());
    // The dataset's own name for the aggregate, as the World Bank crawl stores it.
    area.codes = Some(vec![Code::new("WLD", "World")]);
    let dataset = Dataset::create(
        &pool,
        &NewDataset::long(
            source.id,
            "wdi",
            "World Development Indicators",
            vec![indicator, area],
        ),
    )
    .await
    .expect("create dataset");

    let mut series = BTreeMap::new();
    for indicator in ["GDP", "POP"] {
        for area in AREAS {
            let new_series = NewEconomicSeries {
                source_id: source.id,
                external_id: format!("{indicator}.{area}"),
                title: format!("{indicator} {area}"),
                frequency: "Annual".to_string(),
                dataset_id: dataset.id,
                dimensions: [("indicator", indicator), ("area", area)]
                    .into_iter()
                    .collect::<SeriesDimensions>(),
                ..Default::default()
            };
            let created = EconomicSeries::create(&pool, &new_series)
                .await
                .expect("create series");
            series.insert((indicator.to_string(), area.to_string()), created.id);
        }
    }
    // An inactive series and one missing the `across` dimension key entirely: neither should
    // ever appear in a result, whatever it filters on.
    EconomicSeries::create(
        &pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: "GDP.CAN".to_string(),
            title: "GDP CAN (inactive)".to_string(),
            frequency: "Annual".to_string(),
            dataset_id: dataset.id,
            dimensions: [("indicator", "GDP"), ("area", "CAN")]
                .into_iter()
                .collect::<SeriesDimensions>(),
            is_active: false,
            ..Default::default()
        },
    )
    .await
    .expect("create inactive series");
    EconomicSeries::create(
        &pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: "GDP.no-area".to_string(),
            title: "GDP with no area dimension".to_string(),
            frequency: "Annual".to_string(),
            dataset_id: dataset.id,
            dimensions: [("indicator", "GDP")]
                .into_iter()
                .collect::<SeriesDimensions>(),
            ..Default::default()
        },
    )
    .await
    .expect("create series with no area dimension");

    let seeded = Seeded {
        dataset_id: dataset.id,
        series,
    };

    let points: Vec<SeedPoint> = vec![
        ("GDP", "USA", d(2022), Some("100"), d(2022), true),
        ("GDP", "USA", d(2023), Some("105"), d(2023), true),
        ("GDP", "USA", d(2023), Some("110"), d(2024), false),
        ("GDP", "DEU", d(2022), Some("50"), d(2022), true),
        ("GDP", "DEU", d(2023), None, d(2023), true),
        ("GDP", "JPN", d(2021), Some("30"), d(2021), true),
        // Same revision_date as the original: the revision must win.
        ("GDP", "JPN", d(2021), Some("35"), d(2021), false),
        ("GDP", "BRA", d(2023), Some("20"), d(2023), true),
        // Superseded to null: the current value at 2023 is null, not the "20" original.
        ("GDP", "BRA", d(2023), None, d(2024), false),
        ("GDP", "WLD", d(2023), Some("500"), d(2023), true),
        ("POP", "USA", d(2023), Some("335"), d(2023), true),
        ("POP", "DEU", d(2023), Some("84"), d(2023), true),
        ("POP", "JPN", d(2023), Some("124"), d(2023), true),
        ("POP", "BRA", d(2023), Some("216"), d(2023), true),
        ("POP", "WLD", d(2023), Some("8000"), d(2023), true),
    ];
    let rows: Vec<_> = points
        .into_iter()
        .map(|(indicator, area, date, value, revision_date, original)| {
            (
                data_points::series_id.eq(seeded.series_id(indicator, area)),
                data_points::date.eq(date),
                data_points::value.eq(value.map(dec)),
                data_points::revision_date.eq(revision_date),
                data_points::is_original_release.eq(original),
            )
        })
        .collect();
    let mut conn = pool.get().await.expect("connection");
    diesel::insert_into(data_points::table)
        .values(&rows)
        .execute(&mut conn)
        .await
        .expect("insert data points");

    seeded
}

fn request(seeded: &Seeded, indicator: &str, date: CrossSectionDate) -> CrossSectionRequest {
    CrossSectionRequest {
        dataset_id: seeded.dataset_id,
        measure: None,
        filter: [("indicator".to_string(), indicator.to_string())]
            .into_iter()
            .collect(),
        across: "area".to_string(),
        date,
        as_of: None,
    }
}

/// (key, date, value) of each row.
fn summary(rows: &[CrossSectionRow]) -> Vec<(&str, Option<NaiveDate>, Option<BigDecimal>)> {
    rows.iter()
        .map(|r| (r.key.as_str(), r.date, r.value.clone()))
        .collect()
}

#[tokio::test]
async fn latest_returns_each_key_at_its_own_latest_non_null_date() {
    let seeded = seed().await;
    let mut conn = connect().await;

    let rows = cross_section(
        &mut conn,
        areas(),
        &request(&seeded, "GDP", CrossSectionDate::Latest),
    )
    .await
    .unwrap();

    assert_eq!(
        summary(&rows),
        vec![
            // Only observation superseded to null: no non-null value at any date.
            ("BRA", None, None),
            // 2023 is null, so 2022.
            ("DEU", Some(d(2022)), Some(dec("50"))),
            // The revision "35", not the original "30" (same revision date).
            ("JPN", Some(d(2021)), Some(dec("35"))),
            // The 2023 revision, not the original release.
            ("USA", Some(d(2023)), Some(dec("110"))),
            ("WLD", Some(d(2023)), Some(dec("500"))),
        ]
    );
    // The inactive CAN series and the one with no `area` dimension never appear: neither
    // `AND s.is_active` nor `AND s.dimensions ? $2` can be dropped and still pass this
    // exact-list assertion (the missing key would otherwise fail to decode as a String).
    assert!(rows.iter().all(|r| r.key != "CAN"));
    for row in &rows {
        assert_eq!(
            row.series_id,
            seeded.series_id("GDP", &row.key),
            "{}",
            row.key
        );
    }
}

#[tokio::test]
async fn fixed_date_returns_that_date_with_missing_values_as_none() {
    let seeded = seed().await;
    let mut conn = connect().await;

    let rows = cross_section(
        &mut conn,
        areas(),
        &request(&seeded, "GDP", CrossSectionDate::On(d(2023))),
    )
    .await
    .unwrap();
    assert_eq!(
        summary(&rows),
        vec![
            // Superseded to null; must not fall back to the original "20".
            ("BRA", Some(d(2023)), None),
            // Stored as null.
            ("DEU", Some(d(2023)), None),
            // Only has 2021.
            ("JPN", Some(d(2023)), None),
            ("USA", Some(d(2023)), Some(dec("110"))),
            ("WLD", Some(d(2023)), Some(dec("500"))),
        ]
    );

    let rows = cross_section(
        &mut conn,
        areas(),
        &request(&seeded, "GDP", CrossSectionDate::On(d(2022))),
    )
    .await
    .unwrap();
    let values: Vec<_> = rows.iter().map(|r| r.value.clone()).collect();
    assert_eq!(
        values,
        vec![None, Some(dec("50")), None, Some(dec("100")), None]
    );
}

#[tokio::test]
async fn as_of_reads_the_revision_known_on_that_day() {
    let seeded = seed().await;
    let mut conn = connect().await;

    // Before USA's 2023 revision (revision_date 2024-12-31): still the original 105.
    let mut req = request(&seeded, "GDP", CrossSectionDate::On(d(2023)));
    req.as_of = Some(d(2023));
    let rows = cross_section(&mut conn, areas(), &req).await.unwrap();
    let usa = rows.iter().find(|r| r.key == "USA").unwrap();
    assert_eq!(usa.value, Some(dec("105")));

    // On or after the revision: the revised 110.
    req.as_of = Some(d(2024));
    let rows = cross_section(&mut conn, areas(), &req).await.unwrap();
    let usa = rows.iter().find(|r| r.key == "USA").unwrap();
    assert_eq!(usa.value, Some(dec("110")));
}

#[tokio::test]
async fn as_of_same_revision_date_a_revision_beats_the_original() {
    let seeded = seed().await;
    let mut conn = connect().await;

    // JPN 2021: original 30 and revision 35 share revision_date 2021-12-31.
    let mut req = request(&seeded, "GDP", CrossSectionDate::On(d(2021)));
    req.as_of = Some(d(2021));
    let rows = cross_section(&mut conn, areas(), &req).await.unwrap();
    let jpn = rows.iter().find(|r| r.key == "JPN").unwrap();
    assert_eq!(jpn.value, Some(dec("35")));

    // Before that revision_date, nothing was published yet.
    req.as_of = Some(NaiveDate::from_ymd_opt(2021, 1, 1).unwrap());
    let rows = cross_section(&mut conn, areas(), &req).await.unwrap();
    let jpn = rows.iter().find(|r| r.key == "JPN").unwrap();
    assert_eq!(jpn.value, None);
}

#[tokio::test]
async fn as_of_with_latest_uses_what_was_known_on_that_day() {
    let seeded = seed().await;
    let mut conn = connect().await;

    // BRA 2023 was originally 20; only a later revision (revision_date 2024-12-31)
    // superseded it to null. As of 2023, the 20 was still the latest known value.
    let mut req = request(&seeded, "GDP", CrossSectionDate::Latest);
    req.as_of = Some(d(2023));
    let rows = cross_section(&mut conn, areas(), &req).await.unwrap();
    let bra = rows.iter().find(|r| r.key == "BRA").unwrap();
    assert_eq!(bra.date, Some(d(2023)));
    assert_eq!(bra.value, Some(dec("20")));

    // Without asOf, the current value (superseded to null) wins instead.
    let rows = cross_section(
        &mut conn,
        areas(),
        &request(&seeded, "GDP", CrossSectionDate::Latest),
    )
    .await
    .unwrap();
    let bra = rows.iter().find(|r| r.key == "BRA").unwrap();
    assert_eq!(bra.value, None);
}

#[tokio::test]
async fn as_of_excludes_a_pre_vintage_legacy_row() {
    // A series crawled before this crate tracked vintages has a row tagged as its own
    // original release but really meaning "current value as of that old crawl" (PR #184's
    // `exclude_synthetic_legacy_rows`). `asOf` must not let it stand in for "known on this
    // day" once real vintages exist for the same date.
    let pool = create_pool(&migrated_url().await).await.expect("pool");
    let source = DataSource::create(
        &pool,
        NewDataSource {
            name: format!("Cross-section Legacy Row Test Source {}", Uuid::new_v4()),
            description: None,
            base_url: "https://cross-section.example.com/api".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: None,
            api_key_name: None,
        },
    )
    .await
    .expect("create data source");
    let area = DatasetComponent::new("area", "Area", ComponentType::String);
    let dataset = Dataset::create(
        &pool,
        &NewDataset::long(source.id, "legacy", "Legacy Row Test Dataset", vec![area]),
    )
    .await
    .expect("create dataset");
    let series = EconomicSeries::create(
        &pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: "VAL.XXX".to_string(),
            title: "VAL XXX".to_string(),
            frequency: "Annual".to_string(),
            dataset_id: dataset.id,
            dimensions: [("area", "XXX")].into_iter().collect::<SeriesDimensions>(),
            ..Default::default()
        },
    )
    .await
    .expect("create series");

    let obs_date = d(2025);
    let rows = [
        // Legacy row: revision_date == date, tagged original.
        (obs_date, dec("30"), d(2025), true),
        // Real vintage, found much later, also tagged as the date's original release.
        (
            obs_date,
            dec("31"),
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            true,
        ),
        (
            obs_date,
            dec("32"),
            NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            false,
        ),
    ]
    .into_iter()
    .map(|(date, value, revision_date, original)| {
        (
            data_points::series_id.eq(series.id),
            data_points::date.eq(date),
            data_points::value.eq(Some(value)),
            data_points::revision_date.eq(revision_date),
            data_points::is_original_release.eq(original),
        )
    })
    .collect::<Vec<_>>();
    let mut conn = pool.get().await.expect("connection");
    diesel::insert_into(data_points::table)
        .values(&rows)
        .execute(&mut conn)
        .await
        .expect("insert data points");

    let mut conn = connect().await;
    let mut req = CrossSectionRequest {
        dataset_id: dataset.id,
        measure: None,
        filter: BTreeMap::new(),
        across: "area".to_string(),
        date: CrossSectionDate::On(obs_date),
        as_of: Some(NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
    };
    let result = cross_section(&mut conn, areas(), &req).await.unwrap();
    // As of 2026-07-01, the real vintage known then (31) already beats the legacy row on
    // revision_date, exclusion or not.
    assert_eq!(result[0].value, Some(dec("31")));

    // As of the legacy row's own revision_date, no real vintage is visible yet, so without
    // exclusion the legacy row would be the only candidate and would win. Exclusion must drop
    // it rather than let it stand in for "known on this day".
    req.as_of = Some(obs_date);
    let result = cross_section(&mut conn, areas(), &req).await.unwrap();
    assert_eq!(result[0].value, None);
}

#[tokio::test]
async fn areas_come_from_the_reference_file_with_aggregates_marked() {
    let seeded = seed().await;
    let mut conn = connect().await;

    let rows = cross_section(
        &mut conn,
        areas(),
        &request(&seeded, "POP", CrossSectionDate::Latest),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), AREAS.len());

    let by_key: BTreeMap<_, _> = rows.iter().map(|r| (r.key.as_str(), r)).collect();
    let usa = by_key["USA"].area.as_ref().expect("USA area");
    assert_eq!(usa.kind, AreaKind::Country);
    assert_eq!(usa.name, "United States");
    assert_eq!(usa.iso3.as_deref(), Some("USA"));
    assert_eq!(usa.iso_numeric, Some(840));

    let wld = by_key["WLD"].area.as_ref().expect("WLD area");
    assert_eq!(wld.kind, AreaKind::Aggregate);
    assert_eq!(wld.name, "World", "named by the dataset");
    assert_eq!(wld.iso3, None);
    assert_eq!(by_key["WLD"].value, Some(dec("8000")));
}

#[tokio::test]
async fn area_is_none_when_across_is_not_the_countries_codelist() {
    let seeded = seed().await;
    let mut conn = connect().await;

    let rows = cross_section(
        &mut conn,
        areas(),
        &CrossSectionRequest {
            dataset_id: seeded.dataset_id,
            measure: Some(VALUE_MEASURE.to_string()),
            filter: [("area".to_string(), "USA".to_string())]
                .into_iter()
                .collect(),
            across: "indicator".to_string(),
            date: CrossSectionDate::Latest,
            as_of: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        summary(&rows),
        vec![
            ("GDP", Some(d(2023)), Some(dec("110"))),
            ("POP", Some(d(2023)), Some(dec("335"))),
        ]
    );
    assert!(rows.iter().all(|r| r.area.is_none()));
}

#[tokio::test]
async fn unknown_dataset_is_not_found() {
    let mut conn = connect().await;
    let err = cross_section(
        &mut conn,
        areas(),
        &CrossSectionRequest {
            dataset_id: Uuid::new_v4(),
            measure: None,
            filter: BTreeMap::new(),
            across: "area".to_string(),
            date: CrossSectionDate::Latest,
            as_of: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn invalid_requests_are_bad_requests() {
    let seeded = seed().await;
    let mut conn = connect().await;
    let base = request(&seeded, "GDP", CrossSectionDate::Latest);

    let mut unknown_across = base.clone();
    unknown_across.across = "sex".to_string();

    let mut filter_sets_across = base.clone();
    filter_sets_across
        .filter
        .insert("area".to_string(), "USA".to_string());

    let mut unknown_filter = base.clone();
    unknown_filter
        .filter
        .insert("sex".to_string(), "F".to_string());

    let mut unpinned = base.clone();
    unpinned.filter.clear();

    let mut unknown_measure = base.clone();
    unknown_measure.measure = Some("obs_status".to_string());

    for (name, bad) in [
        ("unknown across", unknown_across),
        ("filter sets across", filter_sets_across),
        ("unknown filter dimension", unknown_filter),
        ("dimension left unset", unpinned),
        ("unknown measure", unknown_measure),
    ] {
        let err = cross_section(&mut conn, areas(), &bad).await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)), "{name}: {err:?}");
    }
}

#[tokio::test]
async fn a_declared_measure_other_than_value_is_a_bad_request() {
    // Train 1 stores every dataset long, so a dataset that declares a second measure (not
    // yet written to data_points) must still be rejected, not just an undeclared name.
    let pool = create_pool(&migrated_url().await).await.expect("pool");
    let source = DataSource::create(
        &pool,
        NewDataSource {
            name: format!("Cross-section Multi-measure Test Source {}", Uuid::new_v4()),
            description: None,
            base_url: "https://cross-section.example.com/api".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: None,
            api_key_name: None,
        },
    )
    .await
    .expect("create data source");
    let dataset = Dataset::create(
        &pool,
        &NewDataset {
            source_id: source.id,
            code: "multi_measure".to_string(),
            name: "Multi-measure Test Dataset".to_string(),
            description: None,
            dimensions: econ_graph_core::models::dataset::DatasetComponents(vec![
                DatasetComponent::new("area", "Area", ComponentType::String),
            ]),
            measures: econ_graph_core::models::dataset::DatasetComponents(vec![
                DatasetComponent::value_measure(),
                DatasetComponent::new("delta", "Delta", ComponentType::Decimal),
            ]),
            attributes: econ_graph_core::models::dataset::DatasetComponents::default(),
            default_measure: VALUE_MEASURE.to_string(),
        },
    )
    .await
    .expect("create dataset");

    let mut conn = connect().await;
    let err = cross_section(
        &mut conn,
        areas(),
        &CrossSectionRequest {
            dataset_id: dataset.id,
            measure: Some("delta".to_string()),
            filter: BTreeMap::new(),
            across: "area".to_string(),
            date: CrossSectionDate::Latest,
            as_of: None,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, AppError::BadRequest(_)), "{err:?}");
}

#[tokio::test]
async fn query_count_does_not_grow_with_the_number_of_keys() {
    let seeded = seed().await;
    let mut conn = connect().await;
    let queries = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&queries);
    conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
        if matches!(event, InstrumentationEvent::StartQuery { .. }) {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    for (indicator, keys) in [("GDP", AREAS.len()), ("NONE", 0)] {
        queries.store(0, Ordering::SeqCst);
        let rows = cross_section(
            &mut conn,
            areas(),
            &request(&seeded, indicator, CrossSectionDate::Latest),
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), keys);
        assert_eq!(
            AtomicUsize::load(&queries, Ordering::SeqCst),
            2,
            "{indicator}"
        );
    }
}
