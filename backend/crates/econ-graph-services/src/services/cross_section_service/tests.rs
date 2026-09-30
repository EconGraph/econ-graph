// Database tests for cross-sections.
//
// These need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database (CI runs them against its Postgres service). They run the embedded migrations
// once, never drop the schema, and give every test its own data source.
//
// The seeded dataset is `wdi` with dimensions `indicator` (GDP, POP) and `area` (the
// countries code list), over BRA, DEU, JPN, USA and the aggregate WLD:
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
                dataset_id: Some(dataset.id),
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
            dataset_id: Some(dataset.id),
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
            dataset_id: Some(dataset.id),
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

    for (indicator, keys, date) in [
        ("GDP", AREAS.len(), CrossSectionDate::Latest),
        ("POP", AREAS.len(), CrossSectionDate::Latest),
        ("NONE", 0, CrossSectionDate::Latest),
        ("GDP", AREAS.len(), CrossSectionDate::On(d(2023))),
        ("POP", AREAS.len(), CrossSectionDate::On(d(2023))),
        ("NONE", 0, CrossSectionDate::On(d(2023))),
    ] {
        queries.store(0, Ordering::SeqCst);
        let rows = cross_section(&mut conn, areas(), &request(&seeded, indicator, date))
            .await
            .unwrap();
        assert_eq!(rows.len(), keys);
        assert_eq!(
            AtomicUsize::load(&queries, Ordering::SeqCst),
            2,
            "{indicator} {date:?}"
        );
    }
}
