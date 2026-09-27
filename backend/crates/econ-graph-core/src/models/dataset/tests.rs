// Database tests for datasets and dataset columns on series.
//
// These need a reachable Postgres in TEST_DATABASE_URL, or DATABASE_URL naming a *test*
// database. They run the embedded migrations once (idempotent), never drop the schema, and
// give every test its own data source so rows from earlier runs can't interfere.

use super::*;
use crate::models::data_source::{DataSource, NewDataSource};
use crate::models::economic_series::{EconomicSeries, NewEconomicSeries};
use crate::models::series_metadata::{NewSeriesMetadata, SeriesMetadata};
use crate::schema::{economic_series, series_metadata};
use diesel::result::Error as DieselError;

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
            crate::database::run_migrations(&url)
                .await
                .expect("migrations failed");
        })
        .await;
    crate::database::create_pool(&url)
        .await
        .expect("Failed to connect to test database")
}

async fn new_source(pool: &DatabasePool) -> DataSource {
    DataSource::create(
        pool,
        NewDataSource {
            name: format!("Dataset Test Source {}", Uuid::new_v4()),
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

fn bds_dataset(source_id: Uuid) -> NewDataset {
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
    new_dataset
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

fn new_series(source_id: Uuid, external_id: &str) -> NewEconomicSeries {
    NewEconomicSeries {
        source_id,
        external_id: external_id.to_string(),
        title: format!("Series {external_id}"),
        frequency: "Annual".to_string(),
        ..Default::default()
    }
}

fn new_metadata(source_id: Uuid, external_id: &str) -> NewSeriesMetadata {
    NewSeriesMetadata {
        source_id,
        external_id: external_id.to_string(),
        title: format!("Series {external_id}"),
        description: None,
        units: None,
        frequency: Some("Annual".to_string()),
        geographic_level: None,
        data_url: None,
        api_endpoint: None,
        is_active: true,
        dataset_id: None,
        dimensions: SeriesDimensions::default(),
        default_measure: None,
    }
}

/// The constraint a failed write violated.
fn violated<T: std::fmt::Debug>(result: Result<T, DieselError>) -> String {
    match result {
        Err(DieselError::DatabaseError(_, info)) => info
            .constraint_name()
            .unwrap_or_else(|| panic!("no constraint named: {}", info.message()))
            .to_string(),
        other => panic!("expected a constraint violation, got {other:?}"),
    }
}

fn violated_app<T: std::fmt::Debug>(result: AppResult<T>) -> String {
    match result {
        Err(AppError::Database(e)) => violated::<T>(Err(e)),
        other => panic!("expected a database error, got {other:?}"),
    }
}

#[tokio::test]
async fn dataset_round_trips_components() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let new_dataset = bds_dataset(source.id);

    let created = Dataset::create(&pool, &new_dataset).await.expect("create");
    assert_eq!(created.code, "BDS");
    assert_eq!(created.dimensions, new_dataset.dimensions);
    assert_eq!(created.measures, new_dataset.measures);
    assert_eq!(created.attributes, new_dataset.attributes);
    assert_eq!(created.default_measure, VALUE_MEASURE);

    let by_id = Dataset::find_by_id(&pool, created.id).await.unwrap();
    assert_eq!(by_id.as_ref(), Some(&created));
    let by_code = Dataset::find_by_code(&pool, source.id, "BDS")
        .await
        .unwrap();
    assert_eq!(by_code, Some(created.clone()));
    assert_eq!(
        Dataset::find_by_code(&pool, source.id, "NOPE")
            .await
            .unwrap(),
        None
    );

    // Dimensions keep their declared order, which the canonical series key relies on.
    let names: Vec<_> = created
        .dimensions
        .0
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    assert_eq!(names, ["geo_level", "state", "variable"]);
    assert_eq!(
        created.dimensions.0[1].codes.as_ref().unwrap()[0],
        Code::new("06", "California")
    );

    assert_eq!(
        violated_app(Dataset::create(&pool, &new_dataset).await),
        "datasets_source_code_key"
    );
}

#[tokio::test]
async fn database_rejects_undeclared_default_measure() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let mut new_dataset = NewDataset::long(source.id, "X", "X", vec![]);
    new_dataset.default_measure = "firms".to_string();

    // Bypass validate_components to check the database constraint itself.
    let mut conn = pool.get().await.unwrap();
    let result = diesel::insert_into(datasets::table)
        .values(&new_dataset)
        .execute(&mut conn)
        .await;
    assert_eq!(violated(result), "datasets_default_measure_declared");
}

#[tokio::test]
async fn dimensioned_series_round_trips_and_filters() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(source.id))
        .await
        .unwrap();

    let mut ca = new_series(source.id, "BDS/state.06.ESTAB");
    ca.dataset_id = Some(dataset.id);
    ca.dimensions = state_dims("06");
    let ca = EconomicSeries::create(&pool, &ca)
        .await
        .expect("create series");
    assert_eq!(ca.dataset_id, Some(dataset.id));
    assert_eq!(ca.dimensions, state_dims("06"));
    assert_eq!(ca.default_measure, None);

    let mut ny = new_series(source.id, "BDS/state.36.ESTAB");
    ny.dataset_id = Some(dataset.id);
    ny.dimensions = state_dims("36");
    ny.default_measure = Some(VALUE_MEASURE.to_string());
    let ny = EconomicSeries::create(&pool, &ny).await.unwrap();
    assert_eq!(ny.default_measure.as_deref(), Some(VALUE_MEASURE));

    // Containment filter, as crossSection will use (served by the GIN index).
    let filter: SeriesDimensions = [("state", "06")].into_iter().collect();
    let mut conn = pool.get().await.unwrap();
    let found: Vec<Uuid> = economic_series::table
        .filter(economic_series::dataset_id.eq(dataset.id))
        .filter(economic_series::dimensions.contains(filter))
        .select(economic_series::id)
        .load(&mut conn)
        .await
        .unwrap();
    assert_eq!(found, vec![ca.id]);

    // The same columns on series_metadata.
    let mut meta = new_metadata(source.id, "BDS/state.06.ESTAB");
    meta.dataset_id = Some(dataset.id);
    meta.dimensions = state_dims("06");
    let meta = SeriesMetadata::get_or_create(&pool, source.id, &meta.external_id.clone(), &meta)
        .await
        .expect("create metadata");
    assert_eq!(meta.dataset_id, Some(dataset.id));
    assert_eq!(meta.dimensions, state_dims("06"));

    // get_or_create on an existing row updates the dataset columns too.
    let mut rediscovered = new_metadata(source.id, "BDS/state.06.ESTAB");
    rediscovered.dataset_id = Some(dataset.id);
    rediscovered.dimensions = [
        ("geo_level", "state"),
        ("state", "06"),
        ("variable", "FIRM"),
    ]
    .into_iter()
    .collect();
    rediscovered.default_measure = Some(VALUE_MEASURE.to_string());
    let updated =
        SeriesMetadata::get_or_create(&pool, source.id, "BDS/state.06.ESTAB", &rediscovered)
            .await
            .expect("update metadata");
    assert_eq!(updated.id, meta.id);
    assert_eq!(updated.dataset_id, Some(dataset.id));
    assert_eq!(updated.dimensions, rediscovered.dimensions);
    assert_eq!(updated.default_measure.as_deref(), Some(VALUE_MEASURE));

    // A rediscovery with no dataset clears all three.
    let cleared = SeriesMetadata::get_or_create(
        &pool,
        source.id,
        "BDS/state.06.ESTAB",
        &new_metadata(source.id, "BDS/state.06.ESTAB"),
    )
    .await
    .expect("clear dataset columns");
    assert_eq!(cleared.dataset_id, None);
    assert!(cleared.dimensions.0.is_empty());
    assert_eq!(cleared.default_measure, None);
}

#[tokio::test]
async fn unique_index_rejects_duplicate_dimension_key() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(source.id))
        .await
        .unwrap();
    let mut other = bds_dataset(source.id);
    other.code = "BDS_OTHER".to_string();
    let other = Dataset::create(&pool, &other).await.unwrap();

    let mut first = new_series(source.id, "BDS/state.06.ESTAB");
    first.dataset_id = Some(dataset.id);
    first.dimensions = state_dims("06");
    EconomicSeries::create(&pool, &first).await.unwrap();

    let mut duplicate = new_series(source.id, "CENSUS_BDS_ESTAB_state_06");
    duplicate.dataset_id = Some(dataset.id);
    duplicate.dimensions = state_dims("06");
    assert_eq!(
        violated_app(EconomicSeries::create(&pool, &duplicate).await),
        "uq_economic_series_dataset_dimensions"
    );

    // The same key in another dataset is a different series.
    duplicate.dataset_id = Some(other.id);
    EconomicSeries::create(&pool, &duplicate).await.unwrap();

    // series_metadata has the same index.
    let mut conn = pool.get().await.unwrap();
    let mut meta = new_metadata(source.id, "BDS/state.06.ESTAB");
    meta.dataset_id = Some(dataset.id);
    meta.dimensions = state_dims("06");
    diesel::insert_into(series_metadata::table)
        .values(&meta)
        .execute(&mut conn)
        .await
        .expect("first metadata row");
    meta.external_id = "CENSUS_BDS_ESTAB_state_06".to_string();
    let result = diesel::insert_into(series_metadata::table)
        .values(&meta)
        .execute(&mut conn)
        .await;
    assert_eq!(violated(result), "uq_series_metadata_dataset_dimensions");
}

#[tokio::test]
async fn dimensionless_dataset_holds_many_series() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &NewDataset::long(source.id, "FRED", "FRED", vec![]))
        .await
        .unwrap();

    for external_id in ["GDP", "UNRATE"] {
        let mut series = new_series(source.id, external_id);
        series.dataset_id = Some(dataset.id);
        let series = EconomicSeries::create(&pool, &series).await.unwrap();
        assert!(series.dimensions.0.is_empty());
    }
}

#[tokio::test]
async fn series_dataset_must_belong_to_its_source() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let other = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(other.id))
        .await
        .unwrap();

    let mut series = new_series(source.id, "BDS/state.06.ESTAB");
    series.dataset_id = Some(dataset.id);
    series.dimensions = state_dims("06");
    assert_eq!(
        violated_app(EconomicSeries::create(&pool, &series).await),
        "economic_series_dataset_source_fkey"
    );

    let mut meta = new_metadata(source.id, "BDS/state.06.ESTAB");
    meta.dataset_id = Some(dataset.id);
    meta.dimensions = state_dims("06");
    let mut conn = pool.get().await.unwrap();
    let result = diesel::insert_into(series_metadata::table)
        .values(&meta)
        .execute(&mut conn)
        .await;
    assert_eq!(violated(result), "series_metadata_dataset_source_fkey");
}

#[tokio::test]
async fn database_rejects_malformed_dimensions() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(source.id))
        .await
        .unwrap();
    let mut conn = pool.get().await.unwrap();

    for table in ["economic_series", "series_metadata"] {
        let cases = [
            (
                Some(dataset.id),
                r#"{"state": 6}"#,
                "dimensions_is_string_object",
            ),
            (
                Some(dataset.id),
                r#"{"state": ["06"]}"#,
                "dimensions_is_string_object",
            ),
            (
                Some(dataset.id),
                r#"{"state": []}"#,
                "dimensions_is_string_object",
            ),
            (
                Some(dataset.id),
                r#"{"state": null}"#,
                "dimensions_is_string_object",
            ),
            (Some(dataset.id), r#"["06"]"#, "dimensions_is_string_object"),
            (None, r#"{"state": "06"}"#, "dimensions_need_dataset"),
        ];
        for (dataset_id, dimensions, constraint) in cases {
            let result = diesel::sql_query(format!(
                "INSERT INTO {table} (source_id, external_id, title, frequency, dataset_id, \
                 dimensions) VALUES ($1, $2, 'Bad', 'Annual', $3, $4::jsonb)"
            ))
            .bind::<diesel::sql_types::Uuid, _>(source.id)
            .bind::<diesel::sql_types::Text, _>(format!("BAD_{}", Uuid::new_v4()))
            .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(dataset_id)
            .bind::<diesel::sql_types::Text, _>(dimensions)
            .execute(&mut conn)
            .await;
            assert_eq!(
                violated(result),
                format!("{table}_{constraint}"),
                "{table} {dimensions}"
            );
        }

        let result = diesel::sql_query(format!(
            "INSERT INTO {table} (source_id, external_id, title, frequency, default_measure) \
             VALUES ($1, $2, 'Bad', 'Annual', 'value')"
        ))
        .bind::<diesel::sql_types::Uuid, _>(source.id)
        .bind::<diesel::sql_types::Text, _>(format!("BAD_{}", Uuid::new_v4()))
        .execute(&mut conn)
        .await;
        assert_eq!(
            violated(result),
            format!("{table}_default_measure_needs_dataset")
        );
    }
}

#[tokio::test]
async fn database_rejects_components_rust_cannot_read() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let mut conn = pool.get().await.unwrap();

    let cases = [
        ("dimensions", r#"{}"#),
        ("dimensions", r#"[5]"#),
        ("dimensions", r#"[{"name": "state", "type": "string"}]"#),
        (
            "attributes",
            r#"[{"name": "f", "label": "F", "type": "time"}]"#,
        ),
        (
            "attributes",
            r#"[{"name": 1, "label": "F", "type": "string"}]"#,
        ),
        (
            "measures",
            r#"[{"name": "value", "label": "Value", "type": "decimal"}, 5]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "unit": 5}]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "codes": "x"}]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "codes": [{"code": 1, "label": "One"}]}]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "codes": [{"code": "x"}]}]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "codes": [{"code": "x", "label": "X", "unit": []}]}]"#,
        ),
        (
            "dimensions",
            r#"[{"name": "a", "label": "A", "type": "string", "codelist": {}}]"#,
        ),
    ];
    for (column, components) in cases {
        let result = diesel::sql_query(format!(
            "INSERT INTO datasets (source_id, code, name, {column}) VALUES ($1, $2, 'Bad', $3::jsonb)"
        ))
        .bind::<diesel::sql_types::Uuid, _>(source.id)
        .bind::<diesel::sql_types::Text, _>(format!("BAD_{}", Uuid::new_v4()))
        .bind::<diesel::sql_types::Text, _>(components)
        .execute(&mut conn)
        .await;
        assert_eq!(
            violated(result),
            format!("datasets_{column}_valid"),
            "{components}"
        );
    }
}

#[tokio::test]
async fn used_dataset_cannot_be_deleted_but_its_source_can() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(source.id))
        .await
        .unwrap();
    let mut series = new_series(source.id, "BDS/state.06.ESTAB");
    series.dataset_id = Some(dataset.id);
    series.dimensions = state_dims("06");
    let series = EconomicSeries::create(&pool, &series).await.unwrap();

    let mut conn = pool.get().await.unwrap();
    let result = diesel::delete(datasets::table.find(dataset.id))
        .execute(&mut conn)
        .await;
    assert_eq!(violated(result), "economic_series_dataset_source_fkey");

    diesel::delete(crate::schema::data_sources::table.find(source.id))
        .execute(&mut conn)
        .await
        .expect("deleting the source cascades");
    assert_eq!(Dataset::find_by_id(&pool, dataset.id).await.unwrap(), None);
    let left: i64 = economic_series::table
        .find(series.id)
        .count()
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

fn assert_invalid(dataset: &NewDataset, expected: &str) {
    match dataset.validate_components() {
        Err(AppError::ValidationError(msg)) => {
            assert!(
                msg.contains(expected),
                "{msg:?} does not mention {expected:?}"
            )
        }
        other => panic!("expected a validation error mentioning {expected:?}, got {other:?}"),
    }
}

#[test]
fn validate_components_catches_what_the_database_cannot() {
    let source_id = Uuid::new_v4();
    assert!(bds_dataset(source_id).validate_components().is_ok());

    let mut duplicate = bds_dataset(source_id);
    duplicate.attributes.0.push(DatasetComponent::new(
        "state",
        "State again",
        ComponentType::String,
    ));
    assert_invalid(&duplicate, "more than once");

    let mut undeclared = bds_dataset(source_id);
    undeclared.default_measure = "firms".to_string();
    assert_invalid(&undeclared, "is not one of its measures");

    let mut no_measures = bds_dataset(source_id);
    no_measures.measures.0.clear();
    assert_invalid(&no_measures, "declares no measures");

    let mut both = wdi_dataset(source_id);
    both.dimensions.0[1].codes = Some(vec![Code::new("USA", "United States")]);
    assert_invalid(&both, "both codes and codelist");

    let mut repeated_code = bds_dataset(source_id);
    repeated_code.dimensions.0[1]
        .codes
        .as_mut()
        .unwrap()
        .push(Code::new("06", "California again"));
    assert_invalid(&repeated_code, "lists code \"06\" more than once");

    assert!(wdi_dataset(source_id).validate_components().is_ok());

    let mut blank = bds_dataset(source_id);
    blank.dimensions.0[0].name = " ".to_string();
    assert_invalid(&blank, "no name");
}

/// The WDI shape: `indicator` has an inline code list with a unit per code, `area` names the
/// shared `countries` code list.
fn wdi_dataset(source_id: Uuid) -> NewDataset {
    let mut indicator = DatasetComponent::new("indicator", "Indicator", ComponentType::String);
    indicator.codes = Some(vec![Code {
        code: "NY.GDP.PCAP.CD".to_string(),
        label: "GDP per capita".to_string(),
        unit: Some("current US$".to_string()),
        description: Some("Gross domestic product divided by midyear population".to_string()),
    }]);
    let mut area = DatasetComponent::new("area", "Country or area", ComponentType::String);
    area.codelist = Some("countries".to_string());
    NewDataset::long(
        source_id,
        "WDI",
        "World Development Indicators",
        vec![indicator, area],
    )
}

#[tokio::test]
async fn dataset_round_trips_both_code_list_forms() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let new_dataset = wdi_dataset(source.id);

    let created = Dataset::create(&pool, &new_dataset).await.expect("create");
    assert_eq!(created.dimensions, new_dataset.dimensions);
    let [indicator, area] = created.dimensions.0.as_slice() else {
        panic!("expected two dimensions");
    };
    assert_eq!(
        indicator.codes.as_ref().unwrap()[0].unit.as_deref(),
        Some("current US$")
    );
    assert_eq!(indicator.codelist, None);
    assert_eq!(area.codelist.as_deref(), Some("countries"));
    assert_eq!(area.codes, None);
}

#[test]
fn components_serialize_to_the_documented_json_shape() {
    let json = serde_json::to_value(&wdi_dataset(Uuid::nil()).dimensions).unwrap();
    let expected = serde_json::json!([
        {
            "name": "indicator",
            "label": "Indicator",
            "type": "string",
            "codes": [{
                "code": "NY.GDP.PCAP.CD",
                "label": "GDP per capita",
                "unit": "current US$",
                "description": "Gross domestic product divided by midyear population",
            }],
        },
        {"name": "area", "label": "Country or area", "type": "string", "codelist": "countries"},
    ]);
    assert_eq!(json, expected);
    assert_eq!(
        serde_json::to_value(DatasetComponents(vec![DatasetComponent::value_measure()])).unwrap(),
        serde_json::json!([{"name": "value", "label": "Value", "type": "decimal"}])
    );

    // Hand-written definitions (optional keys left out) parse back to the same value.
    let parsed: DatasetComponents = serde_json::from_value(expected).unwrap();
    assert_eq!(parsed, wdi_dataset(Uuid::nil()).dimensions);
}
