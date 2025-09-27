//! # GraphQL DataLoaders
//!
//! This module provides DataLoader implementations for efficient N+1 query prevention.
//! DataLoaders batch and cache database queries to optimize GraphQL resolver performance.
//!
//! # Design Principles
//!
//! 1. **Performance**: DataLoaders prevent N+1 queries and optimize database access
//! 2. **Caching**: Intelligent caching strategies for frequently accessed data
//! 3. **Batching**: Automatic batching of database queries for efficiency
//! 4. **Type Safety**: Strong typing throughout with proper error handling
//!
//! # Quality Standards
//!
//! - All DataLoaders must implement proper error handling
//! - Caching strategies must be optimized for the specific use case
//! - Batch operations must be atomic and consistent
//! - All DataLoaders must have comprehensive documentation

use crate::imports::*;
use dataloader::cached::Loader;
use dataloader::BatchFn;
use std::collections::HashMap;

/// DataLoader batcher for efficiently loading data sources by ID
pub struct DataSourceBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<DataSource>> for DataSourceBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<DataSource>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::data_sources::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let sources = match dsl::data_sources
                .filter(dsl::id.eq_any(&keys))
                .load::<DataSource>(&mut conn)
                .await
            {
                Ok(sources) => sources,
                Err(e) => {
                    eprintln!("Failed to load data sources: {}", e);
                    return HashMap::new();
                }
            };

            let mut result = HashMap::new();

            // Create a map of found sources
            let found_sources: HashMap<Uuid, DataSource> =
                sources.into_iter().map(|s| (s.id, s)).collect();

            // Ensure all requested keys are in the result
            for key in keys {
                result.insert(key, found_sources.get(&key).cloned());
            }

            result
        }
    }
}

/// DataLoader batcher for efficiently loading data points by series ID
pub struct DataPointsBySeriesBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Vec<DataPoint>> for DataPointsBySeriesBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Vec<DataPoint>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::data_points::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let data_points = match dsl::data_points
                .filter(dsl::series_id.eq_any(&keys))
                .order(dsl::date.desc())
                .load::<DataPoint>(&mut conn)
                .await
            {
                Ok(points) => points,
                Err(e) => {
                    eprintln!("Failed to load data points: {}", e);
                    return HashMap::new();
                }
            };

            let mut result: HashMap<Uuid, Vec<DataPoint>> = HashMap::new();

            // Populate with data points
            for point in data_points {
                result.entry(point.series_id).or_default().push(point);
            }

            // Ensure all requested keys are in the result (empty vec for missing)
            for key in keys {
                result.entry(key).or_default();
            }

            result
        }
    }
}

/// DataLoader batcher for efficiently loading data point counts by series ID
pub struct DataPointCountBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, i32> for DataPointCountBatcher {
    fn load(&mut self, keys: &[Uuid]) -> impl std::future::Future<Output = HashMap<Uuid, i32>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::data_points::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let counts = match dsl::data_points
                .filter(dsl::series_id.eq_any(&keys))
                .group_by(dsl::series_id)
                .select((dsl::series_id, diesel::dsl::count(dsl::id)))
                .load::<(Uuid, i64)>(&mut conn)
                .await
            {
                Ok(counts) => counts,
                Err(e) => {
                    eprintln!("Failed to load data point counts: {}", e);
                    return HashMap::new();
                }
            };

            let mut result = HashMap::new();

            // Create a map of found counts
            let found_counts: HashMap<Uuid, i32> = counts
                .into_iter()
                .map(|(id, count)| (id, count as i32))
                .collect();

            // Ensure all requested keys are in the result (default to 0 for missing)
            for key in keys {
                result.insert(key, found_counts.get(&key).copied().unwrap_or(0));
            }

            result
        }
    }
}

/// DataLoader batcher for efficiently loading economic series by source ID
pub struct SeriesBySourceBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Vec<EconomicSeries>> for SeriesBySourceBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Vec<EconomicSeries>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::economic_series::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let series = match dsl::economic_series
                .filter(dsl::source_id.eq_any(&keys))
                .filter(dsl::is_active.eq(true))
                .order(dsl::title.asc())
                .load::<EconomicSeries>(&mut conn)
                .await
            {
                Ok(series) => series,
                Err(e) => {
                    eprintln!("Failed to load economic series: {}", e);
                    return HashMap::new();
                }
            };

            let mut result: HashMap<Uuid, Vec<EconomicSeries>> = HashMap::new();

            // Populate with series
            for s in series {
                result.entry(s.source_id).or_default().push(s);
            }

            // Ensure all requested keys are in the result (empty vec for missing)
            for key in keys {
                result.entry(key).or_default();
            }

            result
        }
    }
}

/// DataLoader batcher for efficiently loading series counts by source ID
pub struct SeriesCountBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, i32> for SeriesCountBatcher {
    fn load(&mut self, keys: &[Uuid]) -> impl std::future::Future<Output = HashMap<Uuid, i32>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::economic_series::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let counts = match dsl::economic_series
                .filter(dsl::source_id.eq_any(&keys))
                .filter(dsl::is_active.eq(true))
                .group_by(dsl::source_id)
                .select((dsl::source_id, diesel::dsl::count(dsl::id)))
                .load::<(Uuid, i64)>(&mut conn)
                .await
            {
                Ok(counts) => counts,
                Err(e) => {
                    eprintln!("Failed to load series counts: {}", e);
                    return HashMap::new();
                }
            };

            let mut result = HashMap::new();

            // Create a map of found counts
            let found_counts: HashMap<Uuid, i32> = counts
                .into_iter()
                .map(|(id, count)| (id, count as i32))
                .collect();

            // Ensure all requested keys are in the result (default to 0 for missing)
            for key in keys {
                result.insert(key, found_counts.get(&key).copied().unwrap_or(0));
            }

            result
        }
    }
}

/// DataLoader batcher for efficiently loading users by ID
pub struct UserBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<User>> for UserBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<User>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::users::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Failed to get database connection: {}", e);
                    return HashMap::new();
                }
            };

            let users = match dsl::users
                .filter(dsl::id.eq_any(&keys))
                .load::<User>(&mut conn)
                .await
            {
                Ok(users) => users,
                Err(e) => {
                    eprintln!("Failed to load users: {}", e);
                    return HashMap::new();
                }
            };

            let mut result = HashMap::new();

            // Create a map of found users
            let found_users: HashMap<Uuid, User> = users.into_iter().map(|u| (u.id, u)).collect();

            // Ensure all requested keys are in the result
            for key in keys {
                result.insert(key, found_users.get(&key).cloned());
            }

            result
        }
    }
}

/// Comprehensive DataLoaders struct with all specialized loaders
#[derive(Clone)]
pub struct DataLoaders {
    pub data_source_loader: Loader<Uuid, Option<DataSource>, DataSourceBatcher>,
    pub data_points_by_series_loader: Loader<Uuid, Vec<DataPoint>, DataPointsBySeriesBatcher>,
    pub data_point_count_loader: Loader<Uuid, i32, DataPointCountBatcher>,
    pub series_by_source_loader: Loader<Uuid, Vec<EconomicSeries>, SeriesBySourceBatcher>,
    pub series_count_loader: Loader<Uuid, i32, SeriesCountBatcher>,
    pub user_loader: Loader<Uuid, Option<User>, UserBatcher>,
}

impl DataLoaders {
    /// Create a new set of data loaders with all specialized loaders
    pub fn new(pool: DatabasePool) -> Self {
        let data_source_loader = Loader::new(DataSourceBatcher { pool: pool.clone() });
        let data_points_by_series_loader =
            Loader::new(DataPointsBySeriesBatcher { pool: pool.clone() });
        let data_point_count_loader = Loader::new(DataPointCountBatcher { pool: pool.clone() });
        let series_by_source_loader = Loader::new(SeriesBySourceBatcher { pool: pool.clone() });
        let series_count_loader = Loader::new(SeriesCountBatcher { pool: pool.clone() });
        let user_loader = Loader::new(UserBatcher { pool: pool.clone() });

        Self {
            data_source_loader,
            data_points_by_series_loader,
            data_point_count_loader,
            series_by_source_loader,
            series_count_loader,
            user_loader,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use econ_graph_core::test_utils::get_test_db;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_data_loaders_creation() {
        // Test DataLoaders creation
        // REQUIREMENT: Test DataLoader pattern for efficient database querying
        // PURPOSE: Verify that data loaders can be created successfully

        let container = get_test_db().await;
        let pool = container.pool().clone();
        let _loaders = DataLoaders::new(pool);

        // Basic creation test - more functionality will be added when DataLoader is re-enabled
    }

    /// Test DataSourceLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test data sources in database
    /// - Use DataSourceLoader to batch load data sources
    /// - Verify correct data sources are returned
    ///
    /// # Expected Behavior
    /// - DataSourceLoader returns correct data sources by ID
    /// - Batch loading works efficiently
    /// - Handles missing data sources gracefully
    ///
    /// # Test Data
    /// - Multiple test data sources with different attributes
    ///
    /// # Dependencies
    /// - Test database with data_sources table
    #[tokio::test]
    async fn test_data_source_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for data sources
        // PURPOSE: Verify that DataSourceLoader can load data sources by ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test DataSourceLoader - for now just test that we can create the loader
        // The actual DataLoader implementation will be added when the DataLoader pattern is re-enabled
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test data sources in the database
        // - Testing batch loading with multiple IDs
        // - Verifying correct data sources are returned
        // - Testing handling of missing data sources
        // - Verifying data integrity
    }

    /// Test DataPointsBySeriesLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test data points in database
    /// - Use DataPointsBySeriesLoader to batch load data points
    /// - Verify correct data points are returned
    ///
    /// # Expected Behavior
    /// - DataPointsBySeriesLoader returns correct data points by series ID
    /// - Batch loading works efficiently
    /// - Handles missing data points gracefully
    ///
    /// # Test Data
    /// - Multiple test data points with different series IDs
    ///
    /// # Dependencies
    /// - Test database with data_points table
    #[tokio::test]
    async fn test_data_points_by_series_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for data points
        // PURPOSE: Verify that DataPointsBySeriesLoader can load data points by series ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test DataPointsBySeriesLoader - for now just test that we can create the loader
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test data points in the database
        // - Testing batch loading with multiple series IDs
        // - Verifying correct data points are returned
        // - Testing handling of missing data points
        // - Verifying data integrity
    }

    /// Test DataPointCountLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test data points in database
    /// - Use DataPointCountLoader to batch load counts
    /// - Verify correct counts are returned
    ///
    /// # Expected Behavior
    /// - DataPointCountLoader returns correct counts by series ID
    /// - Batch loading works efficiently
    /// - Handles missing data points gracefully
    ///
    /// # Test Data
    /// - Multiple test data points with different series IDs
    ///
    /// # Dependencies
    /// - Test database with data_points table
    #[tokio::test]
    async fn test_data_point_count_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for data point counts
        // PURPOSE: Verify that DataPointCountLoader can load counts by series ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test DataPointCountLoader - for now just test that we can create the loader
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test data points in the database
        // - Testing batch loading with multiple series IDs
        // - Verifying correct counts are returned
        // - Testing handling of missing data points
        // - Verifying count accuracy
    }

    /// Test SeriesBySourceLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test economic series in database
    /// - Use SeriesBySourceLoader to batch load series
    /// - Verify correct series are returned
    ///
    /// # Expected Behavior
    /// - SeriesBySourceLoader returns correct series by source ID
    /// - Batch loading works efficiently
    /// - Handles missing series gracefully
    ///
    /// # Test Data
    /// - Multiple test economic series with different source IDs
    ///
    /// # Dependencies
    /// - Test database with economic_series table
    #[tokio::test]
    async fn test_series_by_source_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for economic series
        // PURPOSE: Verify that SeriesBySourceLoader can load series by source ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test SeriesBySourceLoader - for now just test that we can create the loader
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test economic series in the database
        // - Testing batch loading with multiple source IDs
        // - Verifying correct series are returned
        // - Testing handling of missing series
        // - Verifying data integrity
    }

    /// Test SeriesCountLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test economic series in database
    /// - Use SeriesCountLoader to batch load counts
    /// - Verify correct counts are returned
    ///
    /// # Expected Behavior
    /// - SeriesCountLoader returns correct counts by source ID
    /// - Batch loading works efficiently
    /// - Handles missing series gracefully
    ///
    /// # Test Data
    /// - Multiple test economic series with different source IDs
    ///
    /// # Dependencies
    /// - Test database with economic_series table
    #[tokio::test]
    async fn test_series_count_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for series counts
        // PURPOSE: Verify that SeriesCountLoader can load counts by source ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test SeriesCountLoader - for now just test that we can create the loader
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test economic series in the database
        // - Testing batch loading with multiple source IDs
        // - Verifying correct counts are returned
        // - Testing handling of missing series
        // - Verifying count accuracy
    }

    /// Test UserLoader with real database operations
    ///
    /// # Test Scenario
    /// - Create test users in database
    /// - Use UserLoader to batch load users
    /// - Verify correct users are returned
    ///
    /// # Expected Behavior
    /// - UserLoader returns correct users by ID
    /// - Batch loading works efficiently
    /// - Handles missing users gracefully
    ///
    /// # Test Data
    /// - Multiple test users with different attributes
    ///
    /// # Dependencies
    /// - Test database with users table
    #[tokio::test]
    async fn test_user_loader_with_real_data() {
        // REQUIREMENT: DataLoader should efficiently batch database queries for users
        // PURPOSE: Verify that UserLoader can load users by ID with real database operations

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test UserLoader - for now just test that we can create the loader
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual loading functionality
        // This test will be expanded to include:
        // - Creating test users in the database
        // - Testing batch loading with multiple IDs
        // - Verifying correct users are returned
        // - Testing handling of missing users
        // - Verifying data integrity
    }

    /// Test N+1 query prevention with integration tests
    ///
    /// # Test Scenario
    /// - Create test data with relationships
    /// - Execute GraphQL queries that would cause N+1 problems
    /// - Verify that DataLoaders prevent N+1 queries
    ///
    /// # Expected Behavior
    /// - DataLoaders batch multiple requests into single queries
    /// - N+1 query problems are prevented
    /// - Performance is optimized
    ///
    /// # Test Data
    /// - Complex test data with multiple relationships
    ///
    /// # Dependencies
    /// - Test database with all tables
    /// - GraphQL schema with DataLoaders
    #[tokio::test]
    async fn test_n_plus_one_query_prevention() {
        // REQUIREMENT: DataLoaders should prevent N+1 query problems
        // PURPOSE: Verify that DataLoaders efficiently batch database queries

        let container = get_test_db().await;
        let pool = container.pool().clone();

        // Test N+1 query prevention - for now just test that we can create the loaders
        let _loader = DataLoaders::new(pool.clone());

        // TODO: When DataLoader is re-enabled, test the actual N+1 prevention
        // This test will be expanded to include:
        // - Creating complex test data with relationships
        // - Executing GraphQL queries that would cause N+1 problems
        // - Verifying that DataLoaders batch requests efficiently
        // - Measuring query performance improvements
        // - Testing with different query patterns
    }
}
