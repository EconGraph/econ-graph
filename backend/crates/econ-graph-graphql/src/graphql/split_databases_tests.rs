//! The schema on separate app and data databases, and on an app database alone.
//!
//! Each database holds only its own migration set, so a resolver that reads or writes through
//! the wrong pool fails with "relation does not exist" instead of passing by accident. Needs
//! `DATABASE_URL` (the test Postgres server, where scratch databases are created); skipped
//! otherwise.

use crate::graphql::context::{GraphQLContext, DATA_SERVED_REMOTELY, DATA_SERVED_REMOTELY_MESSAGE};
use crate::graphql::schema::{create_schema_with_data, create_schema_with_data_plane, sdl};
use async_graphql::Response;
use diesel::sql_types::{BigInt, Text, Uuid as SqlUuid};
use diesel_async::RunQueryDsl;
use econ_graph_core::config::DataGraphqlConfig;
use econ_graph_core::database::{
    create_pool, run_migration_set, AppPool, DataPool, DatabasePool, MigrationSet,
};
use econ_graph_core::error::{DATA_PLANE_UNAVAILABLE, DATA_PLANE_UNAVAILABLE_MESSAGE};
use econ_graph_core::models::User;
use econ_graph_core::test_utils::ScratchDatabase;
use econ_graph_services::services::collaboration_service::CollaborationService;
use econ_graph_services::services::series_check::DataPlaneClient;
use serde_json::{json, Value};
use std::sync::Arc;

/// An app database and a data database, each with only its own set applied.
struct SplitDatabases {
    _app_db: ScratchDatabase,
    _data_db: ScratchDatabase,
    app: AppPool,
    data: DataPool,
}

async fn split_databases() -> Option<SplitDatabases> {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set; skipping split-database schema tests");
        return None;
    }
    let app_db = ScratchDatabase::create("split_app");
    let data_db = ScratchDatabase::create("split_data");
    run_migration_set(&app_db.url, MigrationSet::App)
        .await
        .expect("app migrations");
    run_migration_set(&data_db.url, MigrationSet::Data)
        .await
        .expect("data migrations");
    let app = AppPool(create_pool(&app_db.url).await.expect("app pool"));
    let data = DataPool(create_pool(&data_db.url).await.expect("data pool"));
    Some(SplitDatabases {
        _app_db: app_db,
        _data_db: data_db,
        app,
        data,
    })
}

#[derive(diesel::QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

async fn count(pool: &DatabasePool, query: &str) -> i64 {
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(query)
        .get_result::<Count>(&mut conn)
        .await
        .unwrap_or_else(|e| panic!("{query}: {e}"))
        .n
}

/// Whether `table` exists in the database behind `pool`.
async fn has_table(pool: &DatabasePool, table: &str) -> bool {
    count(
        pool,
        &format!("SELECT count(*) AS n FROM pg_tables WHERE schemaname = 'public' AND tablename = '{table}'"),
    )
    .await
        == 1
}

/// A series with two observations in the data database; returns its id.
async fn seed_series(data: &DataPool) -> String {
    #[derive(diesel::QueryableByName)]
    struct Id {
        #[diesel(sql_type = SqlUuid)]
        id: uuid::Uuid,
    }
    let mut conn = data.get().await.unwrap();
    let source = diesel::sql_query("SELECT id FROM data_sources ORDER BY name LIMIT 1")
        .get_result::<Id>(&mut conn)
        .await
        .expect("the data baseline seeds data sources")
        .id;
    let dataset = econ_graph_core::test_utils::test_dataset_id(&mut conn, source).await;
    let series = diesel::sql_query(
        "INSERT INTO economic_series \
             (source_id, external_id, title, frequency, is_active, dataset_id, start_date, end_date) \
         VALUES ($1, $2, 'Split series', 'Monthly', TRUE, $3, '2024-01-01', '2024-02-01') \
         RETURNING id",
    )
    .bind::<SqlUuid, _>(source)
    .bind::<Text, _>(format!("split-{}", uuid::Uuid::new_v4()))
    .bind::<SqlUuid, _>(dataset)
    .get_result::<Id>(&mut conn)
    .await
    .expect("create test series")
    .id;
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release) \
         VALUES ($1, '2024-01-01', 1.5, '2024-01-15', TRUE), \
                ($1, '2024-02-01', 2.5, '2024-02-15', TRUE)",
    )
    .bind::<SqlUuid, _>(series)
    .execute(&mut conn)
    .await
    .expect("create test observations");
    series.to_string()
}

async fn make_user(app: &AppPool, name: &str) -> User {
    User::get_or_create_for_subject(
        app,
        uuid::Uuid::new_v4(),
        Some(&format!("{name}@split.test")),
        true,
        Some(name),
    )
    .await
    .expect("create test user")
}

/// Runs `query` as `user` (holding every role when `staff`) on a schema over `app` and `data`.
async fn run(
    app: &AppPool,
    data: Option<&DataPool>,
    user: &User,
    staff: bool,
    query: &str,
) -> Response {
    let schema = create_schema_with_data(
        app.clone(),
        data.cloned(),
        Arc::new(GraphQLContext::for_test_user(Some(user.clone()), staff)),
    );
    async_graphql::Schema::execute(&schema, query).await
}

/// Runs `query` as `user` on a schema over the app database alone that checks series through
/// `data_plane`.
async fn run_with_data_plane(
    app: &AppPool,
    data_plane: &Arc<DataPlaneClient>,
    user: &User,
    query: &str,
) -> Response {
    let schema = create_schema_with_data_plane(
        app.clone(),
        None,
        Some(data_plane.clone()),
        Arc::new(GraphQLContext::for_test_user(Some(user.clone()), false)),
    );
    async_graphql::Schema::execute(&schema, query).await
}

/// The `Host` the backend sends the data plane in these tests, as `DATA_GRAPHQL_HOST`.
const DATA_GRAPHQL_HOST: &str = "br-split.localhost";

fn data_plane(url: String, timeout: std::time::Duration) -> Arc<DataPlaneClient> {
    Arc::new(
        DataPlaneClient::new(&DataGraphqlConfig {
            forward_authorization: false,
            allow_insecure_authorization: false,
            url,
            host: Some(DATA_GRAPHQL_HOST.to_string()),
            timeout,
        })
        .expect("data plane client"),
    )
}

/// A stand-in data plane's `/data/graphql` that knows only the series `known`, in FRED.
struct StubDataPlane {
    known: String,
}

impl wiremock::Respond for StubDataPlane {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let exists = body["variables"]["id"].as_str() == Some(self.known.as_str());
        let data = match body["operationName"].as_str() {
            Some("SeriesCatalog") => json!({
                "seriesCatalog": exists.then_some("Federal Reserve Economic Data (FRED)")
            }),
            _ => json!({ "seriesExists": exists }),
        };
        wiremock::ResponseTemplate::new(200).set_body_json(json!({ "data": data }))
    }
}

/// The response's single error, which must be the data-plane-unavailable one.
fn assert_data_plane_unavailable(response: &Response) {
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let error = &response.errors[0];
    assert_eq!(error.message, DATA_PLANE_UNAVAILABLE_MESSAGE);
    let code = error
        .extensions
        .as_ref()
        .and_then(|e| e.get("code"))
        .cloned();
    assert_eq!(
        code,
        Some(async_graphql::Value::from(DATA_PLANE_UNAVAILABLE))
    );
}

/// The response's data, failing the test on any error.
fn ok(response: Response) -> Value {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().unwrap()
}

/// The response's single error, which must be the data-served-remotely one.
fn assert_served_remotely(response: &Response) {
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let error = &response.errors[0];
    assert_eq!(error.message, DATA_SERVED_REMOTELY_MESSAGE);
    let code = error
        .extensions
        .as_ref()
        .and_then(|e| e.get("code"))
        .cloned();
    assert_eq!(code, Some(async_graphql::Value::from(DATA_SERVED_REMOTELY)));
}

fn create_annotation(series_id: &str) -> String {
    format!(
        r#"mutation {{ createAnnotation(input: {{ seriesId: "{series_id}", annotationDate: "2024-01-01",
            title: "Split", content: "Note", annotationType: "note", isPublic: true }}) {{ id seriesId }} }}"#
    )
}

/// With two databases, reads of series and observations come from the data database, and
/// users, annotations, comments and audit rows go to the app database.
#[tokio::test]
#[serial_test::serial]
async fn rows_land_in_their_own_database() {
    let Some(db) = split_databases().await else {
        return;
    };
    let (app, data) = (&db.app, &db.data);
    assert!(has_table(app, "users").await && !has_table(app, "economic_series").await);
    assert!(has_table(data, "economic_series").await && !has_table(data, "users").await);

    let series_id = seed_series(data).await;
    let user = make_user(app, "author").await;
    let staff = make_user(app, "staff").await;

    // Data reads, including the dataloader-backed fields.
    let body = ok(run(
        app,
        Some(data),
        &user,
        false,
        &format!(
            r#"{{ series(id: "{series_id}") {{ title dataPointCount source {{ name }}
                    dataset {{ code }} dataPoints {{ value }} latestObservation {{ date }} }}
                dataSources {{ name seriesCount }}
                seriesList {{ totalCount }} }}"#
        ),
    )
    .await);
    assert_eq!(body["series"]["title"], "Split series");
    assert_eq!(body["series"]["dataPointCount"], 2);
    assert_eq!(body["series"]["dataPoints"].as_array().unwrap().len(), 2);
    assert_eq!(body["series"]["latestObservation"]["date"], "2024-02-01");
    assert_eq!(body["seriesList"]["totalCount"], 1);

    // An annotation on that series is written to the app database, after its series is found
    // in the data database; one on an unknown series is refused.
    let created = ok(run(
        app,
        Some(data),
        &user,
        false,
        &create_annotation(&series_id),
    )
    .await);
    let annotation_id = created["createAnnotation"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let unknown_id = uuid::Uuid::new_v4().to_string();
    let unknown = run(
        app,
        Some(data),
        &user,
        false,
        &create_annotation(&unknown_id),
    )
    .await;
    assert_eq!(unknown.errors.len(), 1, "{:?}", unknown.errors);
    assert_eq!(
        unknown.errors[0].message,
        format!("Series not found: {unknown_id}")
    );
    ok(run(
        app,
        Some(data),
        &user,
        false,
        &format!(
            r#"mutation {{ addComment(input: {{ annotationId: "{annotation_id}", content: "Agreed" }}) {{ id }} }}"#
        ),
    )
    .await);
    let listed = ok(run(
        app,
        Some(data),
        &user,
        false,
        &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ id }} }}"#),
    )
    .await);
    assert_eq!(
        listed["annotationsForSeries"],
        json!([{ "id": annotation_id }])
    );
    assert_eq!(
        count(app, "SELECT count(*) AS n FROM chart_annotations").await,
        1
    );
    assert_eq!(
        count(app, "SELECT count(*) AS n FROM annotation_comments").await,
        1
    );

    // triggerCrawl queues in the data database and audits in the app database.
    ok(run(
        app,
        Some(data),
        &staff,
        true,
        r#"mutation { triggerCrawl(input: { source: "FRED", seriesIds: ["SPLITTEST"] }) { enqueuedCount } }"#,
    )
    .await);
    assert_eq!(
        count(data, "SELECT count(*) AS n FROM crawl_queue").await,
        1
    );
    assert_eq!(
        count(
            app,
            "SELECT count(*) AS n FROM audit_logs WHERE action = 'trigger_crawl'"
        )
        .await,
        1
    );

    // systemHealth reads users from one database and the queue from the other.
    let health = ok(run(
        app,
        Some(data),
        &staff,
        true,
        "{ systemHealth { metrics { totalUsers queueItems } } crawlerStatus { isRunning } }",
    )
    .await);
    assert_eq!(health["systemHealth"]["metrics"]["totalUsers"], 2);
    assert_eq!(health["systemHealth"]["metrics"]["queueItems"], 1);
}

/// With no data database, app operations work and every data field fails with
/// `DATA_SERVED_REMOTELY`; the schema is the same either way.
#[tokio::test]
#[serial_test::serial]
async fn without_a_data_database_data_fields_are_served_remotely() {
    let Some(db) = split_databases().await else {
        return;
    };
    let app = &db.app;
    let user = make_user(app, "author").await;
    let staff = make_user(app, "staff").await;
    let series_id = uuid::Uuid::new_v4().to_string();

    for (as_staff, query) in [
        (false, format!(r#"{{ series(id: "{series_id}") {{ title }} }}"#)),
        (false, "{ dataSources { name } }".to_string()),
        (false, "{ seriesList { totalCount } }".to_string()),
        (false, r#"{ searchSeries(query: "gdp") { totalCount } }"#.to_string()),
        (false, "{ datasets { code } }".to_string()),
        (
            false,
            format!(r#"{{ seriesData(seriesId: "{series_id}") {{ nodes {{ value }} }} }}"#),
        ),
        (true, "{ crawlerStatus { isRunning } }".to_string()),
        (true, "{ queueStatistics { totalItems } }".to_string()),
        (
            true,
            r#"mutation { triggerCrawl(input: { source: "FRED", seriesIds: ["GDP"] }) { enqueuedCount } }"#
                .to_string(),
        ),
    ] {
        let actor = if as_staff { &staff } else { &user };
        let response = run(app, None, actor, as_staff, &query).await;
        assert_served_remotely(&response);
    }
    // The refused triggerCrawl is still audited, as unavailable.
    assert_eq!(
        count(
            app,
            "SELECT count(*) AS n FROM audit_logs \
             WHERE action = 'trigger_crawl' AND details->>'outcome' = 'unavailable'"
        )
        .await,
        1
    );

    // App operations. With neither a data database nor a data plane no series' catalog can be
    // checked, so an annotation on a series is refused and none are listed; users and
    // systemHealth (with no queue count) still answer.
    let refused = run(app, None, &user, false, &create_annotation(&series_id)).await;
    assert_eq!(refused.errors.len(), 1, "{:?}", refused.errors);
    assert_eq!(
        refused.errors[0].message,
        format!("Series not found: {series_id}")
    );
    let listed = ok(run(
        app,
        None,
        &user,
        false,
        &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ title }} }}"#),
    )
    .await);
    assert_eq!(listed["annotationsForSeries"], json!([]));
    let health = ok(run(
        app,
        None,
        &staff,
        true,
        "{ systemHealth { metrics { totalUsers queueItems } } users { totalCount } }",
    )
    .await);
    assert_eq!(health["systemHealth"]["metrics"]["totalUsers"], 2);
    assert_eq!(health["systemHealth"]["metrics"]["queueItems"], Value::Null);
    assert_eq!(health["users"]["totalCount"], 2);

    // A configured but unreachable data database also leaves the app metrics and a null count.
    let unreachable = DataPool(
        DatabasePool::builder()
            .connection_timeout(std::time::Duration::from_secs(1))
            .build_unchecked(
                diesel_async::pooled_connection::AsyncDieselConnectionManager::<
                    diesel_async::AsyncPgConnection,
                >::new("postgres://nobody@127.0.0.1:1/none"),
            ),
    );
    let health = ok(run(
        app,
        Some(&unreachable),
        &staff,
        true,
        "{ systemHealth { metrics { totalUsers queueItems } } }",
    )
    .await);
    assert_eq!(health["systemHealth"]["metrics"]["totalUsers"], 2);
    assert_eq!(health["systemHealth"]["metrics"]["queueItems"], Value::Null);

    // The schema doesn't depend on the databases.
    let with_data = create_schema_with_data(app.clone(), Some(db.data.clone()), ()).sdl();
    let without_data = create_schema_with_data(app.clone(), None, ()).sdl();
    assert_eq!(with_data, without_data);
    assert_eq!(without_data, sdl());
}

/// With no data database but a data plane URL, a new annotation's series is checked over
/// HTTP: the request carries the configured `Host` and no credentials.
#[tokio::test]
#[serial_test::serial]
async fn with_a_data_plane_url_the_series_is_checked_over_http() {
    let Some(db) = split_databases().await else {
        return;
    };
    let app = &db.app;
    let user = make_user(app, "author").await;
    let known = uuid::Uuid::new_v4().to_string();
    let stub = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/data/graphql"))
        .respond_with(StubDataPlane {
            known: known.clone(),
        })
        .mount(&stub)
        .await;
    let data_plane = data_plane(
        format!("{}/data/graphql", stub.uri()),
        std::time::Duration::from_secs(2),
    );

    let created =
        ok(run_with_data_plane(app, &data_plane, &user, &create_annotation(&known)).await);
    assert_eq!(created["createAnnotation"]["seriesId"], known.as_str());

    let unknown = uuid::Uuid::new_v4().to_string();
    let refused = run_with_data_plane(app, &data_plane, &user, &create_annotation(&unknown)).await;
    assert_eq!(refused.errors.len(), 1, "{:?}", refused.errors);
    assert_eq!(
        refused.errors[0].message,
        format!("Series not found: {unknown}")
    );
    assert_eq!(
        count(app, "SELECT count(*) AS n FROM chart_annotations").await,
        1
    );

    // The known series: its catalog, then its existence; the unknown one: its catalog only.
    let requests = stub.received_requests().await.expect("recorded requests");
    let operations: Vec<_> = requests
        .iter()
        .map(|request| {
            serde_json::from_slice::<Value>(&request.body).unwrap()["operationName"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        operations,
        ["SeriesCatalog", "SeriesExists", "SeriesCatalog"]
    );
    for request in &requests {
        assert!(request.headers.get("authorization").is_none());
        assert!(request.headers.get("cookie").is_none());
        assert_eq!(request.headers.get("host").unwrap(), DATA_GRAPHQL_HOST);
    }
}

/// A data plane that is down or slower than the timeout refuses the annotation with
/// `DATA_PLANE_UNAVAILABLE` and writes nothing. Listing the annotations already made fails
/// the same way, since it needs the series' catalog from the data plane.
#[tokio::test]
#[serial_test::serial]
async fn an_unavailable_data_plane_refuses_the_annotation() {
    let Some(db) = split_databases().await else {
        return;
    };
    let app = &db.app;
    let user = make_user(app, "author").await;
    let series_id = uuid::Uuid::new_v4().to_string();
    // An annotation stored earlier, straight through the service (which checks no catalog).
    CollaborationService::new(app.0.clone())
        .create_annotation(
            user.id,
            uuid::Uuid::parse_str(&series_id).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            None,
            "Split".to_string(),
            "Note".to_string(),
            "note".to_string(),
            None,
            true,
            None,
        )
        .await
        .expect("store an annotation");

    let slow = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_json(json!({ "data": { "seriesExists": true } }))
                .set_delay(std::time::Duration::from_secs(5)),
        )
        .mount(&slow)
        .await;
    let failing = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&failing)
        .await;
    // A port nothing listens on.
    let down_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    for url in [
        format!("{}/data/graphql", slow.uri()),
        format!("{}/data/graphql", failing.uri()),
        format!("http://127.0.0.1:{down_port}/data/graphql"),
    ] {
        let data_plane = data_plane(url, std::time::Duration::from_millis(300));
        let started = std::time::Instant::now();
        let response =
            run_with_data_plane(app, &data_plane, &user, &create_annotation(&series_id)).await;
        assert_data_plane_unavailable(&response);
        assert!(started.elapsed() < std::time::Duration::from_secs(3));
        assert_eq!(
            count(app, "SELECT count(*) AS n FROM chart_annotations").await,
            1
        );

        // Reading the annotations needs the series' catalog, so it fails closed too: a
        // revoked catalog stays revoked while the data plane is down.
        assert_data_plane_unavailable(
            &run_with_data_plane(
                app,
                &data_plane,
                &user,
                &format!(r#"{{ annotationsForSeries(seriesId: "{series_id}") {{ title }} }}"#),
            )
            .await,
        );
    }
}

/// A series row with no observations exists for annotations, as for `seriesExists`, though
/// `series(id)` hides it.
#[tokio::test]
#[serial_test::serial]
async fn a_series_without_observations_can_be_annotated() {
    let Some(db) = split_databases().await else {
        return;
    };
    let (app, data) = (&db.app, &db.data);
    let user = make_user(app, "author").await;
    let empty = {
        let mut conn = data.get().await.unwrap();
        econ_graph_core::test_utils::insert_series_without_observations(&mut conn)
            .await
            .to_string()
    };
    let unknown = uuid::Uuid::new_v4().to_string();

    let body = ok(run(
        app,
        Some(data),
        &user,
        false,
        &format!(
            r#"{{ empty: seriesExists(id: "{empty}") unknown: seriesExists(id: "{unknown}")
                series(id: "{empty}") {{ id }} }}"#
        ),
    )
    .await);
    assert_eq!(
        body,
        json!({ "empty": true, "unknown": false, "series": null })
    );

    let created = ok(run(app, Some(data), &user, false, &create_annotation(&empty)).await);
    assert_eq!(created["createAnnotation"]["seriesId"], empty.as_str());

    // Without a data database, `seriesExists` is a data field like the rest.
    assert_served_remotely(
        &run(
            app,
            None,
            &user,
            false,
            &format!(r#"{{ seriesExists(id: "{empty}") }}"#),
        )
        .await,
    );
}
