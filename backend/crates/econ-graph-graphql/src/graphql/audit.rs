//! Writes the admin audit trail (`audit_logs`) and the security events (`security_events`)
//! that the `auditLogs` and `securityEvents` queries read.
//!
//! Neither write can fail the request: a failed insert is logged, counted (see
//! [`econ_graph_core::models::admin::audit_log_write_failures`]) and the reply is unchanged.
//!
//! - Every admin mutation attempt by a caller who holds the mutation's role writes one audit
//!   row whose `details.outcome` says how it ended ([`Outcome`]). `details` names the fields
//!   an update touched, never their values.
//! - A signed-in caller refused for a missing role writes a `permission_denied` security
//!   event, at most one per request for each (caller, permission): a query that aliases one
//!   admin field a hundred times still writes one row. Writes start at the denial and the
//!   [`AuditWrites`] extension waits for them before returning a normal response.
//! - Request cancellation leaves detached writes running. An admin attempt interrupted
//!   before its result is known is recorded as `cancelled`; it may have committed. Process
//!   crashes/runtime shutdown can still interrupt writes; this is not a durable queue.

use crate::graphql::context::GraphQLContext;
use crate::imports::*;
use async_graphql::extensions::{Extension, ExtensionContext, ExtensionFactory, NextExecute};
use econ_graph_core::models::admin::{security_event_type, NewAuditLog, NewSecurityEvent};
use serde_json::{json, Value};
use std::collections::HashSet;

/// How an admin mutation attempt ended, stored as `details.outcome` on its audit row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The change was made.
    Success,
    /// The input was refused before anything was changed.
    Invalid,
    /// The target does not exist.
    NotFound,
    /// The change conflicts with existing data (an email already in use).
    Conflict,
    /// The caller holds the mutation's role but may not make this change.
    Denied,
    /// Refused because it would suspend or delete the caller's own account.
    SelfLockout,
    /// The database failed.
    Error,
    /// This backend has no data database, so the data the mutation needs is served elsewhere.
    Unavailable,
}

impl Outcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Invalid => "invalid",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Denied => "denied",
            Self::SelfLockout => "self_lockout",
            Self::Error => "error",
            Self::Unavailable => "unavailable",
        }
    }
}

/// A failed admin mutation: the error returned to the caller and the outcome audited.
pub(crate) struct Failure {
    outcome: Outcome,
    error: GraphQLError,
}

impl From<diesel::result::Error> for Failure {
    /// Audit transaction setup, commit, and rollback errors as database failures.
    fn from(error: diesel::result::Error) -> Self {
        fail(Outcome::Error, error)
    }
}

/// Attach an [`Outcome`] to an error, for an admin mutation's audited body.
pub(crate) trait OrOutcome<T> {
    fn or_outcome(self, outcome: Outcome) -> std::result::Result<T, Failure>;
}

impl<T, E: Into<GraphQLError>> OrOutcome<T> for std::result::Result<T, E> {
    fn or_outcome(self, outcome: Outcome) -> std::result::Result<T, Failure> {
        self.map_err(|error| Failure {
            outcome,
            error: error.into(),
        })
    }
}

/// A [`Failure`] with `outcome`, returning `error` to the caller.
pub(crate) fn fail(outcome: Outcome, error: impl Into<GraphQLError>) -> Failure {
    Failure {
        outcome,
        error: error.into(),
    }
}

/// Owns an audit attempt before its first database await. Cancellation cannot reveal whether
/// an in-flight statement committed, so dropping it records `cancelled`, not `success`.
/// This is best-effort across process/runtime shutdown, as are normal audit writes.
pub(crate) struct Attempt {
    pool: Option<AppPool>,
    row: Option<NewAuditLog>,
}

impl Attempt {
    pub(crate) fn new(
        ctx: &Context<'_>,
        action: &str,
        resource_type: &str,
        resource_id: Option<String>,
        mut details: Value,
    ) -> Self {
        let (Ok(pool), Ok(context)) = (ctx.data::<AppPool>(), ctx.data::<Arc<GraphQLContext>>())
        else {
            return Self {
                pool: None,
                row: None,
            };
        };
        let Some(user) = context.user.as_ref() else {
            return Self {
                pool: None,
                row: None,
            };
        };
        details["request_id"] = json!(context.request_id);
        details["outcome"] = json!("cancelled");
        Self {
            pool: Some(pool.clone()),
            row: Some(NewAuditLog {
                user_id: user.id,
                user_name: user.name.clone(),
                action: action.into(),
                resource_type: resource_type.into(),
                resource_id,
                ip_address: context.client_ip.clone(),
                user_agent: context.user_agent.clone(),
                details: Some(details),
            }),
        }
    }

    pub(crate) async fn finish<T>(
        mut self,
        mut details: Value,
        result: std::result::Result<T, Failure>,
    ) -> Result<T> {
        let outcome = match &result {
            Ok(_) => Outcome::Success,
            Err(failure) => failure.outcome,
        };
        if let (Some(row), Some(pool)) = (self.row.take(), self.pool.as_ref()) {
            let mut row = row;
            details["request_id"] = row.details.as_ref().unwrap()["request_id"].clone();
            details["outcome"] = json!(outcome.as_str());
            row.details = Some(details);
            row.record(pool).await;
        }
        result.map_err(|failure| failure.error)
    }
}

impl Drop for Attempt {
    fn drop(&mut self) {
        if let (Some(row), Some(pool)) = (self.row.take(), self.pool.as_ref()) {
            // Detached immediately: dropping the request must not abort its audit write.
            row.spawn_record(pool);
        }
    }
}

/// Permission denials observed and writes started for this request.
#[derive(Debug, Default)]
pub(crate) struct PendingDenials {
    /// Every (caller, permission) denied in this request, written or not.
    seen: HashSet<(uuid::Uuid, String)>,
    /// Started writes that [`AuditWrites`] waits for on normal completion.
    writes: Vec<tokio::task::JoinHandle<()>>,
}

/// Record that the signed-in caller `user_id` was refused `permission`, with `description`
/// saying why.
///
/// Called from synchronous role checks: start the write immediately, and let the
/// [`AuditWrites`] extension await it before returning a normal response. Only the first
/// denial of each (caller, permission) in a request is kept.
pub(crate) fn record_permission_denied(
    ctx: &Context<'_>,
    user_id: uuid::Uuid,
    permission: String,
    description: String,
) {
    let (Ok(context), Ok(pool)) = (ctx.data::<Arc<GraphQLContext>>(), ctx.data::<AppPool>()) else {
        return;
    };
    let mut pending = context
        .denials
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !pending.seen.insert((user_id, permission.clone())) {
        return;
    }
    pending.writes.push(
        NewSecurityEvent {
            event_type: security_event_type::PERMISSION_DENIED.into(),
            user_id: Some(user_id),
            user_email: context.user.as_ref().map(|user| user.email.clone()),
            severity: "medium".into(),
            ip_address: context.client_ip.clone(),
            user_agent: context.user_agent.clone(),
            description,
            metadata: Some(json!({ "request_id": context.request_id, "permission": permission })),
        }
        .spawn_record(pool),
    );
}

/// Schema extension that awaits the request's already-started denial writes after the
/// operation runs, before `Schema::execute` returns a normal response.
pub(crate) struct AuditWrites;

impl ExtensionFactory for AuditWrites {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(AuditWrites)
    }
}

#[async_trait::async_trait]
impl Extension for AuditWrites {
    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> async_graphql::Response {
        let response = next.run(ctx, operation_name).await;
        if let Some(context) = ctx.data_opt::<Arc<GraphQLContext>>() {
            let writes = std::mem::take(
                &mut context
                    .denials
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .writes,
            );
            for write in writes {
                let _ = write.await;
            }
        }
        response
    }
}

// ---------------------------------------------------------------------
// DB-backed: need DATABASE_URL (skipped otherwise).
// ---------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::context::ClientInfo;
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

    fn schema_for(
        pool: &DatabasePool,
        caller: &User,
        roles: Vec<Role>,
    ) -> async_graphql::Schema<
        crate::graphql::query::Query,
        crate::graphql::mutation::Mutation,
        async_graphql::EmptySubscription,
    > {
        create_schema_with_data(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool.clone())),
            Arc::new(GraphQLContext::signed_in(caller.clone(), roles)),
        )
    }

    async fn run(pool: &DatabasePool, caller: &User, roles: Vec<Role>, query: &str) -> Value {
        let response = schema_for(pool, caller, roles).execute(query).await;
        assert!(response.errors.is_empty(), "{query}: {:?}", response.errors);
        response.data.into_json().unwrap()
    }

    /// `auditLogs` rows written by `actor`, newest first.
    async fn audit_rows(pool: &DatabasePool, actor: &User) -> Vec<Value> {
        let reader = insert_user(pool, "audit log reader").await;
        let data = run(
            pool,
            &reader,
            staff(),
            &format!(
                r#"{{ auditLogs(filter: {{ userId: "{}" }}) {{ nodes {{ action resourceId ipAddress userAgent details }} }} }}"#,
                actor.id
            ),
        )
        .await;
        data["auditLogs"]["nodes"].as_array().unwrap().clone()
    }

    /// `securityEvents` rows about `user`.
    async fn security_rows(pool: &DatabasePool, user: &User) -> Vec<Value> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::models::admin::SecurityEvent;
        use econ_graph_core::schema::security_events;
        let mut conn = pool.get().await.unwrap();
        let events: Vec<SecurityEvent> = security_events::table
            .filter(security_events::user_id.eq(user.id))
            .select(SecurityEvent::as_select())
            .load(&mut conn)
            .await
            .unwrap();
        events
            .into_iter()
            .map(|e| {
                json!({
                    "eventType": e.event_type,
                    "userEmail": e.user_email,
                    "description": e.description,
                    "ipAddress": e.ip_address,
                    "userAgent": e.user_agent,
                })
            })
            .collect()
    }

    fn details(row: &Value) -> Value {
        serde_json::from_str(row["details"].as_str().unwrap()).unwrap()
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
        for node in nodes {
            assert_eq!(details(node)["outcome"], "success", "{node}");
        }
        let update = details(&nodes[3]);
        assert_eq!(update["fields"], json!(["name", "theme"]));
        // Field names only, never the values.
        assert!(!nodes[3]["details"].as_str().unwrap().contains("Renamed"));
        let crawl = details(&nodes[4]);
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

    /// Deleting an admin keeps the audit rows they wrote: `auditLogs` returns them with
    /// `userDeleted: true`, no `userId`, and the name they had when they acted.
    #[tokio::test]
    async fn audit_rows_outlive_their_author() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let author = insert_user(&pool, "departing admin").await;
        let remover = insert_user(&pool, "remaining admin").await;
        let target = insert_user(&pool, "audit subject").await;

        run(
            &pool,
            &author,
            staff(),
            &format!(r#"mutation {{ suspendUser(id: "{}") }}"#, target.id),
        )
        .await;
        run(
            &pool,
            &remover,
            staff(),
            &format!(r#"mutation {{ deleteUser(id: "{}") }}"#, author.id),
        )
        .await;

        let data = run(
            &pool,
            &remover,
            staff(),
            r#"{ auditLogs(filter: { action: "suspend_user" }, pagination: { first: 100 }) { nodes { userId userName userDeleted resourceId } } }"#,
        )
        .await;
        let rows: Vec<&Value> = data["auditLogs"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["resourceId"] == target.id.to_string())
            .collect();
        assert_eq!(
            rows,
            [&json!({
                "userId": null,
                "userName": "departing admin",
                "userDeleted": true,
                "resourceId": target.id.to_string(),
            })]
        );
    }

    /// A failed admin mutation still writes an audit row, whose outcome says why it failed;
    /// the error message and the refused values stay out of it.
    #[tokio::test]
    async fn failed_admin_mutations_write_an_outcome_row() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let admin = insert_user(&pool, "audit failing admin").await;
        let other = insert_user(&pool, "audit other user").await;
        let missing = Uuid::new_v4();
        let other_id = other.id;
        let taken = other.email.clone();

        let attempts = [
            // Not found.
            format!(r#"mutation {{ suspendUser(id: "{missing}") }}"#),
            // Validation: nothing to change, an id that is not a UUID, a crawl with no source.
            format!(r#"mutation {{ updateUser(id: "{other_id}", input: {{}}) {{ id }} }}"#),
            r#"mutation { deleteUser(id: "not-a-uuid") }"#.to_string(),
            r#"mutation { triggerCrawl(input: { seriesIds: ["AUDITTEST"] }) { enqueuedCount } }"#
                .to_string(),
            // Conflict: another account's email.
            format!(
                r#"mutation {{ updateUser(id: "{}", input: {{ email: "{taken}" }}) {{ id }} }}"#,
                admin.id
            ),
            // Database error: a theme longer than its column.
            format!(
                r#"mutation {{ updateUser(id: "{other_id}", input: {{ theme: "{}" }}) {{ id }} }}"#,
                "x".repeat(64)
            ),
            // Self-lockout: suspending or deleting the caller's own account.
            format!(r#"mutation {{ suspendUser(id: "{}") }}"#, admin.id),
            format!(r#"mutation {{ deleteUser(id: "{}") }}"#, admin.id),
            format!(
                r#"mutation {{ updateUser(id: "{}", input: {{ isActive: false }}) {{ id }} }}"#,
                admin.id
            ),
        ];
        for query in &attempts {
            let response = schema_for(&pool, &admin, staff())
                .execute(query.as_str())
                .await;
            assert!(!response.errors.is_empty(), "{query} should fail");
        }
        // Denied: a narrow admin acting on someone else.
        let response = schema_for(&pool, &admin, vec![Role::AdminUsersSuspend])
            .execute(format!(r#"mutation {{ suspendUser(id: "{other_id}") }}"#))
            .await;
        assert_eq!(response.errors[0].message, "Insufficient permissions");

        let rows = audit_rows(&pool, &admin).await;
        let mut got: Vec<(String, String, Value)> = rows
            .iter()
            .map(|row| {
                (
                    row["action"].as_str().unwrap().to_string(),
                    details(row)["outcome"].as_str().unwrap().to_string(),
                    row["resourceId"].clone(),
                )
            })
            .collect();
        got.reverse(); // oldest first
        let id = |u: Uuid| json!(u.to_string());
        assert_eq!(
            got,
            [
                ("suspend_user".into(), "not_found".into(), id(missing)),
                ("update_user".into(), "invalid".into(), id(other_id)),
                ("delete_user".into(), "invalid".into(), Value::Null),
                ("trigger_crawl".into(), "invalid".into(), Value::Null),
                ("update_user".into(), "conflict".into(), id(admin.id)),
                ("update_user".into(), "error".into(), id(other_id)),
                ("suspend_user".into(), "self_lockout".into(), id(admin.id)),
                ("delete_user".into(), "self_lockout".into(), id(admin.id)),
                ("update_user".into(), "self_lockout".into(), id(admin.id)),
                ("suspend_user".into(), "denied".into(), id(other_id)),
            ]
        );
        for row in &rows {
            let text = row["details"].as_str().unwrap();
            assert!(!text.contains(&taken) && !text.contains("xxxx"), "{text}");
        }
        assert_eq!(details(&rows[1])["fields"], json!(["isActive"]));
        assert_eq!(details(&rows[4])["fields"], json!(["theme"]));
    }

    /// A query that aliases an admin field many times writes one denial row, and it is written
    /// by the time `execute` returns (no waiting on a background task).
    #[tokio::test]
    async fn aliased_denials_write_one_row_before_execute_returns() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let viewer = insert_user(&pool, "audit alias viewer").await;
        let aliases: String = (0..50)
            .map(|i| format!("a{i}: users {{ totalCount }} "))
            .collect();
        let response = schema_for(&pool, &viewer, vec![])
            .execute(format!("{{ {aliases} }}"))
            .await;
        assert_eq!(response.errors.len(), 50);

        let events = security_rows(&pool, &viewer).await;
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0]["eventType"], "permission_denied");
        assert_eq!(events[0]["userEmail"], viewer.email.as_str());
        assert_eq!(events[0]["description"], "missing role admin.users:read");

        // A different permission in the same request is its own row.
        let response = schema_for(&pool, &viewer, vec![])
            .execute(
                "{ a: users { totalCount } b: auditLogs { totalCount } c: users { totalCount } }",
            )
            .await;
        assert_eq!(response.errors.len(), 3);
        assert_eq!(security_rows(&pool, &viewer).await.len(), 3);
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

        let denied = schema_for(&pool, &viewer, vec![])
            .execute("{ users { totalCount } }")
            .await;
        assert_eq!(denied.errors[0].message, "Insufficient permissions");
        let denied = schema_for(&pool, &narrow, vec![Role::AdminUsersSuspend])
            .execute(format!(
                r#"mutation {{ suspendUser(id: "{}") }}"#,
                viewer.id
            ))
            .await;
        assert_eq!(denied.errors[0].message, "Insufficient permissions");

        let query = "{ securityEvents(limit: 100) { eventType userId userEmail description } }";
        let data = run(&pool, &reader, vec![Role::AdminSecurityRead], query).await;
        let events: Vec<Value> = data["securityEvents"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                e["userId"] == viewer.id.to_string() || e["userId"] == narrow.id.to_string()
            })
            .cloned()
            .collect();
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

    /// Audit rows and security events carry the request's client address and user agent.
    #[tokio::test]
    async fn audit_rows_and_events_carry_the_client() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let admin = insert_user(&pool, "audit client admin").await;
        let viewer = insert_user(&pool, "audit client viewer").await;
        let target = insert_user(&pool, "audit client target").await;
        let client = ClientInfo::new(Some("203.0.113.7".into()), Some("AuditTest/1.0"));
        let schema = |user: &User, roles| {
            create_schema_with_data(
                econ_graph_core::database::AppPool(pool.clone()),
                Some(econ_graph_core::database::DataPool(pool.clone())),
                Arc::new(
                    GraphQLContext::signed_in(user.clone(), roles).with_client(client.clone()),
                ),
            )
        };

        let response = schema(&admin, staff())
            .execute(format!(
                r#"mutation {{ suspendUser(id: "{}") }}"#,
                target.id
            ))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        let response = schema(&viewer, vec![])
            .execute("{ users { totalCount } }")
            .await;
        assert_eq!(response.errors.len(), 1);

        let rows = audit_rows(&pool, &admin).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["ipAddress"], "203.0.113.7");
        assert_eq!(rows[0]["userAgent"], "AuditTest/1.0");
        let events = security_rows(&pool, &viewer).await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["ipAddress"], "203.0.113.7");
        assert_eq!(events[0]["userAgent"], "AuditTest/1.0");
    }

    /// A long user agent is cut to a fixed number of characters, on a character boundary.
    #[test]
    fn user_agent_is_truncated() {
        let long = "é".repeat(2000);
        let client = ClientInfo::new(None, Some(&long));
        let kept = client.user_agent.unwrap();
        assert_eq!(kept.chars().count(), ClientInfo::USER_AGENT_MAX_CHARS);
        assert_eq!(ClientInfo::new(None, Some("   ")).user_agent, None);
    }
    /// Observe the server-side lock wait instead of racing cancellation against a timer.
    async fn wait_for_locked_statement(pool: &DatabasePool, fragment: &str) {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        #[derive(QueryableByName)]
        struct Waiting {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            waiting: bool,
        }
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let mut conn = pool.get().await.unwrap();
                let row: Waiting = diesel::sql_query("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND pid <> pg_backend_pid() AND wait_event_type = 'Lock' AND query LIKE $1) AS waiting")
                    .bind::<diesel::sql_types::Text, _>(format!("%{fragment}%"))
                    .get_result(&mut conn).await.unwrap();
                if row.waiting { break; }
                tokio::task::yield_now().await;
            }
        }).await.expect("statement reached the held database lock");
    }

    async fn wait_for_audit(pool: &DatabasePool, actor: &User) -> Vec<Value> {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let rows = audit_rows(pool, actor).await;
                if !rows.is_empty() {
                    return rows;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("detached audit write completed")
    }

    /// Disconnect while the action itself waits in PostgreSQL. Its outcome is unknown,
    /// but the attempt must still be recorded after the request future is gone.
    #[tokio::test]
    async fn cancellation_during_mutation_records_unknown_outcome() {
        use diesel_async::SimpleAsyncConnection;
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let actor = insert_user(&pool, "cancel actor").await;
        let target = insert_user(&pool, "cancel target").await;
        let mut lock = pool.get().await.unwrap();
        lock.batch_execute("BEGIN; LOCK TABLE users IN SHARE MODE")
            .await
            .unwrap();
        let schema = schema_for(&pool, &actor, staff());
        let request = tokio::spawn(async move {
            schema
                .execute(format!("mutation {{ suspendUser(id: \"{}\") }}", target.id))
                .await
        });
        wait_for_locked_statement(&pool, "UPDATE").await;
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        lock.batch_execute("ROLLBACK").await.unwrap();
        let rows = wait_for_audit(&pool, &actor).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(details(&rows[0])["outcome"], "cancelled");
    }

    /// Cancellation during the audit insert cannot drop a successful action's row or
    /// produce an extra `cancelled` row from the attempt guard.
    #[tokio::test]
    async fn cancellation_during_audit_insert_preserves_one_success_row() {
        use diesel_async::SimpleAsyncConnection;
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let actor = insert_user(&pool, "cancel audit actor").await;
        let target = insert_user(&pool, "cancel audit target").await;
        let mut lock = pool.get().await.unwrap();
        lock.batch_execute("BEGIN; LOCK TABLE audit_logs IN SHARE MODE")
            .await
            .unwrap();
        let schema = schema_for(&pool, &actor, staff());
        let request = tokio::spawn(async move {
            schema
                .execute(format!("mutation {{ suspendUser(id: \"{}\") }}", target.id))
                .await
        });
        wait_for_locked_statement(&pool, "INSERT INTO \"audit_logs\"").await;
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        lock.batch_execute("ROLLBACK").await.unwrap();
        let rows = wait_for_audit(&pool, &actor).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(details(&rows[0])["outcome"], "success");
    }

    #[tokio::test]
    async fn cancellation_during_denial_write_preserves_event() {
        use diesel_async::SimpleAsyncConnection;
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let actor = insert_user(&pool, "cancel denial actor").await;
        let mut lock = pool.get().await.unwrap();
        lock.batch_execute("BEGIN; LOCK TABLE security_events IN SHARE MODE")
            .await
            .unwrap();
        let schema = schema_for(&pool, &actor, vec![]);
        let request = tokio::spawn(async move { schema.execute("{ users { totalCount } }").await });
        wait_for_locked_statement(&pool, "INSERT INTO \"security_events\"").await;
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        lock.batch_execute("ROLLBACK").await.unwrap();
        let rows = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let rows = security_rows(&pool, &actor).await;
                if !rows.is_empty() {
                    break rows;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["eventType"], "permission_denied");
    }

    #[tokio::test]
    async fn admin_pagination_rejects_bad_cursors_and_handles_maximum_offset() {
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let reader = insert_user(&pool, "cursor reader").await;
        let schema = schema_for(&pool, &reader, staff());
        for field in ["auditLogs", "securityEventsPage"] {
            for cursor in ["broken", "-1", "9223372036854775808"] {
                let response = schema
                    .execute(format!(
                        r#"{{ {field}(pagination: {{after: "{cursor}"}}) {{ totalCount }} }}"#
                    ))
                    .await;
                assert_eq!(response.errors.len(), 1, "{field} {cursor}");
                assert!(response.errors[0].message.contains("after"));
                assert_eq!(
                    response.errors[0].extensions.as_ref().unwrap().get("code"),
                    Some(&async_graphql::Value::from("BAD_USER_INPUT"))
                );
            }
            let data = run(&pool, &reader, staff(), &format!(r#"{{ {field}(pagination: {{after: "9223372036854775807", first: 100}}) {{ nodes {{ id }} pageInfo {{ hasNextPage endCursor }} }} }}"#)).await;
            assert!(data[field]["nodes"].as_array().unwrap().is_empty());
            assert_eq!(data[field]["pageInfo"]["hasNextPage"], false);
            assert!(data[field]["pageInfo"]["endCursor"].is_null());
        }
    }

    #[tokio::test]
    async fn security_event_pages_filter_type_and_time_and_reach_older_rows() {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::security_events;
        let Some((pool, _guard)) = db().await else {
            return;
        };
        let reader = insert_user(&pool, "event page reader").await;
        let event_type = format!("test_{}", Uuid::new_v4());
        let before = chrono::Utc::now();
        for index in 0..105 {
            NewSecurityEvent {
                event_type: event_type.clone(),
                user_id: Some(reader.id),
                user_email: None,
                severity: "medium".into(),
                ip_address: None,
                user_agent: None,
                description: format!("event {index}"),
                metadata: None,
            }
            .record(&pool)
            .await;
        }
        let after = chrono::Utc::now();
        // Give all matching events one timestamp to exercise the stable ID tie-breaker.
        let mut conn = pool.get().await.unwrap();
        diesel::update(security_events::table.filter(security_events::event_type.eq(&event_type)))
            .set(security_events::created_at.eq(before + (after - before) / 2))
            .execute(&mut conn)
            .await
            .unwrap();
        drop(conn);
        let filter = format!(
            r#"eventType: "{event_type}", createdAfter: "{}", createdBefore: "{}""#,
            before.to_rfc3339(),
            after.to_rfc3339()
        );
        let page = |cursor: &str| {
            format!(
                r#"{{ securityEventsPage(filter: {{{filter}}}, pagination: {{ first: 100, after: "{cursor}" }}) {{ totalCount nodes {{ id }} pageInfo {{ hasNextPage endCursor }} }} }}"#
            )
        };
        let first = run(&pool, &reader, staff(), &page("0")).await;
        assert_eq!(first["securityEventsPage"]["totalCount"], 105);
        assert_eq!(
            first["securityEventsPage"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            100
        );
        assert_eq!(first["securityEventsPage"]["pageInfo"]["endCursor"], "100");
        let second = run(&pool, &reader, staff(), &page("100")).await;
        assert_eq!(
            second["securityEventsPage"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_eq!(
            second["securityEventsPage"]["pageInfo"]["hasNextPage"],
            false
        );
        let ids: HashSet<_> = first["securityEventsPage"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .chain(second["securityEventsPage"]["nodes"].as_array().unwrap())
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids.len(), 105);
        let empty = run(&pool, &reader, staff(), &format!(r#"{{ securityEvents(filter: {{eventType: "{event_type}", createdAfter: "{}"}}) {{ id }} }}"#, after.to_rfc3339())).await;
        assert!(empty["securityEvents"].as_array().unwrap().is_empty());
    }
}
