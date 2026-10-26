//! Each protected resolver asks for exactly one fine-grained role. Four of them (updateUser,
//! deleteUser, suspendUser, activateUser) also gate on `require_manageable`, which runs before
//! the pool is touched and passes without a database when the caller acts on their own account.
//! Suspending or deleting that own account is refused outright (ECO-411), also before the pool.
//!
//! For every resolver, a caller holding only that role gets past the check, and a caller
//! holding every other role is refused. None of these cases needs a database: the pool never
//! connects, so an allowed call fails later, on its first query, with an error that is not an
//! authorization error. The four `require_manageable`-gated resolvers target the caller's own
//! id here so the self-action path clears the gate before failing on the pool; acting on
//! another user is covered separately below, proving the gate runs before any connection is
//! attempted.

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
    let schema = create_schema_with_data(
        econ_graph_core::database::AppPool(unreachable_pool()),
        Some(econ_graph_core::database::DataPool(unreachable_pool())),
        Arc::new(ctx),
    );
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
///
/// `self_id` is the id the four `require_manageable`-gated mutations target: passing the
/// caller's own id lets the self-action path clear that gate so the resolver reaches the
/// (unreachable) pool, the same as every other entry here. deleteUser and suspendUser stop
/// short of the pool on their own id (the self-lockout refusal), still past the role check.
fn protected(self_id: Uuid) -> Vec<(&'static str, Role, String)> {
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
            format!(r#"mutation {{ updateUser(id: "{self_id}", input: {{ name: "a" }}) {{ __typename }} }}"#),
        ),
        (
            "deleteUser",
            Role::AdminUsersDelete,
            format!(r#"mutation {{ deleteUser(id: "{self_id}") }}"#),
        ),
        (
            "suspendUser",
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ suspendUser(id: "{self_id}") }}"#),
        ),
        (
            "activateUser",
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ activateUser(id: "{self_id}") }}"#),
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
            "securityEventsPage",
            Role::AdminSecurityRead,
            "{ securityEventsPage { totalCount } }".into(),
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
    // One caller for every entry, so the `require_manageable`-gated resolvers can target its
    // own id and clear that gate too.
    let me = user();
    for (name, role, query) in protected(me.id) {
        let errs = errors(caller_with(me.clone(), [role]), &query).await;
        assert!(
            !errs.iter().any(|e| is_auth_error(e)),
            "{name} with only {role}: {errs:?}"
        );
    }
}

#[tokio::test]
async fn each_resolver_denies_a_caller_holding_every_other_role() {
    for (name, role, query) in protected(Uuid::new_v4()) {
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
    for (name, _, query) in protected(Uuid::new_v4()) {
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

/// Regression for #260: a narrow-role caller acting on another user is refused by
/// `require_manageable` before the resolver ever calls `pool.get()`. The pool here never
/// connects, so the single "Insufficient permissions" error (not a connection error) proves
/// the gate runs first.
#[tokio::test]
async fn gate_denies_another_user_before_the_pool_connects() {
    let id = Uuid::new_v4();
    for (role, query) in [
        (
            Role::AdminUsersUpdate,
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ name: "a" }}) {{ __typename }} }}"#
            ),
        ),
        (
            Role::AdminUsersDelete,
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
        ),
        (
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        ),
        (
            Role::AdminUsersSuspend,
            format!(r#"mutation {{ activateUser(id: "{id}") }}"#),
        ),
    ] {
        let errs = errors(caller_with(user(), [role]), &query).await;
        assert_eq!(errs, ["Insufficient permissions"], "{query}: {errs:?}");
    }
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

    // A narrow two-role caller only clears `require_manageable` on their own account, so
    // target the caller's own id here to reach the (unreachable) pool, as above. Reactivating
    // is the change allowed on one's own account; deactivating it is refused (ECO-411).
    let me = user();
    let self_query = format!(
        r#"mutation {{ updateUser(id: "{}", input: {{ isActive: true }}) {{ __typename }} }}"#,
        me.id
    );
    let errs = errors(
        caller_with(me, [Role::AdminUsersUpdate, Role::AdminUsersSuspend]),
        &self_query,
    )
    .await;
    assert!(!errs.iter().any(|e| is_auth_error(e)), "{errs:?}");
}

/// Regression for ECO-411: suspending, deactivating or deleting one's own account would lock
/// the caller out for good (a suspended account cannot even call `activateUser`). It is refused
/// with a validation error before the pool is touched, even for the full staff set.
#[tokio::test]
async fn own_account_cannot_be_suspended_or_deleted() {
    let me = user();
    let id = me.id;
    let staff: Vec<Role> = Role::all()
        .iter()
        .copied()
        .filter(|r| r.is_staff())
        .collect();
    for (query, expected) in [
        (
            format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
            "You cannot suspend your own account",
        ),
        (
            format!(
                r#"mutation {{ updateUser(id: "{id}", input: {{ isActive: false }}) {{ __typename }} }}"#
            ),
            "You cannot suspend your own account",
        ),
        (
            format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
            "You cannot delete your own account",
        ),
    ] {
        for roles in [staff.clone(), Role::all().to_vec()] {
            let errs = errors(caller_with(me.clone(), roles), &query).await;
            assert_eq!(errs, [expected], "{query}: {errs:?}");
        }
    }
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
        econ_graph_core::database::AppPool(pool.clone()),
        Some(econ_graph_core::database::DataPool(pool.clone())),
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

/// A narrow role still works on the caller's own account (unaffected by the gate). Suspending
/// or deleting it is refused instead; see `own_account_is_left_intact_by_suspend_and_delete`.
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
    let activate_target = insert_user(&pool).await;

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
            activate_target.clone(),
            vec![Role::AdminUsersSuspend],
            format!(
                r#"mutation {{ activateUser(id: "{}") }}"#,
                activate_target.id
            ),
        ),
    ] {
        let schema = create_schema_with_data(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool.clone())),
            Arc::new(caller_with(me, roles)),
        );
        let errs: Vec<String> = schema
            .execute(query.as_str())
            .await
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect();
        assert!(errs.is_empty(), "{query}: {errs:?}");
    }
}

/// Regression for ECO-411 against PostgreSQL: a staff admin's attempts to suspend, deactivate
/// or delete their own account leave the row in place and active, so they can still sign in.
#[tokio::test]
async fn own_account_is_left_intact_by_suspend_and_delete() {
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
    let staff: Vec<Role> = Role::all()
        .iter()
        .copied()
        .filter(|r| r.is_staff())
        .collect();
    for query in [
        format!(r#"mutation {{ suspendUser(id: "{id}") }}"#),
        format!(
            r#"mutation {{ updateUser(id: "{id}", input: {{ isActive: false }}) {{ __typename }} }}"#
        ),
        format!(r#"mutation {{ deleteUser(id: "{id}") }}"#),
    ] {
        let schema = create_schema_with_data(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool.clone())),
            Arc::new(caller_with(me.clone(), staff.clone())),
        );
        let errs: Vec<String> = schema
            .execute(query.as_str())
            .await
            .errors
            .into_iter()
            .map(|e| e.message)
            .collect();
        assert_eq!(errs.len(), 1, "{query}: {errs:?}");
        assert!(errs[0].contains("your own account"), "{query}: {errs:?}");
    }
    let unchanged = read_user(&pool, id)
        .await
        .expect("own account still exists");
    assert!(unchanged.is_active);
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
        let schema = create_schema_with_data(
            econ_graph_core::database::AppPool(pool.clone()),
            Some(econ_graph_core::database::DataPool(pool.clone())),
            Arc::new(caller_with(me.clone(), roles)),
        );
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

/// Deleting an inviter keeps other recipients' grants while removing the
/// deleted user's sessions, private content and access grants.
#[tokio::test]
async fn deleting_a_chart_inviter_preserves_other_users_access() {
    use diesel::prelude::*;
    use diesel_async::{RunQueryDsl, SimpleAsyncConnection};
    use econ_graph_core::schema::chart_collaborators;

    let url = required_database_url();
    let _guard = crate::graphql::TEST_DB_LOCK.lock().await;
    econ_graph_core::database::run_migrations(&url)
        .await
        .unwrap();
    let pool = econ_graph_core::database::create_pool(&url).await.unwrap();
    let inviter = insert_user(&pool).await;
    let recipient = insert_user(&pool).await;
    let chart = Uuid::new_v4();
    let own_annotation = Uuid::new_v4();
    let other_annotation = Uuid::new_v4();
    let own_comment = Uuid::new_v4();
    let other_comment = Uuid::new_v4();
    let mut conn = pool.get().await.unwrap();
    conn.batch_execute(&format!(
        "INSERT INTO chart_collaborators (chart_id, user_id, invited_by, role)
         VALUES ('{chart}', '{recipient}', '{inviter}', 'viewer'),
                ('{chart}', '{inviter}', '{recipient}', 'owner');
         INSERT INTO user_sessions (user_id, token_hash, expires_at)
         VALUES ('{inviter}', 'delete-inviter-test', NOW() + interval '1 day');
         INSERT INTO user_data_source_preferences (user_id, data_source_id)
         VALUES ('{inviter}', '{chart}');
         INSERT INTO chart_annotations (id, user_id, chart_id, annotation_date, title)
         VALUES ('{own_annotation}', '{inviter}', '{chart}', '2024-01-01', 'Own'),
                ('{other_annotation}', '{recipient}', '{chart}', '2024-01-01', 'Other');
         INSERT INTO annotation_comments (id, annotation_id, user_id, content)
         VALUES ('{own_comment}', '{other_annotation}', '{inviter}', 'Own comment'),
                ('{other_comment}', '{other_annotation}', '{recipient}', 'Other comment');",
        inviter = inviter.id,
        recipient = recipient.id,
    ))
    .await
    .unwrap();
    drop(conn);

    let staff: Vec<_> = Role::all()
        .iter()
        .copied()
        .filter(|r| r.is_staff())
        .collect();
    // The down migration restores the old restriction. Prove the fixture exposes
    // the original deletion failure, then reapply the forward migration and retry.
    let mut conn = pool.get().await.unwrap();
    conn.batch_execute(include_str!(
        "../../../../migrations/app/2026-10-26-000451_keep_chart_grants_on_inviter_delete/down.sql"
    ))
    .await
    .unwrap();
    drop(conn);
    let query = format!(r#"mutation {{ deleteUser(id: "{}") }}"#, inviter.id);
    let old_errors = run_user_mutation(&pool, &staff, &query).await;
    assert_eq!(old_errors.len(), 1, "old FK must prevent deletion");
    assert!(read_user(&pool, inviter.id).await.is_some());
    let mut conn = pool.get().await.unwrap();
    conn.batch_execute(include_str!(
        "../../../../migrations/app/2026-10-26-000451_keep_chart_grants_on_inviter_delete/up.sql"
    ))
    .await
    .unwrap();
    drop(conn);
    let errs = run_user_mutation(&pool, &staff, &query).await;
    assert!(errs.is_empty(), "{errs:?}");
    assert!(read_user(&pool, inviter.id).await.is_none());
    assert!(read_user(&pool, recipient.id).await.is_some());

    let mut conn = pool.get().await.unwrap();
    let grant: (Option<Uuid>, Option<String>) = chart_collaborators::table
        .filter(chart_collaborators::chart_id.eq(chart))
        .filter(chart_collaborators::user_id.eq(recipient.id))
        .select((chart_collaborators::invited_by, chart_collaborators::role))
        .first(&mut conn)
        .await
        .unwrap();
    assert_eq!(grant, (None, Some("viewer".into())));
    // Fail on any changed cascade policy or accidental deletion of the survivor's content.
    conn.batch_execute(&format!(
        "DO $$ BEGIN
         ASSERT NOT EXISTS (SELECT 1 FROM user_sessions WHERE user_id = '{inviter}');
         ASSERT NOT EXISTS (SELECT 1 FROM user_data_source_preferences WHERE user_id = '{inviter}');
         ASSERT NOT EXISTS (SELECT 1 FROM chart_collaborators WHERE user_id = '{inviter}');
         ASSERT NOT EXISTS (SELECT 1 FROM chart_annotations WHERE id = '{own_annotation}');
         ASSERT NOT EXISTS (SELECT 1 FROM annotation_comments WHERE id = '{own_comment}');
         ASSERT EXISTS (SELECT 1 FROM chart_annotations WHERE id = '{other_annotation}');
         ASSERT EXISTS (SELECT 1 FROM annotation_comments WHERE id = '{other_comment}');
         END $$;",
        inviter = inviter.id,
    ))
    .await
    .unwrap();
}
