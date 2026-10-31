//! Executable version-1 consumer operations against current Postgres in both database shapes.
//! DATABASE_URL must name a disposable Postgres server on which scratch databases can be created.
use async_graphql::{Request, Variables};
use econ_graph_auth::Role;
use econ_graph_core::{
    config::DataGraphqlConfig,
    database::{self, AppPool, DataPool, DatabasePool, MigrationSet},
    models::{DataSource, User},
    test_utils::ScratchDatabase,
};
use econ_graph_graphql::graphql::{
    context::GraphQLContext,
    schema::{create_schema, create_schema_with_data_plane},
};
use econ_graph_services::services::series_check::DataPlaneClient;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

const OPERATIONS: &str = include_str!("../../../../docs/api/operations/train3-data.graphql");
const SERIES: &str = "8933cde1-0db6-454c-8675-805b4386f301";
const DATASET: &str = "8933cde1-0db6-454c-8675-805b4386f302";
const EXACT: &str = "123456789012345678901234567890.1234567890123456789";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    id: Uuid,
    series_id: Uuid,
    date: String,
    // BigDecimal's GraphQL scalar is a decimal string, never a JSON floating-point number.
    value: Option<String>,
    revision_date: String,
    is_original_release: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    has_previous_page: bool,
    start_cursor: Option<String>,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ObservationPage {
    total_count: usize,
    nodes: Vec<Observation>,
    page_info: PageInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageResponse {
    series_data: ObservationPage,
}

struct Fixture {
    app: DatabasePool,
    data: DatabasePool,
    _app_db: ScratchDatabase,
    _data_db: Option<ScratchDatabase>,
}

impl Fixture {
    async fn new(split: bool) -> Self {
        use diesel_async::RunQueryDsl;
        let app_db = ScratchDatabase::create("data_contract_app");
        let data_db = split.then(|| ScratchDatabase::create("data_contract_data"));
        let data_url = data_db.as_ref().map_or(&app_db.url, |db| &db.url);
        database::run_migration_set(data_url, MigrationSet::Data)
            .await
            .unwrap();
        database::run_migration_set(&app_db.url, MigrationSet::App)
            .await
            .unwrap();
        let app = database::create_pool(&app_db.url).await.unwrap();
        let data = database::create_pool(data_url).await.unwrap();
        let fred = DataSource::find_by_name(&data, &DataSource::fred().name)
            .await
            .unwrap()
            .unwrap();
        let mut conn = data.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO datasets (id, source_id, code, name, dimensions) \
             VALUES ($1, $2, 'train3-contract-v1', 'Contract fixture', \
             '[{\"name\":\"area\",\"label\":\"Area\",\"type\":\"string\"}]')",
        )
        .bind::<diesel::sql_types::Uuid, _>(Uuid::parse_str(DATASET).unwrap())
        .bind::<diesel::sql_types::Uuid, _>(fred.id)
        .execute(&mut conn)
        .await
        .unwrap();
        // This legacy, non-derived series UUID is part of the transport contract.
        diesel::sql_query(
            "INSERT INTO economic_series \
             (id, source_id, dataset_id, external_id, title, units, frequency, start_date, end_date, dimensions) \
             VALUES ($1, $2, $3, 'TRAIN3-CONTRACT-V1', 'Train 3 contract', 'Percent', 'Monthly', \
             '2024-01-01', '2024-03-01', '{\"area\":\"USA\"}')",
        )
        .bind::<diesel::sql_types::Uuid, _>(Uuid::parse_str(SERIES).unwrap())
        .bind::<diesel::sql_types::Uuid, _>(fred.id)
        .bind::<diesel::sql_types::Uuid, _>(Uuid::parse_str(DATASET).unwrap())
        .execute(&mut conn)
        .await
        .unwrap();
        diesel::sql_query(
            "INSERT INTO data_points (id, series_id, date, value, revision_date, is_original_release) VALUES \
             ('8933cde1-0db6-454c-8675-805b4386f311', $1, '2024-01-01', $2::numeric, '2024-01-15', true), \
             ('8933cde1-0db6-454c-8675-805b4386f312', $1, '2024-01-01', 12.5, '2024-02-15', false), \
             ('8933cde1-0db6-454c-8675-805b4386f313', $1, '2024-02-01', NULL, '2024-02-15', true), \
             ('8933cde1-0db6-454c-8675-805b4386f314', $1, '2024-02-01', 20, '2024-03-15', false), \
             ('8933cde1-0db6-454c-8675-805b4386f315', $1, '2024-03-01', 25, '2024-03-15', true)",
        )
        .bind::<diesel::sql_types::Uuid, _>(Uuid::parse_str(SERIES).unwrap())
        .bind::<diesel::sql_types::Text, _>(EXACT)
        .execute(&mut conn)
        .await
        .unwrap();
        drop(conn);
        Self {
            app,
            data,
            _app_db: app_db,
            _data_db: data_db,
        }
    }

    fn schema(
        &self,
    ) -> async_graphql::Schema<
        econ_graph_graphql::graphql::query::Query,
        econ_graph_graphql::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    > {
        create_schema(AppPool(self.app.clone()), Some(DataPool(self.data.clone())))
    }
}

fn anonymous(roles: &[Role]) -> Arc<GraphQLContext> {
    Arc::new(GraphQLContext::anonymous().with_anonymous_catalog_roles(roles.iter().copied()))
}

fn operation(name: &str, variables: Value, context: Arc<GraphQLContext>) -> Request {
    Request::new(OPERATIONS)
        .operation_name(name)
        .variables(Variables::from_json(variables))
        .data(context)
}

async fn page(f: &Fixture, variables: Value) -> ObservationPage {
    let response = f
        .schema()
        .execute(operation(
            "Train3ObservationPage",
            variables,
            anonymous(&[Role::CatalogFredRead]),
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    serde_json::from_value::<PageResponse>(response.data.into_json().unwrap())
        .unwrap()
        .series_data
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn versioned_operations_preserve_observations_in_separate_and_combined_databases() {
    for split in [true, false] {
        let f = Fixture::new(split).await;
        let all = page(&f, json!({"seriesId": SERIES})).await;
        assert_eq!(all.total_count, 5);
        assert_eq!(all.nodes.len(), 5);
        assert_eq!(all.nodes[0].value.as_deref(), Some(EXACT));
        assert_eq!(
            all.nodes[0].id.to_string(),
            "8933cde1-0db6-454c-8675-805b4386f311"
        );
        assert!(all.nodes[0].is_original_release);
        assert_eq!(all.nodes[0].revision_date, "2024-01-15");
        assert_eq!(all.nodes[2].value, None);
        assert!(all.nodes.iter().all(|o| o.series_id.to_string() == SERIES));

        for (filter, count, values) in [
            (
                json!({"latestRevisionOnly": true}),
                3,
                vec![Some("12.5"), Some("20"), Some("25")],
            ),
            (
                json!({"originalOnly": true}),
                3,
                vec![Some(EXACT), None, Some("25")],
            ),
            (json!({"asOf": "2024-02-15"}), 2, vec![Some("12.5"), None]),
            (
                json!({"startDate": "2024-02-01", "endDate": "2024-02-01"}),
                2,
                vec![None, Some("20")],
            ),
        ] {
            let p = page(&f, json!({"seriesId": SERIES, "filter": filter})).await;
            assert_eq!(p.total_count, count, "{filter}");
            assert_eq!(
                p.nodes
                    .iter()
                    .map(|o| o.value.as_deref())
                    .collect::<Vec<_>>(),
                values,
                "{filter}"
            );
        }

        // Transform defaults choose latest before choosing the comparison base. Offset pages
        // retain the same comparison context as an unpaged request.
        let transformed = page(
            &f,
            json!({"seriesId": SERIES, "transformation": "PERCENT_CHANGE"}),
        )
        .await;
        assert_eq!(transformed.total_count, 3);
        assert_eq!(
            transformed.nodes[1]
                .value
                .as_ref()
                .unwrap()
                .parse::<bigdecimal::BigDecimal>()
                .unwrap(),
            bigdecimal::BigDecimal::from(60)
        );
        let continued = page(&f, json!({"seriesId": SERIES, "transformation": "PERCENT_CHANGE", "first": 1, "after": "1"})).await;
        assert_eq!(continued.nodes[0].value, transformed.nodes[1].value);
        assert_eq!(continued.nodes[0].date, "2024-02-01");
        assert!(continued.page_info.has_next_page);
        assert!(continued.page_info.has_previous_page);
        assert_eq!(continued.page_info.start_cursor.as_deref(), Some("2"));
        assert_eq!(continued.page_info.end_cursor.as_deref(), Some("2"));

        let explicit_all = page(&f, json!({"seriesId": SERIES, "transformation": "PERCENT_CHANGE", "filter": {"latestRevisionOnly": false}})).await;
        assert_eq!(explicit_all.total_count, 5);
        let empty = page(&f, json!({"seriesId": SERIES, "first": 0, "after": "5"})).await;
        assert_eq!(empty.total_count, 5);
        assert!(empty.nodes.is_empty());
        assert_eq!(empty.page_info.start_cursor, None);
        assert_eq!(empty.page_info.end_cursor.as_deref(), Some("5"));

        let response = f
            .schema()
            .execute(operation(
                "Train3SeriesSubject",
                json!({"seriesId": SERIES, "datasetId": DATASET}),
                anonymous(&[Role::CatalogFredRead]),
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["series"]["id"], SERIES);
        assert_eq!(data["series"]["units"], "Percent");
        assert_eq!(data["series"]["dataset"]["id"], DATASET);
        assert_eq!(data["series"]["dataset"]["defaultMeasure"], "value");
        assert_eq!(data["dataset"]["code"], "train3-contract-v1");
        assert_eq!(data["series"]["dataPointCount"], 5);
        assert_eq!(data["series"]["dataPoints"].as_array().unwrap().len(), 5);
        assert_eq!(data["series"]["dataPoints"][0]["value"], EXACT);
        assert_eq!(data["series"]["dataPoints"][2]["value"], Value::Null);
        assert_eq!(data["series"]["latest"].as_array().unwrap().len(), 3);
        assert_eq!(data["series"]["latest"][0]["value"], "12.5");
        assert_eq!(data["series"]["latest"][1]["value"], "20");
        assert_eq!(data["series"]["original"][0]["value"], EXACT);
        assert_eq!(data["series"]["vintage"].as_array().unwrap().len(), 2);
        assert_eq!(data["series"]["vintage"][0]["value"], "12.5");
        assert_eq!(data["series"]["vintage"][1]["value"], Value::Null);
        assert_eq!(data["series"]["transformed"].as_array().unwrap().len(), 3);
        assert_eq!(data["series"]["latestObservation"]["value"], "25");
        assert_eq!(data["series"]["recentDataPoints"][1]["value"], "20");
        assert_eq!(data["seriesExists"], true);
        assert_eq!(data["seriesCatalog"], DataSource::fred().name);
        assert_eq!(
            data["crossSection"],
            json!([{"key":"USA", "seriesId":SERIES, "date":"2024-02-01", "value":"20", "flags":[]} ])
        );
        let user = User::get_or_create_for_subject(
            &f.app,
            Uuid::new_v4(),
            Some("contract@example.test"),
            true,
            Some("Contract reader"),
        )
        .await
        .unwrap();
        let signed = f
            .schema()
            .execute(operation(
                "Train3SeriesSubject",
                json!({"seriesId":SERIES,"datasetId":DATASET}),
                Arc::new(GraphQLContext::signed_in(
                    user.clone(),
                    [Role::CatalogFredRead],
                )),
            ))
            .await;
        assert!(signed.errors.is_empty(), "{:?}", signed.errors);
        assert_eq!(
            signed.data.into_json().unwrap(),
            data,
            "signed and anonymous readers with the same grants see the same contract"
        );
        let denied = f
            .schema()
            .execute(operation(
                "Train3ObservationPage",
                json!({"seriesId":SERIES}),
                Arc::new(
                    GraphQLContext::signed_in(user, [])
                        .with_anonymous_catalog_roles([Role::CatalogFredRead]),
                ),
            ))
            .await;
        assert!(denied.errors.is_empty());
        assert_eq!(
            denied.data.into_json().unwrap()["seriesData"]["totalCount"],
            0,
            "signed callers do not inherit anonymous grants"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_grants_do_not_leak_through_a_reused_schema_or_request_cache() {
    let f = Fixture::new(true).await;
    let schema = f.schema();
    for (roles, expected_count) in [
        (&[Role::CatalogFredRead][..], 1),
        (&[][..], 0),
        (&[Role::CatalogFredRead][..], 1),
    ] {
        // Multiple fields share this request's cache; a later request receives a fresh context.
        let response = schema.execute(Request::new("{ seriesList(pagination:{first:1}) {totalCount nodes{id}} searchSeries(query:\"Train 3 contract\",first:1){totalCount} dataSources{id} }").data(anonymous(roles))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let data = response.data.into_json().unwrap();
        assert_eq!(data["seriesList"]["totalCount"], expected_count);
        assert_eq!(data["searchSeries"]["totalCount"], expected_count);
        assert_eq!(
            data["dataSources"].as_array().unwrap().len(),
            expected_count
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_only_routing_and_unavailable_annotation_targets_fail_closed() {
    let f = Fixture::new(true).await;
    let user = User::get_or_create_for_subject(
        &f.app,
        Uuid::new_v4(),
        Some("contract@example.test"),
        true,
        Some("Contract author"),
    )
    .await
    .unwrap();
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let plane = Arc::new(
        DataPlaneClient::new(&DataGraphqlConfig {
            url: server.uri(),
            host: None,
            timeout: std::time::Duration::from_secs(2),
            forward_authorization: false,
            allow_insecure_authorization: false,
        })
        .unwrap(),
    );
    let schema = create_schema_with_data_plane(AppPool(f.app.clone()), None, Some(plane), ());
    let response = schema
        .execute(operation(
            "Train3ObservationPage",
            json!({"seriesId": SERIES}),
            anonymous(&[Role::CatalogFredRead]),
        ))
        .await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("DATA_SERVED_REMOTELY"))
    );
    let response = schema.execute(Request::new(format!(r#"mutation {{ createAnnotation(input: {{seriesId:"{SERIES}",annotationDate:"2024-02-01",annotationValue:"20",title:"Unavailable target",content:"Must not persist",annotationType:"note",isPublic:true}}) {{id}} }}"#)).data(Arc::new(GraphQLContext::signed_in(user.clone(), [Role::CatalogFredRead, Role::AnnotationCreate])))).await;
    assert_eq!(response.errors.len(), 1);
    assert_eq!(
        response.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("DATA_PLANE_UNAVAILABLE"))
    );
    let annotations =
        econ_graph_services::services::collaboration_service::CollaborationService::new(
            f.app.clone(),
        )
        .get_annotations_for_series(SERIES, Some(user.id))
        .await
        .unwrap();
    assert!(annotations.is_empty());
    server.verify().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_lifecycle_stays_indistinguishable_from_missing_for_granted_callers() {
    for split in [true, false] {
        let f = Fixture::new(split).await;
        let user = User::get_or_create_for_subject(&f.app, Uuid::new_v4(), None, false, None)
            .await
            .unwrap();
        {
            use diesel_async::RunQueryDsl;
            diesel::sql_query("INSERT INTO series_lifecycle(source_id,external_id,status,scope_fingerprint) SELECT source_id,external_id,'restricted','contract' FROM economic_series WHERE id=$1")
            .bind::<diesel::sql_types::Uuid,_>(Uuid::parse_str(SERIES).unwrap()).execute(&mut f.data.get().await.unwrap()).await.unwrap();
        }
        let missing = Uuid::new_v4();
        for signed in [false, true] {
            let context = || {
                if signed {
                    Arc::new(GraphQLContext::signed_in(
                        user.clone(),
                        [Role::CatalogFredRead],
                    ))
                } else {
                    anonymous(&[Role::CatalogFredRead])
                }
            };
            let read = |id: String| {
                format!(
                    r#"{{series(id:"{id}"){{id dataPointCount}} seriesExists(id:"{id}") seriesCatalog(id:"{id}") seriesData(seriesId:"{id}"){{totalCount nodes{{id}}}}}}"#
                )
            };
            let restricted = f
                .schema()
                .execute(Request::new(read(SERIES.into())).data(context()))
                .await;
            let unknown = f
                .schema()
                .execute(Request::new(read(missing.to_string())).data(context()))
                .await;
            assert!(restricted.errors.is_empty(), "{:?}", restricted.errors);
            assert!(unknown.errors.is_empty());
            assert_eq!(
                restricted.data, unknown.data,
                "restricted identities must not become an existence oracle"
            );
            let discovery=f.schema().execute(Request::new(r#"{seriesList(pagination:{first:1}){totalCount} searchSeries(query:"Train 3 contract",first:1){totalCount}}"#).data(context())).await;
            assert!(discovery.errors.is_empty());
            let data = discovery.data.into_json().unwrap();
            assert_eq!(data["seriesList"]["totalCount"], 0);
            assert_eq!(data["searchSeries"]["totalCount"], 0);
        }
        // Lifecycle retirement keeps retained history readable; only legal restriction denies.
        {
            use diesel_async::RunQueryDsl;
            diesel::sql_query(
            "UPDATE series_lifecycle SET status='retired' WHERE external_id='TRAIN3-CONTRACT-V1'",
        )
        .execute(&mut f.data.get().await.unwrap())
        .await
        .unwrap();
        }
        assert_eq!(page(&f, json!({"seriesId":SERIES})).await.total_count, 5);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn synthetic_vintages_cannot_hide_an_earlier_real_release() {
    use diesel_async::SimpleAsyncConnection;
    for split in [true, false] {
        let f = Fixture::new(split).await;
        f.data.get().await.unwrap().batch_execute(&format!("UPDATE data_points SET revision_date='2023-12-15' WHERE id='8933cde1-0db6-454c-8675-805b4386f311'; INSERT INTO data_points(id,series_id,date,revision_date,value,is_original_release) VALUES('8933cde1-0db6-454c-8675-805b4386f316','{SERIES}','2024-01-01','2024-01-01',999,true)")).await.unwrap();
        for (filter, count) in [
            (json!({"asOf":"2024-01-10"}), 1),
            (json!({"originalOnly":true,"latestRevisionOnly":true}), 3),
            (json!({"originalOnly":true,"asOf":"2024-01-10"}), 1),
        ] {
            let p = page(&f, json!({"seriesId":SERIES,"filter":filter})).await;
            assert_eq!(p.total_count, count, "{filter}");
            assert_eq!(
                p.nodes[0].id.to_string(),
                "8933cde1-0db6-454c-8675-805b4386f311",
                "{filter}"
            );
            assert_eq!(p.nodes[0].value.as_deref(), Some(EXACT), "{filter}");
            let direct=f.schema().execute(Request::new(format!(r#"query($filter:DataFilter){{series(id:"{SERIES}"){{dataPoints(filter:$filter){{id value revisionDate}}}}}}"#)).variables(Variables::from_json(json!({"filter":filter}))).data(anonymous(&[Role::CatalogFredRead]))).await;
            assert!(direct.errors.is_empty(), "{:?}", direct.errors);
            let direct = direct.data.into_json().unwrap();
            assert_eq!(
                direct["series"]["dataPoints"].as_array().unwrap().len(),
                count
            );
            assert_eq!(
                direct["series"]["dataPoints"][0],
                json!({"id":"8933cde1-0db6-454c-8675-805b4386f311","value":EXACT,"revisionDate":"2023-12-15"})
            );
        }
        // Real-original evidence newer than AsOf still excludes its synthetic row.
        f.data.get().await.unwrap().batch_execute("UPDATE data_points SET revision_date='2024-01-15' WHERE id='8933cde1-0db6-454c-8675-805b4386f311'").await.unwrap();
        assert_eq!(
            page(
                &f,
                json!({"seriesId":SERIES,"filter":{"asOf":"2024-01-10"}})
            )
            .await
            .total_count,
            0
        );
    }
}
