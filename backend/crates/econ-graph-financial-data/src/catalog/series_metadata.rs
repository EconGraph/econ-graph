//! Series metadata structures for the data catalog
//!
//! This module defines the metadata structures used to track
//! time series information, data ranges, and coverage details.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata for a financial time series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesMetadata {
    /// Unique identifier for the series
    pub series_id: Uuid,

    /// External identifier (e.g., "GDP_USA", "CPI_FRED_12345")
    pub external_id: String,

    /// Human-readable title
    pub title: String,

    /// Data frequency (e.g., "Daily", "Weekly", "Monthly", "Quarterly", "Annual")
    pub frequency: String,

    /// Data source (e.g., "FRED", "BLS", "BEA", "Custom")
    pub source: String,

    /// Data range information
    pub data_range: DataRange,

    /// Data coverage details
    pub coverage: DataCoverage,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Last data refresh timestamp
    pub last_refresh: DateTime<Utc>,

    /// Additional metadata tags
    pub tags: Vec<String>,

    /// Data quality score (0.0 to 1.0)
    pub quality_score: f64,

    /// Whether the series is active (still being updated)
    pub is_active: bool,
}

/// Data range information for a series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRange {
    /// Earliest date with data
    pub start_date: Option<NaiveDate>,

    /// Latest date with data
    pub end_date: Option<NaiveDate>,

    /// Total number of data points
    pub total_points: u64,

    /// Number of missing data points
    pub missing_points: u64,

    /// Data completeness percentage (0.0 to 1.0)
    pub completeness: f64,
}

/// Data coverage information for a series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCoverage {
    /// List of partitions that contain data for this series
    pub partitions: Vec<String>, // e.g., ["year=2024/month=01", "year=2024/month=02"]

    /// File paths where data is stored
    pub file_paths: Vec<String>,

    /// Total storage size in bytes
    pub storage_size: u64,

    /// Compression ratio (0.0 to 1.0)
    pub compression_ratio: f64,

    /// Last file modification time
    pub last_file_update: DateTime<Utc>,
}

impl SeriesMetadata {
    /// Create new series metadata
    pub fn new(
        series_id: Uuid,
        external_id: String,
        title: String,
        frequency: String,
        source: String,
    ) -> Self {
        let now = Utc::now();

        Self {
            series_id,
            external_id,
            title,
            frequency,
            source,
            data_range: DataRange::default(),
            coverage: DataCoverage::default(),
            created_at: now,
            updated_at: now,
            last_refresh: now,
            tags: Vec::new(),
            quality_score: 1.0,
            is_active: true,
        }
    }

    /// Update data range information
    pub fn update_data_range(
        &mut self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        total_points: u64,
        missing_points: u64,
    ) {
        self.data_range.start_date = start_date;
        self.data_range.end_date = end_date;
        self.data_range.total_points = total_points;
        self.data_range.missing_points = missing_points;

        if total_points > 0 {
            self.data_range.completeness =
                (total_points - missing_points) as f64 / total_points as f64;
        } else {
            self.data_range.completeness = 0.0;
        }

        self.updated_at = Utc::now();
    }

    /// Update coverage information
    pub fn update_coverage(
        &mut self,
        partitions: Vec<String>,
        file_paths: Vec<String>,
        storage_size: u64,
        compression_ratio: f64,
    ) {
        self.coverage.partitions = partitions;
        self.coverage.file_paths = file_paths;
        self.coverage.storage_size = storage_size;
        self.coverage.compression_ratio = compression_ratio;
        self.coverage.last_file_update = Utc::now();
        self.updated_at = Utc::now();
    }

    /// Add a tag to the series
    pub fn add_tag(&mut self, tag: String) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a tag from the series
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
        self.updated_at = Utc::now();
    }

    /// Check if series has data in a date range
    pub fn has_data_in_range(&self, start_date: NaiveDate, end_date: NaiveDate) -> bool {
        match (self.data_range.start_date, self.data_range.end_date) {
            (Some(series_start), Some(series_end)) => {
                series_start <= end_date && series_end >= start_date
            }
            (Some(series_start), None) => series_start <= end_date,
            (None, Some(series_end)) => series_end >= start_date,
            (None, None) => false,
        }
    }

    /// Get data freshness (time since last refresh)
    pub fn get_freshness(&self) -> chrono::Duration {
        Utc::now() - self.last_refresh
    }

    /// Check if data is stale (older than specified duration)
    pub fn is_stale(&self, max_age: chrono::Duration) -> bool {
        self.get_freshness() > max_age
    }
}

impl Default for DataRange {
    fn default() -> Self {
        Self {
            start_date: None,
            end_date: None,
            total_points: 0,
            missing_points: 0,
            completeness: 0.0,
        }
    }
}

impl Default for DataCoverage {
    fn default() -> Self {
        Self {
            partitions: Vec::new(),
            file_paths: Vec::new(),
            storage_size: 0,
            compression_ratio: 1.0,
            last_file_update: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_series_metadata_creation() {
        let series_id = Uuid::new_v4();
        let metadata = SeriesMetadata::new(
            series_id,
            "GDP_USA".to_string(),
            "Gross Domestic Product - USA".to_string(),
            "Quarterly".to_string(),
            "BEA".to_string(),
        );

        assert_eq!(metadata.series_id, series_id);
        assert_eq!(metadata.external_id, "GDP_USA");
        assert_eq!(metadata.title, "Gross Domestic Product - USA");
        assert_eq!(metadata.frequency, "Quarterly");
        assert_eq!(metadata.source, "BEA");
        assert!(metadata.is_active);
        assert_eq!(metadata.quality_score, 1.0);
    }

    #[test]
    fn test_data_range_update() {
        let mut metadata = SeriesMetadata::new(
            Uuid::new_v4(),
            "TEST".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end_date = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

        metadata.update_data_range(Some(start_date), Some(end_date), 365, 5);

        assert_eq!(metadata.data_range.start_date, Some(start_date));
        assert_eq!(metadata.data_range.end_date, Some(end_date));
        assert_eq!(metadata.data_range.total_points, 365);
        assert_eq!(metadata.data_range.missing_points, 5);
        assert!((metadata.data_range.completeness - 0.9863).abs() < 0.001); // (365-5)/365
    }

    #[test]
    fn test_date_range_check() {
        let mut metadata = SeriesMetadata::new(
            Uuid::new_v4(),
            "TEST".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end_date = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();

        metadata.update_data_range(Some(start_date), Some(end_date), 365, 0);

        // Test overlapping ranges
        assert!(metadata.has_data_in_range(
            NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 6, 30).unwrap()
        ));

        // Test non-overlapping ranges
        assert!(!metadata.has_data_in_range(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()
        ));
    }

    #[test]
    fn test_tags_management() {
        let mut metadata = SeriesMetadata::new(
            Uuid::new_v4(),
            "TEST".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        metadata.add_tag("economic".to_string());
        metadata.add_tag("gdp".to_string());
        assert_eq!(metadata.tags.len(), 2);

        metadata.add_tag("economic".to_string()); // Duplicate
        assert_eq!(metadata.tags.len(), 2); // Should not add duplicate

        metadata.remove_tag("gdp");
        assert_eq!(metadata.tags.len(), 1);
        assert_eq!(metadata.tags[0], "economic");
    }

    #[test]
    fn test_freshness_check() {
        let metadata = SeriesMetadata::new(
            Uuid::new_v4(),
            "TEST".to_string(),
            "Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );

        let freshness = metadata.get_freshness();
        assert!(freshness < Duration::seconds(1)); // Should be very recent

        // Test stale check
        assert!(!metadata.is_stale(Duration::hours(1)));

        // Create a very old metadata to test staleness
        let old_metadata = SeriesMetadata::new(
            Uuid::new_v4(),
            "OLD_TEST".to_string(),
            "Old Test Series".to_string(),
            "Daily".to_string(),
            "Test".to_string(),
        );
        // Manually set updated_at to be old
        let old_metadata = SeriesMetadata {
            updated_at: Utc::now() - Duration::hours(2),
            ..old_metadata
        };
        assert!(old_metadata.is_stale(Duration::hours(1)));
    }
}
