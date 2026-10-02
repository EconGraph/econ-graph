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
            "crawlerStatus",
            Role::AdminSystemRead,
            "{ crawlerStatus { isRunning } }".into(),
        ),
        (
            "queueStatistics",
            Role::AdminSystemRead,
            "{ queueStatistics { totalItems } }".into(),
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

/// Security regressions must execute against PostgreSQL, never silently pass without it.
fn required_database_url() -> String {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL is required for user administration security regressions");
    assert!(
        !url.trim().is_empty(),
        "DATABASE_URL must not be empty for user administration security regressions"
    );
    url
}
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

/// Regression test for the #260 gap: a caller holding only the one role a mutation checks
/// could act on any other user, including a super_admin, because the backend cannot see
/// another user's roles and stopped requiring the full staff set before this test was added.
#[tokio::test]
async fn narrow_role_cannot_act_on_another_user() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    // Stands in for a super_admin: the backend has no record of another user's roles, so the
    // gate treats every other user the same way regardless of how privileged they are.
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
            vec![Role::AdminUsersDelete],
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
    ] {
        let errs = run_user_mutation(&pool, &roles, &query).await;
        assert_eq!(errs.len(), 1, "{query}: {errs:?}");
        assert!(
            errs[0].contains("Insufficient permissions"),
            "{query}: {errs:?}"
        );
    }
    let unchanged = read_user(&pool, id).await.unwrap();
    assert_eq!(unchanged.name, target.name);
    assert!(unchanged.is_active);
}

#[tokio::test]
async fn full_staff_set_can_act_on_another_user() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let target = insert_user(&pool).await;
    let id = target.id;
    let staff: Vec<Role> = Role::all()
        .iter()
        .copied()
        .filter(|r| r.is_staff())
        .collect();

    for (roles, query) in [
        (
            staff.clone(),
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ name: "Updated" }}) {{ __typename }} }}"#
            ),
        ),
        (
            staff.clone(),
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            staff.clone(),
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
        (
            staff.clone(),
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
    ] {
        let errs = run_user_mutation(&pool, &roles, &query).await;
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }
    assert!(read_user(&pool, id).await.is_none());
}

/// A narrow role still works on the caller's own account (unaffected by the gate). Each
/// mutation needs its own account: delete removes the row, so it runs last.
#[tokio::test]
async fn narrow_role_can_act_on_self() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");

    let update_target = insert_user(&pool).await;
    let suspend_target = insert_user(&pool).await;
    let activate_target = insert_user(&pool).await;
    let delete_target = insert_user(&pool).await;

    for (me, roles, query) in [
        (
            update_target.clone(),
            vec![Role::AdminUsersUpdate],
            format!(
                r#"mutation {{ updateUser(id: "{}", input: {{ name: "Updated" }}) {{ __typename }} }}"#,
                update_target.id
            ),
        ),
        (
            suspend_target.clone(),
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ suspendUser(id: "{}") }}"#, suspend_target.id),
        ),
        (
            activate_target.clone(),
            vec![Role::AdminUsersSuspend],
            format!(
                r#"mutation {{ activateUser(id: "{}") }}"#,
                activate_target.id
            ),
        ),
        (
            delete_target.clone(),
            vec![Role::AdminUsersDelete],
            format!(r#"mutation {{ deleteUser(id: "{}") }}"#, delete_target.id),
        ),
    ] {
        let schema = create_schema_with_data(pool.clone(), Arc::new(caller_with(me, roles)));
        let errs: Vec<String> = schema
            .execute(query.as_str())
            .await
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect();
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }
    assert!(read_user(&pool, delete_target.id).await.is_none());
}

/// A narrow role denied on another user gets refused before the mutation can tell whether
/// that target even exists: the gate must not leak "User not found" for a nonexistent id.
#[tokio::test]
async fn narrow_role_denial_does_not_leak_whether_the_target_exists() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let nonexistent_id = Uuid::new_v4();

    for (roles, query) in [
        (
            vec![Role::AdminUsersUpdate],
            format!(
                r#"mutation {{ updateUser(id: "{nonexistent_id}", input: {{ name: "Updated" }}) {{ __typename }} }}"#
            ),
        ),
        (
            vec![Role::AdminUsersDelete],
            format!(r#"mutation {{ deleteUser(id: "{nonexistent_id}") }}"#),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ suspendUser(id: "{nonexistent_id}") }}"#),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(r#"mutation {{ activateUser(id: "{nonexistent_id}") }}"#),
        ),
    ] {
        let errs = run_user_mutation(&pool, &roles, &query).await;
        assert_eq!(errs, ["Insufficient permissions"], "{query}: {errs:?}");
    }
}

/// Self access bypasses only the staff-set gate, never the named operation role.
#[tokio::test]
async fn self_access_still_requires_named_operation_roles() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let me = insert_user(&pool).await;
    let id = me.id;
    for (roles, query) in [
        (
            all_but(Role::AdminUsersUpdate),
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ name: "Denied" }}) {{ __typename }} }}"#
            ),
        ),
        (
            all_but(Role::AdminUsersDelete),
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
        (
            all_but(Role::AdminUsersSuspend),
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            all_but(Role::AdminUsersSuspend),
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
        (
            vec![Role::AdminUsersUpdate],
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ isActive: false }}) {{ __typename }} }}"#
            ),
        ),
        (
            vec![Role::AdminUsersSuspend],
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ isActive: false }}) {{ __typename }} }}"#
            ),
        ),
    ] {
        let schema =
            create_schema_with_data(pool.clone(), Arc::new(caller_with(me.clone(), roles)));
        let errs: Vec<String> = schema
            .execute(query.as_str())
            .await
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect();
        assert_eq!(errs, ["Insufficient permissions"], "{query}: {errs:?}");
    }
    let unchanged = read_user(&pool, id).await.unwrap();
    assert_eq!(unchanged.name, me.name);
    assert!(unchanged.is_active);
}

#[tokio::test]
async fn update_user_email_conflict_leaves_other_fields_unchanged() {
    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .expect("migrations");
    let pool = econ_graph_core::database::create_pool(&url)
        .await
        .expect("pool");
    let target = insert_user(&pool).await;
    let other = insert_user(&pool).await;
    // Acting on another user (not the caller's own account) needs the full staff set.
    let staff: Vec<Role> = Role::all()
        .iter()
        .copied()
        .filter(|r| r.is_staff())
        .collect();
    let query = format!(
        r#"mutation {{ updateUser(id: "{}", input: {{ name: "Should not persist", theme: "dark", email: "{}" }}) {{ __typename }} }}"#,
        target.id, other.email
    );
    let errs = run_user_mutation(&pool, &staff, &query).await;
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
    let errs = run_user_mutation(&pool, &staff, &query).await;
    assert!(errs.is_empty(), "{errs:?}");
    let changed = read_user(&pool, target.id).await.unwrap();
    assert_eq!(changed.name, "Updated");
    assert_eq!(changed.email, target.email);
    assert_eq!(changed.last_login_at, target.last_login_at);
}
