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
