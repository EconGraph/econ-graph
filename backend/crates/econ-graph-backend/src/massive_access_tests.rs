//! Massive, a market data vendor, is readable only with `catalog.massive_market_data:read`, a
//! role the realm grants to nobody (ECO-210).
//!
//! One Massive series in the `stock_price_daily` dataset, with its observations in Iceberg only,
//! is read on every read path: the combined `/graphql` (which the browser's CSV export reads
//! through `series.dataPoints`), the independent data service's `/graphql` (what the gateway
//! serves as `/data/graphql`), and the MCP tools and resources. Real signed tokens, verified over
//! HTTP, carry each caller's roles as the realm file grants them: an anonymous visitor (the
//! anonymous client's grants, fetched as in production), a signed-in user, staff (`support`),
//! `admin`, `super_admin`, and a dev-readonly token. None of them sees the series, its source,
//! its dataset, its observations, a search hit or a count. A test-only token holding exactly the
//! Massive role reads all of it, so the empty answers come from the role check, not from missing
//! data. A public BEA series, readable by every realm caller, keeps the empty answers honest.
//!
//! Iceberg is selected per process, so the test reruns itself in a child process with the
//! Iceberg observation backend configured.
use super::{build_routes_for_plane, data_access, data_access::ServicePlane, Databases};
use chrono::{NaiveDate, TimeZone, Utc};
use diesel_async::SimpleAsyncConnection;
use econ_graph_auth::{testkit::TestIssuer, OidcVerifier, Role};
use econ_graph_core::{
    database::{create_pool, run_migration_set, MigrationSet},
    models::{DataSource, Observation},
    test_utils::ScratchDatabase,
    AppPool, DataPool, DatabasePool,
};
use econ_graph_graphql::graphql::{context::GraphQLContext, schema::create_schema_with_data};
use econ_graph_mcp::mcp_server::EconGraphMcpServer;
use econ_graph_timeseries::{SqlCatalog, StorageConfig};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Arc};
use uuid::Uuid;

const TEST: &str = "massive_access_tests::massive_data_needs_its_role_on_every_read_path";
const CHILD: &str = "MASSIVE_ACCESS_CHILD";
const REALM: &str = include_str!("../../../../config/keycloak/econ-graph-realm.json");
const API_CLIENT: &str = "econ-graph-api";

/// Unadjusted closes, one per trading day. Distinctive, so a leak anywhere in a response shows.
const CLOSES: [&str; 3] = ["187.4321", "188.0107", "186.9953"];
/// 1 to 3 July 2019, all NYSE trading days. No timestamp the test creates falls in 2019.
const DAYS: [(u32, u32); 3] = [(7, 1), (7, 2), (7, 3)];
/// The series' external id: `{share_class_figi}:{field}`.
const EXTERNAL_ID: &str = "BBG001S5N8V8:close";
/// An id that exists nowhere: a denied series or dataset must be answered exactly as this one.
const MISSING: Uuid = Uuid::from_u128(0x6d61_7373_6976_6500_0000_0000_0000_0001);
/// A word only the Massive series' title starts with: suggestions complete title prefixes.
const TITLE_WORD: &str = "Tickeraccessaapl";

/// Runs [`run`] in a child process with the Iceberg observation backend selected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn massive_data_needs_its_role_on_every_read_path() {
    if std::env::var(CHILD).is_err() {
        let warehouse = std::env::temp_dir().join(format!("massive-access-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&warehouse).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .env("TIMESERIES_BACKEND", "iceberg")
            .env("TIMESERIES_TARGET_ID", Uuid::new_v4().to_string())
            .env("ICEBERG_WAREHOUSE", &warehouse)
            .env("ICEBERG_MAX_SCAN_ROWS", "100000")
            .env_remove("TIMESERIES_ICEBERG_MIRROR")
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&warehouse);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "the Iceberg child failed: {stdout} {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"),
            "the Iceberg child did not run the test: {stdout}"
        );
        return;
    }
    run().await;
}

/// The seeded data: ids to look up, and every string whose appearance in a response means
/// Massive data leaked.
struct Seeded {
    source: DataSource,
    dataset: Uuid,
    series: Uuid,
    bea_series: Uuid,
}

impl Seeded {
    /// The Massive ids, names, dates and prices no unauthorized response may contain.
    fn leaks(&self) -> Vec<String> {
        let mut leaks = vec![
            self.source.id.to_string(),
            self.dataset.to_string(),
            self.series.to_string(),
            EXTERNAL_ID.to_string(),
            TITLE_WORD.to_string(),
            self.source.name.clone(),
            "2019-07-0".to_string(),
        ];
        leaks.extend(CLOSES.iter().map(|close| close.to_string()));
        leaks
    }

    /// Fail if any Massive id, name, date or price appears anywhere in `response`.
    fn assert_hidden(&self, who: &str, path: &str, response: &Value) {
        let text = response.to_string();
        for leak in self.leaks() {
            assert!(
                !text.contains(&leak),
                "{who}: {path} leaked {leak:?}: {text}"
            );
        }
    }
}

/// Seed the data, start the services, and read every path as every caller.
async fn run() {
    let app_db = ScratchDatabase::create("massive_access_app");
    let data_db = ScratchDatabase::create("massive_access_data");
    run_migration_set(&app_db.url, MigrationSet::App)
        .await
        .unwrap();
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .unwrap();
    let app = AppPool(create_pool(&app_db.url).await.unwrap());
    let data = create_pool(&data_db.url).await.unwrap();
    let seeded = seed(&data).await;

    // The realm's grants for each caller. None of them may hold the Massive role.
    let massive = Role::CatalogMassiveMarketDataRead;
    let anonymous_roles = anonymous_scope();
    let signed_in: Vec<(&str, Vec<Role>)> = ["user", "support", "admin", "super_admin"]
        .into_iter()
        .map(|name| (name, realm_grants(name)))
        .collect();
    let dev_readonly: Vec<Role> = realm_grants("super_admin")
        .into_iter()
        .filter(|role| role.is_catalog())
        .collect();
    for (who, roles) in signed_in.iter().map(|(who, roles)| (*who, roles)).chain([
        ("anonymous", &anonymous_roles),
        ("dev-readonly", &dev_readonly),
    ]) {
        assert!(
            !roles.contains(&massive),
            "the realm grants {who} {massive}"
        );
        assert!(
            roles.contains(&Role::CatalogBeaRead),
            "{who} must read the BEA control series"
        );
    }

    let issuer = TestIssuer::start().await;
    let (combined, _combined_server) = serve_combined(&issuer, &app, &data, &anonymous_roles).await;
    let (data_plane, _data_server) = serve_data_plane(&issuer, &app, &data);
    let data_token = |roles: &[Role], azp: &str| {
        let names: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
        let mut claims = issuer.claims(Uuid::new_v4(), &names);
        claims["aud"] = json!([data_access::DATA_AUDIENCE]);
        claims["azp"] = json!(azp);
        issuer.sign(&claims)
    };
    let app_token = |roles: &[Role]| {
        let names: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
        issuer.mint_token(Uuid::new_v4(), &names)
    };
    let verifier = issuer.verifier();
    let mcp_context = |token: Option<String>| {
        let app = app.clone();
        let verifier = &verifier;
        let anonymous_roles = anonymous_roles.clone();
        async move {
            match token {
                Some(token) => {
                    let caller = econ_graph_auth::authenticate(
                        &app,
                        Some(verifier),
                        Some(&format!("Bearer {token}")),
                    )
                    .await
                    .unwrap()
                    .expect("a signed-in caller");
                    GraphQLContext::new(Some(caller))
                }
                None => GraphQLContext::anonymous().with_anonymous_catalog_roles(anonymous_roles),
            }
        }
    };

    // Callers without the role: nothing on any path.
    let anonymous = ("anonymous", None::<String>);
    for (who, token) in std::iter::once(anonymous).chain(
        signed_in
            .iter()
            .map(|(who, roles)| (*who, Some(app_token(roles)))),
    ) {
        let reads = graphql_reads(&combined, token.as_deref(), &seeded).await;
        assert_hidden_reads(who, "/graphql", &reads, &seeded);
        let tools = mcp_reads(mcp_context(token).await, &app, &data, &seeded).await;
        assert_hidden_mcp(who, &tools, &seeded);
    }
    // The independent data service: each signed-in caller's roles on a data-audience token,
    // and the dev-readonly client's token. Anonymous callers are refused there outright.
    assert_eq!(
        post(&data_plane, None, "{ __typename }").await.0,
        401,
        "the data service must refuse anonymous callers"
    );
    for (who, token) in signed_in
        .iter()
        .map(|(who, roles)| (*who, data_token(roles, "econ-graph-web")))
        .chain([(
            "dev-readonly",
            data_token(&dev_readonly, "econ-graph-dev-readonly"),
        )])
    {
        let reads = graphql_reads(&data_plane, Some(&token), &seeded).await;
        assert_hidden_reads(who, "/data/graphql", &reads, &seeded);
    }

    // Staff operational views name queue jobs (ids and status, by design) but carry no
    // observation value, date or price.
    let admin_roles = &signed_in.iter().find(|(who, _)| *who == "admin").unwrap().1;
    let (status, operations) = post(
        &combined,
        Some(&app_token(admin_roles)),
        r#"{ crawlerJobs(limit: 100) { source seriesId kind status errorMessage scheduledFor startedAt finishedAt updatedAt }
            crawlerSources { source name enabled }
            crawlerStatus { isRunning lastCrawl nextScheduledCrawl sources { source } }
            queueStatistics { totalItems oldestPending } }"#,
    )
    .await;
    assert_eq!(status, 200, "{operations}");
    assert!(operations.get("errors").is_none(), "{operations}");
    assert!(
        operations["data"]["crawlerJobs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|job| job["seriesId"] == EXTERNAL_ID),
        "the Massive queue job is listed: {operations}"
    );
    let values = operations.to_string();
    for leak in CLOSES.iter().copied().chain(["2019-07-0"]) {
        assert!(
            !values.contains(leak),
            "admin operational views leaked {leak:?}: {values}"
        );
    }

    // A test-only token holding exactly the Massive role reads everything, on every path.
    let only = [massive];
    let reads = graphql_reads(&combined, Some(&app_token(&only)), &seeded).await;
    assert_shown_reads("massive-only", "/graphql", &reads, &seeded);
    let reads = graphql_reads(
        &data_plane,
        Some(&data_token(&only, "econ-graph-web")),
        &seeded,
    )
    .await;
    assert_shown_reads("massive-only", "/data/graphql", &reads, &seeded);
    let tools = mcp_reads(
        mcp_context(Some(app_token(&only))).await,
        &app,
        &data,
        &seeded,
    )
    .await;
    assert_shown_mcp(&tools, &seeded);
}

/// Seed the Massive series (Iceberg observations only) and the public BEA control series, plus
/// a queue job for the Massive series, then select the Iceberg observation backend.
async fn seed(data: &DatabasePool) -> Seeded {
    let source = match DataSource::find_by_name(data, "Massive (market data vendor)")
        .await
        .unwrap()
    {
        Some(source) => source,
        None => DataSource::create(data, DataSource::massive())
            .await
            .unwrap(),
    };
    assert_eq!(
        Role::for_catalog(&source.name),
        Some(Role::CatalogMassiveMarketDataRead)
    );
    let bea = DataSource::find_by_name(data, &DataSource::bea().name)
        .await
        .unwrap()
        .unwrap();
    let (series, bea_series, bea_dataset) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut conn = data.get().await.unwrap();
    // The dataset exists once its migration has run; create it otherwise.
    #[derive(diesel::QueryableByName)]
    struct Id {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
    }
    let dataset = {
        use diesel_async::RunQueryDsl;
        diesel::sql_query(
            r#"INSERT INTO datasets (source_id, code, name, dimensions) VALUES ($1, 'stock_price_daily',
               'Daily stock prices', '[{"name":"ticker","label":"Ticker","type":"string"},
               {"name":"exchange","label":"Exchange","type":"string"},
               {"name":"currency","label":"Currency","type":"string"},
               {"name":"field","label":"Field","type":"string"}]')
               ON CONFLICT (source_id, code) DO UPDATE SET code = EXCLUDED.code RETURNING id"#,
        )
        .bind::<diesel::sql_types::Uuid, _>(source.id)
        .get_result::<Id>(&mut conn)
        .await
        .unwrap()
        .id
    };
    conn.batch_execute(&format!(
        r#"INSERT INTO economic_series (id, source_id, dataset_id, external_id, title, frequency,
               units, start_date, end_date, dimensions)
           VALUES ('{series}', '{massive}', '{dataset}', '{EXTERNAL_ID}',
               '{TITLE_WORD} Accesstest close', 'Daily', 'USD', '2019-07-01', '2019-07-03',
               '{{"ticker":"AAPL","exchange":"XNAS","currency":"USD","field":"close"}}');
           INSERT INTO datasets (id, source_id, code, name) VALUES ('{bea_dataset}', '{bea_id}',
               'massive_access_control', 'Control');
           INSERT INTO economic_series (id, source_id, dataset_id, external_id, title, frequency,
               start_date, end_date)
           VALUES ('{bea_series}', '{bea_id}', '{bea_dataset}', 'ACCESS-CONTROL',
               'Accesstest control', 'Annual', '2019-01-01', '2019-01-01');
           INSERT INTO crawl_queue (source, series_id, kind, status, priority, max_retries)
           VALUES ('MASSIVE', '{EXTERNAL_ID}', 'fetch_series', 'pending', 5, 3)"#,
        massive = source.id,
        bea_id = bea.id,
    ))
    .await
    .unwrap();
    drop(conn);

    let stamp = Utc.with_ymd_and_hms(2019, 7, 4, 0, 0, 0).unwrap();
    let observation = |series: Uuid, (month, day): (u32, u32), value: &str| {
        let date = NaiveDate::from_ymd_opt(2019, month, day).unwrap();
        Observation {
            id: Uuid::new_v4(),
            series_id: series,
            date,
            value: Some(value.parse().unwrap()),
            revision_date: date,
            is_original_release: true,
            created_at: stamp,
            updated_at: stamp,
            attributes: None,
        }
    };
    let catalog = SqlCatalog::new(data.clone(), StorageConfig::from_env().unwrap());
    for (dataset, rows) in [
        (
            dataset,
            DAYS.iter()
                .zip(CLOSES)
                .map(|(day, close)| observation(series, *day, close))
                .collect::<Vec<_>>(),
        ),
        (bea_dataset, vec![observation(bea_series, (1, 1), "42")]),
    ] {
        let prepared = catalog
            .prepare_append(Uuid::new_v4(), dataset, &rows)
            .await
            .unwrap();
        let mut conn = data.get().await.unwrap();
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                SqlCatalog::commit_prepared_conn(conn, &prepared).await
            })
            .await
            .unwrap();
    }
    // Iceberg is the observation store, as after the cutover: there is no Postgres copy.
    let target = std::env::var("TIMESERIES_TARGET_ID").unwrap();
    data.get()
        .await
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO timeseries_recovery_state (target_id, target_label, backend, epoch)
             VALUES ('{target}', 'massive access', 'iceberg', 1);
             DELETE FROM data_points"
        ))
        .await
        .unwrap();
    Seeded {
        source,
        dataset,
        series,
        bea_series,
    }
}

/// The realm's `econ-graph-api` roles granted through realm role `name`, with the realm's
/// default roles every account holds.
fn realm_grants(name: &str) -> Vec<Role> {
    let realm: Value = serde_json::from_str(REALM).unwrap();
    let roles = realm["roles"]["realm"].as_array().unwrap();
    let mut seen = BTreeSet::new();
    let mut granted = BTreeSet::new();
    let mut pending = vec![name.to_string(), "default-roles-econ-graph".to_string()];
    while let Some(next) = pending.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        let Some(role) = roles.iter().find(|role| role["name"] == next.as_str()) else {
            continue;
        };
        let composites = &role["composites"];
        for realm_role in composites["realm"].as_array().into_iter().flatten() {
            pending.push(realm_role.as_str().unwrap().to_string());
        }
        for client_role in composites["client"][API_CLIENT]
            .as_array()
            .into_iter()
            .flatten()
        {
            granted.insert(client_role.as_str().unwrap().to_string());
        }
    }
    granted
        .iter()
        .map(|name| {
            name.parse()
                .unwrap_or_else(|_| panic!("{name} is not a role"))
        })
        .collect()
}

/// The catalog roles the realm maps to the anonymous reader's client.
fn anonymous_scope() -> Vec<Role> {
    let realm: Value = serde_json::from_str(REALM).unwrap();
    realm["clientScopeMappings"][API_CLIENT]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["client"] == "econgraph-anonymous")
        .unwrap()["roles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().parse::<Role>().unwrap())
        .filter(|role| role.is_catalog())
        .collect()
}

/// A spawned HTTP server, stopped when dropped.
struct Server(tokio::task::JoinHandle<()>);
impl Drop for Server {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Serve `plane` on a free local port, verifying tokens with `verifier`.
fn serve(
    app: &AppPool,
    data: &DatabasePool,
    verifier: OidcVerifier,
    plane: ServicePlane,
) -> (String, Server) {
    let databases = Databases {
        app: app.clone(),
        data: Some(DataPool(data.clone())),
    };
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

/// The combined service, whose anonymous callers get the anonymous client's grants from a
/// token endpoint, as from Keycloak.
async fn serve_combined(
    issuer: &TestIssuer,
    app: &AppPool,
    data: &DatabasePool,
    anonymous_roles: &[Role],
) -> (String, (Server, Server)) {
    use warp::Filter as _;
    let names: Vec<&str> = anonymous_roles.iter().map(|role| role.as_str()).collect();
    let token = issuer.mint_token(Uuid::new_v4(), &names);
    let tokens = warp::post()
        .map(move || warp::reply::json(&json!({ "access_token": token, "expires_in": 300 })));
    let (token_addr, token_server) = warp::serve(tokens).bind_ephemeral(([127, 0, 0, 1], 0));
    let token_server = Server(tokio::spawn(token_server));
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
    assert_eq!(
        verifier.anonymous_catalog_roles(),
        anonymous_roles.iter().copied().collect()
    );
    let (url, server) = serve(app, data, verifier, ServicePlane::Combined);
    (url, (server, token_server))
}

/// The independent data service, which accepts only data-audience tokens.
fn serve_data_plane(issuer: &TestIssuer, app: &AppPool, data: &DatabasePool) -> (String, Server) {
    let mut config = issuer.config();
    config.audience = data_access::DATA_AUDIENCE.into();
    let verifier = OidcVerifier::with_client(
        config,
        reqwest::Client::builder().no_proxy().build().unwrap(),
    );
    serve(app, data, verifier, ServicePlane::Data)
}

/// POST `query` to `url`'s `/graphql`, with `token` as the bearer token if given.
async fn post(url: &str, token: Option<&str>, query: &str) -> (u16, Value) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut request = client
        .post(format!("{url}/graphql"))
        .json(&json!({ "query": query }));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let reply = request.send().await.unwrap();
    (reply.status().as_u16(), reply.json().await.unwrap())
}

/// Every series and observation read, one request per root that answers a denial with an
/// error, so one error can't hide another root's answer.
struct Reads {
    listing: Value,
    cross_section: Value,
    /// The same cross section of a dataset that doesn't exist, which a denial must match.
    missing_cross_section: Value,
    data_source: Value,
}

/// Read every series and observation root on `url` as the holder of `token`.
async fn graphql_reads(url: &str, token: Option<&str>, s: &Seeded) -> Reads {
    let listing = format!(
        r#"{{ dataSources {{ id name seriesCount series {{ totalCount nodes {{ id }} }} }}
            datasets {{ id code }}
            dataset(id: "{dataset}") {{ id code }}
            seriesList(pagination: {{first: 100}}) {{ totalCount nodes {{ id title }} }}
            bySource: seriesList(filter: {{sourceId: "{source}"}}) {{ totalCount nodes {{ id }} }}
            searchSeries(query: "Accesstest", first: 50) {{ totalCount series {{ id title }} }}
            bySourceSearch: searchSeries(query: "Accesstest", source: "{source}") {{ totalCount series {{ id }} }}
            searchSuggestions(partialQuery: "{prefix}") {{ suggestion }}
            series(id: "{series}") {{ id title dataPointCount
                dataPoints {{ date value }}
                recentDataPoints(limit: 5) {{ date value }}
                latestObservation {{ date value }} }}
            seriesExists(id: "{series}")
            seriesCatalog(id: "{series}")
            seriesByExternalId(sourceName: "{name}", externalId: "{EXTERNAL_ID}") {{ id }}
            seriesData(seriesId: "{series}") {{ totalCount nodes {{ date value }} }} }}"#,
        dataset = s.dataset,
        source = s.source.id,
        series = s.series,
        name = s.source.name,
        prefix = &TITLE_WORD[..8],
    );
    let cross_section = |dataset: Uuid| {
        format!(
            r#"{{ crossSection(datasetId: "{dataset}", across: "ticker", latest: true,
                filter: [{{dimension: "exchange", value: "XNAS"}}, {{dimension: "currency", value: "USD"}},
                         {{dimension: "field", value: "close"}}]) {{ key date value }} }}"#
        )
    };
    let data_source = format!(r#"{{ dataSource(id: "{}") {{ id name }} }}"#, s.source.id);
    let mut answers = Vec::new();
    for query in [
        listing,
        cross_section(s.dataset),
        cross_section(MISSING),
        data_source,
    ] {
        let (status, body) = post(url, token, &query).await;
        assert_eq!(status, 200, "{url}: {body}");
        answers.push(body);
    }
    let [listing, cross_section, missing_cross_section, data_source] = answers.try_into().unwrap();
    Reads {
        listing,
        cross_section,
        missing_cross_section,
        data_source,
    }
}

/// `answer` with the id the caller asked for replaced, since an answer may echo the request.
fn requested(answer: &Value, id: Uuid) -> Value {
    serde_json::from_str(&answer.to_string().replace(&id.to_string(), "<requested>")).unwrap()
}

/// The `id` of each object in a JSON list.
fn ids(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

/// Fail unless the GraphQL reads show no Massive data, while the control series is readable.
fn assert_hidden_reads(who: &str, path: &str, reads: &Reads, s: &Seeded) {
    let listing = &reads.listing;
    assert!(listing.get("errors").is_none(), "{who} {path}: {listing}");
    let d = &listing["data"];
    // The control series is readable, so these answers come from the role check.
    assert!(
        ids(&d["seriesList"]["nodes"]).contains(&s.bea_series.to_string()),
        "{who} {path}: {listing}"
    );
    assert_eq!(d["searchSeries"]["totalCount"], 1, "{who} {path}");
    assert_eq!(
        ids(&d["searchSeries"]["series"]),
        [s.bea_series.to_string()],
        "{who} {path}"
    );
    for counted in ["bySource", "bySourceSearch"] {
        assert_eq!(d[counted]["totalCount"], 0, "{who} {path}: {counted}");
    }
    assert_eq!(d["searchSuggestions"], json!([]), "{who} {path}");
    assert!(d["dataset"].is_null(), "{who} {path}");
    assert!(d["series"].is_null(), "{who} {path}");
    assert_eq!(d["seriesExists"], false, "{who} {path}");
    assert!(d["seriesCatalog"].is_null(), "{who} {path}");
    assert!(d["seriesByExternalId"].is_null(), "{who} {path}");
    assert_eq!(d["seriesData"]["totalCount"], 0, "{who} {path}");
    s.assert_hidden(who, &format!("{path} listing"), listing);

    // A cross section of the dataset answers exactly as for a dataset that doesn't exist, whose
    // error names the requested id and nothing else.
    let denied = requested(&reads.cross_section, s.dataset);
    assert_eq!(
        denied,
        requested(&reads.missing_cross_section, MISSING),
        "{who} {path}: crossSection"
    );
    s.assert_hidden(who, &format!("{path} crossSection"), &denied);
    // The source itself is refused by id, as for every restricted catalog.
    assert_eq!(
        reads.data_source["errors"][0]["extensions"]["code"], "FORBIDDEN",
        "{who} {path}: {}",
        reads.data_source
    );
    s.assert_hidden(who, &format!("{path} dataSource"), &reads.data_source);
}

/// Fail unless the GraphQL reads show the Massive series, its observations and its counts.
fn assert_shown_reads(who: &str, path: &str, reads: &Reads, s: &Seeded) {
    let listing = &reads.listing;
    assert!(listing.get("errors").is_none(), "{who} {path}: {listing}");
    let d = &listing["data"];
    let series = s.series.to_string();
    let source = d["dataSources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == s.source.id.to_string())
        .unwrap_or_else(|| panic!("{who} {path}: Massive is not listed: {listing}"));
    assert_eq!(source["seriesCount"], 1, "{who} {path}");
    assert_eq!(source["series"]["totalCount"], 1, "{who} {path}");
    assert!(
        ids(&d["datasets"]).contains(&s.dataset.to_string()),
        "{who} {path}"
    );
    assert_eq!(d["dataset"]["code"], "stock_price_daily", "{who} {path}");
    for list in ["seriesList", "bySource"] {
        assert_eq!(d[list]["totalCount"], 1, "{who} {path}: {list}");
        assert_eq!(
            ids(&d[list]["nodes"]),
            std::slice::from_ref(&series),
            "{who} {path}: {list}"
        );
    }
    for search in ["searchSeries", "bySourceSearch"] {
        assert_eq!(d[search]["totalCount"], 1, "{who} {path}: {search}");
        assert_eq!(
            ids(&d[search]["series"]),
            std::slice::from_ref(&series),
            "{who} {path}: {search}"
        );
    }
    assert!(
        d["searchSuggestions"]
            .to_string()
            .to_lowercase()
            .contains(&TITLE_WORD.to_lowercase()),
        "{who} {path}: {}",
        d["searchSuggestions"]
    );
    let values = |points: &Value| -> Vec<String> {
        points
            .as_array()
            .unwrap()
            .iter()
            .map(|point| point["value"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(d["series"]["id"], series.as_str(), "{who} {path}");
    assert_eq!(d["series"]["dataPointCount"], 3, "{who} {path}");
    assert_eq!(values(&d["series"]["dataPoints"]), CLOSES, "{who} {path}");
    assert_eq!(
        d["series"]["latestObservation"]["value"], CLOSES[2],
        "{who} {path}"
    );
    assert_eq!(
        values(&d["series"]["recentDataPoints"]).len(),
        3,
        "{who} {path}"
    );
    assert_eq!(d["seriesExists"], true, "{who} {path}");
    assert_eq!(d["seriesCatalog"], s.source.name.as_str(), "{who} {path}");
    assert_eq!(
        d["seriesByExternalId"]["id"],
        series.as_str(),
        "{who} {path}"
    );
    assert_eq!(d["seriesData"]["totalCount"], 3, "{who} {path}");
    assert_eq!(values(&d["seriesData"]["nodes"]), CLOSES, "{who} {path}");

    let cross = &reads.cross_section;
    assert!(cross.get("errors").is_none(), "{who} {path}: {cross}");
    assert_eq!(
        cross["data"]["crossSection"],
        json!([{"key": "AAPL", "date": "2019-07-03", "value": CLOSES[2]}]),
        "{who} {path}"
    );
    assert_eq!(
        reads.data_source["data"]["dataSource"]["name"],
        s.source.name.as_str(),
        "{who} {path}: {}",
        reads.data_source
    );
}

/// What the MCP tools and resources answer, each as JSON (an error as its message).
struct McpReads {
    search: Value,
    series_data: Value,
    metadata: Value,
    /// `get_series_data` and `get_series_metadata` for a series that doesn't exist, which a
    /// denial must match.
    missing_series_data: Value,
    missing_metadata: Value,
    data_sources: Value,
    catalog: Value,
}

/// Call every MCP data tool and resource with `context` as the caller.
async fn mcp_reads(
    context: GraphQLContext,
    app: &AppPool,
    data: &DatabasePool,
    s: &Seeded,
) -> McpReads {
    let server = EconGraphMcpServer::new(app.clone(), Some(DataPool(data.clone())))
        .with_auth_context(Arc::new(context));
    let answer = |result: anyhow::Result<Value>| match result {
        Ok(value) => value,
        Err(error) => json!({ "error": error.to_string() }),
    };
    let series = json!({ "series_id": s.series.to_string() });
    let missing = json!({ "series_id": MISSING.to_string() });
    McpReads {
        missing_series_data: answer(
            server
                .handle_tool_call("get_series_data", missing.clone())
                .await,
        ),
        missing_metadata: answer(
            server
                .handle_tool_call("get_series_metadata", missing)
                .await,
        ),
        search: answer(
            server
                .handle_tool_call(
                    "search_economic_series",
                    json!({ "query": "Accesstest", "limit": 50 }),
                )
                .await,
        ),
        series_data: answer(
            server
                .handle_tool_call("get_series_data", series.clone())
                .await,
        ),
        metadata: answer(server.handle_tool_call("get_series_metadata", series).await),
        data_sources: answer(server.get_data_sources().await),
        catalog: answer(server.get_series_catalog().await),
    }
}

/// Fail unless the MCP answers show no Massive data, while the control series is found.
fn assert_hidden_mcp(who: &str, tools: &McpReads, s: &Seeded) {
    // Asked for the Massive series, the tools answer exactly as for a series that doesn't exist.
    let series_data = requested(&tools.series_data, s.series);
    let metadata = requested(&tools.metadata, s.series);
    assert_eq!(
        series_data,
        requested(&tools.missing_series_data, MISSING),
        "{who}: MCP get_series_data"
    );
    assert_eq!(
        metadata,
        requested(&tools.missing_metadata, MISSING),
        "{who}: MCP get_series_metadata"
    );
    for (path, value) in [
        ("MCP search_economic_series", &tools.search),
        ("MCP get_series_data", &series_data),
        ("MCP get_series_metadata", &metadata),
        ("MCP data sources", &tools.data_sources),
        ("MCP series catalog", &tools.catalog),
    ] {
        s.assert_hidden(who, path, value);
    }
    // The control series is found, so the empty answers come from the role check.
    assert!(
        tools.search.to_string().contains(&s.bea_series.to_string()),
        "{who}: {}",
        tools.search
    );
    assert!(
        tools.series_data.get("error").is_some(),
        "{who}: MCP get_series_data served Massive: {}",
        tools.series_data
    );
}

/// Fail unless the MCP answers show the Massive series and its observations.
fn assert_shown_mcp(tools: &McpReads, s: &Seeded) {
    let series = s.series.to_string();
    assert!(
        tools.search.to_string().contains(&series),
        "{}",
        tools.search
    );
    let data = tools.series_data.to_string();
    for close in CLOSES {
        assert!(data.contains(close), "MCP get_series_data: {data}");
    }
    assert!(
        tools.metadata.to_string().contains(EXTERNAL_ID)
            || tools.metadata.to_string().contains(&series),
        "{}",
        tools.metadata
    );
    assert!(
        tools
            .data_sources
            .to_string()
            .contains(&s.source.id.to_string()),
        "{}",
        tools.data_sources
    );
    assert!(
        tools.catalog.to_string().contains(&series),
        "{}",
        tools.catalog
    );
}
