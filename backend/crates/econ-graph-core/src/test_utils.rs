//! Test utilities for the econ-graph-core crate

use crate::database::DatabasePool;
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

/// A database created for one test on `DATABASE_URL`'s server (the test Postgres), and dropped
/// when it goes out of scope, for tests that need an empty database or several separate ones
/// (an app database and a data database, say) without touching the shared one.
pub struct ScratchDatabase {
    admin_url: String,
    name: String,
    /// The scratch database's URL.
    pub url: String,
}

impl ScratchDatabase {
    /// Create an empty database whose name starts with `eg_scratch_<tag>_`.
    pub fn create(tag: &str) -> Self {
        use diesel::{Connection, RunQueryDsl};

        let admin_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
        let name = format!("eg_scratch_{tag}_{}", uuid::Uuid::new_v4().simple());
        let mut conn =
            diesel::PgConnection::establish(&admin_url).expect("Should connect to DATABASE_URL");
        diesel::sql_query(format!("CREATE DATABASE {name}"))
            .execute(&mut conn)
            .expect("Should create a scratch database");
        let url = scratch_url(&admin_url, &name);
        Self {
            admin_url,
            name,
            url,
        }
    }

    /// A synchronous connection to the scratch database.
    pub fn connect(&self) -> diesel::PgConnection {
        use diesel::Connection;
        diesel::PgConnection::establish(&self.url).expect("Should connect to the scratch database")
    }
}

/// `admin_url` pointed at database `name`. A `dbname` query parameter would override the path
/// (libpq takes a repeated keyword's last value), so it is removed; other parameters are kept.
fn scratch_url(admin_url: &str, name: &str) -> String {
    let mut url = url::Url::parse(admin_url).expect("DATABASE_URL should be a URL");
    url.set_path(&format!("/{name}"));
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| key != "dbname")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear().extend_pairs(kept);
    }
    url.to_string()
}

#[cfg(test)]
mod scratch_url_tests {
    use super::scratch_url;

    #[test]
    fn the_scratch_database_name_wins_over_a_dbname_parameter() {
        assert_eq!(
            scratch_url(
                "postgres://u:p@h:5432/shared?dbname=shared&sslmode=disable",
                "eg_s"
            ),
            "postgres://u:p@h:5432/eg_s?sslmode=disable"
        );
        assert_eq!(
            scratch_url("postgres://u:p@h:5432/shared?dbname=shared", "eg_s"),
            "postgres://u:p@h:5432/eg_s"
        );
        assert_eq!(
            scratch_url("postgres://h/shared", "eg_s"),
            "postgres://h/eg_s"
        );
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

/// A test database and a connection pool to it.
///
/// [`TestContainer::new`] gives a test a database of its own on the test server, so that
/// [`TestContainer::clean_database`], which drops the schema, never drops one another test is
/// using. [`get_test_db`] shares one database between tests that only read and insert.
pub struct TestContainer {
    pool: DatabasePool,
    database_url: String,
    /// The test's own database, dropped with the last handle to it; `None` when shared.
    _scratch: Option<Arc<ScratchDatabase>>,
    #[cfg(test)]
    _container: Option<Arc<ContainerAsync<GenericImage>>>,
}

impl TestContainer {
    /// A migrated database of the caller's own: a scratch database on the server
    /// `DATABASE_URL` names, dropped when the returned value is. In this crate's own tests
    /// without `DATABASE_URL`, an ephemeral Postgres container instead.
    pub async fn new() -> Self {
        #[cfg(test)]
        if std::env::var("DATABASE_URL").is_err() {
            return Self::in_container().await;
        }
        let scratch = tokio::task::spawn_blocking(|| ScratchDatabase::create("test"))
            .await
            .expect("Should create a scratch database");
        crate::database::run_migrations(&scratch.url)
            .await
            .expect("Should migrate the scratch database");
        let pool = crate::database::create_pool(&scratch.url)
            .await
            .expect("Failed to connect to the scratch database");
        Self {
            pool,
            database_url: scratch.url.clone(),
            _scratch: Some(Arc::new(scratch)),
            #[cfg(test)]
            _container: None,
        }
    }

    /// The database `DATABASE_URL` names, as it is (in this crate's own tests without it, an
    /// ephemeral container): what [`get_test_db`] shares.
    async fn shared() -> Self {
        #[cfg(test)]
        if std::env::var("DATABASE_URL").is_err() {
            return Self::in_container().await;
        }
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/econ_graph_test".to_string());
        let pool = crate::database::create_pool(&database_url)
            .await
            .expect("Failed to connect to test database. Set DATABASE_URL to a reachable Postgres instance.");
        Self {
            pool,
            database_url,
            _scratch: None,
            #[cfg(test)]
            _container: None,
        }
    }

    /// An ephemeral Postgres container, unmigrated: tests migrate it with
    /// [`clean_database`](Self::clean_database).
    #[cfg(test)]
    async fn in_container() -> Self {
        let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ))
            .with_env_var("POSTGRES_DB", "econ_graph_test")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "postgres")
            .start()
            .await
            .expect("Failed to start container");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get port");
        let database_url = format!("postgres://postgres:postgres@localhost:{port}/econ_graph_test");
        let pool = crate::database::create_pool(&database_url)
            .await
            .expect("Failed to connect to testcontainer database");
        Self {
            pool,
            database_url,
            _scratch: None,
            _container: Some(Arc::new(container)),
        }
    }

    /// The same database (and container) with a pool of its own, opened on the calling
    /// runtime.
    async fn with_own_pool(&self) -> Self {
        let pool = crate::database::create_pool(&self.database_url)
            .await
            .expect("Failed to connect to test database");
        Self {
            pool,
            database_url: self.database_url.clone(),
            _scratch: self._scratch.clone(),
            #[cfg(test)]
            _container: self._container.clone(),
        }
    }

    /// Get the database pool
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    /// The URL of this test's database, for a test that needs a pool or connection of its own.
    pub fn database_url(&self) -> &str {
        &self.database_url
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
        crate::database::run_migrations(&self.database_url).await?;

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

/// Inserts a series with no observations (no start or end date) under the first data source,
/// in its [`TEST_DATASET`], and returns its id: the shape of a discovered series whose data has
/// not been fetched.
///
/// Panics if the data database has no data source or a query fails.
pub async fn insert_series_without_observations(
    conn: &mut diesel_async::AsyncPgConnection,
) -> uuid::Uuid {
    use diesel_async::RunQueryDsl;

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: uuid::Uuid,
    }
    let source = diesel::sql_query("SELECT id FROM data_sources ORDER BY name LIMIT 1")
        .get_result::<Row>(conn)
        .await
        .expect("the data baseline seeds data sources")
        .id;
    let dataset = test_dataset_id(conn, source).await;
    diesel::sql_query(
        "INSERT INTO economic_series (source_id, external_id, title, frequency, is_active, dataset_id) \
         VALUES ($1, $2, 'Series without observations', 'Monthly', TRUE, $3) RETURNING id",
    )
    .bind::<diesel::sql_types::Uuid, _>(source)
    .bind::<diesel::sql_types::Text, _>(format!("empty-{}", uuid::Uuid::new_v4()))
    .bind::<diesel::sql_types::Uuid, _>(dataset)
    .get_result::<Row>(conn)
    .await
    .expect("creating a series without observations")
    .id
}

/// The test database every test shares: its URL (and, in this crate's own tests, its container)
/// is resolved once.
static TEST_DB: OnceCell<Arc<TestContainer>> = OnceCell::const_new();

/// The shared test database, with a connection pool of the caller's own.
///
/// The pool isn't shared: each `#[tokio::test]` runs on a runtime of its own, and a
/// diesel-async connection's driver task lives on the runtime that opened it. A pool shared
/// across tests hands one test connections that die when another test's runtime ends, and
/// their queries fail with "connection closed" (or the pool times out replacing them).
pub async fn get_test_db() -> Arc<TestContainer> {
    let shared = TEST_DB
        .get_or_init(|| async { Arc::new(TestContainer::shared().await) })
        .await;
    Arc::new(shared.with_own_pool().await)
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
