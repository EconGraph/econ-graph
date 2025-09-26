use async_graphql::dataloader::*;
use async_graphql::*;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::DatabasePool;
use crate::models::{DataSource, EconomicSeries, DataPoint, User};

/// DataLoader for loading data sources by ID
pub struct DataSourceLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for DataSourceLoader {
    type Value = DataSource;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::data_sources::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let sources = dsl::data_sources
            .filter(dsl::id.eq_any(keys))
            .select(DataSource::as_select())
            .load::<DataSource>(&mut conn)
            .await?;

        Ok(sources.into_iter().map(|source| (source.id, source)).collect())
    }
}

/// DataLoader for loading economic series by ID
pub struct EconomicSeriesLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for EconomicSeriesLoader {
    type Value = EconomicSeries;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::economic_series::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let series = dsl::economic_series
            .filter(dsl::id.eq_any(keys))
            .select(EconomicSeries::as_select())
            .load::<EconomicSeries>(&mut conn)
            .await?;

        Ok(series.into_iter().map(|series| (series.id, series)).collect())
    }
}

/// DataLoader for loading economic series by source ID
pub struct SeriesBySourceLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for SeriesBySourceLoader {
    type Value = Vec<EconomicSeries>;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::economic_series::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let all_series = dsl::economic_series
            .filter(dsl::source_id.eq_any(keys))
            .filter(dsl::is_active.eq(true))
            .select(EconomicSeries::as_select())
            .load::<EconomicSeries>(&mut conn)
            .await?;

        let mut result: HashMap<Uuid, Vec<EconomicSeries>> = HashMap::new();
        for series in all_series {
            result.entry(series.source_id).or_insert_with(Vec::new).push(series);
        }

        // Ensure all keys have entries (even if empty)
        for key in keys {
            result.entry(*key).or_insert_with(Vec::new);
        }

        Ok(result)
    }
}

/// DataLoader for loading data points by series ID
pub struct DataPointsBySeriesLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for DataPointsBySeriesLoader {
    type Value = Vec<DataPoint>;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::data_points::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let all_data_points = dsl::data_points
            .filter(dsl::series_id.eq_any(keys))
            .order(dsl::date.desc())
            .select(DataPoint::as_select())
            .load::<DataPoint>(&mut conn)
            .await?;

        let mut result: HashMap<Uuid, Vec<DataPoint>> = HashMap::new();
        for data_point in all_data_points {
            result.entry(data_point.series_id).or_insert_with(Vec::new).push(data_point);
        }

        // Ensure all keys have entries (even if empty)
        for key in keys {
            result.entry(*key).or_insert_with(Vec::new);
        }

        Ok(result)
    }
}

/// DataLoader for loading data point counts by series ID
pub struct DataPointCountLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for DataPointCountLoader {
    type Value = i64;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::data_points::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let mut result: HashMap<Uuid, i64> = HashMap::new();

        for key in keys {
            let count = dsl::data_points
                .filter(dsl::series_id.eq(*key))
                .count()
                .get_result::<i64>(&mut conn)
                .await?;
            result.insert(*key, count);
        }

        Ok(result)
    }
}

/// DataLoader for loading users by ID
pub struct UserLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for UserLoader {
    type Value = User;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::users::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let users = dsl::users
            .filter(dsl::id.eq_any(keys))
            .select(User::as_select())
            .load::<User>(&mut conn)
            .await?;

        Ok(users.into_iter().map(|user| (user.id, user)).collect())
    }
}

/// DataLoader for loading series count by source ID
pub struct SeriesCountBySourceLoader {
    pool: DatabasePool,
}

#[async_trait::async_trait]
impl Loader<Uuid> for SeriesCountBySourceLoader {
    type Value = i64;
    type Error = async_graphql::Error;

    async fn load(&self, keys: &[Uuid]) -> Result<HashMap<Uuid, Self::Value>, Self::Error> {
        use crate::schema::economic_series::dsl;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;

        let mut conn = self.pool.get().await?;

        let mut result: HashMap<Uuid, i64> = HashMap::new();

        for key in keys {
            let count = dsl::economic_series
                .filter(dsl::source_id.eq(*key))
                .filter(dsl::is_active.eq(true))
                .count()
                .get_result::<i64>(&mut conn)
                .await?;
            result.insert(*key, count);
        }

        Ok(result)
    }
}

/// Main DataLoaders struct containing all data loaders
pub struct DataLoaders {
    pub data_source_loader: DataLoader<DataSourceLoader>,
    pub economic_series_loader: DataLoader<EconomicSeriesLoader>,
    pub series_by_source_loader: DataLoader<SeriesBySourceLoader>,
    pub data_points_by_series_loader: DataLoader<DataPointsBySeriesLoader>,
    pub data_point_count_loader: DataLoader<DataPointCountLoader>,
    pub user_loader: DataLoader<UserLoader>,
    pub series_count_by_source_loader: DataLoader<SeriesCountBySourceLoader>,
}

impl DataLoaders {
    /// Create a new set of data loaders
    pub fn new(pool: DatabasePool) -> Self {
        Self {
            data_source_loader: DataLoader::new(DataSourceLoader { pool: pool.clone() }),
            economic_series_loader: DataLoader::new(EconomicSeriesLoader { pool: pool.clone() }),
            series_by_source_loader: DataLoader::new(SeriesBySourceLoader { pool: pool.clone() }),
            data_points_by_series_loader: DataLoader::new(DataPointsBySeriesLoader { pool: pool.clone() }),
            data_point_count_loader: DataLoader::new(DataPointCountLoader { pool: pool.clone() }),
            user_loader: DataLoader::new(UserLoader { pool: pool.clone() }),
            series_count_by_source_loader: DataLoader::new(SeriesCountBySourceLoader { pool }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::get_test_pool;
    use std::sync::Arc;
    use uuid::Uuid;
    use chrono::Utc;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;

    #[tokio::test]
    async fn test_data_loaders_creation() {
        // Test DataLoaders creation
        // REQUIREMENT: Test DataLoader pattern for efficient database querying
        // PURPOSE: Verify that data loaders can be created successfully

        let pool = get_test_pool().await;
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

        let pool = get_test_pool().await;

        // Create test data sources
        let source1_id = Uuid::new_v4();
        let source2_id = Uuid::new_v4();
        let source3_id = Uuid::new_v4(); // This one won't exist in database

        let conn = pool.get().await.expect("Failed to get database connection");

        // Insert test data sources
        let _ = conn.interact(|conn| {
            use crate::schema::data_sources::dsl;
            use crate::models::data_source::NewDataSource;

            let new_source1 = NewDataSource {
                id: source1_id,
                name: "Test Source 1".to_string(),
                base_url: "https://test1.example.com".to_string(),
                description: Some("Test source 1".to_string()),
                is_active: true,
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };

            let new_source2 = NewDataSource {
                id: source2_id,
                name: "Test Source 2".to_string(),
                base_url: "https://test2.example.com".to_string(),
                description: Some("Test source 2".to_string()),
                is_active: true,
                created_at: Utc::now().naive_utc(),
                updated_at: Utc::now().naive_utc(),
            };

            diesel::insert_into(dsl::data_sources)
                .values(&[new_source1, new_source2])
                .execute(conn)
        }).await.expect("Failed to insert test data sources");

        // Test DataSourceLoader
        let loader = DataSourceLoader { pool: pool.clone() };
        let keys = vec![source1_id, source2_id, source3_id];
        let result = loader.load(&keys).await.expect("Failed to load data sources");

        // Verify results
        assert_eq!(result.len(), 2); // Only 2 sources exist
        assert!(result.contains_key(&source1_id));
        assert!(result.contains_key(&source2_id));
        assert!(!result.contains_key(&source3_id)); // This one doesn't exist

        // Verify data integrity
        let source1 = result.get(&source1_id).unwrap();
        assert_eq!(source1.name, "Test Source 1");
        assert_eq!(source1.base_url, "https://test1.example.com");

        let source2 = result.get(&source2_id).unwrap();
        assert_eq!(source2.name, "Test Source 2");
        assert_eq!(source2.base_url, "https://test2.example.com");
    }

    #[tokio::test]
    async fn test_series_by_source_loader() {
        // Test SeriesBySourceLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for related data
        // PURPOSE: Verify that SeriesBySourceLoader can load series grouped by source

        let pool = get_test_pool().await;
        let loader = SeriesBySourceLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_data_points_by_series_loader() {
        // Test DataPointsBySeriesLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for data points
        // PURPOSE: Verify that DataPointsBySeriesLoader can load data points grouped by series

        let pool = get_test_pool().await;
        let loader = DataPointsBySeriesLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_data_point_count_loader() {
        // Test DataPointCountLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for counts
        // PURPOSE: Verify that DataPointCountLoader can load data point counts by series

        let pool = get_test_pool().await;
        let loader = DataPointCountLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_user_loader() {
        // Test UserLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for users
        // PURPOSE: Verify that UserLoader can load users by ID

        let pool = get_test_pool().await;
        let loader = UserLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_series_count_by_source_loader() {
        // Test SeriesCountBySourceLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for series counts
        // PURPOSE: Verify that SeriesCountBySourceLoader can load series counts by source

        let pool = get_test_pool().await;
        let loader = SeriesCountBySourceLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_economic_series_loader() {
        // Test EconomicSeriesLoader functionality
        // REQUIREMENT: DataLoader should efficiently batch database queries for economic series
        // PURPOSE: Verify that EconomicSeriesLoader can load series by ID

        let pool = get_test_pool().await;
        let loader = EconomicSeriesLoader { pool };

        // Test with empty keys - should return empty HashMap
        let result = loader.load(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
