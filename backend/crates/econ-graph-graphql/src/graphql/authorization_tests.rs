//! Each protected resolver asks for exactly one fine-grained role.
//!
//! For every resolver, a caller holding only that role gets past the check, and a caller
//! holding every other role is refused. None of these cases needs a database: the pool never
//! connects, so an allowed call fails later, on its first query, with an error that is not an
//! authorization error.

use crate::graphql::context::GraphQLContext;
use crate::graphql::schema::create_schema_with_data;
use econ_graph_auth::Role;
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

/// A signed-in caller holding exactly `roles`.
fn caller_with(user: User, roles: impl IntoIterator<Item = Role>) -> GraphQLContext {
    GraphQLContext::signed_in(user, roles)
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
        let errs = errors(GraphQLContext::anonymous(), &query).await;
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

/// Roles are assigned in the identity provider: updateUser no longer takes one.
#[tokio::test]
async fn update_user_no_longer_takes_a_role() {
    let id = Uuid::new_v4();
    let query = format!(
        r#"mutation {{ updateUser(id: "{id}", input: {{ role: "super_admin" }}) {{ __typename }} }}"#
    );
    let errs = errors(caller_with(user(), Role::all().iter().copied()), &query).await;
    assert_eq!(errs.len(), 1, "{query}: {errs:?}");
    assert!(errs[0].contains("role"), "{query}: {errs:?}");
    assert!(!is_auth_error(&errs[0]), "{query}: {errs:?}");
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

/// DB-backed user administration checks skip when `DATABASE_URL` is unavailable.
async fn insert_user(pool: &DatabasePool) -> User {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::users;
    let new_user = econ_graph_core::models::NewUser {
        email: format!("{}@example.test", Uuid::new_v4()),
        name: "target".into(),
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

async fn read_user(pool: &DatabasePool, id: Uuid) -> Option<User> {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::schema::users;
    let mut conn = pool.get().await.unwrap();
    users::table
        .find(id)
        .select(User::as_select())
        .first(&mut conn)
        .await
        .optional()
        .unwrap()
}

async fn run_user_mutation(pool: &DatabasePool, roles: &[Role], query: &str) -> Vec<String> {
    let schema = create_schema_with_data(
        pool.clone(),
        Arc::new(caller_with(user(), roles.iter().copied())),
    );
    schema
        .execute(query)
        .await
        .errors
        .into_iter()
        .map(|error| error.message)
        .collect()
}

#[tokio::test]
async fn named_user_admin_roles_can_act_on_another_user() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping user administration test");
        return;
    };
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let target = insert_user(&pool).await;
    let id = target.id;

    for (roles, query) in [
        (
            vec![Role::AdminUsersUpdate],
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ name: "Updated" }}) {{ __typename }} }}"#
            ),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
        (
            vec![Role::AdminUsersDelete],
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
    ] {
        let errs = run_user_mutation(&pool, &roles, &query).await;
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }
    assert!(read_user(&pool, id).await.is_none());
}

#[tokio::test]
async fn update_user_email_conflict_leaves_other_fields_unchanged() {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping user update test");
        return;
    };
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let target = insert_user(&pool).await;
    let other = insert_user(&pool).await;
    let query = format!(
        r#"mutation {{ updateUser(id: "{}", input: {{ name: "Should not persist", theme: "dark", email: "{}" }}) {{ __typename }} }}"#,
        target.id, other.email
    );
    let errs = run_user_mutation(&pool, &[Role::AdminUsersUpdate], &query).await;
    assert_eq!(errs, ["User with this email already exists"]);
    let unchanged = read_user(&pool, target.id).await.unwrap();
    assert_eq!(unchanged.name, target.name);
    assert_eq!(unchanged.theme, target.theme);
    assert_eq!(unchanged.email, target.email);

    // Fields absent from the input, including login metadata, are not rewritten.
    let query = format!(
        r#"mutation {{ updateUser(id: "{}", input: {{ name: "Updated" }}) {{ __typename }} }}"#,
        target.id
    );
    let errs = run_user_mutation(&pool, &[Role::AdminUsersUpdate], &query).await;
    assert!(errs.is_empty(), "{errs:?}");
    let changed = read_user(&pool, target.id).await.unwrap();
    assert_eq!(changed.name, "Updated");
    assert_eq!(changed.email, target.email);
    assert_eq!(changed.last_login_at, target.last_login_at);
}
