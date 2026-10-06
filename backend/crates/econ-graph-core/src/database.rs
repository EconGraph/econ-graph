use diesel_async::pooled_connection::{
    bb8::Pool, bb8::PooledConnection, AsyncDieselConnectionManager,
};
use diesel_async::AsyncPgConnection;
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

/// Run database migrations
/// Note: Migrations require a synchronous connection
pub async fn run_migrations(database_url: &str) -> AppResult<()> {
    use diesel::{Connection, RunQueryDsl};
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

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

        let migration_result = conn
            .run_pending_migrations(MIGRATIONS)
            .map(|_| ())
            .map_err(|e| AppError::InternalError(format!("Failed to run migrations: {}", e)));

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

/// The migration version (diesel's zero-padded timestamp, e.g. `"20261001000100"`) of the
/// newest migration embedded in this binary: the schema version it requires at minimum to run
/// correctly.
fn minimum_schema_version() -> AppResult<String> {
    use diesel::migration::{Migration, MigrationSource};
    use diesel_migrations::{embed_migrations, EmbeddedMigrations};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

    let migrations = MigrationSource::<diesel::pg::Pg>::migrations(&MIGRATIONS).map_err(|e| {
        AppError::InternalError(format!("Failed to read embedded migrations: {}", e))
    })?;
    migrations
        .iter()
        .map(|m| m.name().version().to_string())
        .max()
        .ok_or_else(|| AppError::InternalError("No embedded migrations found".to_string()))
}

#[derive(diesel::QueryableByName)]
struct SchemaVersionRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    version: String,
}

/// The schema-version half of [`readiness_check`], run directly on `conn` rather than a pool so
/// a caller can run it inside its own transaction: the database's latest applied migration must
/// be at least as new as this binary's own minimum (`>=`, not `==`): a pod still running the
/// previous binary, while a newer one is mid-rollout migrating the database further ahead, stays
/// ready rather than flipping to not-ready on a schema it can still work with.
async fn check_schema_version(conn: &mut AsyncPgConnection) -> AppResult<()> {
    let row: SchemaVersionRow = diesel_async::RunQueryDsl::get_result(
        diesel::sql_query(
            "SELECT version FROM __diesel_schema_migrations ORDER BY version DESC LIMIT 1",
        ),
        conn,
    )
    .await
    .map_err(|e| AppError::DatabaseError(format!("Failed to read schema version: {}", e)))?;

    let minimum = minimum_schema_version()?;
    if row.version < minimum {
        return Err(AppError::DatabaseError(format!(
            "database schema version {} is older than this binary's minimum {}",
            row.version, minimum
        )));
    }
    Ok(())
}

/// Readiness check for Kubernetes' `/readyz`: the database must be reachable and its schema
/// current enough; see [`check_schema_version`].
pub async fn readiness_check(pool: &DatabasePool) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(|e| {
        AppError::InternalError(format!("Failed to get database connection: {}", e))
    })?;
    check_schema_version(&mut conn).await
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

        let minimum = minimum_schema_version().expect("should read embedded migrations");

        // Delete the newest migration row inside an uncommitted transaction, never committed
        // or restored by hand: even a cancelled test run (its pooled connection dropped
        // mid-transaction) leaves this database, which other tests and local dev setups
        // share, completely unchanged, since Postgres rolls back an open transaction when its
        // connection closes. Nothing between BEGIN and ROLLBACK unwraps or asserts: a pooled
        // connection goes back to the pool on drop without closing it, so a panic while the
        // transaction is still open would hand a later pool user a connection sitting inside
        // it, with the row still deleted.
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
        let check_result = check_schema_version(&mut conn).await;

        diesel::sql_query("ROLLBACK")
            .execute(&mut conn)
            .await
            .expect("should roll back, undoing the delete regardless of what happened above");

        delete_result.expect("should delete the latest migration row");
        let err = check_result
            .expect_err("a schema missing this binary's newest migration should not be ready");
        assert!(matches!(err, AppError::DatabaseError(_)), "{err:?}");

        readiness_check(&pool)
            .await
            .expect("the untouched schema should still be ready after the rollback");
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
