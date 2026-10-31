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

/// Company research consumes the same independently signed data boundary as macro series.
/// Run through `scripts/train5/with-company-api.sh` to exercise the feature candidate;
/// the ordinary release profile proves that the disabled feature cannot expose its data.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_company_http_keeps_catalog_audience_and_app_content_boundaries() {
    use diesel_async::RunQueryDsl;
    use econ_graph_core::models::{
        AnnotationAnchor, CompanyFact, CompanyFactSubject, ResearchPeriod, SubjectRef,
    };

    let auth_db = ScratchDatabase::create("company_boundary_auth");
    let data_db = ScratchDatabase::create("company_boundary_data");
    run_migration_set(&auth_db.url, MigrationSet::App)
        .await
        .unwrap();
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .unwrap();
    let auth = AppPool(create_pool(&auth_db.url).await.unwrap());
    let data = create_pool(&data_db.url).await.unwrap();
    let day = chrono::NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    let fact = CompanyFact::new(
        CompanyFactSubject {
            cik: "0000000042".into(),
            taxonomy: "us-gaap".into(),
            concept: "Assets".into(),
            period: ResearchPeriod::Instant { date: day },
            unit: "USD".into(),
        },
        "0000000042-25-000001".into(),
        chrono::NaiveDate::from_ymd_opt(2025, 2, 1).unwrap(),
        "12345678901234567890.123456789".parse().unwrap(),
        "10-K".into(),
    )
    .unwrap();
    {
        let mut conn = data.get().await.unwrap();
        diesel::sql_query("INSERT INTO companies(id,cik,name,sic_code) VALUES($1,'0000000042','Company boundary fixture','3571')")
            .bind::<diesel::sql_types::Uuid, _>(Uuid::new_v4())
            .execute(&mut conn).await.unwrap();
        diesel::sql_query("INSERT INTO company_facts(id,cik,taxonomy,concept,period_end,unit,accession,filed,value,form) VALUES($1,'0000000042','us-gaap','Assets','2024-12-31','USD','0000000042-25-000001','2025-02-01',$2,'10-K')")
            .bind::<diesel::sql_types::Uuid, _>(fact.id)
            .bind::<diesel::sql_types::Numeric, _>(&fact.value)
            .execute(&mut conn).await.unwrap();
    }
    let anchor = AnnotationAnchor::V1 {
        target: fact.reference().unwrap(),
        chart_id: None,
        observation_date: None,
        observation_value: None,
    };
    let anchor_literal = serde_json::to_string(&serde_json::to_string(&anchor).unwrap()).unwrap();
    let query = format!(
        r#"{{
        companies {{ nodes {{ cik }} }}
        company(cik:"42") {{ cik }}
        companyFacts(cik:"42") {{ nodes {{ value }} }}
        financialStatement(cik:"42",kind:BALANCE_SHEET) {{ lines {{ key }} }}
        companyRatios(cik:"42",periodEnd:"2024-12-31") {{ name }}
        companyPeers(cik:"42",periodEnd:"2024-12-31") {{ company {{ cik }} }}
        companyResearchTargetExists(anchor:{anchor_literal})
    }}"#
    );
    let issuer = TestIssuer::start().await;
    let mut config = issuer.config();
    config.audience = super::data_access::DATA_AUDIENCE.into();
    let verifier = OidcVerifier::with_client(
        config,
        reqwest::Client::builder().no_proxy().build().unwrap(),
    );
    let (url, _server) = serve(
        Databases {
            app: auth,
            data: Some(DataPool(data)),
        },
        verifier,
        ServicePlane::Data,
    );
    let subject = Uuid::new_v4();
    let mut claims = issuer.claims(subject, &["catalog.sec:read"]);
    claims["aud"] = json!(["econ-graph-data"]);
    let token = issuer.sign(&claims);

    for bad in [None, Some("not-a-token")] {
        assert_eq!(post(&url, bad, &query).await.0, 401);
    }
    let app_only = issuer.mint_token(subject, &["catalog.sec:read"]);
    assert_eq!(post(&url, Some(&app_only), &query).await.0, 401);
    let mut wrong_roles = claims.clone();
    wrong_roles["roles"] = json!(["annotation:create"]);
    assert_eq!(
        post(&url, Some(&issuer.sign(&wrong_roles)), &query).await.0,
        403
    );
    wrong_roles["roles"] = json!(["catalog.fred:read"]);
    // Query each root separately: one non-null error in an aggregate query would
    // otherwise erase the whole response and hide another root's missing guard.
    for root_query in [
        "{ companies { nodes { cik } } }".to_owned(),
        "{ company(cik:\"42\") { cik } }".to_owned(),
        "{ companyFacts(cik:\"42\") { nodes { value } } }".to_owned(),
        "{ financialStatement(cik:\"42\",kind:BALANCE_SHEET) { lines { key } } }".to_owned(),
        "{ companyRatios(cik:\"42\",periodEnd:\"2024-12-31\") { name } }".to_owned(),
        "{ companyPeers(cik:\"42\",periodEnd:\"2024-12-31\") { company { cik } } }".to_owned(),
        format!("{{ companyResearchTargetExists(anchor:{anchor_literal}) }}"),
    ] {
        let (status, denied) = post(&url, Some(&issuer.sign(&wrong_roles)), &root_query).await;
        assert_eq!(status, 200, "{root_query}: {denied}");
        assert_eq!(
            denied["errors"][0]["extensions"]["code"],
            if cfg!(flag_financial_components) {
                "FORBIDDEN"
            } else {
                "FEATURE_DISABLED"
            },
            "{root_query}: {denied}"
        );
        assert!(
            denied["data"].is_null()
                || denied["data"]
                    .as_object()
                    .is_some_and(|fields| fields.values().all(Value::is_null)),
            "{root_query}: {denied}"
        );
    }

    let (status, result) = post(&url, Some(&token), &query).await;
    assert_eq!(status, 200, "{result}");
    if cfg!(flag_financial_components) {
        assert!(result.get("errors").is_none(), "{result}");
        assert_eq!(result["data"]["companies"]["nodes"][0]["cik"], "0000000042");
        assert_eq!(result["data"]["company"]["cik"], "0000000042");
        assert_eq!(
            result["data"]["companyFacts"]["nodes"][0]["value"],
            "12345678901234567890.123456789"
        );
        assert!(!result["data"]["financialStatement"]["lines"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(!result["data"]["companyRatios"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(result["data"]["companyPeers"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(result["data"]["companyResearchTargetExists"], true);
        // Exercise the app's actual preflight HTTP client against this signed data
        // service, including its opt-in bearer forwarding and wrong-audience failure.
        let client = econ_graph_services::services::series_check::DataPlaneClient::new(
            &econ_graph_core::config::DataGraphqlConfig {
                forward_authorization: true,
                allow_insecure_authorization: true, // isolated loopback transport only
                url: format!("{url}/graphql"),
                host: None,
                timeout: std::time::Duration::from_secs(5),
            },
        )
        .unwrap();
        assert!(client
            .for_caller(Some(&format!("Bearer {token}")))
            .company_research_target_exists(&anchor)
            .await
            .unwrap());
        assert!(client
            .for_caller(Some(&format!("Bearer {app_only}")))
            .company_research_target_exists(&anchor)
            .await
            .is_err());
        assert!(client
            .company_research_target_exists(&anchor)
            .await
            .is_err());
        let mut absent = anchor;
        let AnnotationAnchor::V1 {
            target: SubjectRef::FiledFact { filing, .. },
            ..
        } = &mut absent
        else {
            unreachable!()
        };
        filing.accession = "0000000042-25-000002".into();
        let literal = serde_json::to_string(&serde_json::to_string(&absent).unwrap()).unwrap();
        let result = post(
            &url,
            Some(&token),
            &format!("{{companyResearchTargetExists(anchor:{literal})}}"),
        )
        .await
        .1;
        assert_eq!(
            result["data"]["companyResearchTargetExists"], false,
            "{result}"
        );
    } else {
        assert_eq!(
            result["errors"][0]["extensions"]["code"], "FEATURE_DISABLED",
            "{result}"
        );
    }
    // A valid SEC reader never gains the app-owned notes surface or write capability.
    for forbidden in [
        "{ companyNotes(cik:\"0000000042\") { id content } }",
        "{ ...Notes } fragment Notes on Query { hidden:companyNotes(cik:\"0000000042\") {id} }",
        "{ company(cik:\"42\") {cik} companyNotes(cik:\"0000000042\") @skip(if:true) {id} }",
        "mutation { deleteCompanyNote(id:\"00000000-0000-0000-0000-000000000001\") }",
    ] {
        let (status, result) = post(&url, Some(&token), forbidden).await;
        assert_eq!(status, 403, "{result}");
        assert_eq!(
            result["errors"][0]["extensions"]["code"], "DATA_READ_ONLY",
            "{result}"
        );
    }
}
