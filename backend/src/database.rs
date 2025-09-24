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
    // CRITICAL FIX: Clear PostgreSQL environment variables that override connection string
    // These variables take precedence over the DATABASE_URL and cause authentication failures
    std::env::remove_var("PGUSER");
    std::env::remove_var("PGPASSWORD");
    std::env::remove_var("PGDATABASE");
    std::env::remove_var("PGHOST");
    std::env::remove_var("PGPORT");
    info!("🧹 Cleared PostgreSQL environment variables to use DATABASE_URL");

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

/// Run database migrations
/// Note: Migrations require a synchronous connection
pub async fn run_migrations(database_url: &str) -> AppResult<()> {
    use diesel::Connection;
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

        // Debug: Check environment variables that could affect database connection
        info!("🔍 Migration connection environment variables:");
        info!("  - USER: {:?}", std::env::var("USER").ok());
        info!("  - PGUSER: {:?}", std::env::var("PGUSER").ok());
        info!("  - PGPASSWORD: {:?}", std::env::var("PGPASSWORD").is_ok());
        info!("  - PGDATABASE: {:?}", std::env::var("PGDATABASE").ok());

        // CRITICAL FIX: Clear PostgreSQL environment variables that override connection string
        // These variables take precedence over the DATABASE_URL and cause authentication failures
        std::env::remove_var("PGUSER");
        std::env::remove_var("PGPASSWORD");
        std::env::remove_var("PGDATABASE");
        std::env::remove_var("PGHOST");
        std::env::remove_var("PGPORT");
        info!("🧹 Cleared PostgreSQL environment variables to use DATABASE_URL");

        // Try to establish connection
        let mut conn = diesel::PgConnection::establish(&formatted_url).map_err(|e| {
            let error = AppError::InternalError(format!(
                "Failed to establish sync connection for migrations: {}",
                e
            ));
            error.log_with_context("Database migration connection attempt");
            error
        })?;

        info!("Database connection established, running migrations...");

        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| AppError::InternalError(format!("Failed to run migrations: {}", e)))?;

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
