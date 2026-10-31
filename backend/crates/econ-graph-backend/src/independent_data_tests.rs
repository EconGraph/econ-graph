//! Real HTTP + signed JWT + Postgres acceptance for the independent entry points.
use super::{build_routes_for_plane, data_access::ServicePlane, Databases};
use econ_graph_auth::{testkit::TestIssuer, OidcVerifier};
use econ_graph_core::{
    database::{create_pool, run_migration_set, MigrationSet},
    models::{DataSource, EconomicSeries, NewEconomicSeries},
    test_utils::{test_dataset_id, ScratchDatabase},
    AppPool, DataPool,
};
use econ_graph_graphql::graphql::schema::create_schema_with_data;
use serde_json::{json, Value};
use std::{
    io::BufRead,
    os::unix::fs::OpenOptionsExt,
    process::{Command, Stdio},
    sync::Arc,
};
use uuid::Uuid;

struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn serve(databases: Databases, verifier: OidcVerifier, plane: ServicePlane) -> (String, Server) {
    let schema = create_schema_with_data(databases.app.clone(), databases.data.clone(), ());
    let routes = build_routes_for_plane(
        databases,
        None,
        schema,
        Some(Arc::new(verifier)),
        &[],
        false,
        plane,
    );
    let (addr, server) = warp::serve(routes).bind_ephemeral(([127, 0, 0, 1], 0));
    (format!("http://{addr}"), Server(tokio::spawn(server)))
}

async fn post(url: &str, token: Option<&str>, query: &str) -> (u16, Value) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut request = client
        .post(format!("{url}/graphql"))
        .json(&json!({"query":query}));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let reply = request.send().await.unwrap();
    (reply.status().as_u16(), reply.json().await.unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_data_http_enforces_audience_roles_and_read_only_composition() {
    // A configured test server is required; no silent skipped acceptance.
    let auth_db = ScratchDatabase::create("independent_data_auth");
    let data_db = ScratchDatabase::create("independent_data");
    let local_db = ScratchDatabase::create("independent_local_app");
    for db in [&auth_db, &local_db] {
        run_migration_set(&db.url, MigrationSet::App).await.unwrap();
    }
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .unwrap();
    let auth = AppPool(create_pool(&auth_db.url).await.unwrap());
    let data = create_pool(&data_db.url).await.unwrap();
    let local = AppPool(create_pool(&local_db.url).await.unwrap());
    let fred = DataSource::find_by_name(&data, &DataSource::fred().name)
        .await
        .unwrap()
        .unwrap();
    let dataset = test_dataset_id(&mut data.get().await.unwrap(), fred.id).await;
    let series = EconomicSeries::create(
        &data,
        &NewEconomicSeries {
            source_id: fred.id,
            dataset_id: dataset,
            external_id: "INDEPENDENT".into(),
            title: "Independent access fixture".into(),
            end_date: chrono::NaiveDate::from_ymd_opt(2024, 1, 1),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    {
        use diesel_async::RunQueryDsl;
        diesel::sql_query("INSERT INTO data_points(series_id,date,value,revision_date) VALUES($1,'2024-01-01',12345678901234567890.123456789,'2024-01-15')")
            .bind::<diesel::sql_types::Uuid,_>(series.id).execute(&mut data.get().await.unwrap()).await.unwrap();
    }
    let issuer = TestIssuer::start().await;
    let mut config = issuer.config();
    config.audience = super::data_access::DATA_AUDIENCE.into();
    let verifier = OidcVerifier::with_client(
        config.clone(),
        reqwest::Client::builder().no_proxy().build().unwrap(),
    );
    let (url, _data_server) = serve(
        Databases {
            app: auth.clone(),
            data: Some(DataPool(data.clone())),
        },
        verifier,
        ServicePlane::Data,
    );
    let (app_url, _app_server) = serve(
        Databases {
            app: local.clone(),
            data: None,
        },
        issuer.verifier(),
        ServicePlane::App,
    );
    let subject = Uuid::new_v4();
    let mut claims = issuer.claims(subject, &["catalog.fred:read"]);
    claims["aud"] = json!(["econ-graph-data"]);
    claims["azp"] = json!("econ-graph-dev-readonly");
    let readonly = issuer.sign(&claims);
    let query = format!(
        "{{seriesData(seriesId:\"{}\"){{totalCount nodes{{value}}}}}}",
        series.id
    );
    let (status, result) = post(&url, Some(&readonly), &query).await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(
        result["data"]["seriesData"],
        json!({"totalCount":1,"nodes":[{"value":"12345678901234567890.123456789"}]})
    );
    for token in [
        None,
        Some("invalid"),
        Some(issuer.mint_token(subject, &["catalog.fred:read"]).as_str()),
    ] {
        assert_eq!(post(&url, token, &query).await.0, 401);
    }
    let mut missing_aud = claims.clone();
    missing_aud.as_object_mut().unwrap().remove("aud");
    assert_eq!(
        post(&url, Some(&issuer.sign(&missing_aud)), &query).await.0,
        401
    );
    let mut denied = claims.clone();
    denied["roles"] = json!(["admin.crawlers:manage"]);
    assert_eq!(post(&url, Some(&issuer.sign(&denied)), &query).await.0, 403);
    denied["roles"] = json!([
        "catalog.fred:read",
        "admin.crawlers:manage",
        "admin.system:read"
    ]);
    for operation in [
        "mutation {triggerCrawl{success}}",
        "{innocent:crawlerStatus{isRunning}}",
        "query { ...Control } fragment Control on Query { users {totalCount} }",
    ] {
        let (status, result) = post(&url, Some(&issuer.sign(&denied)), operation).await;
        assert_eq!(status, 403, "{result}");
        assert_eq!(result["errors"][0]["extensions"]["code"], "DATA_READ_ONLY");
    }
    // A data-only developer credential cannot be used against the fresh app service.
    assert_eq!(post(&app_url, Some(&readonly), "{__typename}").await.0, 401);
    let dual = {
        let mut token = claims.clone();
        token["aud"] = json!(["econ-graph-api", "econ-graph-data"]);
        issuer.sign(&token)
    };
    let app_query = format!("{{user(userId:\"{subject}\"){{id}}}}");
    let (app_result, data_result) = tokio::join!(
        post(&app_url, Some(&dual), &app_query),
        post(&url, Some(&dual), &query)
    );
    assert_eq!(app_result.1["data"]["user"]["id"], subject.to_string());
    assert_eq!(data_result.1["data"]["seriesData"]["totalCount"], 1);

    // Exercise the actual loopback dev proxy with the signed read-only token and real
    // data HTTP service, not a stand-in GraphQL resolver or an auth-context bypass.
    let path = std::env::temp_dir().join(format!("econgraph-readonly-{}", Uuid::new_v4()));
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .unwrap();
        writeln!(f, "{readonly}").unwrap();
    }
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../scripts/data-access/dev-data-proxy.py");
    let child = Command::new("python3")
        .arg(script)
        .args([
            "--data-url",
            &format!("{url}/graphql"),
            "--port",
            "0",
            "--token-file",
        ])
        .arg(&path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    struct Proxy {
        child: std::process::Child,
        path: std::path::PathBuf,
    }
    impl Drop for Proxy {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_file(&self.path);
        }
    }
    let mut proxy = Proxy { child, path };
    let mut proxy_url = String::new();
    std::io::BufReader::new(proxy.child.stdout.take().unwrap())
        .read_line(&mut proxy_url)
        .unwrap();
    let (status, result) = post(proxy_url.trim(), Some("must-not-be-forwarded"), &query).await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(result["data"]["seriesData"]["totalCount"], 1);
    assert_eq!(
        post(proxy_url.trim(), None, "mutation{triggerCrawl{success}}")
            .await
            .0,
        403
    );

    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    assert_eq!(
        client
            .post(format!("{url}/mcp"))
            .bearer_auth(&dual)
            .body("{}")
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        404
    );
    assert_eq!(
        post(&url, Some(&readonly), &query).await.1["data"]["seriesData"]["totalCount"],
        1
    );
    // Separate auth DB and fresh app DB have no queue tables; no control operation reached
    // the data database, whose only queue remains empty.
    {
        use diesel_async::RunQueryDsl;
        #[derive(diesel::QueryableByName)]
        struct Count {
            #[diesel(sql_type=diesel::sql_types::BigInt)]
            count: i64,
        }
        assert_eq!(
            diesel::sql_query("SELECT count(*) AS count FROM crawl_queue")
                .get_result::<Count>(&mut data.get().await.unwrap())
                .await
                .unwrap()
                .count,
            0
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_data_does_not_downgrade_an_unverifiable_token_to_anonymous() {
    let issuer = TestIssuer::start().await;
    let mut config = issuer.config();
    config.audience = super::data_access::DATA_AUDIENCE.into();
    config.jwks_url = Some("http://127.0.0.1:1/keys".into());
    let verifier = OidcVerifier::with_client(
        config,
        reqwest::Client::builder().no_proxy().build().unwrap(),
    );
    let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
        diesel_async::AsyncPgConnection,
    >::new("postgres://nobody@127.0.0.1:1/none");
    let pool = AppPool(econ_graph_core::DatabasePool::builder().build_unchecked(manager));
    let (url, _server) = serve(
        Databases {
            app: pool,
            data: None,
        },
        verifier,
        ServicePlane::Data,
    );
    let mut claims = issuer.claims(Uuid::new_v4(), &["catalog.fred:read"]);
    claims["aud"] = json!(["econ-graph-data"]);
    let (status, result) = post(&url, Some(&issuer.sign(&claims)), "{__typename}").await;
    assert_eq!(status, 503, "{result}");
    assert_eq!(
        result["errors"][0]["extensions"]["code"],
        "IDENTITY_UNAVAILABLE"
    );
}
