//! Series metadata model for storing discovered series information
//!
//! This module provides the SeriesMetadata model for storing information about
//! economic data series discovered from various APIs.

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::schema::series_metadata;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Series metadata model representing discovered series information
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = series_metadata)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SeriesMetadata {
    /// Unique identifier
    pub id: Uuid,
    /// Reference to the data source
    pub source_id: Uuid,
    /// External series identifier from the API
    pub external_id: String,
    /// Series title
    pub title: String,
    /// Series description
    pub description: Option<String>,
    /// Units of measurement
    pub units: Option<String>,
    /// Data frequency
    pub frequency: Option<String>,
    /// Geographic coverage level
    pub geographic_level: Option<String>,
    /// URL to the data
    pub data_url: Option<String>,
    /// API endpoint for this series
    pub api_endpoint: Option<String>,
    /// When this series was last discovered
    pub last_discovered_at: Option<DateTime<Utc>>,
    /// Whether this series is currently active
    pub is_active: bool,
    /// Creation timestamp
    pub created_at: Option<DateTime<Utc>>,
    /// Last update timestamp
    pub updated_at: Option<DateTime<Utc>>,
}

/// New series metadata for insertion
#[derive(Debug, Clone, Serialize, Deserialize, Insertable)]
#[diesel(table_name = series_metadata)]
pub struct NewSeriesMetadata {
    /// Reference to the data source
    pub source_id: Uuid,
    /// External series identifier from the API
    pub external_id: String,
    /// Series title
    pub title: String,
    /// Series description
    pub description: Option<String>,
    /// Units of measurement
    pub units: Option<String>,
    /// Data frequency
    pub frequency: Option<String>,
    /// Geographic coverage level
    pub geographic_level: Option<String>,
    /// URL to the data
    pub data_url: Option<String>,
    /// API endpoint for this series
    pub api_endpoint: Option<String>,
    /// Whether this series is currently active
    pub is_active: bool,
}

/// Update series metadata
#[derive(Debug, Clone, Serialize, Deserialize, AsChangeset)]
#[diesel(table_name = series_metadata)]
pub struct UpdateSeriesMetadata {
    /// Series title
    pub title: Option<String>,
    /// Series description
    pub description: Option<String>,
    /// Units of measurement
    pub units: Option<String>,
    /// Data frequency
    pub frequency: Option<String>,
    /// Geographic coverage level
    pub geographic_level: Option<String>,
    /// URL to the data
    pub data_url: Option<String>,
    /// API endpoint for this series
    pub api_endpoint: Option<String>,
    /// When this series was last discovered
    pub last_discovered_at: Option<DateTime<Utc>>,
    /// Whether this series is currently active
    pub is_active: bool,
}

impl Default for UpdateSeriesMetadata {
    fn default() -> Self {
        Self {
            title: None,
            description: None,
            units: None,
            frequency: None,
            geographic_level: None,
            data_url: None,
            api_endpoint: None,
            last_discovered_at: None,
            is_active: false,
        }
    }
}

impl SeriesMetadata {
    /// Get or create series metadata
    pub async fn get_or_create(
        pool: &DatabasePool,
        source_id: Uuid,
        external_id: &str,
        new_metadata: &NewSeriesMetadata,
    ) -> AppResult<Self> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;

        // Try to find existing metadata
        let existing = dsl::series_metadata
            .filter(dsl::source_id.eq(source_id))
            .filter(dsl::external_id.eq(external_id))
            .first::<Self>(&mut conn)
            .await
            .optional()?;

        if let Some(existing) = existing {
            // Update existing metadata
            let update = UpdateSeriesMetadata {
                title: Some(new_metadata.title.clone()),
                description: new_metadata.description.clone(),
                units: new_metadata.units.clone(),
                frequency: new_metadata.frequency.clone(),
                geographic_level: new_metadata.geographic_level.clone(),
                data_url: new_metadata.data_url.clone(),
                api_endpoint: new_metadata.api_endpoint.clone(),
                last_discovered_at: Some(Utc::now()),
                is_active: new_metadata.is_active,
            };

            let updated = diesel::update(dsl::series_metadata.filter(dsl::id.eq(existing.id)))
                .set(&update)
                .get_result::<Self>(&mut conn)
                .await?;

            Ok(updated)
        } else {
            // Create new metadata
            let created = diesel::insert_into(dsl::series_metadata)
                .values(new_metadata)
                .get_result::<Self>(&mut conn)
                .await?;

            Ok(created)
        }
    }

    /// Find series metadata by source and external ID
    pub async fn find_by_external_id(
        pool: &DatabasePool,
        source_id: Uuid,
        external_id: &str,
    ) -> AppResult<Option<Self>> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;
        let metadata = dsl::series_metadata
            .filter(dsl::source_id.eq(source_id))
            .filter(dsl::external_id.eq(external_id))
            .first::<Self>(&mut conn)
            .await
            .optional()?;

        Ok(metadata)
    }

    /// Get all active series metadata for a source
    pub async fn find_by_source(pool: &DatabasePool, source_id: Uuid) -> AppResult<Vec<Self>> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;
        let metadata = dsl::series_metadata
            .filter(dsl::source_id.eq(source_id))
            .filter(dsl::is_active.eq(true))
            .order(dsl::title.asc())
            .load::<Self>(&mut conn)
            .await?;

        Ok(metadata)
    }

    /// Mark series as inactive
    pub async fn deactivate(
        pool: &DatabasePool,
        source_id: Uuid,
        external_id: &str,
    ) -> AppResult<()> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;
        diesel::update(
            dsl::series_metadata
                .filter(dsl::source_id.eq(source_id))
                .filter(dsl::external_id.eq(external_id)),
        )
        .set(dsl::is_active.eq(false))
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    /// Get recently discovered series
    pub async fn find_recently_discovered(pool: &DatabasePool, hours: i32) -> AppResult<Vec<Self>> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;
        let cutoff = Utc::now() - chrono::Duration::hours(hours as i64);

        let metadata = dsl::series_metadata
            .filter(dsl::last_discovered_at.gt(cutoff))
            .filter(dsl::is_active.eq(true))
            .order(dsl::last_discovered_at.desc())
            .load::<Self>(&mut conn)
            .await?;

        Ok(metadata)
    }

    /// Find all series metadata
    pub async fn find_all(pool: &DatabasePool) -> AppResult<Vec<Self>> {
        use crate::schema::series_metadata::dsl;

        let mut conn = pool.get().await?;

        let metadata = diesel_async::RunQueryDsl::load(dsl::series_metadata, &mut conn).await?;

        Ok(metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestContainer;
    use tokio;

    #[tokio::test]
    async fn test_series_metadata_creation() {
        let container = TestContainer::new().await;
        let pool = &container.pool;

        // Create a test data source first
        let test_source = crate::models::data_source::NewDataSource {
            name: "Test Source".to_string(),
            description: Some("Test description".to_string()),
            base_url: "https://api.test.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://api.test.com/docs".to_string()),
        };

        let data_source = crate::models::data_source::DataSource::get_or_create(pool, test_source)
            .await
            .unwrap();

        // Create new series metadata
        let new_metadata = NewSeriesMetadata {
            source_id: data_source.id,
            external_id: "TEST_SERIES_001".to_string(),
            title: "Test Economic Series".to_string(),
            description: Some("A test economic series for unit testing".to_string()),
            units: Some("Index (2015=100)".to_string()),
            frequency: Some("Monthly".to_string()),
            geographic_level: Some("Country".to_string()),
            data_url: Some("https://api.test.com/data/TEST_SERIES_001".to_string()),
            api_endpoint: Some("https://api.test.com/v1/series/TEST_SERIES_001".to_string()),
            is_active: true,
        };

        // Test creation
        let created =
            SeriesMetadata::get_or_create(pool, data_source.id, "TEST_SERIES_001", &new_metadata)
                .await;
        assert!(created.is_ok());
        let metadata = created.unwrap();
        assert_eq!(metadata.external_id, "TEST_SERIES_001");
        assert_eq!(metadata.title, "Test Economic Series");

        // Test retrieval
        let found =
            SeriesMetadata::find_by_external_id(pool, data_source.id, "TEST_SERIES_001").await;
        assert!(found.is_ok());
        let found_metadata = found.unwrap();
        assert!(found_metadata.is_some());
        assert_eq!(found_metadata.unwrap().title, "Test Economic Series");

        // Test update
        let updated_metadata = NewSeriesMetadata {
            title: "Updated Test Economic Series".to_string(),
            ..new_metadata
        };
        let updated = SeriesMetadata::get_or_create(
            pool,
            data_source.id,
            "TEST_SERIES_001",
            &updated_metadata,
        )
        .await;
        assert!(updated.is_ok());
        let updated_result = updated.unwrap();
        assert_eq!(updated_result.title, "Updated Test Economic Series");
    }

    #[tokio::test]
    async fn test_series_metadata_deactivation() {
        let container = TestContainer::new().await;
        let pool = &container.pool;

        // Create a test data source first
        let test_source = crate::models::data_source::NewDataSource {
            name: "Test Source 2".to_string(),
            description: Some("Test description 2".to_string()),
            base_url: "https://api.test2.com".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 100,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some("https://api.test2.com/docs".to_string()),
        };

        let data_source = crate::models::data_source::DataSource::get_or_create(pool, test_source)
            .await
            .unwrap();

        // Create new series metadata
        let new_metadata = NewSeriesMetadata {
            source_id: data_source.id,
            external_id: "TEST_SERIES_002".to_string(),
            title: "Test Economic Series 2".to_string(),
            description: Some("A test economic series for deactivation testing".to_string()),
            units: Some("Percent".to_string()),
            frequency: Some("Quarterly".to_string()),
            geographic_level: Some("Region".to_string()),
            data_url: Some("https://api.test2.com/data/TEST_SERIES_002".to_string()),
            api_endpoint: Some("https://api.test2.com/v1/series/TEST_SERIES_002".to_string()),
            is_active: true,
        };

        let created =
            SeriesMetadata::get_or_create(pool, data_source.id, "TEST_SERIES_002", &new_metadata)
                .await
                .unwrap();
        assert!(created.is_active);

        // Test deactivation
        let deactivate_result =
            SeriesMetadata::deactivate(pool, data_source.id, "TEST_SERIES_002").await;
        assert!(deactivate_result.is_ok());

        // Verify deactivation
        let found = SeriesMetadata::find_by_external_id(pool, data_source.id, "TEST_SERIES_002")
            .await
            .unwrap();
        assert!(found.is_some());
        assert!(!found.unwrap().is_active);
    }
}
