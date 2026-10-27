//! Catalog authorization against the production library (including missing-context denial).
//! Requires DATABASE_URL pointing at disposable Postgres; creates its own scratch database.
use async_graphql::{Request, Response};
use econ_graph_auth::{Principal, Role};
use econ_graph_core::{
    database::{AppPool, DataPool, DatabasePool},
    models::{DataSource, EconomicSeries, NewEconomicSeries},
    test_utils::{test_dataset_id, ScratchDatabase},
};
use econ_graph_graphql::graphql::{
    context::GraphQLContext,
    schema::{create_schema, create_schema_with_data},
};
use std::sync::Arc;
use uuid::Uuid;

async fn execute(pool: &DatabasePool, context: GraphQLContext, query: &str) -> Response {
    create_schema_with_data(
        AppPool(pool.clone()),
        Some(DataPool(pool.clone())),
        Arc::new(context),
    )
    .execute(Request::new(query))
    .await
}

fn anonymous(roles: &[Role]) -> GraphQLContext {
    GraphQLContext::anonymous().with_anonymous_catalog_roles(roles.iter().copied())
}

async fn seed(pool: &DatabasePool, source: DataSource, suffix: &str) -> (Uuid, Uuid) {
    let dataset = test_dataset_id(&mut pool.get().await.unwrap(), source.id).await;
    let series = EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: format!("CATALOG-{suffix}"),
            title: format!("Catalogtest {suffix}"),
            dataset_id: dataset,
            end_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    (series.id, dataset)
}

#[tokio::test]
async fn catalog_grants_cover_discovery_pagination_and_direct_reads_without_identity_fallback() {
    let db = ScratchDatabase::create("catalog_auth");
    econ_graph_core::database::run_migrations(&db.url)
        .await
        .unwrap();
    let pool = econ_graph_core::database::create_pool(&db.url)
        .await
        .unwrap();
    let fred = DataSource::find_by_name(&pool, &DataSource::fred().name)
        .await
        .unwrap()
        .unwrap();
    let bea = DataSource::find_by_name(&pool, &DataSource::bea().name)
        .await
        .unwrap()
        .unwrap();
    let mut unknown = DataSource::fred();
    unknown.name = "Unmapped catalog".into();
    let unknown = DataSource::create(&pool, unknown).await.unwrap();
    let (fred_series, fred_dataset) = seed(&pool, fred.clone(), "FRED").await;
    let (bea_series, bea_dataset) = seed(&pool, bea.clone(), "BEA").await;
    let (unknown_series, _) = seed(&pool, unknown, "UNKNOWN").await;
    let fred_only = [Role::CatalogFredRead];

    let query = r#"{ dataSources { id seriesCount series { totalCount } }
        datasets { id } seriesList(pagination:{first:1}) { totalCount nodes { id } }
        searchSeries(query:"Catalogtest", first:1) { totalCount series { id } }
        searchSuggestions(partialQuery:"Catalogtest") { suggestion } }"#;
    let result = execute(&pool, anonymous(&fred_only), query).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["dataSources"].as_array().unwrap().len(), 1);
    assert_eq!(data["dataSources"][0]["id"], fred.id.to_string());
    assert_eq!(data["datasets"].as_array().unwrap().len(), 1);
    assert_eq!(data["datasets"][0]["id"], fred_dataset.to_string());
    for field in ["seriesList", "searchSeries"] {
        assert_eq!(data[field]["totalCount"], 1);
    }
    assert_eq!(
        data["seriesList"]["nodes"][0]["id"],
        fred_series.to_string()
    );
    assert_eq!(
        data["searchSeries"]["series"][0]["id"],
        fred_series.to_string()
    );
    assert!(data["searchSuggestions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["suggestion"].as_str().unwrap().contains("FRED")));

    let allowed = execute(&pool, anonymous(&fred_only), &format!(r#"{{ series(id:"{fred_series}") {{ id recentDataPoints {{ id }} }} seriesExists(id:"{fred_series}") seriesData(seriesId:"{fred_series}") {{ totalCount }} }}"#)).await;
    assert!(allowed.errors.is_empty(), "{:?}", allowed.errors);
    for query in [
        format!(r#"{{ series(id:"{bea_series}") {{ id }} }}"#),
        format!(r#"{{ seriesExists(id:"{bea_series}") }}"#),
        format!(r#"{{ seriesData(seriesId:"{bea_series}") {{ totalCount }} }}"#),
        format!(
            r#"{{ seriesByExternalId(sourceName:"{}",externalId:"CATALOG-BEA") {{ id }} }}"#,
            bea.name
        ),
        format!(r#"{{ dataSource(id:"{}") {{ name }} }}"#, bea.id),
        format!(r#"{{ dataset(id:"{bea_dataset}") {{ id }} }}"#),
        format!(
            r#"{{ crossSection(datasetId:"{bea_dataset}",across:"area",latest:true) {{ key }} }}"#
        ),
        format!(r#"{{ series(id:"{unknown_series}") {{ id }} }}"#),
    ] {
        let result = execute(&pool, anonymous(&fred_only), &query).await;
        assert!(!result.errors.is_empty(), "access leaked: {query}");
        assert_eq!(
            result.errors[0].extensions.as_ref().unwrap().get("code"),
            Some(&async_graphql::Value::from("FORBIDDEN")),
            "{query}: {:?}",
            result.errors
        );
    }
    let mixed = execute(&pool, anonymous(&fred_only), &format!(r#"{{ open:series(id:"{fred_series}") {{ id }} blocked:series(id:"{bea_series}") {{ id }} }}"#)).await;
    assert_eq!(mixed.errors.len(), 1);
    let mixed = mixed.data.into_json().unwrap();
    assert_eq!(mixed["open"]["id"], fred_series.to_string());
    assert!(mixed["blocked"].is_null());

    // Explicit source filters cannot escape the authorization scope, nor can offsets leak counts.
    let query = format!(
        r#"{{ seriesList(filter:{{sourceId:"{}"}}) {{totalCount}}
        searchSeries(query:"Catalogtest", source:"{}", after:"20") {{totalCount}} }}"#,
        bea.id, bea.id
    );
    let result = execute(&pool, anonymous(&fred_only), &query).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["seriesList"]["totalCount"], 0);
    assert_eq!(data["searchSeries"]["totalCount"], 0);

    // Anonymous grants never supplement a signed-in user's own denied catalog role.
    let mut signed_in = anonymous(&[Role::CatalogFredRead, Role::CatalogBeaRead]);
    signed_in.principal = Some(Principal::new(Uuid::new_v4(), [Role::CatalogBeaRead]));
    let result = execute(&pool, signed_in, r#"{dataSources{id}}"#).await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["dataSources"],
        serde_json::json!([{"id":bea.id.to_string()}])
    );

    let missing = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())))
        .execute("{dataSources{id} seriesList{totalCount}}")
        .await;
    assert!(missing.errors.is_empty());
    let missing = missing.data.into_json().unwrap();
    assert_eq!(missing["dataSources"], serde_json::json!([]));
    assert_eq!(missing["seriesList"]["totalCount"], 0);
    drop(pool);
}
