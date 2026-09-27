//! Each protected resolver asks for exactly one fine-grained role.
//!
//! For every resolver, a caller holding only that role gets past the check, and a caller
//! holding every other role is refused. None of these cases needs a database: the pool never
//! connects, so an allowed call fails later, on its first query, with an error that is not an
//! authorization error.

use crate::graphql::context::GraphQLContext;
use crate::graphql::schema::create_schema_with_data;
use econ_graph_auth::{Principal, Role};
use econ_graph_core::models::User;
use econ_graph_core::DatabasePool;
use std::sync::Arc;
use uuid::Uuid;

/// A pool that never connects.
fn unreachable_pool() -> DatabasePool {
    let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
        diesel_async::AsyncPgConnection,
    >::new("postgres://nobody@127.0.0.1:1/none");
    DatabasePool::builder()
        .connection_timeout(std::time::Duration::from_millis(200))
        .build_unchecked(manager)
}

fn user() -> User {
    let now = chrono::Utc::now();
    User {
        id: Uuid::new_v4(),
        email: "caller@example.test".into(),
        name: "caller".into(),
        avatar_url: None,
        provider: "email".into(),
        provider_id: None,
        password_hash: None,
        role: "viewer".into(),
        organization: None,
        theme: "light".into(),
        default_chart_type: "line".into(),
        notifications_enabled: false,
        collaboration_enabled: false,
        is_active: true,
        email_verified: true,
        created_at: now,
        updated_at: now,
        last_login_at: None,
    }
}

/// A signed-in caller holding exactly `roles`, whatever their legacy `users.role`.
fn caller_with(user: User, roles: impl IntoIterator<Item = Role>) -> GraphQLContext {
    let mut ctx = GraphQLContext::new(Some(user));
    ctx.principal = Some(Principal::new(ctx.user.as_ref().unwrap().id, roles));
    ctx
}

async fn errors(ctx: GraphQLContext, query: &str) -> Vec<String> {
    let schema = create_schema_with_data(unreachable_pool(), Arc::new(ctx));
    schema
        .execute(query)
        .await
        .errors
        .into_iter()
        .map(|e| e.message)
        .collect()
}

fn is_auth_error(message: &str) -> bool {
    message.contains("Insufficient permissions") || message.contains("Authentication required")
}

/// Every protected resolver, the role it requires, and a request that reaches it.
fn protected() -> Vec<(&'static str, Role, String)> {
    let id = Uuid::new_v4();
    vec![
        (
            "triggerCrawl",
            Role::AdminCrawlersManage,
            r#"mutation { triggerCrawl(input: { sources: ["FRED"], seriesIds: ["GDP"] }) { __typename } }"#.into(),
        ),
        (
            "createUser",
            Role::AdminUsersCreate,
            r#"mutation { createUser(input: { email: "a@example.test", name: "a", role: "viewer" }) { __typename } }"#.into(),
        ),
        (
            "updateUser",
            Role::AdminUsersUpdate,
            format!(r#"mutation {{ updateUser(id: "{id}", input: {{ name: "a" }}) {{ __typename }} }}"#),
        ),
        (
            "deleteUser",
            Role::AdminUsersDelete,
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
        (
            "suspendUser",
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            "activateUser",
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
        (
            "forceLogoutUser",
            Role::AdminSessionsRevoke,
            format!(r#"mutation {{ forceLogoutUser(id: "{id}") }}"#),
        ),
        (
            "createAnnotation",
            Role::AnnotationCreate,
            format!(
                r#"mutation {{ createAnnotation(input: {{ seriesId: "{id}", annotationDate: "2024-01-01", title: "t", content: "c", annotationType: "note" }}) {{ __typename }} }}"#
            ),
        ),
        (
            "addComment",
            Role::AnnotationComment,
            format!(
                r#"mutation {{ addComment(input: {{ annotationId: "{id}", content: "c" }}) {{ __typename }} }}"#
            ),
        ),
        (
            "shareChart",
            Role::ChartShare,
            format!(
                r#"mutation {{ shareChart(input: {{ targetUserId: "{id}", chartId: "{id}", permissionLevel: "view" }}) {{ __typename }} }}"#
            ),
        ),
        (
            "users",
            Role::AdminUsersRead,
            "{ users { __typename } }".into(),
        ),
        (
            "user (another user)",
            Role::AdminUsersRead,
            format!(r#"{{ user(userId: "{id}") {{ __typename }} }}"#),
        ),
        (
            "userSessions",
            Role::AdminSessionsRead,
            "{ userSessions { __typename } }".into(),
        ),
        (
            "activeSessions",
            Role::AdminSessionsRead,
            "{ activeSessions { __typename } }".into(),
        ),
        (
            "systemHealth",
            Role::AdminSystemRead,
            "{ systemHealth { __typename } }".into(),
        ),
        (
            "securityEvents",
            Role::AdminSecurityRead,
            "{ securityEvents { __typename } }".into(),
        ),
        (
            "auditLogs",
            Role::AdminAuditRead,
            "{ auditLogs { __typename } }".into(),
        ),
    ]
}

#[tokio::test]
async fn each_resolver_allows_a_caller_holding_only_its_role() {
    for (name, role, query) in protected() {
        let errs = errors(caller_with(user(), [role]), &query).await;
        assert!(
            !errs.iter().any(|e| is_auth_error(e)),
            "{name} with only {role}: {errs:?}"
        );
    }
}

#[tokio::test]
async fn each_resolver_denies_a_caller_holding_every_other_role() {
    for (name, role, query) in protected() {
        let others = Role::all().iter().copied().filter(|&r| r != role);
        let errs = errors(caller_with(user(), others), &query).await;
        assert_eq!(errs.len(), 1, "{name} without {role}: {errs:?}");
        assert!(
            errs[0].contains("Insufficient permissions"),
            "{name} without {role}: {errs:?}"
        );
    }
}

#[tokio::test]
async fn each_resolver_asks_an_anonymous_caller_to_sign_in() {
    for (name, _, query) in protected() {
        let errs = errors(GraphQLContext::new(None), &query).await;
        assert_eq!(errs.len(), 1, "{name}: {errs:?}");
        assert!(
            errs[0].contains("Authentication required"),
            "{name}: {errs:?}"
        );
    }
}

#[tokio::test]
async fn a_caller_with_no_roles_may_read_their_own_user_record() {
    let me = user();
    let query = format!(r#"{{ user(userId: "{}") {{ __typename }} }}"#, me.id);
    let errs = errors(caller_with(me, []), &query).await;
    assert!(!errs.iter().any(|e| is_auth_error(e)), "{errs:?}");
}

/// The legacy all-staff check is gone for good: resolvers name the one role they need.
#[test]
fn no_resolver_uses_the_legacy_admin_check() {
    let needle = ["require", "_admin"].concat();
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut stack = vec![crates];
    let mut hits = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "target") {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && std::fs::read_to_string(&path).unwrap().contains(&needle)
            {
                hits.push(path.display().to_string());
            }
        }
    }
    assert!(hits.is_empty(), "{needle} is still used in {hits:?}");
}

/// Every role except `missing`.
fn all_but(missing: Role) -> Vec<Role> {
    Role::all()
        .iter()
        .copied()
        .filter(|&r| r != missing)
        .collect()
}

#[tokio::test]
async fn a_user_admin_cannot_grant_staff_roles_they_lack() {
    let id = Uuid::new_v4();
    let cases = [
        (
            Role::AdminUsersCreate,
            r#"mutation { createUser(input: { email: "a@example.test", name: "a", role: "admin" }) { __typename } }"#.to_string(),
        ),
        (
            Role::AdminUsersUpdate,
            format!(r#"mutation {{ updateUser(id: "{id}", input: {{ role: "super_admin" }}) {{ __typename }} }}"#),
        ),
    ];
    for (role, query) in cases {
        // Only the admin role for this mutation: granting `admin` would add every staff role.
        let errs = errors(caller_with(user(), [role]), &query).await;
        assert_eq!(errs.len(), 1, "{role}: {errs:?}");
        assert!(
            errs[0].contains("Insufficient permissions"),
            "{role}: {errs:?}"
        );

        // Holding every role, granting `admin` passes the check.
        let errs = errors(caller_with(user(), Role::all().iter().copied()), &query).await;
        assert!(!errs.iter().any(|e| is_auth_error(e)), "{role}: {errs:?}");
    }
}

#[tokio::test]
async fn a_user_admin_may_grant_a_non_staff_role() {
    let query = r#"mutation { createUser(input: { email: "a@example.test", name: "a", role: "viewer" }) { __typename } }"#;
    let errs = errors(caller_with(user(), [Role::AdminUsersCreate]), query).await;
    assert!(!errs.iter().any(|e| is_auth_error(e)), "{errs:?}");
}

#[tokio::test]
async fn an_unknown_legacy_role_is_refused() {
    let query = r#"mutation { createUser(input: { email: "a@example.test", name: "a", role: "root" }) { __typename } }"#;
    let errs = errors(caller_with(user(), Role::all().iter().copied()), query).await;
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("Unknown role: root"), "{errs:?}");
}

#[tokio::test]
async fn changing_is_active_needs_the_suspend_role() {
    let id = Uuid::new_v4();
    let query = format!(
        r#"mutation {{ updateUser(id: "{id}", input: {{ isActive: false }}) {{ __typename }} }}"#
    );
    let errs = errors(
        caller_with(user(), all_but(Role::AdminUsersSuspend)),
        &query,
    )
    .await;
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert!(errs[0].contains("Insufficient permissions"), "{errs:?}");

    let errs = errors(
        caller_with(user(), [Role::AdminUsersUpdate, Role::AdminUsersSuspend]),
        &query,
    )
    .await;
    assert!(!errs.iter().any(|e| is_auth_error(e)), "{errs:?}");
}

/// Needs `DATABASE_URL` (skipped otherwise): the target check runs after the target is read.
async fn insert_user(pool: &DatabasePool, legacy_role: &str) -> Uuid {
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::users;
    let new_user = econ_graph_core::models::NewUser {
        email: format!("{}@example.test", Uuid::new_v4()),
        name: legacy_role.into(),
        avatar_url: None,
        provider: "email".into(),
        provider_id: None,
        password_hash: None,
        role: legacy_role.into(),
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
        .returning(users::id)
        .get_result(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_narrow_staff_role_cannot_act_on_a_fuller_admin() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed target check test");
        return;
    };
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let run = |roles: Vec<Role>, query: String| {
        let pool = pool.clone();
        async move {
            let schema = create_schema_with_data(pool, Arc::new(caller_with(user(), roles)));
            let resp = schema.execute(query.as_str()).await;
            resp.errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
        }
    };
    let mutations = |id: Uuid| {
        vec![
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ email: "{}@example.test" }}) {{ __typename }} }}"#,
                Uuid::new_v4()
            ),
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
            format!(r#"mutation {{ forceLogoutUser(id: "{id}") }}"#),
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ]
    };
    // Every role but one staff role a legacy admin holds.
    let narrow = all_but(Role::AdminAuditRead);

    let admin = insert_user(&pool, "admin").await;
    let admin_email = {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;
        let mut conn = pool.get().await.unwrap();
        users::table
            .filter(users::id.eq(admin))
            .select(users::email)
            .first::<String>(&mut conn)
            .await
            .unwrap()
    };
    for query in mutations(admin) {
        let errs = run(narrow.clone(), query.clone()).await;
        assert_eq!(errs.len(), 1, "{query}: {errs:?}");
        assert!(
            errs[0].contains("Insufficient permissions"),
            "{query}: {errs:?}"
        );
    }
    // Nothing was changed: the admin still exists, active, with their original email.
    {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::users;
        let mut conn = pool.get().await.unwrap();
        let (email, is_active): (String, bool) = users::table
            .filter(users::id.eq(admin))
            .select((users::email, users::is_active))
            .first(&mut conn)
            .await
            .expect("the admin was deleted");
        assert!(is_active);
        assert_eq!(email, admin_email);
    }

    // The same caller may act on a user who holds no staff roles.
    let viewer = insert_user(&pool, "viewer").await;
    for query in mutations(viewer) {
        let errs = run(narrow.clone(), query.clone()).await;
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }

    // A full admin may act on another admin.
    for query in mutations(admin) {
        let errs = run(Role::all().to_vec(), query.clone()).await;
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }
}
