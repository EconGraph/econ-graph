use diesel_async::pooled_connection::{
    bb8::Pool, bb8::PooledConnection, AsyncDieselConnectionManager,
};
use diesel_async::AsyncPgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
// use diesel::prelude::*; // Not needed for async operations
use std::time::Duration;
use tracing::info;

use crate::error::{AppError, AppResult};

/// Type alias for the database pool
pub type DatabasePool = Pool<AsyncPgConnection>;

/// Type alias for a pooled connection
pub type PooledConn<'a> = PooledConnection<'a, AsyncPgConnection>;

/// A pool on the app database (`APP_DATABASE_URL`): users and everything they create.
///
/// A newtype rather than an alias so that one context (async-graphql's, say) can hold both pools
/// and tell them apart. Services and model functions take `&DatabasePool`; this derefs to it.
#[derive(Clone)]
pub struct AppPool(pub DatabasePool);

/// A pool on the data database (`DATA_DATABASE_URL`): sources, series, observations, crawl
/// state, SEC data. See [`AppPool`].
#[derive(Clone)]
pub struct DataPool(pub DatabasePool);

impl std::ops::Deref for AppPool {
    type Target = DatabasePool;

    fn deref(&self) -> &DatabasePool {
        &self.0
    }
}

impl std::ops::Deref for DataPool {
    type Target = DatabasePool;

    fn deref(&self) -> &DatabasePool {
        &self.0
    }
}

/// A database URL with its password (and any other userinfo) removed, safe to log.
/// Falls back to a fixed placeholder if `database_url` doesn't parse as a URL, so a
/// malformed value never lands in a log verbatim.
///
/// libpq also accepts credentials as URI query parameters (e.g. `?password=...`), so the
/// query string and fragment are dropped entirely rather than deny-listing parameter names.
pub fn redact_database_url(database_url: &str) -> String {
    match url::Url::parse(database_url) {
        Ok(mut url) => {
            let _ = url.set_password(None);
            let _ = url.set_username("");
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<unparseable database URL, redacted>".to_string(),
    }
}

/// Connections a pool from [`create_pool`] opens at most.
pub const DEFAULT_POOL_SIZE: u32 = 20;

/// Create a database connection pool of at most [`DEFAULT_POOL_SIZE`] connections.
pub async fn create_pool(database_url: &str) -> AppResult<DatabasePool> {
    create_pool_with_size(database_url, DEFAULT_POOL_SIZE).await
}

/// Create a database connection pool of at most `max_size` connections.
pub async fn create_pool_with_size(database_url: &str, max_size: u32) -> AppResult<DatabasePool> {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);

    let pool = Pool::builder()
        .max_size(max_size)
        .connection_timeout(Duration::from_secs(30))
        .idle_timeout(Some(Duration::from_secs(300))) // 5 minutes idle timeout
        .build(config)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to create database pool: {}", e)))?;

    info!("Database connection pool created successfully");
    Ok(pool)
}

/// Test database connectivity
pub async fn test_connection(pool: &DatabasePool) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(|e| {
        let error_msg = format!("Failed to get database connection: {}", e);
        tracing::error!("Database connection pool error: {}", error_msg);
        AppError::InternalError(error_msg)
    })?;

    // Test with a simple query
    let result: i32 = diesel_async::RunQueryDsl::get_result(
        diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1")),
        &mut conn,
    )
    .await
    .map_err(|e| AppError::InternalError(format!("Database connection test failed: {}", e)))?;

    if result == 1 {
        info!("Database connection test successful");
        Ok(())
    } else {
        Err(AppError::InternalError(
            "Database connection test returned unexpected result".to_string(),
        ))
    }
}

/// Advisory-lock key migrations take out for the duration of the run, so that when several
/// replicas start at once only one of them runs `diesel`'s migrations while the rest block on
/// `pg_advisory_lock` instead of racing each other over the `__diesel_schema_migrations` table.
/// Arbitrary but fixed: any i64 works as long as every replica uses the same one.
///
/// Both migration sets take this one key. Advisory locks are scoped to a database, so runs
/// against separate app and data databases never contend, and two sets in one database run one
/// after the other.
const MIGRATION_LOCK_KEY: i64 = 0x6567_5f6d_6967_7261; // "eg_migra" as bytes

/// The app migration set: users and everything they create. docs/development/MIGRATIONS.md lists
/// both sets and their tables.
pub const APP_MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/app");

/// The data migration set: sources, series, observations, crawl state, SEC data.
pub const DATA_MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/data");

/// One of the two migration sets. When one database holds both they share Diesel's
/// `__diesel_schema_migrations` table, so their versions never overlap (except
/// `00000000000000_diesel_initial_setup`, identical in both; `check_migration_order.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationSet {
    App,
    Data,
}

impl MigrationSet {
    /// The migrations embedded for this set.
    fn migrations(self) -> EmbeddedMigrations {
        match self {
            MigrationSet::App => APP_MIGRATIONS,
            MigrationSet::Data => DATA_MIGRATIONS,
        }
    }

    /// The set's directory name under `backend/migrations`, for logs and errors.
    fn name(self) -> &'static str {
        match self {
            MigrationSet::App => "app",
            MigrationSet::Data => "data",
        }
    }
}

/// The data baseline's version: the single v4.0.0 baseline's, which the data half kept.
const DATA_BASELINE_VERSION: &str = "20261021000100";

/// The app baseline's version (`migrations/app/2026-10-21-000200_v4_0_app_baseline`).
const APP_BASELINE_VERSION: &str = "20261021000200";

/// Run both migration sets against one database: the data set, then the app set.
///
/// Every caller has one database holding both sets, which share one `__diesel_schema_migrations`
/// table, so versions are unique across the two sets. A database built by the single v4.0.0 baseline,
/// before the split, already has the app tables; the app baseline is recorded there without
/// running (see [`is_single_baseline_database`]).
///
/// Note: Migrations require a synchronous connection
pub async fn run_migrations(database_url: &str) -> AppResult<()> {
    run_migration_sets(database_url, &[MigrationSet::Data, MigrationSet::App])
        .await
        .map(|_| ())
}

/// Run one migration set against `database_url`, under [`MIGRATION_LOCK_KEY`], and return the
/// versions it applied, oldest first (empty when the set was already current).
///
/// The app set still recognises a database built by the single v4.0.0 baseline and records the
/// app baseline there without running it (see [`is_single_baseline_database`]); that version is
/// not in the returned list. The database may hold the other set too; it is left as it is.
pub async fn run_migration_set(database_url: &str, set: MigrationSet) -> AppResult<Vec<String>> {
    run_migration_sets(database_url, &[set]).await
}

/// The versions of `set` embedded in this binary and not yet recorded in `database_url`'s
/// `__diesel_schema_migrations`, oldest first: what [`run_migration_set`] would apply. It only
/// reads, and takes no lock, so it changes nothing, not even by creating the migrations table on
/// an empty database. For the app set on a single-baseline database the app baseline is listed,
/// although applying the set records it without running it.
pub async fn pending_migration_versions(
    database_url: &str,
    set: MigrationSet,
) -> AppResult<Vec<String>> {
    let database_url = database_url.to_string();
    tokio::task::spawn_blocking(move || -> AppResult<Vec<String>> {
        use diesel::RunQueryDsl;

        #[derive(diesel::QueryableByName)]
        struct Exists {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            exists: bool,
        }
        #[derive(diesel::QueryableByName)]
        struct Version {
            #[diesel(sql_type = diesel::sql_types::Text)]
            version: String,
        }
        let err = |e: diesel::result::Error| {
            AppError::InternalError(format!("Failed to read the applied migrations: {}", e))
        };

        let mut conn = establish_migration_connection(&database_url)?;
        let has_table = diesel::sql_query(
            "SELECT to_regclass('public.__diesel_schema_migrations') IS NOT NULL AS exists",
        )
        .get_result::<Exists>(&mut conn)
        .map_err(err)?;
        let recorded: std::collections::HashSet<String> = if has_table.exists {
            diesel::sql_query("SELECT version FROM __diesel_schema_migrations")
                .load::<Version>(&mut conn)
                .map_err(err)?
                .into_iter()
                .map(|v| v.version)
                .collect()
        } else {
            Default::default()
        };
        let mut pending: Vec<String> = embedded_versions(set)?
            .into_iter()
            .filter(|v| !recorded.contains(v))
            .collect();
        pending.sort();
        Ok(pending)
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Migration task failed: {}", e)))?
}

/// The versions of every migration embedded in this binary for `set`.
fn embedded_versions(set: MigrationSet) -> AppResult<Vec<String>> {
    use diesel::migration::{Migration, MigrationSource};

    let migrations =
        MigrationSource::<diesel::pg::Pg>::migrations(&set.migrations()).map_err(|e| {
            AppError::InternalError(format!(
                "Failed to read the embedded {} migrations: {}",
                set.name(),
                e
            ))
        })?;
    Ok(migrations
        .iter()
        .map(|m| m.name().version().to_string())
        .collect())
}

/// A synchronous connection to `database_url`, which migrations need.
fn establish_migration_connection(database_url: &str) -> AppResult<diesel::PgConnection> {
    use diesel::Connection;

    // Ensure the database URL is properly formatted
    let formatted_url = if database_url.starts_with("postgresql://") {
        database_url.to_string()
    } else if database_url.starts_with("postgres://") {
        database_url.replace("postgres://", "postgresql://")
    } else {
        format!("postgresql://{}", database_url)
    };

    info!(
        "Attempting to connect to database for migrations: {}",
        redact_database_url(&formatted_url)
    );

    diesel::PgConnection::establish(&formatted_url).map_err(|e| {
        let error = AppError::InternalError(format!(
            "Failed to establish sync connection for migrations: {}",
            e
        ));
        error.log_with_context("Database migration connection attempt");
        error
    })
}

/// Runs `sets` in order against `database_url`, holding [`MIGRATION_LOCK_KEY`] for the whole run,
/// and returns the versions applied, in the order they ran.
pub(crate) async fn run_migration_sets(
    database_url: &str,
    sets: &[MigrationSet],
) -> AppResult<Vec<String>> {
    use diesel::RunQueryDsl;

    let sets = sets.to_vec();
    // Run migrations in a blocking task since migrations are sync
    let database_url = database_url.to_string();
    let applied = tokio::task::spawn_blocking(move || -> AppResult<Vec<String>> {
        let mut conn = establish_migration_connection(&database_url)?;

        info!("Database connection established, waiting for migration lock...");

        // Blocks here until whichever replica got there first releases the lock (or crashes /
        // disconnects, which releases it automatically since it's session-scoped).
        diesel::sql_query(format!("SELECT pg_advisory_lock({MIGRATION_LOCK_KEY})"))
            .execute(&mut conn)
            .map_err(|e| {
                AppError::InternalError(format!("Failed to acquire migration lock: {}", e))
            })?;

        info!("Migration lock acquired, running migrations...");

        // Read before any set runs: see `is_single_baseline_database`.
        let migration_result = (if sets.contains(&MigrationSet::App) {
            is_single_baseline_database(&mut conn)
        } else {
            Ok(false)
        })
        .and_then(|single_baseline| {
            sets.iter().try_fold(Vec::new(), |mut applied, &set| {
                applied.extend(run_migration_set_locked(&mut conn, set, single_baseline)?);
                Ok(applied)
            })
        });

        if let Err(e) =
            diesel::sql_query(format!("SELECT pg_advisory_unlock({MIGRATION_LOCK_KEY})"))
                .execute(&mut conn)
        {
            // The connection is about to be dropped either way, which also releases the lock,
            // so this is a log line rather than a returned error.
            tracing::warn!("Failed to release migration lock explicitly: {}", e);
        }

        let applied = migration_result?;
        info!("Migrations completed successfully");
        Ok(applied)
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Migration task failed: {}", e)))??;

    info!("Database migrations completed successfully");
    Ok(applied)
}

/// Applies one set's pending migrations on `conn`, which already holds [`MIGRATION_LOCK_KEY`],
/// and returns their versions. `single_baseline` is [`is_single_baseline_database`], read before
/// any set ran.
fn run_migration_set_locked(
    conn: &mut diesel::PgConnection,
    set: MigrationSet,
    single_baseline: bool,
) -> AppResult<Vec<String>> {
    use diesel_migrations::MigrationHarness;

    if set == MigrationSet::App && single_baseline {
        record_app_baseline_without_running_it(conn)?;
    }
    info!("Running the {} migrations...", set.name());
    conn.run_pending_migrations(set.migrations())
        .map(|applied| applied.iter().map(|v| v.to_string()).collect())
        .map_err(|e| {
            AppError::InternalError(format!(
                "Failed to run the {} migrations: {}",
                set.name(),
                e
            ))
        })
}

/// The tables the app baseline creates.
const APP_BASELINE_TABLES: [&str; 12] = [
    "users",
    "user_sessions",
    "chart_annotations",
    "annotation_comments",
    "chart_collaborators",
    "user_data_source_preferences",
    "audit_logs",
    "security_events",
    "financial_annotations",
    "annotation_assignments",
    "annotation_replies",
    "annotation_templates",
];

/// Whether this database was built by the single v4.0.0 baseline, before it was split into
/// `migrations/data` and `migrations/app`: the data baseline's version is recorded, the app
/// baseline's is not, and every app baseline table exists. Must be read before any set runs, so
/// a data baseline applied by this same run cannot satisfy it. Anything less (a stray `users`
/// table, say) is not recognised, and the app baseline's guard stops on it.
fn is_single_baseline_database(conn: &mut diesel::PgConnection) -> AppResult<bool> {
    use diesel::RunQueryDsl;

    #[derive(diesel::QueryableByName)]
    struct Answer {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        answer: bool,
    }
    let err = |e: diesel::result::Error| {
        AppError::InternalError(format!(
            "Failed to check for a single-baseline database: {}",
            e
        ))
    };

    // On an empty database the migrations table does not exist yet, and the query below would
    // fail on it, so look for it first.
    let has_table = diesel::sql_query(
        "SELECT to_regclass('public.__diesel_schema_migrations') IS NOT NULL AS answer",
    )
    .get_result::<Answer>(conn)
    .map_err(err)?;
    if !has_table.answer {
        return Ok(false);
    }

    let tables: Vec<String> = APP_BASELINE_TABLES.iter().map(|t| t.to_string()).collect();
    let single = diesel::sql_query(
        "SELECT EXISTS (SELECT 1 FROM __diesel_schema_migrations WHERE version = $1)
             AND NOT EXISTS (SELECT 1 FROM __diesel_schema_migrations WHERE version = $2)
             AND (SELECT bool_and(to_regclass(format('public.%I', t)) IS NOT NULL)
                  FROM unnest($3::text[]) AS t)
             AS answer",
    )
    .bind::<diesel::sql_types::Text, _>(DATA_BASELINE_VERSION)
    .bind::<diesel::sql_types::Text, _>(APP_BASELINE_VERSION)
    .bind::<diesel::sql_types::Array<diesel::sql_types::Text>, _>(tables)
    .get_result::<Answer>(conn)
    .map_err(err)?;
    Ok(single.answer)
}

/// Records the app baseline as applied without running it, on a database that
/// [`is_single_baseline_database`]: it already has the app tables (and their three foreign keys
/// to data tables, which stay; nothing depends on them being gone), so running it would stop at
/// its guard.
fn record_app_baseline_without_running_it(conn: &mut diesel::PgConnection) -> AppResult<()> {
    use diesel::RunQueryDsl;

    let inserted = diesel::sql_query(
        "INSERT INTO __diesel_schema_migrations (version) VALUES ($1) ON CONFLICT DO NOTHING",
    )
    .bind::<diesel::sql_types::Text, _>(APP_BASELINE_VERSION)
    .execute(conn)
    .map_err(|e| {
        AppError::InternalError(format!(
            "Failed to record the app baseline migration: {}",
            e
        ))
    })?;
    if inserted > 0 {
        tracing::warn!(
            "This database was built by the single v4.0.0 baseline migration and already has the app \
             tables; recorded the app baseline {} as applied without running it",
            APP_BASELINE_VERSION
        );
    }
    Ok(())
}

/// Execute an operation with a connection acquired from the database pool.
///
/// This helper does not begin, commit, or roll back a transaction. The caller is
/// responsible for transaction management within the supplied closure if needed.
pub async fn execute_with_connection<T, E, F, Fut>(pool: &DatabasePool, f: F) -> Result<T, E>
where
    F: FnOnce(&mut AsyncPgConnection) -> Fut + Send,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    T: Send,
    E: From<diesel::result::Error> + From<AppError> + Send,
{
    let mut conn = pool.get().await.map_err(|e| {
        let error_msg = format!("Failed to get database connection: {}", e);
        tracing::error!(
            "Database connection pool error in execute_with_connection: {}",
            error_msg
        );
        E::from(AppError::InternalError(error_msg))
    })?;

    // Execute the function with the dereferenced connection
    f(&mut *conn).await
}

/// Check database health
pub async fn check_database_health(pool: &DatabasePool) -> AppResult<()> {
    test_connection(pool).await
}

/// The migration version (diesel's zero-padded timestamp, e.g. `"20261021000100"`) of the
/// newest migration embedded in this binary for `set`: the schema version of that set it requires
/// at minimum to run correctly.
fn minimum_schema_version(set: MigrationSet) -> AppResult<String> {
    embedded_versions(set)?.into_iter().max().ok_or_else(|| {
        AppError::InternalError(format!("No embedded {} migrations found", set.name()))
    })
}

#[derive(diesel::QueryableByName)]
struct RecordedRow {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    recorded: bool,
}

/// The schema-version half of [`readiness_check`], run directly on `conn` rather than a pool so
/// a caller can run it inside its own transaction: for each set in `sets`, the newest migration
/// this binary embeds must be recorded in `__diesel_schema_migrations`. Not "the table's newest
/// version", which mixes the sets when one database holds both.
/// Presence rather than equality: a pod still running the previous binary, while a newer one is
/// mid-rollout migrating the database further ahead, stays ready, since its own newest versions
/// are still recorded.
async fn check_schema_version(
    conn: &mut AsyncPgConnection,
    sets: &[MigrationSet],
) -> AppResult<()> {
    for &set in sets {
        let minimum = minimum_schema_version(set)?;
        let row: RecordedRow = diesel_async::RunQueryDsl::get_result(
            diesel::sql_query(
                "SELECT EXISTS (SELECT 1 FROM __diesel_schema_migrations WHERE version = $1) \
                 AS recorded",
            )
            .bind::<diesel::sql_types::Text, _>(&minimum),
            conn,
        )
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to read schema version: {}", e)))?;

        if !row.recorded {
            return Err(AppError::DatabaseError(format!(
                "database has not applied this binary's newest {} migration {}",
                set.name(),
                minimum
            )));
        }
    }
    Ok(())
}

/// Readiness check for Kubernetes' `/readyz`: the database must be reachable and both migration
/// sets current; see [`check_schema_version`]. One database holds both sets.
pub async fn readiness_check(pool: &DatabasePool) -> AppResult<()> {
    readiness_check_sets(pool, &[MigrationSet::Data, MigrationSet::App]).await
}

/// Readiness of one set's database: reachable, and the newest migration this binary embeds for
/// `set` recorded. The other set's versions, if the database holds them too, are not looked at.
pub async fn readiness_check_set(pool: &DatabasePool, set: MigrationSet) -> AppResult<()> {
    readiness_check_sets(pool, &[set]).await
}

async fn readiness_check_sets(pool: &DatabasePool, sets: &[MigrationSet]) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(|e| {
        AppError::InternalError(format!("Failed to get database connection: {}", e))
    })?;
    check_schema_version(&mut conn, sets).await
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests now use TestContainer directly for better control

    #[test]
    fn redact_database_url_strips_credentials() {
        assert_eq!(
            redact_database_url("postgres://econgraph:s3cret@db.internal:5432/econ_graph"),
            "postgres://db.internal:5432/econ_graph"
        );
    }

    #[test]
    fn redact_database_url_handles_username_only() {
        assert_eq!(
            redact_database_url("postgres://econgraph@db.internal/econ_graph"),
            "postgres://db.internal/econ_graph"
        );
    }

    #[test]
    fn redact_database_url_falls_back_on_unparseable_input() {
        assert_eq!(
            redact_database_url("not a url"),
            "<unparseable database URL, redacted>"
        );
    }

    #[test]
    fn redact_database_url_strips_a_password_query_parameter() {
        assert_eq!(
            redact_database_url("postgresql://db.internal/econ_graph?password=s3cret"),
            "postgresql://db.internal/econ_graph"
        );
    }

    #[test]
    fn redact_database_url_strips_the_whole_query_string_when_mixed_with_other_params() {
        assert_eq!(
            redact_database_url("postgresql://db.internal/econ_graph?sslmode=require&password=x"),
            "postgresql://db.internal/econ_graph"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_database_connection() {
        // Test database connection functionality
        // REQUIREMENT: Database layer testing with testcontainers
        // PURPOSE: Verify database connectivity and basic operations work correctly

        let container = crate::test_utils::TestContainer::new().await;
        let pool = container.pool();

        // Test basic connectivity
        test_connection(pool)
            .await
            .expect("Database connection should work");

        // Test getting a connection from the pool
        let _conn = pool
            .get()
            .await
            .expect("Should be able to get connection from pool");
    }

    /// Proves `run_migrations` actually blocks on the advisory lock rather than racing ahead: it
    /// takes the lock itself on a separate connection, starts `run_migrations` in the background,
    /// and asserts the task is still pending while the lock is held and only completes once the
    /// lock is released. A test that instead just ran two `run_migrations` calls concurrently
    /// would pass even without the lock, since both would likely finish before either raced the
    /// other (few migrations, no contention to force a slow one to wait).
    ///
    /// Connects directly with `DATABASE_URL` (unlike the other tests here) because
    /// `TestContainer` always starts a throwaway Docker container even when pointed at an
    /// external database, and this test's environment has no Docker daemon.
    #[tokio::test]
    #[serial_test::serial]
    async fn test_migrations_block_while_another_session_holds_the_lock() {
        use diesel_async::RunQueryDsl;

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());

        let pool = create_pool(&database_url)
            .await
            .expect("Should connect to DATABASE_URL");

        // Hold the same lock `run_migrations` takes, on our own connection, so a concurrent
        // `run_migrations` call has to block on it rather than racing ahead.
        let mut lock_conn = pool.get().await.expect("Should get a connection");
        diesel::sql_query(format!("SELECT pg_advisory_lock({MIGRATION_LOCK_KEY})"))
            .execute(&mut lock_conn)
            .await
            .expect("Should acquire the lock on the holder connection");

        let url_for_task = database_url.clone();
        let migration_task = tokio::spawn(async move { run_migrations(&url_for_task).await });

        // Give the task time to reach (and block on) the lock.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !migration_task.is_finished(),
            "run_migrations should still be blocked on the advisory lock"
        );

        diesel::sql_query(format!("SELECT pg_advisory_unlock({MIGRATION_LOCK_KEY})"))
            .execute(&mut lock_conn)
            .await
            .expect("Should release the lock");

        tokio::time::timeout(Duration::from_secs(10), migration_task)
            .await
            .expect("run_migrations should complete soon after the lock is released")
            .expect("migration task should not panic")
            .expect("run_migrations should succeed once the lock is available");
    }

    /// `/readyz` must refuse a database it cannot reach, not hang or report ready.
    #[tokio::test]
    async fn readiness_check_fails_when_the_database_is_unreachable() {
        let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
            diesel_async::AsyncPgConnection,
        >::new("postgres://nobody@127.0.0.1:1/none");
        let pool = DatabasePool::builder()
            .connection_timeout(Duration::from_secs(1))
            .build_unchecked(manager);

        let err = readiness_check(&pool)
            .await
            .expect_err("an unreachable database should not be ready");
        assert!(matches!(err, AppError::InternalError(_)), "{err:?}");
    }

    /// A freshly migrated database is ready; one whose applied-migrations table is missing
    /// this binary's newest migration (an older schema than the binary requires) is not,
    /// which is exactly the case a rolling deploy must catch before routing traffic to a pod
    /// whose database hasn't caught up yet.
    ///
    /// Connects directly with `DATABASE_URL`, like `test_migrations_block_while_another_session_holds_the_lock`
    /// above, since this environment has no Docker daemon for `TestContainer`.
    #[tokio::test]
    #[serial_test::serial]
    async fn readiness_check_rejects_a_schema_older_than_this_binarys_minimum() {
        use diesel_async::RunQueryDsl;

        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());

        run_migrations(&database_url)
            .await
            .expect("migrations should apply");

        let pool = create_pool(&database_url)
            .await
            .expect("Should connect to DATABASE_URL");

        readiness_check(&pool)
            .await
            .expect("a freshly migrated schema should be ready");

        // Each set on its own: with both sets in one database, a set behind is caught even
        // though the other set's newer version is recorded.
        for set in [MigrationSet::Data, MigrationSet::App] {
            let minimum = minimum_schema_version(set).expect("should read embedded migrations");

            // Delete the set's newest migration row inside an uncommitted transaction, never
            // committed or restored by hand: even a cancelled test run (its pooled connection
            // dropped mid-transaction) leaves this database, which other tests and local dev
            // setups share, completely unchanged, since Postgres rolls back an open transaction
            // when its connection closes. Nothing between BEGIN and ROLLBACK unwraps or asserts: a
            // pooled connection goes back to the pool on drop without closing it, so a panic while
            // the transaction is still open would hand a later pool user a connection sitting
            // inside it, with the row still deleted.
            let mut conn = pool.get().await.expect("Should get a connection");
            diesel::sql_query("BEGIN")
                .execute(&mut conn)
                .await
                .expect("should start a transaction");

            let delete_result =
                diesel::sql_query("DELETE FROM __diesel_schema_migrations WHERE version = $1")
                    .bind::<diesel::sql_types::Text, _>(minimum)
                    .execute(&mut conn)
                    .await;
            let check_result =
                check_schema_version(&mut conn, &[MigrationSet::Data, MigrationSet::App]).await;

            diesel::sql_query("ROLLBACK")
                .execute(&mut conn)
                .await
                .expect("should roll back, undoing the delete regardless of what happened above");

            assert_eq!(
                delete_result.expect("should delete the latest migration row"),
                1,
                "the {set:?} set's newest migration should have been recorded"
            );
            let err = check_result.expect_err(
                "a schema missing this binary's newest migration of either set should not be ready",
            );
            assert!(matches!(err, AppError::DatabaseError(_)), "{err:?}");
        }

        readiness_check(&pool)
            .await
            .expect("the untouched schema should still be ready after the rollback");
    }

    /// The two baselines are permanent versions (docs/development/MIGRATIONS.md): databases have
    /// them recorded, and `run_migrations` relies on both to recognise a single-baseline
    /// database. A squash, renumbering or removal of either must fail here.
    #[test]
    fn each_set_embeds_its_permanent_baseline_version() {
        use diesel::migration::{Migration, MigrationSource};

        let versions = |set: MigrationSet| -> Vec<String> {
            MigrationSource::<diesel::pg::Pg>::migrations(&set.migrations())
                .expect("should read embedded migrations")
                .iter()
                .map(|m| m.name().version().to_string())
                .collect()
        };
        let app = versions(MigrationSet::App);
        let data = versions(MigrationSet::Data);
        assert!(app.iter().any(|v| v == APP_BASELINE_VERSION), "{app:?}");
        assert!(data.iter().any(|v| v == DATA_BASELINE_VERSION), "{data:?}");
        assert_eq!(APP_BASELINE_VERSION, "20261021000200");
        assert_eq!(DATA_BASELINE_VERSION, "20261021000100");
    }

    /// Both sets record into one `__diesel_schema_migrations` table when one database holds
    /// both, so a version in both sets would be one migration to Diesel and one of them would
    /// never run (docs/development/MIGRATIONS.md, rule 4). Only `00000000000000` is in both, and
    /// each release/v4.0 migration listed below is in exactly the set its list names.
    #[test]
    fn embedded_versions_are_unique_across_both_sets() {
        let app = super::embedded_versions(MigrationSet::App).expect("app versions");
        let data = super::embedded_versions(MigrationSet::Data).expect("data versions");
        for (set, versions) in [("app", &app), ("data", &data)] {
            let mut unique = versions.clone();
            unique.sort();
            unique.dedup();
            assert_eq!(
                unique.len(),
                versions.len(),
                "duplicate {set} version: {versions:?}"
            );
        }
        let shared: Vec<&String> = app.iter().filter(|v| data.contains(v)).collect();
        assert_eq!(shared, ["00000000000000"]);

        for (version, _) in RELEASE_V4_0_APP_MIGRATIONS {
            assert!(
                app.iter().any(|v| v == version),
                "{version} not in the app set"
            );
            assert!(
                !data.iter().any(|v| v == version),
                "{version} in the data set"
            );
        }
        for (version, _) in RELEASE_V4_0_DATA_MIGRATIONS {
            assert!(
                data.iter().any(|v| v == version),
                "{version} not in the data set"
            );
            assert!(
                !app.iter().any(|v| v == version),
                "{version} in the app set"
            );
        }
    }

    use crate::test_utils::ScratchDatabase;

    #[derive(diesel::QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    /// Runs `query`, which selects one text column named `value`, and returns its rows.
    fn text_rows(conn: &mut diesel::PgConnection, query: &str) -> Vec<String> {
        use diesel::RunQueryDsl;
        diesel::sql_query(query)
            .load::<TextRow>(conn)
            .expect("query should run")
            .into_iter()
            .map(|r| r.value)
            .collect()
    }

    /// The versions `sets` embed, sorted and deduplicated: what a database those sets fully
    /// migrated records. Derived from the embedded migrations, so adding a migration to a set
    /// doesn't need these tests changed; the baselines themselves are pinned by
    /// `each_set_embeds_its_permanent_baseline_version`.
    fn embedded_versions(sets: &[MigrationSet]) -> Vec<String> {
        use diesel::migration::{Migration, MigrationSource};

        let mut versions: Vec<String> = sets
            .iter()
            .flat_map(|set| {
                MigrationSource::<diesel::pg::Pg>::migrations(&set.migrations())
                    .expect("should read embedded migrations")
                    .iter()
                    .map(|m| m.name().version().to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        versions.sort();
        versions.dedup();
        versions
    }

    fn recorded_versions(conn: &mut diesel::PgConnection) -> Vec<String> {
        text_rows(
            conn,
            "SELECT version AS value FROM __diesel_schema_migrations ORDER BY version",
        )
    }

    fn public_tables(conn: &mut diesel::PgConnection) -> Vec<String> {
        text_rows(
            conn,
            "SELECT tablename::text AS value FROM pg_tables
             WHERE schemaname = 'public' AND tablename <> '__diesel_schema_migrations'
             ORDER BY tablename",
        )
    }

    /// The tables of each set, as listed in docs/development/MIGRATIONS.md.
    const DATA_TABLES: [&str; 33] = [
        "data_sources",
        "datasets",
        "economic_series",
        "data_points",
        "crawl_queue",
        "crawl_attempts",
        "series_metadata",
        "reference_file_cache",
        "series_fetch_validators",
        "iceberg_catalog",
        "iceberg_commits",
        "timeseries_recovery_state",
        "timeseries_recovery_transitions",
        "countries",
        "global_economic_indicators",
        "global_indicator_data",
        "country_correlations",
        "trade_relationships",
        "global_economic_events",
        "event_country_impacts",
        "leading_indicators",
        "companies",
        "company_facts",
        "company_fact_filings",
        "company_fact_load_runs",
        "financial_statements",
        "financial_line_items",
        "financial_ratios",
        "xbrl_taxonomy_schemas",
        "xbrl_taxonomy_linkbases",
        "xbrl_dts_dependencies",
        "xbrl_instance_dts_references",
        "xbrl_taxonomy_concepts",
    ];
    const APP_TABLES: [&str; 12] = [
        "users",
        "user_sessions",
        "chart_annotations",
        "annotation_comments",
        "chart_collaborators",
        "user_data_source_preferences",
        "audit_logs",
        "security_events",
        "financial_annotations",
        "annotation_assignments",
        "annotation_replies",
        "annotation_templates",
    ];

    fn sorted(tables: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = tables.iter().map(|t| t.to_string()).collect();
        v.sort();
        v
    }

    /// A new database gets both sets, data then app, and records every embedded version.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_applies_both_sets_to_a_new_database() {
        let db = ScratchDatabase::create("both");
        run_migrations(&db.url)
            .await
            .expect("migrations should apply to a new database");

        let mut conn = db.connect();
        let expected = embedded_versions(&[MigrationSet::Data, MigrationSet::App]);
        assert!(expected.iter().any(|v| v == DATA_BASELINE_VERSION));
        assert!(expected.iter().any(|v| v == APP_BASELINE_VERSION));
        assert_eq!(recorded_versions(&mut conn), expected);
        let mut all = DATA_TABLES.to_vec();
        all.extend(APP_TABLES);
        assert_eq!(public_tables(&mut conn), sorted(&all));
    }

    /// Each set builds on an empty database by itself and creates only its own tables, so the
    /// app and data sets can live in separate databases.
    #[tokio::test]
    #[serial_test::serial]
    async fn each_migration_set_builds_only_its_own_tables_in_its_own_database() {
        let data_db = ScratchDatabase::create("data_only");
        let app_db = ScratchDatabase::create("app_only");

        run_migration_set(&data_db.url, MigrationSet::Data)
            .await
            .expect("the data set should apply to an empty database");
        run_migration_set(&app_db.url, MigrationSet::App)
            .await
            .expect("the app set should apply to an empty database");

        let mut data_conn = data_db.connect();
        assert_eq!(public_tables(&mut data_conn), sorted(&DATA_TABLES));
        assert_eq!(
            recorded_versions(&mut data_conn),
            embedded_versions(&[MigrationSet::Data])
        );

        let mut app_conn = app_db.connect();
        assert_eq!(public_tables(&mut app_conn), sorted(&APP_TABLES));
        assert_eq!(
            recorded_versions(&mut app_conn),
            embedded_versions(&[MigrationSet::App])
        );

        // Each database is ready for its own set and not for the other one.
        let data_pool = DataPool(create_pool(&data_db.url).await.expect("pool"));
        readiness_check_set(&data_pool, MigrationSet::Data)
            .await
            .expect("the data database should be current for the data set");
        assert!(readiness_check_set(&data_pool, MigrationSet::App)
            .await
            .is_err());
        let app_pool = AppPool(create_pool(&app_db.url).await.expect("pool"));
        readiness_check_set(&app_pool, MigrationSet::App)
            .await
            .expect("the app database should be current for the app set");
        assert!(readiness_check_set(&app_pool, MigrationSet::Data)
            .await
            .is_err());
    }

    /// One database holding both sets, applied one set at a time: applying the app set leaves the
    /// data set pending (and not ready) rather than touching it, and each set's readiness looks
    /// only at that set's newest version.
    #[tokio::test]
    #[serial_test::serial]
    async fn readiness_is_per_set_when_one_database_holds_both() {
        let db = ScratchDatabase::create("per_set");
        let pool = create_pool(&db.url).await.expect("pool");

        run_migration_set(&db.url, MigrationSet::App)
            .await
            .expect("the app set should apply to an empty database");
        readiness_check_set(&pool, MigrationSet::App)
            .await
            .expect("the app set should be current");
        let err = readiness_check_set(&pool, MigrationSet::Data)
            .await
            .expect_err("the data set has not been applied");
        assert!(err.to_string().contains("newest data migration"), "{err}");
        assert!(readiness_check(&pool).await.is_err());
        let mut conn = db.connect();
        assert_eq!(public_tables(&mut conn), sorted(&APP_TABLES));

        run_migration_set(&db.url, MigrationSet::Data)
            .await
            .expect("the data set should apply next to the app set");
        for set in [MigrationSet::Data, MigrationSet::App] {
            readiness_check_set(&pool, set)
                .await
                .unwrap_or_else(|e| panic!("the {set:?} set should be current: {e}"));
        }
        readiness_check(&pool)
            .await
            .expect("both sets should be current");
        let expected = embedded_versions(&[MigrationSet::Data, MigrationSet::App]);
        assert_eq!(recorded_versions(&mut conn), expected);
        let mut all = DATA_TABLES.to_vec();
        all.extend(APP_TABLES);
        assert_eq!(public_tables(&mut conn), sorted(&all));

        // A second run of each set changes nothing.
        run_migration_set(&db.url, MigrationSet::App)
            .await
            .expect("the app set should be a no-op once current");
        run_migration_set(&db.url, MigrationSet::Data)
            .await
            .expect("the data set should be a no-op once current");
        assert_eq!(recorded_versions(&mut conn), expected);
    }

    /// Every column, constraint, index and trigger in `public`, one line each, sorted.
    fn schema_signature(conn: &mut diesel::PgConnection) -> Vec<String> {
        text_rows(
            conn,
            "SELECT line AS value FROM (
                 SELECT format('column %s.%s %s %s %s', c.relname, a.attname,
                               format_type(a.atttypid, a.atttypmod), a.attnotnull,
                               pg_get_expr(d.adbin, d.adrelid)) AS line
                 FROM pg_attribute a
                 JOIN pg_class c ON c.oid = a.attrelid
                 LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
                 WHERE c.relnamespace = 'public'::regnamespace AND a.attnum > 0
                     AND NOT a.attisdropped AND c.relname <> '__diesel_schema_migrations'
                 UNION ALL
                 SELECT format('constraint %s.%s %s', conrelid::regclass, conname,
                               pg_get_constraintdef(oid))
                 FROM pg_constraint
                 WHERE connamespace = 'public'::regnamespace
                     AND conrelid <> '__diesel_schema_migrations'::regclass
                 UNION ALL
                 SELECT format('index %s', indexdef) FROM pg_indexes
                 WHERE schemaname = 'public' AND tablename <> '__diesel_schema_migrations'
                 UNION ALL
                 SELECT format('trigger %s', pg_get_triggerdef(oid)) FROM pg_trigger
                 WHERE NOT tgisinternal
                 UNION ALL
                 SELECT format('type %s', typname) FROM pg_type
                 WHERE typnamespace = 'public'::regnamespace
             ) t
             ORDER BY line",
        )
    }

    /// Migrations that release/v4.0 ships after its single baseline, as (version, up.sql), one
    /// list per set. release/v4.0 keeps every migration in one directory; these are the same
    /// migrations, with the same versions and byte-identical up.sql, in the set that owns their
    /// tables. A database migrated on release/v4.0 has all of them applied and recorded. Each
    /// forward-port of such a migration adds one entry to the list for its set.
    const RELEASE_V4_0_APP_MIGRATIONS: &[(&str, &str)] = &[(
        "20261024000100",
        include_str!(
            "../../../migrations/app/2026-10-24-000100_audit_logs_keep_on_user_delete/up.sql"
        ),
    )];

    /// See [`RELEASE_V4_0_APP_MIGRATIONS`].
    const RELEASE_V4_0_DATA_MIGRATIONS: &[(&str, &str)] = &[(
        "20261025000100",
        include_str!(
            "../../../migrations/data/2026-10-25-000100_observation_values_unconstrained_numeric/up.sql"
        ),
    )];

    /// The release/v4.0 data migration that widens the observation values (ECO-501).
    const OBSERVATION_NUMERIC_VERSION: &str = "20261025000100";

    /// New data-plane incrementals after the pinned release/v4.0 fixture. Keep this explicit:
    /// deriving it from the embedded migrations would hide an unexpected upgrade path.
    const POST_V4_0_DATA_VERSIONS: [&str; 5] = [
        "20261031010000", // nullable observation attributes
        "20261031020000", // Iceberg SQL catalog and commit receipts
        "20261031040000", // durable recovery target and write fence
        "20261031120000", // source-preserving SEC companyfacts evidence
        "20261031120100", // complete bulk-pair load provenance
    ];

    /// Rebuilds a database created by the single v4.0.0 baseline, before the split, from its two
    /// halves: the data set through its baseline (no later data migration) through Diesel, and
    /// the app baseline's SQL run directly so its version is not recorded. It also adds the
    /// three foreign keys the split dropped, under their original names.
    /// scripts/compare_migrations.sh checks that the halves are the single baseline less those
    /// keys.
    async fn single_baseline_database(name: &str) -> (ScratchDatabase, diesel::PgConnection) {
        use diesel::connection::SimpleConnection;
        use diesel_migrations::MigrationHarness;

        let db = ScratchDatabase::create(name);
        let mut conn = db.connect();
        let pending = conn
            .pending_migrations(MigrationSet::Data.migrations())
            .expect("should read the data migrations");
        for migration in pending {
            if migration.name().version().to_string().as_str() <= DATA_BASELINE_VERSION {
                conn.run_migration(&migration)
                    .expect("the data set through its baseline should apply");
            }
        }
        conn.batch_execute(include_str!(
            "../../../migrations/app/2026-10-21-000200_v4_0_app_baseline/up.sql"
        ))
        .expect("the app baseline's SQL should run");
        conn.batch_execute(
            "ALTER TABLE user_data_source_preferences
                 ADD CONSTRAINT user_data_source_preferences_data_source_id_fkey
                 FOREIGN KEY (data_source_id) REFERENCES data_sources(id) ON DELETE CASCADE;
             ALTER TABLE financial_annotations
                 ADD CONSTRAINT financial_annotations_statement_id_fkey
                 FOREIGN KEY (statement_id) REFERENCES financial_statements(id) ON DELETE CASCADE,
                 ADD CONSTRAINT financial_annotations_line_item_id_fkey
                 FOREIGN KEY (line_item_id) REFERENCES financial_line_items(id) ON DELETE CASCADE;",
        )
        .expect("the single baseline's cross-plane foreign keys should be added");
        assert_eq!(
            recorded_versions(&mut conn),
            ["00000000000000", DATA_BASELINE_VERSION],
            "a single-baseline database has only the data baseline's version"
        );
        assert!(schema_signature(&mut conn)
            .iter()
            .any(|l| l.contains("financial_annotations_statement_id_fkey")));
        (db, conn)
    }

    /// Applies and records `RELEASE_V4_0_APP_MIGRATIONS` and, unless `with_data` is false,
    /// `RELEASE_V4_0_DATA_MIGRATIONS`, merged in version order, as release/v4.0 does: each in
    /// its own transaction, as Diesel runs them. `with_data: false` is a release candidate from
    /// before the data migrations.
    fn apply_release_v4_0_migrations(conn: &mut diesel::PgConnection, with_data: bool) {
        use diesel::connection::SimpleConnection;
        use diesel::Connection;

        let mut migrations: Vec<(&str, &str)> = RELEASE_V4_0_APP_MIGRATIONS.to_vec();
        if with_data {
            migrations.extend_from_slice(RELEASE_V4_0_DATA_MIGRATIONS);
        }
        migrations.sort_by_key(|(version, _)| *version);
        for (version, up) in migrations {
            conn.transaction::<_, diesel::result::Error, _>(|conn| {
                conn.batch_execute(up)?;
                conn.batch_execute(&format!(
                    "INSERT INTO __diesel_schema_migrations (version) VALUES ('{version}')"
                ))
            })
            .unwrap_or_else(|e| panic!("release migration {version} should apply: {e}"));
        }
    }

    /// The app versions this binary embeds that a release/v4.0 database lacks, oldest first:
    /// what the app set applies to it (the app baseline is recorded without running, so it is
    /// not among them).
    fn app_versions_missing_from_release_v4_0() -> Vec<String> {
        embedded_versions(&[MigrationSet::App])
            .into_iter()
            .filter(|v| v.as_str() > APP_BASELINE_VERSION)
            .filter(|v| !RELEASE_V4_0_APP_MIGRATIONS.iter().any(|(r, _)| r == v))
            .collect()
    }

    /// Bring a reference schema forward by running only pending app incrementals.
    /// Its app tables already exist, so neither run nor record the split app baseline:
    /// this independently checks the production baseline-recording path's result.
    fn apply_pending_app_incrementals(conn: &mut diesel::PgConnection) {
        use diesel_migrations::MigrationHarness;

        let pending = conn
            .pending_migrations(MigrationSet::App.migrations())
            .expect("should read pending app migrations");
        for migration in pending {
            if migration.name().version().to_string().as_str() > APP_BASELINE_VERSION {
                conn.run_migration(&migration)
                    .expect("pending app incremental should run");
            }
        }
        assert!(
            !recorded_versions(conn).contains(&APP_BASELINE_VERSION.to_string()),
            "the reference must not record or run the split app baseline"
        );
    }

    /// Runs the data set's pending migrations on a reference schema, through Diesel.
    fn apply_pending_data_migrations(conn: &mut diesel::PgConnection) {
        use diesel_migrations::MigrationHarness;

        conn.run_pending_migrations(MigrationSet::Data.migrations())
            .expect("pending data migrations should run");
    }

    /// A single-baseline database without the later migrations (a release candidate from
    /// before them): the app baseline is recorded, not run, and the later app and data
    /// migrations run. The schema ends up exactly as the single baseline plus those migrations.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_records_the_app_baseline_and_applies_later_app_migrations() {
        let (db, mut conn) = single_baseline_database("single_baseline_rc").await;
        let (_reference, mut reference_conn) =
            single_baseline_database("single_baseline_rc_reference").await;
        apply_pending_data_migrations(&mut reference_conn);
        apply_pending_app_incrementals(&mut reference_conn);
        let before = schema_signature(&mut conn);
        let expected_schema = schema_signature(&mut reference_conn);
        assert_ne!(
            before, expected_schema,
            "the later data and app migrations should change the schema"
        );

        run_migrations(&db.url)
            .await
            .expect("migrations should succeed on a single-baseline database");

        let expected = embedded_versions(&[MigrationSet::Data, MigrationSet::App]);
        assert!(expected.iter().any(|v| v == APP_BASELINE_VERSION));
        assert_eq!(recorded_versions(&mut conn), expected);
        assert_eq!(
            schema_signature(&mut conn),
            expected_schema,
            "only later data and app incrementals should change the schema; the app baseline is recorded, not run"
        );

        run_migrations(&db.url)
            .await
            .expect("a second run should succeed");
        assert_eq!(schema_signature(&mut conn), expected_schema);
    }

    /// A database migrated on release/v4.0 (the single baseline plus its later app migrations,
    /// applied and recorded) receives pending data and app incrementals. The app baseline is
    /// recorded, not run; the resulting schema matches independently applied incrementals.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_records_the_app_baseline_on_a_single_baseline_database() {
        let (db, mut conn) = single_baseline_database("single_baseline").await;
        apply_release_v4_0_migrations(&mut conn, true);
        let (_reference, mut reference_conn) =
            single_baseline_database("single_baseline_reference").await;
        apply_release_v4_0_migrations(&mut reference_conn, true);
        apply_pending_data_migrations(&mut reference_conn);
        apply_pending_app_incrementals(&mut reference_conn);
        let expected_schema = schema_signature(&mut reference_conn);

        run_migrations(&db.url)
            .await
            .expect("migrations should succeed on a release/v4.0 database");

        let expected = embedded_versions(&[MigrationSet::Data, MigrationSet::App]);
        assert!(expected.iter().any(|v| v == DATA_BASELINE_VERSION));
        assert!(expected.iter().any(|v| v == APP_BASELINE_VERSION));
        assert_eq!(recorded_versions(&mut conn), expected);
        assert_eq!(
            schema_signature(&mut conn),
            expected_schema,
            "only pending data and app incrementals should change the release schema"
        );

        run_migrations(&db.url)
            .await
            .expect("a second run should succeed");
        assert_eq!(schema_signature(&mut conn), expected_schema);
    }

    /// A release/v4.0 database already records its numeric-widening migration, but the
    /// five Train 4/5 data incrementals are pending and must block data-plane readiness.
    /// Applying the data set runs exactly those migrations, then applying the app set
    /// records its split baseline without replaying it or applying any data migration.
    /// Both results match independent forward migration of the unchanged release fixture.
    #[tokio::test]
    #[serial_test::serial]
    async fn a_release_v4_0_database_applies_pending_data_incrementals_before_becoming_ready() {
        let (db, mut conn) = single_baseline_database("release_v4_0").await;
        apply_release_v4_0_migrations(&mut conn, true);
        let (_reference, mut reference_conn) =
            single_baseline_database("release_v4_0_reference").await;
        apply_release_v4_0_migrations(&mut reference_conn, true);
        apply_pending_data_migrations(&mut reference_conn);
        apply_pending_app_incrementals(&mut reference_conn);
        let expected_schema = schema_signature(&mut reference_conn);
        let pool = create_pool(&db.url).await.expect("pool");

        assert_eq!(
            pending_migration_versions(&db.url, MigrationSet::Data)
                .await
                .expect("should read the pending data migrations"),
            POST_V4_0_DATA_VERSIONS
        );
        let err = readiness_check_set(&pool, MigrationSet::Data)
            .await
            .expect_err("the release schema must not serve a newer data-plane binary");
        assert!(err.to_string().contains("20261031120100"), "{err}");
        assert_eq!(
            run_migration_set(&db.url, MigrationSet::Data)
                .await
                .expect("the data set should apply its pending incrementals"),
            POST_V4_0_DATA_VERSIONS
        );
        readiness_check_set(&pool, MigrationSet::Data)
            .await
            .expect("the upgraded data set should now be ready");
        assert_eq!(
            run_migration_set(&db.url, MigrationSet::Data)
                .await
                .expect("the second data migration run should be a no-op"),
            Vec::<String>::new()
        );

        let applied = run_migration_set(&db.url, MigrationSet::App)
            .await
            .expect("the app set should apply to a release/v4.0 database");
        assert_eq!(applied, app_versions_missing_from_release_v4_0());
        assert!(!applied.contains(&OBSERVATION_NUMERIC_VERSION.to_string()));
        assert!(applied
            .iter()
            .all(|version| !POST_V4_0_DATA_VERSIONS.contains(&version.as_str())));
        assert!(recorded_versions(&mut conn).contains(&APP_BASELINE_VERSION.to_string()));
        assert_eq!(
            recorded_versions(&mut conn),
            embedded_versions(&[MigrationSet::Data, MigrationSet::App])
        );
        assert_eq!(schema_signature(&mut conn), expected_schema);
    }

    /// A release candidate from before ECO-501 (the single baseline plus the release app
    /// migration only): the data set applies numeric widening followed by the five
    /// Train 4/5 incrementals. Applying the app set first leaves all six pending and never
    /// records them. Both sets end at the independently forward-migrated reference schema.
    #[tokio::test]
    #[serial_test::serial]
    async fn the_data_set_applies_the_observation_numeric_migration_to_an_older_release() {
        let (db, mut conn) = single_baseline_database("pre_eco_501").await;
        apply_release_v4_0_migrations(&mut conn, false);
        let (_reference, mut reference_conn) =
            single_baseline_database("pre_eco_501_reference").await;
        apply_release_v4_0_migrations(&mut reference_conn, true);
        apply_pending_data_migrations(&mut reference_conn);
        apply_pending_app_incrementals(&mut reference_conn);
        let expected_schema = schema_signature(&mut reference_conn);
        let expected_data = [
            OBSERVATION_NUMERIC_VERSION,
            "20261031010000",
            "20261031020000",
            "20261031040000",
            "20261031120000",
            "20261031120100",
        ];

        assert_eq!(
            pending_migration_versions(&db.url, MigrationSet::Data)
                .await
                .expect("should read the pending data migrations"),
            expected_data
        );
        assert!(!pending_migration_versions(&db.url, MigrationSet::App)
            .await
            .expect("should read the pending app migrations")
            .contains(&OBSERVATION_NUMERIC_VERSION.to_string()));

        let applied = run_migration_set(&db.url, MigrationSet::App)
            .await
            .expect("the app set should apply");
        assert_eq!(applied, app_versions_missing_from_release_v4_0());
        let after_app = recorded_versions(&mut conn);
        assert!(
            expected_data
                .iter()
                .all(|version| !after_app.iter().any(|recorded| recorded == version)),
            "the app set must not record any pending data migration"
        );
        assert_eq!(
            pending_migration_versions(&db.url, MigrationSet::Data)
                .await
                .expect("should read the pending data migrations"),
            expected_data,
            "applying the app set leaves every data incremental pending"
        );

        assert_eq!(
            run_migration_set(&db.url, MigrationSet::Data)
                .await
                .expect("the data set should apply"),
            expected_data
        );
        assert_eq!(
            recorded_versions(&mut conn),
            embedded_versions(&[MigrationSet::Data, MigrationSet::App])
        );
        assert_eq!(schema_signature(&mut conn), expected_schema);
    }

    /// In a database holding both sets, a version recorded for one set is neither pending nor an
    /// unknown version for the other: removing the data set's newest record leaves the app set
    /// current, ready and with nothing to apply, and only the data set pending and not ready.
    #[tokio::test]
    #[serial_test::serial]
    async fn a_version_of_one_set_is_not_pending_or_unknown_for_the_other() {
        use diesel::connection::SimpleConnection;

        let db = ScratchDatabase::create("cross_set");
        run_migrations(&db.url)
            .await
            .expect("both sets should apply");
        let pool = create_pool(&db.url).await.expect("pool");
        let mut conn = db.connect();

        for set in [MigrationSet::Data, MigrationSet::App] {
            assert_eq!(
                pending_migration_versions(&db.url, set)
                    .await
                    .expect("should read pending migrations"),
                Vec::<String>::new(),
                "{set:?}"
            );
            readiness_check_set(&pool, set)
                .await
                .unwrap_or_else(|e| panic!("the {set:?} set should be ready: {e}"));
            assert_eq!(
                run_migration_set(&db.url, set)
                    .await
                    .unwrap_or_else(|e| panic!("the {set:?} set should be current: {e}")),
                Vec::<String>::new(),
                "the other set's recorded versions are not unknown to {set:?}"
            );
        }

        let newest_data = minimum_schema_version(MigrationSet::Data).expect("embedded versions");
        assert_eq!(newest_data, "20261031120100");
        conn.batch_execute(&format!(
            "DELETE FROM __diesel_schema_migrations WHERE version = '{newest_data}'"
        ))
        .expect("should delete the data set's newest record");

        assert_eq!(
            pending_migration_versions(&db.url, MigrationSet::Data)
                .await
                .expect("should read pending data migrations"),
            [newest_data.as_str()]
        );
        assert_eq!(
            pending_migration_versions(&db.url, MigrationSet::App)
                .await
                .expect("should read pending app migrations"),
            Vec::<String>::new()
        );
        readiness_check_set(&pool, MigrationSet::App)
            .await
            .expect("the app set should stay ready");
        assert!(readiness_check_set(&pool, MigrationSet::Data)
            .await
            .is_err());
        assert_eq!(
            run_migration_set(&db.url, MigrationSet::App)
                .await
                .expect("the app set should be current"),
            Vec::<String>::new()
        );
        assert!(!recorded_versions(&mut conn).contains(&newest_data));
    }

    /// A database with a stray `users` table and no migrations is not a single-baseline database,
    /// even though the data baseline this same run applies records `20261021000100`: the app
    /// baseline is not recorded without running, so its guard stops the run, and the other 11 app
    /// tables are never silently missing.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_does_not_take_a_stray_users_table_for_a_single_baseline_database() {
        use diesel::connection::SimpleConnection;

        let db = ScratchDatabase::create("stray_users");
        let mut conn = db.connect();
        conn.batch_execute("CREATE TABLE users (id UUID PRIMARY KEY)")
            .expect("should create a stray users table");

        let err = run_migrations(&db.url)
            .await
            .expect_err("the app baseline's guard should stop on the stray users table");
        assert!(
            err.to_string().contains("already has the app tables"),
            "{err}"
        );
        assert!(
            !recorded_versions(&mut conn).contains(&APP_BASELINE_VERSION.to_string()),
            "the app baseline must not be recorded"
        );
    }

    /// The app baseline's guard stops it on a database that already has the app tables but not
    /// the app baseline recorded, when it is run some way other than `run_migrations`.
    #[tokio::test]
    #[serial_test::serial]
    async fn the_app_baseline_guard_stops_on_existing_app_tables() {
        use diesel::connection::SimpleConnection;

        let db = ScratchDatabase::create("app_guard");
        let mut conn = db.connect();
        conn.batch_execute("CREATE TABLE users (id UUID PRIMARY KEY)")
            .expect("should create a users table");
        let err = conn
            .batch_execute(include_str!(
                "../../../migrations/app/2026-10-21-000200_v4_0_app_baseline/up.sql"
            ))
            .expect_err("the guard should stop the app baseline");
        assert!(
            err.to_string().contains("already has the app tables"),
            "{err}"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_basic_query() {
        // Test basic query functionality
        // REQUIREMENT: Database query testing
        // PURPOSE: Verify database queries work correctly with async connections

        let container = crate::test_utils::TestContainer::new().await;
        let pool = container.pool();
        let mut conn = pool
            .get()
            .await
            .expect("Should be able to get connection from pool");

        use diesel_async::RunQueryDsl;
        let result: i32 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1 + 1"))
            .get_result(&mut conn)
            .await
            .expect("Query should work");

        assert_eq!(result, 2);
    }
}
