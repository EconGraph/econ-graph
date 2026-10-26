//! Public lifecycle visibility and search-order regressions against a migrated test database.

use crate::graphql::{schema::create_schema, test_db::migrate_once};
use diesel_async::RunQueryDsl;
use econ_graph_core::{
    database::{AppPool, DataPool, DatabasePool},
    models::search::SearchParams,
    models::{DataSource, EconomicSeries, NewDataSource, NewEconomicSeries},
    test_utils::test_dataset_id,
};
use econ_graph_services::services::search_service::SearchService;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

async fn pool() -> Option<DatabasePool> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return None;
    };
    migrate_once(&url).await;
    Some(econ_graph_core::database::create_pool(&url).await.unwrap())
}

async fn source(pool: &DatabasePool) -> DataSource {
    DataSource::create(
        pool,
        NewDataSource {
            name: format!("lifecycle-visibility-{}", Uuid::new_v4()),
            base_url: "https://example.test".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap()
}

async fn series(
    pool: &DatabasePool,
    source: &DataSource,
    external_id: &str,
    title: &str,
) -> EconomicSeries {
    let dataset_id = test_dataset_id(&mut pool.get().await.unwrap(), source.id).await;
    let series = EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: external_id.into(),
            title: title.into(),
            frequency: "Monthly".into(),
            end_date: chrono::NaiveDate::from_ymd_opt(2026, 1, 1),
            dataset_id,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release)
        VALUES ($1, '2026-01-01', 42, '2026-02-01', true)",
    )
    .bind::<diesel::sql_types::Uuid, _>(series.id)
    .execute(&mut pool.get().await.unwrap())
    .await
    .unwrap();
    series
}

async fn set_status(
    pool: &DatabasePool,
    series: &EconomicSeries,
    status: &str,
    successor: Option<Uuid>,
) {
    // Keep is_active=true deliberately: lifecycle restrictions must not depend on a
    // separately updated convenience flag, or includeInactive could bypass them.
    diesel::sql_query(
        "INSERT INTO series_lifecycle (source_id, external_id, status, since, scope_fingerprint, successor_id)
        VALUES ($1, $2, $3, '2026-10-20T00:00:00Z', 'visibility-test', $4)
        ON CONFLICT (source_id, external_id) DO UPDATE SET status = EXCLUDED.status, successor_id = EXCLUDED.successor_id",
    )
    .bind::<diesel::sql_types::Uuid, _>(series.source_id)
    .bind::<diesel::sql_types::Text, _>(&series.external_id)
    .bind::<diesel::sql_types::Text, _>(status)
    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(successor)
    .execute(&mut pool.get().await.unwrap())
    .await
    .unwrap();
}

async fn run(pool: &DatabasePool, query: String) -> Value {
    let schema = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())));
    let response =
        async_graphql::Schema::execute(&schema, async_graphql::Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

#[tokio::test]
async fn restricted_history_is_kept_but_unreachable_through_public_reads() {
    let Some(pool) = pool().await else { return };
    let source = source(&pool).await;
    let hidden = series(
        &pool,
        &source,
        "SECRET_CATALOG",
        "Restricted catalog visibility fixture",
    )
    .await;
    set_status(&pool, &hidden, "restricted", None).await;
    let data = run(
        &pool,
        format!(
            r#"{{
        series(id:"{id}") {{ id }}
        seriesByExternalId(sourceName:{name}, externalId:"SECRET_CATALOG") {{ id }}
        seriesExists(id:"{id}")
        seriesList(filter:{{sourceId:"{source}"}}) {{ totalCount nodes {{ id }} }}
        dataSource(id:"{source}") {{ seriesCount series {{ totalCount nodes {{ id }} }} }}
        seriesData(seriesId:"{id}") {{ totalCount nodes {{ value }} }}
        searchSeries(query:"catalog", source:"{source}") {{ totalCount series {{ id }} }}
    }}"#,
            id = hidden.id,
            name = json!(source.name),
            source = source.id
        ),
    )
    .await;
    assert!(data["series"].is_null());
    assert!(data["seriesByExternalId"].is_null());
    assert_eq!(data["seriesExists"], false);
    assert_eq!(data["seriesList"]["totalCount"], 0);
    assert_eq!(data["dataSource"]["seriesCount"], 0);
    assert_eq!(data["dataSource"]["series"]["totalCount"], 0);
    assert_eq!(data["seriesData"]["totalCount"], 0);
    assert_eq!(data["searchSeries"]["totalCount"], 0);
    let service = SearchService::new(Arc::new(pool.clone()));
    let params = SearchParams {
        include_inactive: Some(true),
        ..SearchParams::simple("catalog")
    };
    assert!(!service
        .search_series(&params)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == hidden.id));
    assert!(!service
        .get_suggestions("Restricted catalog", 10)
        .await
        .unwrap()
        .iter()
        .any(|s| s.suggestion == hidden.title));
    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type=diesel::sql_types::BigInt)]
        count: i64,
    }
    let retained = diesel::sql_query("SELECT count(*) FROM data_points WHERE series_id=$1")
        .bind::<diesel::sql_types::Uuid, _>(hidden.id)
        .get_result::<Count>(&mut pool.get().await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        retained.count, 1,
        "public restrictions must never delete stored history"
    );
}

#[tokio::test]
async fn retained_series_stay_linkable_and_rank_after_current_matches_even_for_exact_id() {
    let Some(pool) = pool().await else { return };
    let source = source(&pool).await;
    let word = format!("Lifecycle{}", Uuid::new_v4().simple());
    let old = series(&pool, &source, &word, &format!("{word} retired")).await;
    let current = series(&pool, &source, "CURRENT", &format!("{word} current")).await;
    let service = SearchService::new(Arc::new(pool.clone()));
    for status in [
        "discontinued",
        "missing_from_catalog",
        "retired",
        "superseded",
    ] {
        set_status(
            &pool,
            &old,
            status,
            (status == "superseded").then_some(current.id),
        )
        .await;
        let page = service
            .search_series_page(&SearchParams::simple(&word))
            .await
            .unwrap();
        assert_eq!(page.total_count, 2);
        assert_eq!(page.results[0].id, current.id, "{status}");
        assert_eq!(page.results[1].id, old.id);
        let data = run(&pool, format!(r#"{{series(id:"{}") {{id}}}}"#, old.id)).await;
        assert_eq!(data["series"]["id"], old.id.to_string());
    }
}

#[tokio::test]
async fn public_batchers_and_warmed_loaders_recheck_restrictions_without_deleting_history() {
    use crate::graphql::dataloaders::{
        DataLoaders, DataPointCountBatcher, DataPointsBySeriesBatcher, LatestObservationBatcher,
    };
    use crate::graphql::datasets::SeriesDatasetFieldsBatcher;
    use dataloader::BatchFn;

    let Some(pool) = pool().await else { return };
    let source = source(&pool).await;
    let hidden = series(&pool, &source, "WARM_HIDDEN", "Initially public series").await;
    let visible = series(&pool, &source, "STILL_PUBLIC", "Still public series").await;
    let loaders = DataLoaders::new(DataPool(pool.clone()));
    // A single DataLoaders lives as long as a schema in MCP/security servers. Warm every
    // series-dependent loader first, then change policy without recreating the loaders.
    assert_eq!(
        loaders
            .data_points_by_series_loader
            .try_load(hidden.id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        loaders
            .data_point_count_loader
            .try_load(hidden.id)
            .await
            .unwrap(),
        1
    );
    assert!(loaders
        .latest_observation_loader
        .try_load(hidden.id)
        .await
        .unwrap()
        .is_some());
    assert!(loaders
        .series_dataset_fields_loader
        .try_load(hidden.id)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        loaders
            .series_by_source_loader
            .try_load(source.id)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        loaders
            .series_count_loader
            .try_load(source.id)
            .await
            .unwrap(),
        2
    );

    set_status(&pool, &hidden, "restricted", None).await;
    assert!(loaders
        .data_points_by_series_loader
        .try_load(hidden.id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        loaders
            .data_point_count_loader
            .try_load(hidden.id)
            .await
            .unwrap(),
        0
    );
    assert!(loaders
        .latest_observation_loader
        .try_load(hidden.id)
        .await
        .unwrap()
        .is_none());
    assert!(loaders
        .series_dataset_fields_loader
        .try_load(hidden.id)
        .await
        .unwrap()
        .is_none());
    let public = loaders
        .series_by_source_loader
        .try_load(source.id)
        .await
        .unwrap();
    assert_eq!(
        public.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![visible.id]
    );
    assert_eq!(
        loaders
            .series_count_loader
            .try_load(source.id)
            .await
            .unwrap(),
        1
    );

    // Exercise raw BatchFn implementations with both IDs too: returning empty defaults for
    // every key would conceal public data and must fail this regression.
    let keys = [hidden.id, visible.id];
    let points = BatchFn::load(&mut DataPointsBySeriesBatcher { pool: pool.clone() }, &keys).await;
    assert!(points[&hidden.id].is_empty());
    assert_eq!(points[&visible.id].len(), 1);
    let counts = BatchFn::load(&mut DataPointCountBatcher { pool: pool.clone() }, &keys).await;
    assert_eq!(counts[&hidden.id], 0);
    assert_eq!(counts[&visible.id], 1);
    let latest = BatchFn::load(&mut LatestObservationBatcher { pool: pool.clone() }, &keys).await;
    assert!(latest[&hidden.id].is_none());
    assert!(latest[&visible.id].is_some());
    let fields = BatchFn::load(
        &mut SeriesDatasetFieldsBatcher { pool: pool.clone() },
        &keys,
    )
    .await;
    assert!(fields[&hidden.id].is_none());
    assert!(fields[&visible.id].is_some());

    use diesel::{ExpressionMethods, QueryDsl, SelectableHelper};
    use econ_graph_core::schema::data_points;
    assert_eq!(
        data_points::table
            .filter(data_points::series_id.eq(hidden.id))
            .count()
            .get_result::<i64>(&mut pool.get().await.unwrap())
            .await
            .unwrap(),
        1
    );
}

/// Holds the exact series object read before a policy change. This bypasses the root
/// query's SQL deliberately, so nested field protection cannot hide behind the root filter.
struct HeldSeriesQuery(crate::types::EconomicSeriesType);

#[async_graphql::Object]
impl HeldSeriesQuery {
    async fn held(&self) -> crate::types::EconomicSeriesType {
        self.0.clone()
    }
}

#[tokio::test]
async fn held_series_nested_fields_recheck_restrictions_on_a_reused_schema() {
    use crate::graphql::{dataloaders::DataLoaders, schema::SchemaResources};

    let Some(pool) = pool().await else { return };
    let source = source(&pool).await;
    let mut hidden = series(&pool, &source, "HELD_PUBLIC", "Previously public series").await;
    hidden.dimensions = [("state", "06")].into_iter().collect();
    hidden.default_measure = Some("value".into());
    diesel::sql_query(
        "UPDATE economic_series SET dimensions = $2, default_measure = 'value' WHERE id = $1",
    )
    .bind::<diesel::sql_types::Uuid, _>(hidden.id)
    .bind::<diesel::sql_types::Jsonb, _>(serde_json::to_value(&hidden.dimensions).unwrap())
    .execute(&mut pool.get().await.unwrap())
    .await
    .unwrap();
    let schema = async_graphql::Schema::build(
        HeldSeriesQuery(hidden.clone().into()),
        async_graphql::EmptyMutation,
        async_graphql::EmptySubscription,
    )
    .data(DataPool(pool.clone()))
    .data(SchemaResources {
        data_loaders: Some(Arc::new(DataLoaders::new(DataPool(pool.clone())))),
    })
    .finish();
    let query = "{held { recentDataPoints {value} dataPointCount latestObservation {value} \
        dataPoints {value} dataset {id} dimensions {name value} defaultMeasure }}";
    let before = async_graphql::Schema::execute(&schema, query).await;
    assert!(before.errors.is_empty(), "{:?}", before.errors);
    let before = before.data.into_json().unwrap();
    assert_eq!(before["held"]["dataPointCount"], 1);
    assert_eq!(
        before["held"]["recentDataPoints"].as_array().unwrap().len(),
        1
    );
    assert_eq!(before["held"]["dataPoints"].as_array().unwrap().len(), 1);
    assert!(before["held"]["latestObservation"].is_object());
    assert_eq!(
        before["held"]["dataset"]["id"],
        hidden.dataset_id.to_string()
    );
    assert_eq!(
        before["held"]["dimensions"],
        json!([{"name":"state", "value":"06"}])
    );
    assert_eq!(before["held"]["defaultMeasure"], "value");

    set_status(&pool, &hidden, "restricted", None).await;
    let after = async_graphql::Schema::execute(&schema, query).await;
    assert!(after.errors.is_empty(), "{:?}", after.errors);
    let after = after.data.into_json().unwrap();
    assert_eq!(
        after["held"],
        json!({
            "recentDataPoints": [], "dataPointCount": 0, "latestObservation": null,
            "dataPoints": [], "dataset": null, "dimensions": [], "defaultMeasure": null,
        })
    );
    // Underlying series-specific dimension metadata remains retained for internal use.
    use diesel::{ExpressionMethods, QueryDsl, SelectableHelper};
    use econ_graph_core::schema::economic_series;
    let stored = economic_series::table
        .find(hidden.id)
        .select(EconomicSeries::as_select())
        .first::<EconomicSeries>(&mut pool.get().await.unwrap())
        .await
        .unwrap();
    assert_eq!(stored.dimensions, hidden.dimensions);
    assert_eq!(stored.default_measure, hidden.default_measure);
}
