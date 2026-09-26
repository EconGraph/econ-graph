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
use diesel::result::{DatabaseErrorKind, Error as DieselError};

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

fn assert_db_error(result: AppResult<impl std::fmt::Debug>, kind: DatabaseErrorKind) {
    match result {
        Err(AppError::Database(DieselError::DatabaseError(actual, info))) => assert!(
            std::mem::discriminant(&actual) == std::mem::discriminant(&kind),
            "expected {kind:?}, got {actual:?}: {}",
            info.message()
        ),
        other => panic!("expected a {kind:?} database error, got {other:?}"),
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
        created.dimensions.0[1].codes.as_ref().unwrap()["06"],
        "California"
    );

    assert_db_error(
        Dataset::create(&pool, &new_dataset).await,
        DatabaseErrorKind::UniqueViolation,
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
    assert!(
        matches!(
            result,
            Err(DieselError::DatabaseError(
                DatabaseErrorKind::CheckViolation,
                _
            ))
        ),
        "{result:?}"
    );
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
}

#[tokio::test]
async fn unique_index_rejects_duplicate_dimension_key() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let dataset = Dataset::create(&pool, &bds_dataset(source.id))
        .await
        .unwrap();

    let mut first = new_series(source.id, "BDS/state.06.ESTAB");
    first.dataset_id = Some(dataset.id);
    first.dimensions = state_dims("06");
    EconomicSeries::create(&pool, &first).await.unwrap();

    let mut duplicate = new_series(source.id, "CENSUS_BDS_ESTAB_state_06");
    duplicate.dataset_id = Some(dataset.id);
    duplicate.dimensions = state_dims("06");
    assert_db_error(
        EconomicSeries::create(&pool, &duplicate).await,
        DatabaseErrorKind::UniqueViolation,
    );

    // series_metadata has the same index.
    let mut conn = pool.get().await.unwrap();
    for external_id in ["BDS/state.06.ESTAB", "CENSUS_BDS_ESTAB_state_06"] {
        let mut meta = new_metadata(source.id, external_id);
        meta.dataset_id = Some(dataset.id);
        meta.dimensions = state_dims("06");
        let result = diesel::insert_into(series_metadata::table)
            .values(&meta)
            .execute(&mut conn)
            .await;
        if external_id.starts_with("BDS/") {
            result.expect("first metadata row");
        } else {
            assert!(
                matches!(
                    result,
                    Err(DieselError::DatabaseError(
                        DatabaseErrorKind::UniqueViolation,
                        _
                    ))
                ),
                "{result:?}"
            );
        }
    }
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
    assert_db_error(
        EconomicSeries::create(&pool, &series).await,
        DatabaseErrorKind::ForeignKeyViolation,
    );
}

#[tokio::test]
async fn database_rejects_non_string_dimension_values() {
    let pool = test_pool().await;
    let source = new_source(&pool).await;
    let mut conn = pool.get().await.unwrap();

    for bad in [r#"{"state": 6}"#, r#"["06"]"#] {
        let result = diesel::sql_query(
            "INSERT INTO economic_series (source_id, external_id, title, frequency, dimensions) \
             VALUES ($1, $2, 'Bad', 'Annual', $3::jsonb)",
        )
        .bind::<diesel::sql_types::Uuid, _>(source.id)
        .bind::<diesel::sql_types::Text, _>(format!("BAD_{}", Uuid::new_v4()))
        .bind::<diesel::sql_types::Text, _>(bad)
        .execute(&mut conn)
        .await;
        assert!(
            matches!(
                result,
                Err(DieselError::DatabaseError(
                    DatabaseErrorKind::CheckViolation,
                    _
                ))
            ),
            "{bad}: {result:?}"
        );
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
    assert!(duplicate.validate_components().is_err());

    let mut undeclared = bds_dataset(source_id);
    undeclared.default_measure = "firms".to_string();
    assert!(undeclared.validate_components().is_err());

    let mut no_measures = bds_dataset(source_id);
    no_measures.measures.0.clear();
    assert!(no_measures.validate_components().is_err());

    let mut blank = bds_dataset(source_id);
    blank.dimensions.0[0].name = " ".to_string();
    assert!(blank.validate_components().is_err());
}

#[test]
fn components_serialize_to_the_documented_json_shape() {
    let mut state = DatasetComponent::new("state", "State", ComponentType::String);
    state.codes = Some(BTreeMap::from([(
        "06".to_string(),
        "California".to_string(),
    )]));
    let json = serde_json::to_value(DatasetComponents(vec![
        state,
        DatasetComponent::value_measure(),
    ]))
    .unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            {"name": "state", "label": "State", "type": "string", "codes": {"06": "California"}},
            {"name": "value", "label": "Value", "type": "decimal"},
        ])
    );
}
