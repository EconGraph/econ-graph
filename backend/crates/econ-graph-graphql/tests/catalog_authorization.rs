//! Catalog authorization against the production library (including missing-context denial).
//! Requires DATABASE_URL pointing at disposable Postgres; creates its own scratch database.
use async_graphql::{Request, Response};
use econ_graph_auth::{Principal, Role};
use econ_graph_core::{
    config::DataGraphqlConfig,
    database::{AppPool, DataPool, DatabasePool},
    models::{DataSource, EconomicSeries, NewEconomicSeries, User},
    test_utils::{test_dataset_id, ScratchDatabase},
};
use econ_graph_graphql::graphql::{
    context::GraphQLContext,
    schema::{create_schema, create_schema_with_data, create_schema_with_data_plane},
};
use econ_graph_graphql::types::DataSourceType;
use econ_graph_services::services::series_check::DataPlaneClient;
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
    // Six observations, so a denied seriesData page would differ from a missing series' one.
    {
        use diesel_async::RunQueryDsl;
        diesel::sql_query(
            "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release) \
             SELECT $1, make_date(2023, m, 1), m, make_date(2023, m, 15), TRUE \
             FROM generate_series(1, 6) AS m",
        )
        .bind::<diesel::sql_types::Uuid, _>(series.id)
        .execute(&mut pool.get().await.unwrap())
        .await
        .unwrap();
    }
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
    // Lookups by id answer a series or dataset the caller can't read exactly as a missing one,
    // data and errors alike: no existence oracle.
    let missing = Uuid::new_v4();
    let page = "{ totalCount nodes { id } pageInfo { hasNextPage hasPreviousPage startCursor endCursor } }";
    for (denied, absent) in [
        (
            format!(r#"{{ series(id:"{bea_series}") {{ id }} }}"#),
            format!(r#"{{ series(id:"{missing}") {{ id }} }}"#),
        ),
        (
            format!(r#"{{ series(id:"{unknown_series}") {{ id }} }}"#),
            format!(r#"{{ series(id:"{missing}") {{ id }} }}"#),
        ),
        (
            format!(r#"{{ seriesExists(id:"{bea_series}") }}"#),
            format!(r#"{{ seriesExists(id:"{missing}") }}"#),
        ),
        (
            format!(r#"{{ seriesCatalog(id:"{bea_series}") }}"#),
            format!(r#"{{ seriesCatalog(id:"{missing}") }}"#),
        ),
        (
            format!(
                r#"{{ seriesByExternalId(sourceName:"{}",externalId:"CATALOG-BEA") {{ id }} }}"#,
                bea.name
            ),
            format!(
                r#"{{ seriesByExternalId(sourceName:"{}",externalId:"NO-SUCH-SERIES") {{ id }} }}"#,
                bea.name
            ),
        ),
        (
            format!(r#"{{ seriesData(seriesId:"{bea_series}") {page} }}"#),
            format!(r#"{{ seriesData(seriesId:"{missing}") {page} }}"#),
        ),
        (
            format!(r#"{{ seriesData(seriesId:"{bea_series}", first:2, after:"3") {page} }}"#),
            format!(r#"{{ seriesData(seriesId:"{missing}", first:2, after:"3") {page} }}"#),
        ),
        (
            format!(r#"{{ dataset(id:"{bea_dataset}") {{ id }} }}"#),
            format!(r#"{{ dataset(id:"{missing}") {{ id }} }}"#),
        ),
        (
            format!(
                r#"{{ crossSection(datasetId:"{bea_dataset}",across:"area",latest:true) {{ key }} }}"#
            ),
            format!(
                r#"{{ crossSection(datasetId:"{missing}",across:"area",latest:true) {{ key }} }}"#
            ),
        ),
        (
            format!(
                r#"{{ crossSection(datasetId:"{bea_dataset}",across:"no-such-dimension",latest:true) {{ key }} }}"#
            ),
            format!(
                r#"{{ crossSection(datasetId:"{missing}",across:"no-such-dimension",latest:true) {{ key }} }}"#
            ),
        ),
    ] {
        let denied_answer = answer(
            &execute(&pool, anonymous(&fred_only), &denied).await,
            &[bea_series, bea_dataset, unknown_series],
        );
        let absent_answer = answer(
            &execute(&pool, anonymous(&fred_only), &absent).await,
            &[missing],
        );
        assert_eq!(denied_answer, absent_answer, "{denied}");
    }
    let readable = execute(
        &pool,
        anonymous(&fred_only),
        &format!(
            r#"{{ seriesCatalog(id:"{fred_series}") seriesByExternalId(sourceName:"{}",externalId:"CATALOG-FRED") {{ id }} }}"#,
            fred.name
        ),
    )
    .await;
    assert!(readable.errors.is_empty(), "{:?}", readable.errors);
    let readable = readable.data.into_json().unwrap();
    assert_eq!(readable["seriesCatalog"], fred.name.as_str());
    assert_eq!(
        readable["seriesByExternalId"]["id"],
        fred_series.to_string()
    );

    // A data source is refused by id whether or not it exists.
    let result = execute(
        &pool,
        anonymous(&fred_only),
        &format!(r#"{{ dataSource(id:"{}") {{ name }} }}"#, bea.id),
    )
    .await;
    assert_eq!(
        result.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("FORBIDDEN")),
        "{:?}",
        result.errors
    );
    let mixed = execute(&pool, anonymous(&fred_only), &format!(r#"{{ open:series(id:"{fred_series}") {{ id }} blocked:series(id:"{bea_series}") {{ id }} }}"#)).await;
    assert!(mixed.errors.is_empty(), "{:?}", mixed.errors);
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

/// A migrated scratch database with one FRED series and one BEA series.
struct Fixture {
    _db: ScratchDatabase,
    pool: DatabasePool,
    fred: DataSource,
    bea: DataSource,
    fred_series: Uuid,
    bea_series: Uuid,
}

async fn fixture(tag: &str) -> Fixture {
    let db = ScratchDatabase::create(tag);
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
    let (fred_series, _) = seed(&pool, fred.clone(), "FRED").await;
    let (bea_series, _) = seed(&pool, bea.clone(), "BEA").await;
    Fixture {
        _db: db,
        pool,
        fred,
        bea,
        fred_series,
        bea_series,
    }
}

async fn user(pool: &DatabasePool, name: &str) -> User {
    User::get_or_create_for_subject(
        pool,
        Uuid::new_v4(),
        Some(&format!("{name}@catalog.test")),
        true,
        Some(name),
    )
    .await
    .unwrap()
}

/// A signed-in annotator holding `catalogs` and the annotation roles.
fn annotator(user: &User, catalogs: &[Role]) -> GraphQLContext {
    GraphQLContext::signed_in(
        user.clone(),
        [Role::AnnotationCreate, Role::AnnotationComment]
            .into_iter()
            .chain(catalogs.iter().copied()),
    )
}

fn create_annotation(series: impl std::fmt::Display) -> String {
    format!(
        r#"mutation {{ createAnnotation(input: {{ seriesId: "{series}", annotationDate: "2024-01-01",
            annotationValue: "42.5", title: "Catalog note", content: "Note",
            annotationType: "note", isPublic: true }}) {{ id annotationValue }} }}"#
    )
}

/// The response's data and errors (message and code) with every UUID in `ids` replaced, for
/// comparing a denial with the answer for something that doesn't exist.
fn answer(response: &Response, ids: &[Uuid]) -> serde_json::Value {
    let mut text = serde_json::json!({
        "data": response.data.clone().into_json().unwrap(),
        "errors": response.errors.iter().map(|error| serde_json::json!({
            "message": error.message,
            "code": error.extensions.as_ref().and_then(|e| e.get("code")).map(|c| c.to_string()),
        })).collect::<Vec<_>>(),
    })
    .to_string();
    for id in ids {
        text = text.replace(&id.to_string(), "<id>");
    }
    serde_json::from_str(&text).unwrap()
}

/// The response's only error with every UUID in `ids` replaced, for comparing a denial with the
/// answer for something that doesn't exist.
fn only_error(response: &Response, ids: &[Uuid]) -> (String, Option<async_graphql::Value>) {
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let error = &response.errors[0];
    let mut message = error.message.clone();
    for id in ids {
        message = message.replace(&id.to_string(), "<id>");
    }
    let code = error
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("code"))
        .cloned();
    (message, code)
}

/// `createAnnotation` checks catalog access before the series exists, in the data database: a
/// FRED-only user gets the same error for a BEA series as for a series that doesn't exist.
#[tokio::test]
async fn create_annotation_on_an_unreadable_series_looks_like_a_missing_series() {
    let f = fixture("catalog_annotate").await;
    let author = user(&f.pool, "fred-only").await;
    let fred_only = || annotator(&author, &[Role::CatalogFredRead]);

    let created = execute(&f.pool, fred_only(), &create_annotation(f.fred_series)).await;
    assert!(created.errors.is_empty(), "{:?}", created.errors);

    let missing = Uuid::new_v4();
    let denied = execute(&f.pool, fred_only(), &create_annotation(f.bea_series)).await;
    let absent = execute(&f.pool, fred_only(), &create_annotation(missing)).await;
    assert_eq!(
        only_error(&denied, &[f.bea_series]),
        only_error(&absent, &[missing])
    );
    let stored = econ_graph_services::services::collaboration_service::CollaborationService::new(
        f.pool.clone(),
    )
    .get_annotations_for_series(&f.bea_series.to_string(), Some(author.id))
    .await
    .unwrap();
    assert!(stored.is_empty(), "the denied annotation was stored");

    // Anonymous callers can't annotate at all; a BEA grant lets the same user annotate.
    let anonymous_result = execute(
        &f.pool,
        anonymous(&[Role::CatalogBeaRead]),
        &create_annotation(f.bea_series),
    )
    .await;
    assert!(!anonymous_result.errors.is_empty());
    let granted = execute(
        &f.pool,
        annotator(&author, &[Role::CatalogBeaRead]),
        &create_annotation(f.bea_series),
    )
    .await;
    assert!(granted.errors.is_empty(), "{:?}", granted.errors);
}

/// Annotations, their comments, and writes to them follow the series' catalog: after the BEA
/// grant is gone, a FRED-only user (even the author) and anonymous callers see nothing of the
/// BEA annotation, with the same answers as for a missing series or annotation.
#[tokio::test]
async fn annotation_reads_and_writes_follow_the_series_catalog() {
    let f = fixture("catalog_annotation_reads").await;
    let author = user(&f.pool, "author").await;
    let both = [Role::CatalogFredRead, Role::CatalogBeaRead];

    // Made while the author could still read BEA.
    let mut ids = Vec::new();
    for series in [f.bea_series, f.fred_series] {
        let created = execute(
            &f.pool,
            annotator(&author, &both),
            &create_annotation(series),
        )
        .await;
        assert!(created.errors.is_empty(), "{:?}", created.errors);
        let id: Uuid = created.data.into_json().unwrap()["createAnnotation"]["id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let commented = execute(
            &f.pool,
            annotator(&author, &both),
            &format!(
                r#"mutation {{ addComment(input: {{ annotationId: "{id}", content: "Seen" }}) {{ id }} }}"#
            ),
        )
        .await;
        assert!(commented.errors.is_empty(), "{:?}", commented.errors);
        ids.push(id);
    }
    let (bea_annotation, fred_annotation) = (ids[0], ids[1]);

    let list = |series: Uuid| {
        format!(r#"{{ annotationsForSeries(seriesId: "{series}") {{ id annotationValue }} }}"#)
    };
    let comments = |annotation: Uuid| {
        format!(r#"{{ commentsForAnnotation(annotationId: "{annotation}") {{ id }} }}"#)
    };
    let missing_series = Uuid::new_v4();
    let missing_annotation = Uuid::new_v4();
    type Caller = fn(&User) -> GraphQLContext;
    let callers: [(&str, Caller); 2] = [
        ("fred-only author", |user| {
            annotator(user, &[Role::CatalogFredRead])
        }),
        ("anonymous", |_| anonymous(&[Role::CatalogFredRead])),
    ];
    for (who, caller) in callers {
        // Readable catalog: annotations and comments as before.
        let readable = execute(&f.pool, caller(&author), &list(f.fred_series)).await;
        assert!(readable.errors.is_empty(), "{who}: {:?}", readable.errors);
        assert_eq!(
            readable.data.into_json().unwrap()["annotationsForSeries"][0]["annotationValue"],
            "42.5",
            "{who}"
        );
        let readable = execute(&f.pool, caller(&author), &comments(fred_annotation)).await;
        assert!(readable.errors.is_empty(), "{who}: {:?}", readable.errors);

        // Unreadable catalog: the same answers as for a series or annotation that doesn't exist.
        let denied = execute(&f.pool, caller(&author), &list(f.bea_series)).await;
        let absent = execute(&f.pool, caller(&author), &list(missing_series)).await;
        assert!(denied.errors.is_empty(), "{who}: {:?}", denied.errors);
        assert_eq!(
            denied.data.into_json().unwrap(),
            absent.data.into_json().unwrap(),
            "{who}: annotationsForSeries leaked a BEA annotation"
        );
        let denied = execute(&f.pool, caller(&author), &comments(bea_annotation)).await;
        let absent = execute(&f.pool, caller(&author), &comments(missing_annotation)).await;
        assert_eq!(
            only_error(&denied, &[bea_annotation]),
            only_error(&absent, &[missing_annotation]),
            "{who}: commentsForAnnotation"
        );
    }

    // Writes to the BEA annotation answer as for a missing one, even for its author.
    let fred_only = || annotator(&author, &[Role::CatalogFredRead]);
    for write in [
        |id: Uuid| {
            format!(
                r#"mutation {{ addComment(input: {{ annotationId: "{id}", content: "Again" }}) {{ id }} }}"#
            )
        },
        |id: Uuid| {
            format!(
                r#"mutation {{ updateAnnotation(input: {{ annotationId: "{id}", title: "Edited" }}) {{ id annotationValue }} }}"#
            )
        },
    ] {
        let denied = execute(&f.pool, fred_only(), &write(bea_annotation)).await;
        let absent = execute(&f.pool, fred_only(), &write(missing_annotation)).await;
        assert_eq!(
            only_error(&denied, &[bea_annotation]),
            only_error(&absent, &[missing_annotation]),
            "{}",
            write(bea_annotation)
        );
    }

    // Restoring the grant restores access.
    let restored = execute(&f.pool, anonymous(&both), &list(f.bea_series)).await;
    assert!(restored.errors.is_empty(), "{:?}", restored.errors);
    assert_eq!(
        restored.data.into_json().unwrap()["annotationsForSeries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// A backend with neither a data database nor a data plane can't check any series' catalog, so
/// it refuses annotations on series outright, even to callers holding every catalog: no create,
/// no listing, no comments, no writes. (With sign-in on, such a backend does not start.)
#[tokio::test]
async fn backends_without_data_or_a_data_plane_refuse_annotations_on_series() {
    let f = fixture("catalog_no_data").await;
    let author = user(&f.pool, "skip").await;
    let every_catalog: Vec<Role> = Role::all()
        .iter()
        .copied()
        .filter(|role| role.is_catalog())
        .collect();
    // An annotation stored earlier (by a backend that could check), with a comment.
    let stored = execute(
        &f.pool,
        annotator(&author, &every_catalog),
        &create_annotation(f.bea_series),
    )
    .await;
    assert!(stored.errors.is_empty(), "{:?}", stored.errors);
    let annotation: Uuid = stored.data.into_json().unwrap()["createAnnotation"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let app_only = |context: GraphQLContext, query: String| {
        let schema = create_schema_with_data(AppPool(f.pool.clone()), None, Arc::new(context));
        async move { schema.execute(Request::new(query)).await }
    };
    let missing_series = Uuid::new_v4();
    let missing_annotation = Uuid::new_v4();

    // Create: the same error as for a series that doesn't exist, whatever the grants.
    for context in [annotator(&author, &every_catalog), annotator(&author, &[])] {
        let refused = app_only(context, create_annotation(f.bea_series)).await;
        assert_eq!(
            only_error(&refused, &[f.bea_series]),
            only_error(
                &app_only(annotator(&author, &[]), create_annotation(missing_series)).await,
                &[missing_series]
            )
        );
    }

    // Reads: nothing for anonymous callers or the author, even holding every catalog.
    for context in [
        anonymous(&every_catalog),
        anonymous(&[]),
        annotator(&author, &every_catalog),
    ] {
        let listed = app_only(
            context,
            format!(
                r#"{{ annotationsForSeries(seriesId: "{}") {{ annotationValue }} }}"#,
                f.bea_series
            ),
        )
        .await;
        assert!(listed.errors.is_empty(), "{:?}", listed.errors);
        assert_eq!(
            listed.data.into_json().unwrap()["annotationsForSeries"],
            serde_json::json!([])
        );
    }
    let comments =
        |id: Uuid| format!(r#"{{ commentsForAnnotation(annotationId: "{id}") {{ id }} }}"#);
    assert_eq!(
        only_error(
            &app_only(anonymous(&every_catalog), comments(annotation)).await,
            &[annotation]
        ),
        only_error(
            &app_only(anonymous(&every_catalog), comments(missing_annotation)).await,
            &[missing_annotation]
        )
    );
    let add_comment = |id: Uuid| {
        format!(
            r#"mutation {{ addComment(input: {{ annotationId: "{id}", content: "x" }}) {{ id }} }}"#
        )
    };
    assert_eq!(
        only_error(
            &app_only(annotator(&author, &every_catalog), add_comment(annotation)).await,
            &[annotation]
        ),
        only_error(
            &app_only(
                annotator(&author, &every_catalog),
                add_comment(missing_annotation)
            )
            .await,
            &[missing_annotation]
        )
    );
}

/// A stand-in data plane naming each series' catalog, or null for any other series (missing,
/// or unreadable to the data plane's own grants).
struct StubDataPlane(Vec<(Uuid, &'static str)>);

impl wiremock::Respond for StubDataPlane {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let id = body["variables"]["id"].as_str().unwrap_or_default();
        let catalog = self
            .0
            .iter()
            .find(|(series, _)| series.to_string() == id)
            .map(|(_, catalog)| *catalog);
        let data = match body["operationName"].as_str() {
            Some("SeriesCatalog") => serde_json::json!({ "seriesCatalog": catalog }),
            _ => serde_json::json!({ "seriesExists": catalog.is_some() }),
        };
        wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": data }))
    }
}

/// On a backend without a data database, the data plane names the series' catalog and this
/// backend applies its own caller's grants: a FRED-only user can't annotate or read annotations
/// on a BEA series, with the same answer as for a missing series.
#[tokio::test]
async fn app_only_backends_apply_catalog_grants_through_the_data_plane() {
    let db = ScratchDatabase::create("catalog_remote");
    econ_graph_core::database::run_migrations(&db.url)
        .await
        .unwrap();
    let pool = econ_graph_core::database::create_pool(&db.url)
        .await
        .unwrap();
    let (fred_series, bea_series, plane_denied) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(StubDataPlane(vec![
            (fred_series, "Federal Reserve Economic Data (FRED)"),
            (bea_series, "Bureau of Economic Analysis (BEA)"),
        ]))
        .mount(&server)
        .await;
    let plane = Arc::new(
        DataPlaneClient::new(&DataGraphqlConfig {
            forward_authorization: false,
            allow_insecure_authorization: false,
            url: server.uri(),
            host: None,
            timeout: std::time::Duration::from_secs(5),
        })
        .unwrap(),
    );
    let run = |context: GraphQLContext, query: String| {
        let schema = create_schema_with_data_plane(
            AppPool(pool.clone()),
            None,
            Some(plane.clone()),
            Arc::new(context),
        );
        async move { schema.execute(Request::new(query)).await }
    };
    let author = user(&pool, "remote").await;
    let fred_only = || annotator(&author, &[Role::CatalogFredRead]);

    let created = run(fred_only(), create_annotation(fred_series)).await;
    assert!(created.errors.is_empty(), "{:?}", created.errors);
    let missing = Uuid::new_v4();
    let absent = only_error(
        &run(fred_only(), create_annotation(missing)).await,
        &[missing],
    );
    for denied in [bea_series, plane_denied] {
        assert_eq!(
            only_error(
                &run(fred_only(), create_annotation(denied)).await,
                &[denied]
            ),
            absent,
            "{denied}"
        );
    }
    let granted = run(
        annotator(&author, &[Role::CatalogBeaRead]),
        create_annotation(bea_series),
    )
    .await;
    assert!(granted.errors.is_empty(), "{:?}", granted.errors);

    let list =
        |series: Uuid| format!(r#"{{ annotationsForSeries(seriesId: "{series}") {{ id }} }}"#);
    for (context, series, expected) in [
        (fred_only(), fred_series, 1),
        (fred_only(), bea_series, 0),
        (anonymous(&[Role::CatalogFredRead]), bea_series, 0),
        (anonymous(&[Role::CatalogBeaRead]), bea_series, 1),
    ] {
        let listed = run(context, list(series)).await;
        assert!(listed.errors.is_empty(), "{:?}", listed.errors);
        assert_eq!(
            listed.data.into_json().unwrap()["annotationsForSeries"]
                .as_array()
                .unwrap()
                .len(),
            expected,
            "{series}"
        );
    }
}

/// `DataSourceType.series` and `seriesCount` check the source's catalog themselves, so a source
/// object reached by any path can't list a catalog the caller can't read.
#[tokio::test]
async fn data_source_fields_check_the_catalog_themselves() {
    struct Root(DataSourceType);
    #[async_graphql::Object]
    impl Root {
        async fn source(&self) -> DataSourceType {
            self.0.clone()
        }
    }
    let f = fixture("catalog_source_fields").await;
    for (source, allowed) in [(f.fred.clone(), true), (f.bea.clone(), false)] {
        let schema = async_graphql::Schema::build(
            Root(DataSourceType::from(source)),
            async_graphql::EmptyMutation,
            async_graphql::EmptySubscription,
        )
        .data(DataPool(f.pool.clone()))
        .data(Arc::new(anonymous(&[Role::CatalogFredRead])))
        .finish();
        for field in ["seriesCount", "series { totalCount }"] {
            let result = schema.execute(format!("{{ source {{ {field} }} }}")).await;
            if allowed {
                assert!(result.errors.is_empty(), "{field}: {:?}", result.errors);
            } else {
                assert_eq!(result.errors.len(), 1, "{field} leaked BEA");
                assert_eq!(
                    result.errors[0].extensions.as_ref().unwrap().get("code"),
                    Some(&async_graphql::Value::from("FORBIDDEN")),
                    "{field}"
                );
            }
        }
    }
    let _ = (f.fred_series, f.bea_series);
}
