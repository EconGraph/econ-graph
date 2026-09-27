// Database tests for the dataset GraphQL fields and their loaders.
//
// These need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database. They run the embedded migrations once (idempotent), never drop the schema, and
// give every test its own data source so rows from other tests can't interfere.

use crate::graphql::dataloaders::DataLoaders;
use crate::graphql::datasets::{DatasetBatcher, SeriesDatasetFields, SeriesDatasetFieldsBatcher};
use crate::graphql::schema::{create_schema, GraphQLContext};
use crate::security::{SecurityConfig, SecurityMiddleware};
use crate::types::EconomicSeriesType;
use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use dataloader::non_cached::Loader;
use dataloader::BatchFn;
use econ_graph_core::database::{create_pool, run_migrations, DatabasePool};
use econ_graph_core::models::COUNTRIES_CODELIST;
use econ_graph_core::models::{
    Code, ComponentType, DataSource, Dataset, DatasetComponent, EconomicSeries, NewDataSource,
    NewDataset, NewEconomicSeries, SeriesDimensions, SeriesSearchResult,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

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
            "refusing to run dataset tests against database {db_name:?}"
        );
        url
    });
    MIGRATED
        .get_or_init(|| async {
            run_migrations(&url).await.expect("migrations failed");
        })
        .await;
    create_pool(&url)
        .await
        .expect("Failed to connect to test database")
}

async fn new_source(pool: &DatabasePool) -> DataSource {
    DataSource::create(
        pool,
        NewDataSource {
            name: format!("GraphQL Dataset Test Source {}", Uuid::new_v4()),
            description: None,
            base_url: "https://datasets.example.com/api".to_string(),
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
    .expect("create data source")
}

async fn bds_dataset(pool: &DatabasePool, source_id: Uuid) -> Dataset {
    let mut state = DatasetComponent::new("state", "State", ComponentType::String);
    state.codes = Some(vec![
        Code::new("06", "California"),
        Code::new("36", "New York"),
    ]);
    let mut new_dataset = NewDataset::long(
        source_id,
        "BDS",
        "Business Dynamics Statistics",
        vec![
            DatasetComponent::new("geo_level", "Geographic level", ComponentType::String),
            state,
            DatasetComponent::new("variable", "Variable", ComponentType::String),
        ],
    );
    new_dataset.description = Some("Census BDS, stored long".to_string());
    new_dataset.measures.0[0].unit = Some("Count".to_string());
    new_dataset.attributes.0.push(DatasetComponent::new(
        "footnote",
        "Footnote",
        ComponentType::String,
    ));
    Dataset::create(pool, &new_dataset)
        .await
        .expect("create dataset")
}

async fn new_series(
    pool: &DatabasePool,
    source_id: Uuid,
    external_id: &str,
    dataset: Option<(Uuid, SeriesDimensions)>,
) -> EconomicSeries {
    let (dataset_id, dimensions) = dataset.unzip();
    EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id,
            external_id: external_id.to_string(),
            title: format!("Series {external_id}"),
            frequency: "Annual".to_string(),
            dataset_id,
            dimensions: dimensions.unwrap_or_default(),
            ..Default::default()
        },
    )
    .await
    .expect("create series")
}

fn state_dims(state: &str) -> SeriesDimensions {
    [
        ("geo_level", "state"),
        ("state", state),
        ("variable", "ESTAB"),
    ]
    .into_iter()
    .collect()
}

async fn execute(pool: &DatabasePool, query: &str) -> Value {
    let schema = create_schema(pool.clone());
    let response = schema.execute(Request::new(query)).await;
    assert!(
        response.errors.is_empty(),
        "GraphQL errors: {:?}",
        response.errors
    );
    response.data.into_json().expect("response data as JSON")
}

#[tokio::test]
async fn datasets_and_dataset_queries_return_components_with_code_labels() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;

    let data = execute(
        &pool,
        &format!(
            r#"{{
                datasets(sourceId: "{source}") {{
                    id code name description defaultMeasure
                    source {{ id }}
                    dimensions {{ name label type unit codes {{ code label }} }}
                    measures {{ name label type unit }}
                    attributes {{ name }}
                }}
                dataset(id: "{dataset}") {{ code }}
                missing: dataset(id: "{missing}") {{ code }}
            }}"#,
            source = source.id,
            dataset = dataset.id,
            missing = Uuid::new_v4(),
        ),
    )
    .await;

    assert_eq!(
        data["datasets"],
        json!([{
            "id": dataset.id.to_string(),
            "code": "BDS",
            "name": "Business Dynamics Statistics",
            "description": "Census BDS, stored long",
            "defaultMeasure": "value",
            "source": { "id": source.id.to_string() },
            "dimensions": [
                { "name": "geo_level", "label": "Geographic level", "type": "STRING",
                  "unit": null, "codes": [] },
                { "name": "state", "label": "State", "type": "STRING", "unit": null,
                  "codes": [
                      { "code": "06", "label": "California" },
                      { "code": "36", "label": "New York" }
                  ] },
                { "name": "variable", "label": "Variable", "type": "STRING",
                  "unit": null, "codes": [] }
            ],
            "measures": [
                { "name": "value", "label": "Value", "type": "DECIMAL", "unit": "Count" }
            ],
            "attributes": [{ "name": "footnote" }]
        }])
    );
    assert_eq!(data["dataset"], json!({ "code": "BDS" }));
    assert_eq!(data["missing"], Value::Null);
}

#[tokio::test]
async fn series_fields_resolve_dataset_labelled_dimensions_and_default_measure() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;
    let california = new_series(
        &pool,
        source.id,
        "BDS/state.06.ESTAB",
        Some((dataset.id, state_dims("06"))),
    )
    .await;
    let unlabelled = new_series(
        &pool,
        source.id,
        "BDS/state.72.ESTAB",
        Some((dataset.id, state_dims("72"))),
    )
    .await;
    let plain = new_series(&pool, source.id, "PLAIN", None).await;

    let fields = "dataset { id code } dimensions { name label value valueLabel } defaultMeasure";
    let data = execute(
        &pool,
        &format!(
            r#"{{
                ca: series(id: "{california}") {{ {fields} }}
                pr: series(id: "{unlabelled}") {{ dimensions {{ name value valueLabel }} }}
                plain: series(id: "{plain}") {{ {fields} }}
            }}"#,
            california = california.id,
            unlabelled = unlabelled.id,
            plain = plain.id,
        ),
    )
    .await;

    assert_eq!(
        data["ca"],
        json!({
            "dataset": { "id": dataset.id.to_string(), "code": "BDS" },
            "dimensions": [
                { "name": "geo_level", "label": "Geographic level", "value": "state",
                  "valueLabel": null },
                { "name": "state", "label": "State", "value": "06",
                  "valueLabel": "California" },
                { "name": "variable", "label": "Variable", "value": "ESTAB",
                  "valueLabel": null }
            ],
            "defaultMeasure": "value"
        })
    );
    // A code the dataset has no label for keeps its value and a null label.
    assert_eq!(
        data["pr"]["dimensions"][1],
        json!({ "name": "state", "value": "72", "valueLabel": null })
    );
    assert_eq!(
        data["plain"],
        json!({ "dataset": null, "dimensions": [], "defaultMeasure": null })
    );
}

/// A WDI-shaped dataset: indicators coded inline with units, areas from the countries list.
async fn wdi_dataset(pool: &DatabasePool, source_id: Uuid) -> Dataset {
    let mut indicator = DatasetComponent::new("indicator", "Indicator", ComponentType::String);
    let mut gdp = Code::new("NY.GDP.PCAP.CD", "GDP per capita");
    gdp.unit = Some("current US$".to_string());
    gdp.description = Some("GDP divided by midyear population".to_string());
    indicator.codes = Some(vec![gdp]);
    let mut area = DatasetComponent::new("area", "Country or area", ComponentType::String);
    area.codelist = Some(COUNTRIES_CODELIST.to_string());
    Dataset::create(
        pool,
        &NewDataset::long(
            source_id,
            "WDI",
            "World Development Indicators",
            vec![indicator, area],
        ),
    )
    .await
    .expect("create dataset")
}

#[tokio::test]
async fn code_lists_resolve_inline_and_shared_codes_alike() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = wdi_dataset(&pool, source.id).await;
    let germany = new_series(
        &pool,
        source.id,
        "WDI/NY.GDP.PCAP.CD.DEU",
        Some((
            dataset.id,
            [("indicator", "NY.GDP.PCAP.CD"), ("area", "DEU")]
                .into_iter()
                .collect(),
        )),
    )
    .await;

    let data = execute(
        &pool,
        &format!(
            r#"{{
                dataset(id: "{dataset}") {{
                    dimensions {{ name codelist codes {{ code label unit description }} }}
                }}
                series(id: "{germany}") {{
                    dimensions {{ name label value valueLabel valueUnit }}
                }}
            }}"#,
            dataset = dataset.id,
            germany = germany.id,
        ),
    )
    .await;

    let dimensions = &data["dataset"]["dimensions"];
    assert_eq!(
        dimensions[0],
        json!({
            "name": "indicator",
            "codelist": null,
            "codes": [{
                "code": "NY.GDP.PCAP.CD",
                "label": "GDP per capita",
                "unit": "current US$",
                "description": "GDP divided by midyear population"
            }]
        })
    );
    assert_eq!(dimensions[1]["codelist"], "countries");
    let areas = dimensions[1]["codes"].as_array().expect("area codes");
    assert!(areas.len() > 249, "countries and aggregates listed");
    assert!(areas.contains(
        &json!({ "code": "DEU", "label": "Germany", "unit": null, "description": null })
    ));

    assert_eq!(
        data["series"]["dimensions"],
        json!([
            { "name": "indicator", "label": "Indicator", "value": "NY.GDP.PCAP.CD",
              "valueLabel": "GDP per capita", "valueUnit": "current US$" },
            { "name": "area", "label": "Country or area", "value": "DEU",
              "valueLabel": "Germany", "valueUnit": null }
        ])
    );
}

#[tokio::test]
async fn series_default_measure_override_wins_over_dataset() {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::economic_series;

    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;
    let series = new_series(
        &pool,
        source.id,
        "BDS/state.06.ESTAB",
        Some((dataset.id, state_dims("06"))),
    )
    .await;
    let mut conn = pool.get().await.expect("connection");
    diesel::update(economic_series::table.find(series.id))
        .set(economic_series::default_measure.eq("rate"))
        .execute(&mut conn)
        .await
        .expect("set override");

    let data = execute(
        &pool,
        &format!(r#"{{ series(id: "{}") {{ defaultMeasure }} }}"#, series.id),
    )
    .await;
    assert_eq!(data["series"]["defaultMeasure"], "rate");
}

#[tokio::test]
async fn datasets_query_orders_by_code_and_lists_all_sources_without_filter() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let other = new_source(&pool).await;
    let bds = bds_dataset(&pool, source.id).await;
    let abs = Dataset::create(
        &pool,
        &NewDataset::long(source.id, "ABS", "A dataset", vec![]),
    )
    .await
    .expect("create dataset");
    let elsewhere = bds_dataset(&pool, other.id).await;

    let data = execute(
        &pool,
        &format!(
            r#"{{
                mine: datasets(sourceId: "{source}") {{ code }}
                all: datasets {{ id }}
            }}"#,
            source = source.id,
        ),
    )
    .await;

    assert_eq!(data["mine"], json!([{ "code": "ABS" }, { "code": "BDS" }]));
    let all: Vec<&str> = data["all"]
        .as_array()
        .expect("all datasets")
        .iter()
        .filter_map(|d| d["id"].as_str())
        .collect();
    for dataset in [&bds, &abs, &elsewhere] {
        assert!(all.contains(&dataset.id.to_string().as_str()));
    }
}

/// Serves series built the way search results are: without their dataset columns. The test
/// using it checks the fallback loader's output; batching is proven by the loader tests below.
struct SearchLikeQuery {
    results: Vec<SeriesSearchResult>,
}

#[async_graphql::Object]
impl SearchLikeQuery {
    async fn found(&self) -> Vec<EconomicSeriesType> {
        self.results
            .iter()
            .cloned()
            .map(EconomicSeriesType::from)
            .collect()
    }
}

#[tokio::test]
async fn search_results_load_dataset_fields_by_series_id() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;
    let mut results = Vec::new();
    for state in ["06", "36"] {
        let series = new_series(
            &pool,
            source.id,
            &format!("BDS/state.{state}.ESTAB"),
            Some((dataset.id, state_dims(state))),
        )
        .await;
        results.push(SeriesSearchResult {
            id: series.id,
            title: series.title,
            description: None,
            external_id: series.external_id,
            source_id: series.source_id,
            frequency: series.frequency,
            units: String::new(),
            start_date: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            end_date: None,
            last_updated: chrono::Utc::now().naive_utc(),
            is_active: true,
            rank: 1.0,
            similarity_score: 1.0,
        });
    }

    let context = GraphQLContext {
        pool: Arc::new(pool.clone()),
        data_loaders: Arc::new(DataLoaders::new(pool.clone())),
        security: Arc::new(SecurityMiddleware::new(SecurityConfig::default())),
    };
    let schema = Schema::build(
        SearchLikeQuery { results },
        EmptyMutation,
        EmptySubscription,
    )
    .data(context)
    .data(pool.clone())
    .finish();
    let response = schema
        .execute("{ found { dataset { code } dimensions { name valueLabel } defaultMeasure } }")
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().expect("JSON");

    let found = data["found"].as_array().expect("series");
    assert_eq!(found.len(), 2);
    for (series, label) in found.iter().zip(["California", "New York"]) {
        assert_eq!(series["dataset"], json!({ "code": "BDS" }));
        assert_eq!(
            series["dimensions"][1],
            json!({ "name": "state", "valueLabel": label })
        );
        assert_eq!(series["defaultMeasure"], "value");
    }
}

/// Wraps a batch function and counts how many batches (one SQL query each) it runs.
struct Counting<B> {
    inner: B,
    batches: Arc<AtomicUsize>,
}

impl<K, V, B: BatchFn<K, V>> BatchFn<K, V> for Counting<B> {
    fn load(&mut self, keys: &[K]) -> impl std::future::Future<Output = HashMap<K, V>> {
        self.batches.fetch_add(1, Ordering::SeqCst);
        self.inner.load(keys)
    }
}

#[tokio::test]
async fn dataset_loader_runs_one_query_per_batch() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;
    let missing = Uuid::new_v4();

    let batches = Arc::new(AtomicUsize::new(0));
    let loader = Loader::new(Counting {
        inner: DatasetBatcher { pool: pool.clone() },
        batches: batches.clone(),
    });

    let keys = [dataset.id, missing, dataset.id, missing, dataset.id];
    let results = futures::future::join_all(keys.iter().map(|&id| loader.try_load(id))).await;

    assert_eq!(
        batches.load(Ordering::SeqCst),
        1,
        "one batch for concurrent loads"
    );
    for (key, result) in keys.iter().zip(results) {
        let loaded = result.expect("key present in batch result");
        assert_eq!(
            loaded.map(|d| d.id),
            (*key != missing).then_some(dataset.id)
        );
    }
}

#[tokio::test]
async fn series_dataset_fields_loader_runs_one_query_per_batch() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = bds_dataset(&pool, source.id).await;
    let mut ids = Vec::new();
    for state in ["06", "36", "48"] {
        let series = new_series(
            &pool,
            source.id,
            &format!("BDS/state.{state}.ESTAB"),
            Some((dataset.id, state_dims(state))),
        )
        .await;
        ids.push((series.id, state));
    }
    let plain = new_series(&pool, source.id, "PLAIN", None).await;

    let batches = Arc::new(AtomicUsize::new(0));
    let loader = Loader::new(Counting {
        inner: SeriesDatasetFieldsBatcher { pool: pool.clone() },
        batches: batches.clone(),
    });

    let keys: Vec<Uuid> = ids.iter().map(|(id, _)| *id).chain([plain.id]).collect();
    let results = futures::future::join_all(keys.iter().map(|&id| loader.try_load(id))).await;

    assert_eq!(
        batches.load(Ordering::SeqCst),
        1,
        "one batch for concurrent loads"
    );
    for ((_, state), result) in ids.iter().zip(&results) {
        let fields = result
            .as_ref()
            .expect("key present")
            .clone()
            .expect("series found");
        assert_eq!(
            fields,
            SeriesDatasetFields {
                dataset_id: Some(dataset.id),
                dimensions: state_dims(state),
                default_measure: None,
            }
        );
    }
    let plain_fields = results[3].as_ref().expect("key present").clone();
    assert_eq!(plain_fields, Some(SeriesDatasetFields::default()));
}
