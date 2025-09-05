use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::{bb8::Pool, bb8::PooledConnection, AsyncDieselConnectionManager};
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
        .max_size(10)
        .connection_timeout(Duration::from_secs(30))
        .build(config)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create database pool: {}", e)))?;
    
    info!("Database connection pool created successfully");
    Ok(pool)
}

/// Test database connectivity
pub async fn test_connection(pool: &DatabasePool) -> AppResult<()> {
    use diesel_async::RunQueryDsl;
    
    let mut conn = pool.get().await
        .map_err(|e| AppError::Internal(format!("Failed to get database connection: {}", e)))?;
    
    // Test with a simple query
    let result: i32 = diesel_async::RunQueryDsl::get_result(
        diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1")), 
        &mut conn
    )
        .await
        .map_err(|e| AppError::Internal(format!("Database connection test failed: {}", e)))?;
    
    if result == 1 {
        info!("Database connection test successful");
        Ok(())
    } else {
        Err(AppError::Internal("Database connection test returned unexpected result".to_string()))
    }
}

/// Run database migrations
/// Note: Migrations require a synchronous connection
pub async fn run_migrations(database_url: &str) -> AppResult<()> {
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use diesel::Connection;
    
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
    
    // Run migrations in a blocking task since migrations are sync
    let database_url = database_url.to_string();
    tokio::task::spawn_blocking(move || -> AppResult<()> {
        let mut conn = diesel::PgConnection::establish(&database_url)
            .map_err(|e| AppError::Internal(format!("Failed to establish sync connection for migrations: {}", e)))?;
        
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| AppError::Internal(format!("Failed to run migrations: {}", e)))?;
        
        Ok(())
    })
    .await
    .map_err(|e| AppError::Internal(format!("Migration task failed: {}", e)))??;
    
    info!("Database migrations completed successfully");
    Ok(())
}

/// Execute a database transaction
/// Note: Simplified implementation - transactions are complex with current type setup
pub async fn execute_with_connection<T, E, F, Fut>(
    pool: &DatabasePool,
    f: F,
) -> Result<T, E>
where
    F: FnOnce(&mut AsyncPgConnection) -> Fut + Send,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
    T: Send,
    E: From<diesel::result::Error> + From<AppError> + Send,
{
    let mut conn = pool.get().await
        .map_err(|e| E::from(AppError::Internal(format!("Failed to get database connection: {}", e))))?;
    
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
        test_connection(pool).await.expect("Database connection should work");
        
        // Test getting a connection from the pool
        let _conn = pool.get().await.expect("Should be able to get connection from pool");
    }
    
    #[tokio::test]
    #[serial_test::serial]
    async fn test_basic_query() {
        // Test basic query functionality
        // REQUIREMENT: Database query testing
        // PURPOSE: Verify database queries work correctly with async connections
        
        let container = crate::test_utils::TestContainer::new().await;
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Should be able to get connection from pool");
        
        use diesel_async::RunQueryDsl;
        let result: i32 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1 + 1"))
            .get_result(&mut conn)
            .await
            .expect("Query should work");
            
        assert_eq!(result, 2);
    }
}