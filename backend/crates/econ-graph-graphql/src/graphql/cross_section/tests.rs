// The crossSection query through the GraphQL schema. The service's own tests cover the
// cross-section semantics; these check the wiring: registration, argument handling, the
// area fields and error codes.
//
// They need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database, and run the embedded migrations once without dropping the schema.

use async_graphql::{Request, Variables};
use diesel::ExpressionMethods;
use econ_graph_core::database::{create_pool, run_migrations};
use econ_graph_core::models::dataset::{
    ComponentType, Dataset, DatasetComponent, NewDataset, SeriesDimensions,
};
use econ_graph_core::models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries};
use econ_graph_core::schema::data_points;
use econ_graph_services::services::cross_section_service::COUNTRIES_CODELIST;
use serde_json::{json, Value};

use super::*;
use crate::graphql::schema::create_schema;

static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn test_pool() -> DatabasePool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
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
            "refusing to run crossSection tests against database {db_name:?}"
        );
        url
    });
    MIGRATED
        .get_or_init(|| async {
            run_migrations(&url).await.expect("migrations failed");
        })
        .await;
    create_pool(&url).await.expect("connect to test database")
}

/// A dataset `wdi` (dimensions `indicator` and `area`) with GDP for USA (2023: 1.5) and WLD
/// (no observations). Returns the dataset id.
async fn seed(pool: &DatabasePool) -> Uuid {
    let source = DataSource::create(
        pool,
        NewDataSource {
            name: format!("crossSection GraphQL Test Source {}", Uuid::new_v4()),
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

    let mut area = DatasetComponent::new("area", "Area", ComponentType::String);
    area.codelist = Some(COUNTRIES_CODELIST.to_string());
    let dataset = Dataset::create(
        pool,
        &NewDataset::long(
            source.id,
            "wdi",
            "World Development Indicators",
            vec![
                DatasetComponent::new("indicator", "Indicator", ComponentType::String),
                area,
            ],
        ),
    )
    .await
    .expect("create dataset");

    let mut usa_id = None;
    for key in ["USA", "WLD"] {
        let series = EconomicSeries::create(
            pool,
            &NewEconomicSeries {
                source_id: source.id,
                external_id: format!("GDP.{key}"),
                title: format!("GDP {key}"),
                frequency: "Annual".to_string(),
                dataset_id: Some(dataset.id),
                dimensions: [("indicator", "GDP"), ("area", key)]
                    .into_iter()
                    .collect::<SeriesDimensions>(),
                ..Default::default()
            },
        )
        .await
        .expect("create series");
        if key == "USA" {
            usa_id = Some(series.id);
        }
    }

    let date = NaiveDate::from_ymd_opt(2023, 12, 31).unwrap();
    let mut conn = pool.get().await.expect("connection");
    // Called by path: RunQueryDsl in scope would shadow Schema::execute and Vec::first.
    let insert = diesel::insert_into(data_points::table).values((
        data_points::series_id.eq(usa_id.unwrap()),
        data_points::date.eq(date),
        data_points::value.eq(Some("1.5".parse::<BigDecimal>().unwrap())),
        data_points::revision_date.eq(date),
        data_points::is_original_release.eq(true),
    ));
    diesel_async::RunQueryDsl::execute(insert, &mut conn)
        .await
        .expect("insert data point");

    dataset.id
}

const QUERY: &str = r#"
query ($datasetId: ID!, $date: NaiveDate, $latest: Boolean) {
  crossSection(
    datasetId: $datasetId
    filter: [{dimension: "indicator", value: "GDP"}]
    across: "area"
    date: $date
    latest: $latest
  ) {
    key
    area { name iso3 isoNumeric kind }
    date
    value
    flags { name value }
  }
}"#;

async fn run(pool: &DatabasePool, variables: Value) -> async_graphql::Response {
    create_schema(pool.clone())
        .execute(Request::new(QUERY).variables(Variables::from_json(variables)))
        .await
}

fn error_code(response: &async_graphql::Response) -> Option<String> {
    let error = response.errors.first()?;
    match error.extensions.as_ref()?.get("code")? {
        async_graphql::Value::String(code) => Some(code.clone()),
        _ => None,
    }
}

#[tokio::test]
async fn cross_section_returns_areas_and_values() {
    let pool = test_pool().await;
    let dataset_id = seed(&pool).await;

    let response = run(
        &pool,
        json!({"datasetId": dataset_id.to_string(), "latest": true}),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(
        data,
        json!({"crossSection": [
            {
                "key": "USA",
                "area": {"name": "United States", "iso3": "USA", "isoNumeric": 840, "kind": "COUNTRY"},
                "date": "2023-12-31",
                "value": "1.500000",
                "flags": [],
            },
            {
                "key": "WLD",
                "area": {"name": "World", "iso3": null, "isoNumeric": null, "kind": "AGGREGATE"},
                "date": null,
                "value": null,
                "flags": [],
            },
        ]})
    );
}

#[tokio::test]
async fn date_and_latest_are_exclusive() {
    let pool = test_pool().await;
    let dataset_id = seed(&pool).await;

    for variables in [
        json!({"datasetId": dataset_id.to_string()}),
        json!({"datasetId": dataset_id.to_string(), "latest": false}),
        json!({"datasetId": dataset_id.to_string(), "date": "2023-12-31", "latest": true}),
    ] {
        let response = run(&pool, variables.clone()).await;
        assert_eq!(
            error_code(&response).as_deref(),
            Some("BAD_REQUEST"),
            "{variables}"
        );
    }

    let response = run(
        &pool,
        json!({"datasetId": dataset_id.to_string(), "date": "2023-12-31"}),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

#[tokio::test]
async fn a_filter_naming_the_same_dimension_twice_is_a_bad_request() {
    const DUPLICATE_FILTER_QUERY: &str = r#"
query ($datasetId: ID!) {
  crossSection(
    datasetId: $datasetId
    filter: [
      {dimension: "indicator", value: "GDP"}
      {dimension: "indicator", value: "POP"}
    ]
    across: "area"
    latest: true
  ) {
    key
  }
}"#;

    let pool = test_pool().await;
    let dataset_id = seed(&pool).await;
    let response = create_schema(pool)
        .execute(
            Request::new(DUPLICATE_FILTER_QUERY).variables(Variables::from_json(
                json!({"datasetId": dataset_id.to_string()}),
            )),
        )
        .await;
    assert_eq!(error_code(&response).as_deref(), Some("BAD_REQUEST"));
}

#[tokio::test]
async fn unknown_dataset_is_not_found() {
    let pool = test_pool().await;
    let response = run(
        &pool,
        json!({"datasetId": Uuid::new_v4().to_string(), "latest": true}),
    )
    .await;
    assert_eq!(error_code(&response).as_deref(), Some("NOT_FOUND"));

    let response = run(&pool, json!({"datasetId": "not-a-uuid", "latest": true})).await;
    assert_eq!(error_code(&response).as_deref(), Some("BAD_REQUEST"));
}
