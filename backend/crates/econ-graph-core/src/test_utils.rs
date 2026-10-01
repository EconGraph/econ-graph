//! Test utilities for the econ-graph-core crate

use crate::database::{redact_database_url, DatabasePool};
use std::sync::Arc;
use tokio::sync::OnceCell;

#[cfg(test)]
use testcontainers::core::WaitFor;
#[cfg(test)]
use testcontainers::runners::AsyncRunner;
#[cfg(test)]
use testcontainers::{Container, ContainerAsync, GenericImage, ImageExt};

/// Postgres Docker image tag used by every testcontainers-based test.
/// Keep in sync with the `postgres_image` anchors in `.github/workflows/`.
pub const POSTGRES_IMAGE_TAG: &str = "18";

/// Test container for database testing
pub struct TestContainer {
    pool: DatabasePool,
    #[cfg(test)]
    _container: ContainerAsync<GenericImage>,
}

impl TestContainer {
    /// Create a new test container with ephemeral Postgres
    #[cfg(test)]
    pub async fn new() -> Self {
        println!("DEBUG: Creating TestContainer in test mode");
        // Check if we should use testcontainers or external DB
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            println!(
                "DEBUG: Using external DATABASE_URL: {}",
                redact_database_url(&database_url)
            );
            // Use external database if DATABASE_URL is set
            let pool = crate::database::create_pool(&database_url)
                .await
                .expect("Failed to connect to test database");

            // Don't run migrations automatically - let tests handle this after cleaning

            // Create a dummy container for the struct
            let postgres_image = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_DB", "test")
                .with_env_var("POSTGRES_USER", "test")
                .with_env_var("POSTGRES_PASSWORD", "test");

            let container = postgres_image
                .start()
                .await
                .expect("Failed to start container");

            Self {
                pool,
                _container: container,
            }
        } else {
            println!("DEBUG: Using testcontainers for ephemeral database");
            // Use testcontainers for ephemeral database
            let postgres_image = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_DB", "econ_graph_test")
                .with_env_var("POSTGRES_USER", "postgres")
                .with_env_var("POSTGRES_PASSWORD", "postgres");

            let container = postgres_image
                .start()
                .await
                .expect("Failed to start container");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get port");
            let database_url = format!(
                "postgres://postgres:postgres@localhost:{}/econ_graph_test",
                port
            );

            // Create pool
            let pool = crate::database::create_pool(&database_url)
                .await
                .expect("Failed to connect to testcontainer database");

            // Don't run migrations automatically - let tests handle this after cleaning

            Self {
                pool,
                _container: container,
            }
        }
    }

    /// Create a new test container (non-test version for compatibility)
    #[cfg(not(test))]
    pub async fn new() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());

        let pool = crate::database::create_pool(&database_url)
            .await
            .expect("Failed to connect to test database. Set DATABASE_URL to a reachable Postgres instance.");

        // Don't run migrations automatically - let tests handle this after cleaning
        Self { pool }
    }

    /// Get the database pool
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    /// Clean the database for testing
    pub async fn clean_database(&self) -> Result<(), Box<dyn std::error::Error>> {
        use diesel::prelude::*;
        use diesel_async::AsyncPgConnection;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        // Drop the entire public schema and recreate it
        // This ensures a completely fresh database state for each test
        diesel::sql_query("DROP SCHEMA IF EXISTS public CASCADE;")
            .execute(&mut conn)
            .await?;

        diesel::sql_query("CREATE SCHEMA public;")
            .execute(&mut conn)
            .await?;

        // Grant permissions on the public schema
        diesel::sql_query("GRANT ALL ON SCHEMA public TO public;")
            .execute(&mut conn)
            .await?;

        // Run migrations after cleaning to ensure database is ready
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
        crate::database::run_migrations(&database_url).await?;

        Ok(())
    }
}

/// Database test extension trait
pub trait DatabaseTestExt {
    /// Get a test database pool
    fn test_pool(&self) -> &DatabasePool;
}

impl DatabaseTestExt for TestContainer {
    fn test_pool(&self) -> &DatabasePool {
        &self.pool
    }
}

/// Code of the dataset [`test_dataset_id`] creates.
pub const TEST_DATASET: &str = "test";

/// Id of `source_id`'s [`TEST_DATASET`] dataset (no dimensions), created if missing. For tests
/// that insert series by hand: every series needs a dataset of its own source.
///
/// Panics if the query fails.
pub async fn test_dataset_id(
    conn: &mut diesel_async::AsyncPgConnection,
    source_id: uuid::Uuid,
) -> uuid::Uuid {
    use diesel_async::RunQueryDsl;

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: uuid::Uuid,
    }
    // DO UPDATE (a no-op) rather than DO NOTHING, so RETURNING also yields an existing row.
    diesel::sql_query(
        "INSERT INTO datasets (source_id, code, name) VALUES ($1, $2, 'Test dataset') \
         ON CONFLICT (source_id, code) DO UPDATE SET code = EXCLUDED.code RETURNING id",
    )
    .bind::<diesel::sql_types::Uuid, _>(source_id)
    .bind::<diesel::sql_types::Text, _>(TEST_DATASET)
    .get_result::<Row>(conn)
    .await
    .expect("creating the test dataset")
    .id
}

/// Global test database instance
static TEST_DB: OnceCell<Arc<TestContainer>> = OnceCell::const_new();

/// Get or create the global test database
pub async fn get_test_db() -> Arc<TestContainer> {
    TEST_DB
        .get_or_init(|| async { Arc::new(TestContainer::new().await) })
        .await
        .clone()
}

/// Test database function for compatibility
pub async fn db_test() -> Arc<TestContainer> {
    get_test_db().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_testcontainer_creation() {
        println!("DEBUG: Running testcontainer test");
        let container = TestContainer::new().await;
        println!("DEBUG: TestContainer created successfully");
        // Just verify it can be created
        assert!(container.pool().get().await.is_ok());
    }
}
