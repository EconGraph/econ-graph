//! Writes the admin audit trail (`audit_logs`) and the security events (`security_events`)
//! that the `auditLogs` and `securityEvents` queries read.
//!
//! Neither write can fail the request: a failed insert is logged and the reply is unchanged.

use crate::graphql::context::GraphQLContext;
use crate::imports::*;
use econ_graph_core::models::admin::{security_event_type, NewAuditLog, NewSecurityEvent};
use serde_json::{json, Value};

/// Record an admin action the signed-in caller has just completed.
///
/// `details` should be a JSON object; the request id is added to it.
pub(crate) async fn record_admin_action(
    ctx: &Context<'_>,
    action: &str,
    resource_type: &str,
    resource_id: Option<String>,
    mut details: Value,
) {
    let (Ok(pool), Ok(context)) = (
        ctx.data::<DatabasePool>(),
        ctx.data::<Arc<GraphQLContext>>(),
    ) else {
        return;
    };
    let Some(user) = context.user.as_ref() else {
        return;
    };
    if let Some(object) = details.as_object_mut() {
        object.insert("request_id".into(), json!(context.request_id));
    }
    NewAuditLog {
        user_id: user.id,
        user_name: user.name.clone(),
        action: action.into(),
        resource_type: resource_type.into(),
        resource_id,
        ip_address: context.client_ip.clone(),
        user_agent: None,
        details: Some(details),
    }
    .record(pool)
    .await;
}

/// Record that the signed-in caller `user_id` was refused, with `description` saying why.
///
/// Called from the synchronous role checks, so the insert runs on its own task.
pub(crate) fn record_permission_denied(
    ctx: &Context<'_>,
    user_id: uuid::Uuid,
    description: String,
) {
    let (Ok(pool), Ok(context)) = (
        ctx.data::<DatabasePool>(),
        ctx.data::<Arc<GraphQLContext>>(),
    ) else {
        return;
    };
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let event = NewSecurityEvent {
        event_type: security_event_type::PERMISSION_DENIED.into(),
        user_id: Some(user_id),
        user_email: context.user.as_ref().map(|user| user.email.clone()),
        severity: "medium".into(),
        ip_address: context.client_ip.clone(),
        user_agent: None,
        description,
        metadata: Some(json!({ "request_id": context.request_id })),
    };
    let pool = pool.clone();
    runtime.spawn(async move { event.record(&pool).await });
}

// ---------------------------------------------------------------------
// DB-backed: need DATABASE_URL (skipped otherwise).
// ---------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::schema::create_schema_with_data;
    use econ_graph_auth::Role;
    use econ_graph_core::models::{NewUser, User};
    use uuid::Uuid;

    async fn db() -> Option<(DatabasePool, tokio::sync::MutexGuard<'static, ()>)> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed audit tests");
            return None;
        };
        let guard = crate::graphql::TEST_DB_LOCK.lock().await;
        crate::graphql::test_db::migrate_once(&url).await;
        let pool = econ_graph_core::database::create_pool(&url)
            .await
            .expect("pool");
        Some((pool, guard))
    }

    async fn insert_user(pool: &DatabasePool, name: &str) -> User {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;
        let new_user = NewUser {
            email: format!("{}@example.test", Uuid::new_v4()),
            name: name.into(),
            avatar_url: None,
            organization: None,
            theme: "light".into(),
            default_chart_type: "line".into(),
            notifications_enabled: false,
            collaboration_enabled: false,
            email_verified: true,
        };
        let mut conn = pool.get().await.unwrap();
        diesel::insert_into(users::table)
            .values(&new_user)
            .returning(User::as_returning())
            .get_result(&mut conn)
            .await
            .unwrap()
    }

    fn staff() -> Vec<Role> {
        Role::all()
            .iter()
            .copied()
            .filter(|r| r.is_staff())
            .collect()
    }

    async fn run(pool: &DatabasePool, caller: &User, roles: Vec<Role>, query: &str) -> Value {
        let schema = create_schema_with_data(
            pool.clone(),
            Arc::new(GraphQLContext::signed_in(caller.clone(), roles)),
        );
        let response = schema.execute(query).await;
        assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
        response.data.into_json().unwrap()
    }

    /// Each admin mutation leaves one audit row naming the caller, the action and the target,
    /// and `auditLogs` returns them.
    #[tokio::test]
    async fn each_admin_mutation_writes_an_audit_row() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let admin = insert_user(&pool, "audit admin").await;
        let target = insert_user(&pool, "audit target").await;
        let id = target.id;

        for query in [
            r#"mutation { triggerCrawl(input: { source: "FRED", seriesIds: ["AUDITTEST"] }) { enqueuedCount } }"#.to_string(),
            format!(r#"mutation {{ updateUser(id: "{id}", input: {{ name: "Renamed", theme: "dark" }}) {{ id }} }}"#),
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ] {
            run(&pool, &admin, staff(), &query).await;
        }

        let data = run(
            &pool,
            &admin,
            staff(),
            &format!(
                r#"{{ auditLogs(filter: {{ userId: "{}" }}) {{ totalCount nodes {{ userId userName action resourceType resourceId details }} }} }}"#,
                admin.id
            ),
        )
        .await;
        let logs = &data["auditLogs"];
        assert_eq!(logs["totalCount"], 5, "{logs}");
        let nodes = logs["nodes"].as_array().unwrap();
        // Newest first.
        let actions: Vec<&str> = nodes
            .iter()
            .map(|n| n["action"].as_str().unwrap())
            .collect();
        assert_eq!(
            actions,
            [
                "delete_user",
                "activate_user",
                "suspend_user",
                "update_user",
                "trigger_crawl"
            ]
        );
        for node in &nodes[..4] {
            assert_eq!(node["userId"], admin.id.to_string());
            assert_eq!(node["userName"], "audit admin");
            assert_eq!(node["resourceType"], "user");
            assert_eq!(node["resourceId"], id.to_string());
        }
        let update: Value = serde_json::from_str(nodes[3]["details"].as_str().unwrap()).unwrap();
        assert_eq!(update["fields"], json!(["name", "theme"]));
        let crawl: Value = serde_json::from_str(nodes[4]["details"].as_str().unwrap()).unwrap();
        assert_eq!(nodes[4]["resourceType"], "crawl_queue");
        assert_eq!(crawl["series_ids"], json!(["AUDITTEST"]));
        assert!(crawl["request_id"].is_string(), "{crawl}");

        // Paging uses the same offset cursors as `users`.
        let page = run(
            &pool,
            &admin,
            staff(),
            &format!(
                r#"{{ auditLogs(filter: {{ userId: "{}" }}, pagination: {{ first: 2, after: "4" }}) {{ nodes {{ action }} pageInfo {{ hasNextPage hasPreviousPage }} }} }}"#,
                admin.id
            ),
        )
        .await;
        assert_eq!(
            page["auditLogs"]["nodes"],
            json!([{ "action": "trigger_crawl" }])
        );
        assert_eq!(
            page["auditLogs"]["pageInfo"],
            json!({ "hasNextPage": false, "hasPreviousPage": true })
        );

        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::crawl_queue;
        let mut conn = pool.get().await.unwrap();
        diesel::delete(crawl_queue::table.filter(crawl_queue::series_id.eq("AUDITTEST")))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    /// A signed-in caller refused for a missing role, or for acting on another user with a
    /// narrow role, shows up in `securityEvents`.
    #[tokio::test]
    async fn permission_denials_are_security_events() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let viewer = insert_user(&pool, "audit viewer").await;
        let narrow = insert_user(&pool, "audit narrow admin").await;
        let reader = insert_user(&pool, "audit security reader").await;

        let schema = |user: &User, roles: Vec<Role>| {
            create_schema_with_data(
                pool.clone(),
                Arc::new(GraphQLContext::signed_in(user.clone(), roles)),
            )
        };
        let denied = schema(&viewer, vec![])
            .execute("{ users { totalCount } }")
            .await;
        assert_eq!(denied.errors[0].message, "Insufficient permissions");
        let denied = schema(&narrow, vec![Role::AdminUsersSuspend])
            .execute(format!(
                r#"mutation {{ suspendUser(id: "{}") }}"#,
                viewer.id
            ))
            .await;
        assert_eq!(denied.errors[0].message, "Insufficient permissions");

        // The events are written on their own task.
        let query = "{ securityEvents(limit: 100) { eventType userId userEmail description } }";
        let mut events = Vec::new();
        for _ in 0..50 {
            let data = run(&pool, &reader, vec![Role::AdminSecurityRead], query).await;
            events = data["securityEvents"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| {
                    e["userId"] == viewer.id.to_string() || e["userId"] == narrow.id.to_string()
                })
                .cloned()
                .collect();
            if events.len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(events.len(), 2, "{events:?}");
        for event in &events {
            assert_eq!(event["eventType"], "permission_denied");
        }
        let by = |user: &User| {
            events
                .iter()
                .find(|e| e["userId"] == user.id.to_string())
                .unwrap()
        };
        assert_eq!(by(&viewer)["userEmail"], viewer.email.as_str());
        assert_eq!(by(&viewer)["description"], "missing role admin.users:read");
        assert!(
            by(&narrow)["description"]
                .as_str()
                .unwrap()
                .contains("needs every staff role"),
            "{events:?}"
        );
    }
}
