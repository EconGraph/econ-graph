// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The backend's databases at startup and in `/readyz`: the app set is applied, the data
//! database is never migrated, and readiness is checked per database and set.
//!
//! Each test makes its own scratch databases on `DATABASE_URL`'s server and is skipped
//! without it.

use super::{build_routes, Databases};
use econ_graph_core::config::{BackendDatabaseUrls, BackendPoolSizes};
use econ_graph_core::test_utils::ScratchDatabase;
use econ_graph_core::{run_migration_set, MigrationSet};
use econ_graph_core::{AppPool, DataPool};
use econ_graph_graphql::graphql::schema::create_schema_with_data;

fn have_database() -> bool {
    let set = std::env::var("DATABASE_URL").is_ok();
    if !set {
        eprintln!("DATABASE_URL not set; skipping the backend database tests");
    }
    set
}

/// The versions recorded in `db`'s `__diesel_schema_migrations`, or `None` when it has none.
fn recorded_versions(db: &ScratchDatabase) -> Option<Vec<String>> {
    use diesel::RunQueryDsl;
    #[derive(diesel::QueryableByName)]
    struct Version {
        #[diesel(sql_type = diesel::sql_types::Text)]
        version: String,
    }
    let mut conn = db.connect();
    diesel::sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version")
        .load::<Version>(&mut conn)
        .ok()
        .map(|rows| rows.into_iter().map(|r| r.version).collect())
}

/// The number of tables in `db`'s `public` schema.
fn table_count(db: &ScratchDatabase) -> i64 {
    use diesel::RunQueryDsl;
    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    let mut conn = db.connect();
    diesel::sql_query("SELECT count(*) AS n FROM pg_tables WHERE schemaname = 'public'")
        .get_result::<Count>(&mut conn)
        .unwrap()
        .n
}

/// `GET /readyz` through the backend's full route set on `databases`.
async fn readyz(databases: &Databases) -> (u16, serde_json::Value) {
    let schema = create_schema_with_data(databases.app.clone(), databases.data.clone(), ());
    let routes = build_routes(
        databases.clone(),
        None,
        schema,
        None,
        &["http://localhost:3000".to_string()],
        false,
    );
    let res = warp::test::request()
        .method("GET")
        .path("/readyz")
        .reply(&routes)
        .await;
    let body = serde_json::from_slice(res.body()).unwrap();
    (res.status().as_u16(), body)
}

/// A POST of `query` to `/graphql` through the full route set, anonymously.
async fn graphql(databases: &Databases, query: &str) -> serde_json::Value {
    let schema = create_schema_with_data(databases.app.clone(), databases.data.clone(), ());
    let routes = build_routes(
        databases.clone(),
        None,
        schema,
        None,
        &["http://localhost:3000".to_string()],
        false,
    );
    let res = warp::test::request()
        .method("POST")
        .path("/graphql")
        .header("content-type", "application/json")
        .body(serde_json::json!({ "query": query }).to_string())
        .reply(&routes)
        .await;
    assert_eq!(res.status(), 200);
    serde_json::from_slice(res.body()).unwrap()
}

const SIZES: BackendPoolSizes = BackendPoolSizes { app: 3, data: 2 };

async fn open(app: &str, data: Option<&str>) -> Databases {
    Databases::open(
        &BackendDatabaseUrls {
            app: app.to_string(),
            data: data.map(str::to_string),
        },
        SIZES,
    )
    .await
    .expect("startup should succeed")
}

/// Startup applies the app set to the app database and leaves a data database with pending
/// migrations untouched. `/readyz` stays 200 and reports the data set as pending until it is
/// applied by other means.
#[tokio::test]
#[serial_test::serial]
async fn startup_applies_only_the_app_set_and_readyz_reports_the_data_set() {
    if !have_database() {
        return;
    }
    let app_db = ScratchDatabase::create("startup_app");
    let data_db = ScratchDatabase::create("startup_data");
    let databases = open(&app_db.url, Some(&data_db.url)).await;

    let app_versions = recorded_versions(&app_db).expect("the app set is recorded");
    assert!(
        app_versions.iter().any(|v| v == "20261021000200"),
        "{app_versions:?}"
    );
    assert_eq!(
        recorded_versions(&data_db),
        None,
        "the data database was migrated"
    );
    assert_eq!(table_count(&data_db), 0, "the data database was written to");

    let (status, body) = readyz(&databases).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["app"], "ok", "{body}");
    assert_eq!(body["data"], "pending_migrations", "{body}");

    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let (status, body) = readyz(&databases).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"], "ok", "{body}");
}

/// With no data database, `/readyz` checks only the app database, and a data field answers
/// `DATA_SERVED_REMOTELY` through `/graphql`.
#[tokio::test]
#[serial_test::serial]
async fn without_a_data_database_readyz_checks_only_the_app_database() {
    if !have_database() {
        return;
    }
    let app_db = ScratchDatabase::create("app_only");
    let databases = open(&app_db.url, None).await;
    assert!(databases.data.is_none());

    let (status, body) = readyz(&databases).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"], "not_configured", "{body}");

    let body = graphql(&databases, "{ dataSources { name } }").await;
    assert_eq!(
        body["errors"][0]["extensions"]["code"], "DATA_SERVED_REMOTELY",
        "{body}"
    );
    assert_eq!(
        body["errors"][0]["message"],
        "Data is served by the data plane at /data/graphql"
    );
}

/// An unreachable data database is reported, not a reason to leave rotation; an unreachable or
/// unmigrated app database is, and the reason names it.
#[tokio::test]
#[serial_test::serial]
async fn readyz_fails_only_for_the_app_database() {
    if !have_database() {
        return;
    }
    let app_db = ScratchDatabase::create("ready_app");
    let databases = open(&app_db.url, None).await;
    let unreachable = || {
        econ_graph_core::DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(
                diesel_async::pooled_connection::AsyncDieselConnectionManager::<
                    diesel_async::AsyncPgConnection,
                >::new("postgres://nobody@127.0.0.1:1/none"),
            )
    };

    let data_down = Databases {
        app: databases.app.clone(),
        data: Some(DataPool(unreachable())),
    };
    let (status, body) = readyz(&data_down).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["data"], "unreachable", "{body}");

    let app_down = Databases {
        app: AppPool(unreachable()),
        data: None,
    };
    let (status, body) = readyz(&app_down).await;
    assert_eq!(status, 503, "{body}");
    assert!(
        body["reason"]
            .as_str()
            .unwrap()
            .starts_with("app database:"),
        "{body}"
    );

    // A database with the data set but not the app set is not a ready app database.
    let data_only = ScratchDatabase::create("ready_data_only");
    run_migration_set(&data_only.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let behind = Databases {
        app: AppPool(econ_graph_core::create_pool(&data_only.url).await.unwrap()),
        data: None,
    };
    let (status, body) = readyz(&behind).await;
    assert_eq!(status, 503, "{body}");
    assert!(
        body["reason"].as_str().unwrap().contains("app migration"),
        "{body}"
    );
}

/// Readiness is per set: each database must have the newest version its own set embeds,
/// whether the two sets share one database or live in two.
#[tokio::test]
#[serial_test::serial]
async fn the_data_set_is_checked_per_set_in_one_database_and_in_two() {
    if !have_database() {
        return;
    }

    // One database holding both sets (APP_DATABASE_URL = DATA_DATABASE_URL).
    let shared = ScratchDatabase::create("ready_shared");
    let one = open(&shared.url, Some(&shared.url)).await;
    let (_, body) = readyz(&one).await;
    assert_eq!(
        body["data"], "pending_migrations",
        "only the app set is applied: {body}"
    );
    run_migration_set(&shared.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let (_, body) = readyz(&one).await;
    assert_eq!(body["data"], "ok", "{body}");

    // Two databases: a data database holding the app set instead of its own is pending.
    let app_db = ScratchDatabase::create("ready_app2");
    let wrong_data_db = ScratchDatabase::create("ready_wrong_data");
    run_migration_set(&wrong_data_db.url, MigrationSet::App)
        .await
        .expect("app migrations on the wrong database");
    let two = open(&app_db.url, Some(&wrong_data_db.url)).await;
    let (_, body) = readyz(&two).await;
    assert_eq!(body["data"], "pending_migrations", "{body}");

    let data_db = ScratchDatabase::create("ready_data");
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let two = open(&app_db.url, Some(&data_db.url)).await;
    let (status, body) = readyz(&two).await;
    assert_eq!(
        (status, &body["data"]),
        (200, &serde_json::json!("ok")),
        "{body}"
    );
}

/// Equal URLs share one pool of `app + data` connections; different URLs get a pool each.
#[tokio::test]
#[serial_test::serial]
async fn equal_urls_share_one_pool() {
    if !have_database() {
        return;
    }
    let shared = ScratchDatabase::create("pool_shared");
    let one = open(&shared.url, Some(&shared.url)).await;
    let data = one.data.as_ref().unwrap();
    // Every connection the shared pool allows, checked out through the app pool, is one of
    // the data pool's too.
    let mut held = Vec::new();
    for _ in 0..(SIZES.app + SIZES.data) {
        held.push(one.app.get().await.unwrap());
    }
    assert_eq!(data.state().connections, SIZES.app + SIZES.data);
    assert_eq!(data.state().idle_connections, 0);
    drop(held);

    let other = ScratchDatabase::create("pool_other");
    let two = open(&shared.url, Some(&other.url)).await;
    let data = two.data.as_ref().unwrap();
    let mut held = Vec::new();
    for _ in 0..SIZES.app {
        held.push(two.app.get().await.unwrap());
    }
    assert_eq!(two.app.state().connections, SIZES.app);
    assert!(
        data.state().connections < SIZES.app,
        "a separate data pool is untouched"
    );
    drop(held);
}

/// An app-only backend checks series by asking a real data-plane backend over HTTP, and agrees
/// with the data-database check: a series row with no observations exists.
#[tokio::test]
#[serial_test::serial]
async fn the_data_plane_check_agrees_with_the_database_check() {
    use econ_graph_core::config::DataGraphqlConfig;
    use econ_graph_services::services::collaboration_service::CollaborationService;
    use econ_graph_services::services::series_check::{DataPlaneClient, SeriesCheck};
    use std::sync::Arc;

    if !have_database() {
        return;
    }
    // The data plane: a backend with a data database, served on a local port.
    let plane_app = ScratchDatabase::create("plane_app");
    let plane_data = ScratchDatabase::create("plane_data");
    run_migration_set(&plane_data.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let plane = open(&plane_app.url, Some(&plane_data.url)).await;
    let schema = create_schema_with_data(plane.app.clone(), plane.data.clone(), ());
    // Public data now requires explicit anonymous catalog grants. Use a real signed token
    // and the normal token provider path rather than bypassing the HTTP authorization context.
    use warp::Filter as _;
    let issuer = econ_graph_auth::testkit::TestIssuer::start().await;
    let roles: Vec<_> = econ_graph_auth::Role::all()
        .iter()
        .copied()
        .filter(|role| role.is_catalog())
        .map(|role| role.as_str())
        .collect();
    let token = issuer.mint_token(uuid::Uuid::new_v4(), &roles);
    let tokens = warp::post().map(move || {
        warp::reply::json(&serde_json::json!({
            "access_token": token, "expires_in": 300,
        }))
    });
    let (token_addr, token_server) = warp::serve(tokens).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(token_server);
    let verifier = issuer
        .verifier()
        .with_anonymous(econ_graph_auth::anonymous::AnonymousConfig {
            token_url: format!("http://{token_addr}/token"),
            client_id: "econgraph-anonymous".into(),
            client_secret: "test-only".into(),
            stale_max: std::time::Duration::ZERO,
        });
    assert!(matches!(
        verifier.renew_anonymous_grants().await,
        econ_graph_auth::anonymous::Renewal::Renewed(_)
    ));
    let routes = build_routes(
        plane.clone(),
        None,
        schema,
        Some(Arc::new(verifier)),
        &["http://localhost:3000".to_string()],
        false,
    );
    let (addr, server) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
    tokio::spawn(server);

    let data = plane.data.clone().unwrap();
    let empty = {
        let mut conn = data.get().await.unwrap();
        econ_graph_core::test_utils::insert_series_without_observations(&mut conn).await
    };
    let unknown = uuid::Uuid::new_v4();

    // The gateway rewrites /data/graphql to the backend's /graphql.
    let client = Arc::new(
        DataPlaneClient::new(&DataGraphqlConfig {
            forward_authorization: false,
            allow_insecure_authorization: false,
            url: format!("http://{addr}/graphql"),
            host: Some("br-check.localhost".to_string()),
            timeout: std::time::Duration::from_secs(5),
        })
        .unwrap(),
    );
    let remote = SeriesCheck::DataPlane(client);
    let local = SeriesCheck::Database(data.0.clone());
    for (id, exists) in [(empty, true), (unknown, false)] {
        assert_eq!(local.series_exists(id).await.unwrap(), exists, "{id}");
        assert_eq!(remote.series_exists(id).await.unwrap(), exists, "{id}");
    }

    // An app-only backend's annotation on the series without observations is stored.
    let branch_app = ScratchDatabase::create("branch_app");
    let branch = open(&branch_app.url, None).await;
    let user = econ_graph_core::models::User::get_or_create_for_subject(
        &branch.app,
        uuid::Uuid::new_v4(),
        Some("author@branch.test"),
        true,
        Some("author"),
    )
    .await
    .unwrap();
    let service = CollaborationService::with_series_check(branch.app.0.clone(), remote);
    let create = |series| {
        service.create_annotation(
            user.id,
            series,
            chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            None,
            "Note".to_string(),
            "On an empty series".to_string(),
            "note".to_string(),
            None,
            false,
            None,
        )
    };
    let annotation = create(empty)
        .await
        .expect("annotation on an existing series");
    assert_eq!(annotation.series_id, Some(empty.to_string()));
    let refused = create(unknown).await.unwrap_err();
    assert_eq!(refused.to_string(), format!("Series not found: {unknown}"));
}
