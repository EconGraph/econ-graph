//! A catalog restricted by default is hidden from every read path until its role is granted.
//!
//! The Bank of Canada catalog role, `catalog.bank_of_canada:read`, is defined in the realm file
//! but left out of `catalog-public` and of the `econgraph-anonymous` scope mapping. With the
//! anonymous reader's default roles, taken from that realm file, its series must not show up in
//! GraphQL search and suggestions, the data-source series lists and counts, lookups by id or
//! external id, or the MCP tools; with the role granted they all must.
//!
//! Requires `DATABASE_URL` pointing at disposable Postgres; creates its own scratch database.

use std::sync::Arc;

use async_graphql::Request;
use econ_graph_auth::Role;
use econ_graph_core::database::{AppPool, DataPool, DatabasePool};
use econ_graph_core::models::{DataSource, EconomicSeries, NewEconomicSeries};
use econ_graph_core::test_utils::{test_dataset_id, ScratchDatabase};
use econ_graph_graphql::graphql::context::GraphQLContext;
use econ_graph_graphql::graphql::schema::create_schema_with_data;
use econ_graph_mcp::mcp_server::EconGraphMcpServer;
use serde_json::{json, Value};

const REALM: &str = include_str!("../../../../config/keycloak/econ-graph-realm.json");

/// The catalog roles the realm file maps into the anonymous reader's scope: what an anonymous
/// visitor can read on a fresh install.
fn anonymous_default_roles() -> Vec<Role> {
    let realm: Value = serde_json::from_str(REALM).unwrap();
    let entry = realm["clientScopeMappings"]["econ-graph-api"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["client"] == "econgraph-anonymous")
        .unwrap()
        .clone();
    entry["roles"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|name| name.as_str().unwrap().parse::<Role>().ok())
        .filter(|role| role.is_catalog())
        .collect()
}

async fn seed(pool: &DatabasePool, source: &DataSource, external_id: &str) -> String {
    use diesel_async::RunQueryDsl;

    let dataset = test_dataset_id(&mut pool.get().await.unwrap(), source.id).await;
    let id = EconomicSeries::create(
        pool,
        &NewEconomicSeries {
            source_id: source.id,
            external_id: external_id.to_string(),
            title: format!("Restrictedtest {external_id}"),
            dataset_id: dataset,
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .id
    .to_string();
    // dataSources lists only sources with a series that has data (an end date).
    diesel::sql_query(
        "UPDATE economic_series SET start_date = '2023-01-01', end_date = '2024-01-01' \
         WHERE id = $1::uuid",
    )
    .bind::<diesel::sql_types::Text, _>(&id)
    .execute(&mut pool.get().await.unwrap())
    .await
    .unwrap();
    id
}

struct Reads {
    graphql: Value,
    mcp_search: Value,
    mcp_sources: Value,
    mcp_series_data_ok: bool,
}

async fn read_everything(
    pool: &DatabasePool,
    roles: &[Role],
    boc: &DataSource,
    boc_series: &str,
) -> Reads {
    let context =
        Arc::new(GraphQLContext::anonymous().with_anonymous_catalog_roles(roles.iter().copied()));
    let schema = create_schema_with_data(
        AppPool(pool.clone()),
        Some(DataPool(pool.clone())),
        context.clone(),
    );
    let query = format!(
        r#"{{ dataSources {{ id seriesCount series {{ totalCount }} }}
            seriesList(pagination:{{first:50}}) {{ totalCount nodes {{ id }} }}
            searchSeries(query:"Restrictedtest", first:50) {{ totalCount series {{ id }} }}
            searchSuggestions(partialQuery:"Restrictedtest") {{ suggestion }}
            byId: series(id:"{boc_series}") {{ id }}
            exists: seriesExists(id:"{boc_series}")
            byExternalId: seriesByExternalId(sourceName:"{}", externalId:"BOC-RESTRICTED") {{ id }} }}"#,
        boc.name
    );
    let response = schema.execute(Request::new(query)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);

    let mcp = EconGraphMcpServer::new(AppPool(pool.clone()), Some(DataPool(pool.clone())))
        .with_auth_context(context);
    let text = |reply: Value, key: &str| -> Value {
        serde_json::from_str(reply[key][0]["text"].as_str().unwrap()).unwrap()
    };
    let mcp_search = text(
        mcp.search_economic_series(json!({"query": "Restrictedtest", "limit": 50}))
            .await
            .unwrap(),
        "content",
    );
    let mcp_sources = text(mcp.get_data_sources().await.unwrap(), "contents");
    let mcp_series_data_ok = mcp
        .get_series_data(json!({"series_id": boc_series}))
        .await
        .is_ok();
    Reads {
        graphql: response.data.into_json().unwrap(),
        mcp_search,
        mcp_sources,
        mcp_series_data_ok,
    }
}

fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn bank_of_canada_is_hidden_until_its_catalog_role_is_granted() {
    let db = ScratchDatabase::create("restricted_catalog");
    econ_graph_core::database::run_migrations(&db.url)
        .await
        .unwrap();
    let pool = econ_graph_core::database::create_pool(&db.url)
        .await
        .unwrap();
    let bea = DataSource::find_by_name(&pool, &DataSource::bea().name)
        .await
        .unwrap()
        .unwrap();
    let boc = DataSource::create(&pool, DataSource::boc()).await.unwrap();
    let bea_series = seed(&pool, &bea, "BEA-PUBLIC").await;
    let boc_series = seed(&pool, &boc, "BOC-RESTRICTED").await;
    let (bea_id, boc_id) = (bea.id.to_string(), boc.id.to_string());

    let defaults = anonymous_default_roles();
    assert!(defaults.contains(&Role::CatalogBeaRead));
    assert!(!defaults.contains(&Role::CatalogBankOfCanadaRead));

    // Anonymous defaults: the Bank of Canada is hidden everywhere; BEA (public) is not.
    let hidden = read_everything(&pool, &defaults, &boc, &boc_series).await;
    let g = &hidden.graphql;
    let sources = ids(&g["dataSources"]);
    assert!(
        sources.contains(&bea_id) && !sources.contains(&boc_id),
        "{sources:?}"
    );
    for (path, list) in [
        ("seriesList", &g["seriesList"]["nodes"]),
        ("searchSeries", &g["searchSeries"]["series"]),
    ] {
        let found = ids(list);
        assert!(found.contains(&bea_series), "{path}: {found:?}");
        assert!(!found.contains(&boc_series), "{path}: {found:?}");
    }
    assert_eq!(g["searchSeries"]["totalCount"], 1);
    let suggestions = g["searchSuggestions"].to_string();
    assert!(!suggestions.contains("BOC"), "{suggestions}");
    assert!(g["byId"].is_null());
    assert_eq!(g["exists"], false);
    assert!(g["byExternalId"].is_null());
    assert_eq!(
        ids(&hidden.mcp_search["data"]["searchSeries"]["series"]),
        std::slice::from_ref(&bea_series)
    );
    assert!(!ids(&hidden.mcp_sources["data"]["dataSources"]).contains(&boc_id));
    assert!(!hidden.mcp_series_data_ok, "MCP served a restricted series");

    // With the role granted, every path shows it, with its count.
    let mut granted_roles = defaults.clone();
    granted_roles.push(Role::CatalogBankOfCanadaRead);
    let shown = read_everything(&pool, &granted_roles, &boc, &boc_series).await;
    let g = &shown.graphql;
    let boc_row = g["dataSources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["id"] == boc_id.as_str())
        .expect("the Bank of Canada is listed once granted");
    assert_eq!(boc_row["seriesCount"], 1);
    assert_eq!(boc_row["series"]["totalCount"], 1);
    for (path, list) in [
        ("seriesList", &g["seriesList"]["nodes"]),
        ("searchSeries", &g["searchSeries"]["series"]),
    ] {
        assert!(ids(list).contains(&boc_series), "{path}");
    }
    assert_eq!(g["searchSeries"]["totalCount"], 2);
    assert!(g["searchSuggestions"].to_string().contains("BOC"));
    assert_eq!(g["byId"]["id"], boc_series.as_str());
    assert_eq!(g["exists"], true);
    assert_eq!(g["byExternalId"]["id"], boc_series.as_str());
    assert!(ids(&shown.mcp_search["data"]["searchSeries"]["series"]).contains(&boc_series));
    assert!(ids(&shown.mcp_sources["data"]["dataSources"]).contains(&boc_id));
    assert!(shown.mcp_series_data_ok);
}
