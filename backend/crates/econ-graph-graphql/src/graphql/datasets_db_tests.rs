// Database tests for the dataset GraphQL fields and their loaders.
//
// These need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database. They run the embedded migrations once (idempotent), never drop the schema, and
// give every test its own data source so rows from other tests can't interfere.

use crate::graphql::datasets::{DatasetBatcher, SeriesDatasetFields, SeriesDatasetFieldsBatcher};
use crate::graphql::schema::create_schema;
use async_graphql::Request;
use dataloader::non_cached::Loader;
use dataloader::BatchFn;
use econ_graph_core::database::{create_pool, run_migrations, DatabasePool};
use econ_graph_core::models::{
    ComponentType, DataSource, Dataset, DatasetComponent, EconomicSeries, NewDataSource,
    NewDataset, NewEconomicSeries, SeriesDimensions,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
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
    state.codes = Some(BTreeMap::from([
        ("06".to_string(), "California".to_string()),
        ("36".to_string(), "New York".to_string()),
    ]));
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
