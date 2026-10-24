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

/// Create a database connection pool
pub async fn create_pool(database_url: &str) -> AppResult<DatabasePool> {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);

    let pool = Pool::builder()
        .max_size(20)
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
const MIGRATION_LOCK_KEY: i64 = 0x6567_5f6d_6967_7261; // "eg_migra" as bytes

/// The app migration set: users and everything they create (docs/development/TRAIN2_CONTRACTS.md,
/// sections 4 and 5).
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
/// Until DP-2 gives the backend separate app and data URLs, every caller has one database holding
/// both sets (TRAIN2_CONTRACTS.md, section 4). A database built by the single v4.0.0 baseline,
/// before the split, already has the app tables; the app baseline is recorded there without
/// running (see [`record_app_baseline_on_a_single_baseline_database`]).
///
/// Note: Migrations require a synchronous connection
pub async fn run_migrations(database_url: &str) -> AppResult<()> {
    run_migration_sets(database_url, &[MigrationSet::Data, MigrationSet::App]).await
}

/// Runs `sets` in order against `database_url`, holding [`MIGRATION_LOCK_KEY`] for the whole run.
pub(crate) async fn run_migration_sets(database_url: &str, sets: &[MigrationSet]) -> AppResult<()> {
    use diesel::{Connection, RunQueryDsl};

    let sets = sets.to_vec();
    // Run migrations in a blocking task since migrations are sync
    let database_url = database_url.to_string();
    tokio::task::spawn_blocking(move || -> AppResult<()> {
        // Ensure the database URL is properly formatted
        let formatted_url = if database_url.starts_with("postgresql://") {
            database_url
        } else if database_url.starts_with("postgres://") {
            database_url.replace("postgres://", "postgresql://")
        } else {
            format!("postgresql://{}", database_url)
        };

        info!(
            "Attempting to connect to database for migrations: {}",
            redact_database_url(&formatted_url)
        );

        // Try to establish connection
        let mut conn = diesel::PgConnection::establish(&formatted_url).map_err(|e| {
            let error = AppError::InternalError(format!(
                "Failed to establish sync connection for migrations: {}",
                e
            ));
            error.log_with_context("Database migration connection attempt");
            error
        })?;

        info!("Database connection established, waiting for migration lock...");

        // Blocks here until whichever replica got there first releases the lock (or crashes /
        // disconnects, which releases it automatically since it's session-scoped).
        diesel::sql_query(format!("SELECT pg_advisory_lock({MIGRATION_LOCK_KEY})"))
            .execute(&mut conn)
            .map_err(|e| {
                AppError::InternalError(format!("Failed to acquire migration lock: {}", e))
            })?;

        info!("Migration lock acquired, running migrations...");

        let migration_result = sets
            .iter()
            .try_for_each(|&set| run_migration_set_locked(&mut conn, set));

        if let Err(e) =
            diesel::sql_query(format!("SELECT pg_advisory_unlock({MIGRATION_LOCK_KEY})"))
                .execute(&mut conn)
        {
            // The connection is about to be dropped either way, which also releases the lock,
            // so this is a log line rather than a returned error.
            tracing::warn!("Failed to release migration lock explicitly: {}", e);
        }

        migration_result?;
        info!("Migrations completed successfully");
        Ok(())
    })
    .await
    .map_err(|e| AppError::InternalError(format!("Migration task failed: {}", e)))??;

    info!("Database migrations completed successfully");
    Ok(())
}

/// Applies one set's pending migrations on `conn`, which already holds [`MIGRATION_LOCK_KEY`].
fn run_migration_set_locked(conn: &mut diesel::PgConnection, set: MigrationSet) -> AppResult<()> {
    use diesel_migrations::MigrationHarness;

    if set == MigrationSet::App {
        record_app_baseline_on_a_single_baseline_database(conn)?;
    }
    info!("Running the {} migrations...", set.name());
    conn.run_pending_migrations(set.migrations())
        .map(|_| ())
        .map_err(|e| {
            AppError::InternalError(format!(
                "Failed to run the {} migrations: {}",
                set.name(),
                e
            ))
        })
}

/// On a database built by the single v4.0.0 baseline, before it was split into `migrations/data`
/// and `migrations/app`, records the app baseline as applied without running it: that database
/// already has the app tables (and their three foreign keys to data tables, which stay; nothing
/// depends on them being gone), so running it would stop at its guard. Recognised by having the
/// `users` table and the data baseline's version but not the app baseline's. Does nothing
/// anywhere else, including a database that has only the app set.
fn record_app_baseline_on_a_single_baseline_database(
    conn: &mut diesel::PgConnection,
) -> AppResult<()> {
    use diesel::RunQueryDsl;

    // The table does not exist yet on an empty database, and Postgres resolves the INSERT's target
    // before evaluating its WHERE, so look for it first.
    #[derive(diesel::QueryableByName)]
    struct Exists {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        exists: bool,
    }
    let table = diesel::sql_query(
        "SELECT to_regclass('public.__diesel_schema_migrations') IS NOT NULL AS exists",
    )
    .get_result::<Exists>(conn)
    .map_err(|e| {
        AppError::InternalError(format!("Failed to look for the migrations table: {}", e))
    })?;
    if !table.exists {
        return Ok(());
    }

    let inserted = diesel::sql_query(
        "INSERT INTO __diesel_schema_migrations (version)
         SELECT $2
         WHERE to_regclass('public.users') IS NOT NULL
             AND EXISTS (SELECT 1 FROM __diesel_schema_migrations WHERE version = $1)
             AND NOT EXISTS (SELECT 1 FROM __diesel_schema_migrations WHERE version = $2)",
    )
    .bind::<diesel::sql_types::Text, _>(DATA_BASELINE_VERSION)
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
             tables; recorded the app baseline {} as applied without running it (ECO-422)",
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
    use diesel::migration::{Migration, MigrationSource};

    let migrations =
        MigrationSource::<diesel::pg::Pg>::migrations(&set.migrations()).map_err(|e| {
            AppError::InternalError(format!(
                "Failed to read the embedded {} migrations: {}",
                set.name(),
                e
            ))
        })?;
    migrations
        .iter()
        .map(|m| m.name().version().to_string())
        .max()
        .ok_or_else(|| {
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
/// version", which mixes the sets when one database holds both (TRAIN2_CONTRACTS.md, section 4).
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
/// sets current; see [`check_schema_version`]. Until DP-2 there is one database holding both sets.
pub async fn readiness_check(pool: &DatabasePool) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(|e| {
        AppError::InternalError(format!("Failed to get database connection: {}", e))
    })?;
    check_schema_version(&mut conn, &[MigrationSet::Data, MigrationSet::App]).await
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

    /// The two baselines are permanent versions (TRAIN2_CONTRACTS.md, section 4): databases have
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

    /// A database created for one test from `DATABASE_URL`'s server and dropped when the test
    /// ends, so tests that need an empty database don't touch the shared one.
    struct ScratchDatabase {
        admin_url: String,
        name: String,
        url: String,
    }

    impl ScratchDatabase {
        fn create(tag: &str) -> Self {
            use diesel::{Connection, RunQueryDsl};

            let admin_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
            let name = format!("eg_scratch_{tag}_{}", uuid::Uuid::new_v4().simple());
            let mut conn = diesel::PgConnection::establish(&admin_url)
                .expect("Should connect to DATABASE_URL");
            diesel::sql_query(format!("CREATE DATABASE {name}"))
                .execute(&mut conn)
                .expect("Should create a scratch database");
            let mut url = url::Url::parse(&admin_url).expect("DATABASE_URL should be a URL");
            url.set_path(&format!("/{name}"));
            Self {
                admin_url,
                name,
                url: url.to_string(),
            }
        }

        fn connect(&self) -> diesel::PgConnection {
            use diesel::Connection;
            diesel::PgConnection::establish(&self.url)
                .expect("Should connect to the scratch database")
        }
    }

    impl Drop for ScratchDatabase {
        fn drop(&mut self) {
            use diesel::{Connection, RunQueryDsl};
            if let Ok(mut conn) = diesel::PgConnection::establish(&self.admin_url) {
                let _ = diesel::sql_query(format!(
                    "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                    self.name
                ))
                .execute(&mut conn);
            }
        }
    }

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

    /// The tables of each set, from docs/development/TRAIN2_CONTRACTS.md section 5.
    const DATA_TABLES: [&str; 26] = [
        "data_sources",
        "datasets",
        "economic_series",
        "data_points",
        "crawl_queue",
        "crawl_attempts",
        "series_metadata",
        "reference_file_cache",
        "series_fetch_validators",
        "countries",
        "global_economic_indicators",
        "global_indicator_data",
        "country_correlations",
        "trade_relationships",
        "global_economic_events",
        "event_country_impacts",
        "leading_indicators",
        "companies",
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

    /// A new database gets both sets, data then app, and exactly their three versions.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_applies_both_sets_to_a_new_database() {
        let db = ScratchDatabase::create("both");
        run_migrations(&db.url)
            .await
            .expect("migrations should apply to a new database");

        let mut conn = db.connect();
        assert_eq!(
            recorded_versions(&mut conn),
            [
                "00000000000000",
                DATA_BASELINE_VERSION,
                APP_BASELINE_VERSION
            ]
        );
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

        run_migration_sets(&data_db.url, &[MigrationSet::Data])
            .await
            .expect("the data set should apply to an empty database");
        run_migration_sets(&app_db.url, &[MigrationSet::App])
            .await
            .expect("the app set should apply to an empty database");

        let mut data_conn = data_db.connect();
        assert_eq!(public_tables(&mut data_conn), sorted(&DATA_TABLES));
        assert_eq!(
            recorded_versions(&mut data_conn),
            ["00000000000000", DATA_BASELINE_VERSION]
        );

        let mut app_conn = app_db.connect();
        assert_eq!(public_tables(&mut app_conn), sorted(&APP_TABLES));
        assert_eq!(
            recorded_versions(&mut app_conn),
            ["00000000000000", APP_BASELINE_VERSION]
        );

        // Each database is current for its own set and not for the other one.
        let data_pool = create_pool(&data_db.url).await.expect("pool");
        let mut c = data_pool.get().await.expect("connection");
        check_schema_version(&mut c, &[MigrationSet::Data])
            .await
            .expect("the data database should be current for the data set");
        assert!(check_schema_version(&mut c, &[MigrationSet::App])
            .await
            .is_err());
        drop(c);
        let app_pool = create_pool(&app_db.url).await.expect("pool");
        let mut c = app_pool.get().await.expect("connection");
        check_schema_version(&mut c, &[MigrationSet::App])
            .await
            .expect("the app database should be current for the app set");
        assert!(check_schema_version(&mut c, &[MigrationSet::Data])
            .await
            .is_err());
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

    /// A database built by the single v4.0.0 baseline, before the split, is brought forward
    /// without any schema change: the app baseline is recorded, not run.
    ///
    /// The single baseline is rebuilt here from its two halves (the data baseline through
    /// Diesel, the app baseline's SQL run directly so its version is not recorded) plus the three
    /// foreign keys the split dropped, under their original names. scripts/compare_migrations.sh
    /// checks that the halves are the single baseline less those keys.
    #[tokio::test]
    #[serial_test::serial]
    async fn run_migrations_records_the_app_baseline_on_a_single_baseline_database() {
        use diesel::connection::SimpleConnection;

        let db = ScratchDatabase::create("single_baseline");
        run_migration_sets(&db.url, &[MigrationSet::Data])
            .await
            .expect("the data set should apply");
        let mut conn = db.connect();
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
        let before = schema_signature(&mut conn);
        assert!(before
            .iter()
            .any(|l| l.contains("financial_annotations_statement_id_fkey")));

        run_migrations(&db.url)
            .await
            .expect("migrations should succeed on a single-baseline database");

        assert_eq!(
            recorded_versions(&mut conn),
            [
                "00000000000000",
                DATA_BASELINE_VERSION,
                APP_BASELINE_VERSION
            ]
        );
        assert_eq!(
            schema_signature(&mut conn),
            before,
            "recording the app baseline should not change the schema"
        );

        // And it is idempotent: a second run changes nothing either.
        run_migrations(&db.url)
            .await
            .expect("a second run should succeed");
        assert_eq!(schema_signature(&mut conn), before);
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
