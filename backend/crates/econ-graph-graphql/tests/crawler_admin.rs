//! Operator contracts execute against the production schema and scoped scratch Postgres.
use async_graphql::Request;
use diesel::{connection::SimpleConnection, prelude::*};
use econ_graph_auth::Role;
use econ_graph_core::{
    database::{AppPool, DataPool},
    models::User,
    test_utils::ScratchDatabase,
};
use econ_graph_graphql::graphql::{context::GraphQLContext, schema::create_schema};
use std::sync::Arc;
use uuid::Uuid;

async fn setup() -> (ScratchDatabase, econ_graph_core::DatabasePool, User) {
    let db = ScratchDatabase::create("crawler_admin");
    econ_graph_core::run_migrations(&db.url).await.unwrap();
    let pool = econ_graph_core::create_pool(&db.url).await.unwrap();
    let user = User::get_or_create_for_subject(
        &pool,
        Uuid::new_v4(),
        Some("crawler@example.test"),
        true,
        Some("Operator"),
    )
    .await
    .unwrap();
    (db, pool, user)
}
fn caller(user: &User, roles: &[Role]) -> Arc<GraphQLContext> {
    Arc::new(GraphQLContext::signed_in(
        user.clone(),
        roles.iter().copied(),
    ))
}

#[tokio::test]
async fn real_job_history_is_bounded_filtered_and_role_guarded() {
    let (db, pool, user) = setup().await;
    db.connect().batch_execute("INSERT INTO crawl_queue(id,source,series_id,kind,status,priority,max_retries,error_message) VALUES
      ('10000000-0000-4000-8000-000000000001','FRED','GDP','fetch_series','failed',5,3,'transient error: https://api.test?q=secret token=hidden'),
      ('10000000-0000-4000-8000-000000000002','BLS','ALL','discover_catalog','pending',5,3,NULL)").unwrap();
    let schema = create_schema(AppPool(pool.clone()), Some(DataPool(pool.clone())));
    let operation = "{crawlerJobs(limit:1,source:\"FRED\",status:\"failed\"){id source seriesId kind status errorMessage}}";
    for roles in [&[][..], &[Role::AdminCrawlersManage][..]] {
        let result = schema
            .execute(Request::new(operation).data(caller(&user, roles)))
            .await;
        assert!(!result.errors.is_empty());
    }
    let result = schema
        .execute(Request::new(operation).data(caller(&user, &[Role::AdminSystemRead])))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let data = result.data.into_json().unwrap();
    assert_eq!(data["crawlerJobs"].as_array().unwrap().len(), 1);
    assert_eq!(data["crawlerJobs"][0]["seriesId"], "GDP");
    assert_eq!(data["crawlerJobs"][0]["status"], "failed");
    assert!(!data.to_string().contains("secret"));
    assert!(!data.to_string().contains("hidden"));
    for operation in [
        "{crawlerJobs(limit:201){id}}",
        "{crawlerJobs(offset:-1){id}}",
        "{crawlerJobs(status:\"invented\"){id}}",
    ] {
        let result = schema
            .execute(Request::new(operation).data(caller(&user, &[Role::AdminSystemRead])))
            .await;
        assert_eq!(result.errors.len(), 1);
        assert_eq!(
            result.errors[0].extensions.as_ref().unwrap().get("code"),
            Some(&async_graphql::Value::from("BAD_USER_INPUT"))
        );
    }
    drop(schema);
    drop(pool);
}

#[tokio::test]
async fn scheduling_changes_require_manage_role_are_audited_and_leave_queued_work_alone() {
    let (db, pool, user) = setup().await;
    let data_db = ScratchDatabase::create("crawler_admin_data");
    econ_graph_core::run_migration_set(&data_db.url, econ_graph_core::MigrationSet::Data)
        .await
        .unwrap();
    let data_pool = econ_graph_core::create_pool(&data_db.url).await.unwrap();
    data_db.connect().batch_execute("INSERT INTO crawl_queue(source,series_id,kind,status,priority,max_retries) VALUES ('FRED','GDP','fetch_series','pending',5,3)").unwrap();
    let schema = create_schema(AppPool(pool.clone()), Some(DataPool(data_pool.clone())));
    let mutation = "mutation {setCrawlerSourceEnabled(source:\"FRED\",enabled:false)}";
    let denied = schema
        .execute(Request::new(mutation).data(caller(&user, &[Role::AdminSystemRead])))
        .await;
    assert!(!denied.errors.is_empty());
    let result = schema
        .execute(Request::new(mutation).data(caller(&user, &[Role::AdminCrawlersManage])))
        .await;
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.data.into_json().unwrap()["setCrawlerSourceEnabled"],
        false
    );
    let overview = schema
        .execute(
            Request::new("{crawlerSources{source enabled} crawlerJobs{status seriesId}}")
                .data(caller(&user, &[Role::AdminSystemRead])),
        )
        .await;
    assert!(overview.errors.is_empty(), "{:?}", overview.errors);
    let data = overview.data.into_json().unwrap();
    let fred = data["crawlerSources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["source"] == "FRED")
        .unwrap();
    assert_eq!(fred["enabled"], false);
    assert_eq!(data["crawlerJobs"][0]["status"], "pending");
    use econ_graph_core::schema::audit_logs;
    let mut conn = db.connect();
    let rows = audit_logs::table
        .filter(audit_logs::action.eq("set_crawler_source_enabled"))
        .select(audit_logs::details)
        .load::<Option<serde_json::Value>>(&mut conn)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].as_ref().unwrap()["outcome"], "success");
    let unknown = schema
        .execute(
            Request::new("mutation{setCrawlerSourceEnabled(source:\"NOT_REAL\",enabled:true)}")
                .data(caller(&user, &[Role::AdminCrawlersManage])),
        )
        .await;
    assert!(!unknown.errors.is_empty());
    drop(schema);
    drop(data_pool);
    drop(pool);
}

#[tokio::test]
async fn admin_reads_fail_closed_without_the_data_database() {
    let (_db, pool, user) = setup().await;
    let schema = create_schema(AppPool(pool.clone()), None);
    let result = schema
        .execute(Request::new("{crawlerJobs{id}}").data(caller(&user, &[Role::AdminSystemRead])))
        .await;
    assert_eq!(
        result.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&async_graphql::Value::from("DATA_SERVED_REMOTELY"))
    );
    drop(schema);
    drop(pool);
}
