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
            formatted_url
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

/// Execute a database transaction
/// Note: Simplified implementation - transactions are complex with current type setup
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
            "Database connection pool error in transaction: {}",
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

#[cfg(test)]
mod tests {
    use super::*;
    // Tests now use TestContainer directly for better control

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
