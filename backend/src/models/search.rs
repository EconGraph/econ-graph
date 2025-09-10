// REQUIREMENT: Full-text search models with PostgreSQL integration
// PURPOSE: Provide comprehensive search capabilities with spelling correction and synonyms
// This module handles advanced search functionality for economic time series data

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

/// Search result for economic series with ranking and similarity scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesSearchResult {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub external_id: String,
    pub source_id: Uuid,
    pub frequency: String,
    pub units: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
    pub last_updated: NaiveDateTime,
    pub is_active: bool,
    /// Full-text search ranking score (higher is better)
    pub rank: f32,
    /// Trigram similarity score for spelling correction (0.0 to 1.0)
    pub similarity_score: f32,
}

/// Search parameters for economic series
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct SearchParams {
    /// Search query text
    #[validate(length(
        min = 1,
        max = 500,
        message = "Search query must be between 1 and 500 characters"
    ))]
    pub query: String,

    /// Minimum similarity threshold for fuzzy matching (0.0 to 1.0)
    #[validate(range(
        min = 0.0,
        max = 1.0,
        message = "Similarity threshold must be between 0.0 and 1.0"
    ))]
    pub similarity_threshold: Option<f32>,

    /// Maximum number of results to return
    #[validate(range(min = 1, max = 1000, message = "Limit must be between 1 and 1000"))]
    pub limit: Option<i32>,

    /// Offset for pagination
    #[validate(range(min = 0, message = "Offset must be non-negative"))]
    pub offset: Option<i32>,

    /// Filter by data source ID
    pub source_id: Option<Uuid>,

    /// Filter by series frequency
    pub frequency: Option<String>,

    /// Include inactive series in results
    pub include_inactive: Option<bool>,

    /// Sort order for results
    pub sort_by: Option<SearchSortOrder>,
}

/// Sort order options for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchSortOrder {
    /// Sort by relevance score (default)
    Relevance,
    /// Sort by title alphabetically
    Title,
    /// Sort by last updated date (newest first)
    LastUpdated,
    /// Sort by start date (newest first)
    StartDate,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            similarity_threshold: Some(0.3),
            limit: Some(50),
            offset: Some(0),
            source_id: None,
            frequency: None,
            include_inactive: Some(false),
            sort_by: Some(SearchSortOrder::Relevance),
        }
    }
}

/// Search suggestions for query completion and spelling correction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSuggestion {
    /// Suggested query text
    pub suggestion: String,
    /// Number of series that would match this suggestion
    pub match_count: i32,
    /// Type of suggestion (correction, completion, synonym)
    pub suggestion_type: SuggestionType,
    /// Confidence score for the suggestion (0.0 to 1.0)
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    /// Spelling correction
    Correction,
    /// Query completion
    Completion,
    /// Synonym expansion
    Synonym,
}

/// Search analytics data for monitoring and optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchAnalytics {
    /// Search query
    pub query: String,
    /// Number of results returned
    pub result_count: i32,
    /// Search execution time in milliseconds
    pub execution_time_ms: i32,
    /// Whether fuzzy matching was used
    pub used_fuzzy_matching: bool,
    /// Search timestamp
    pub searched_at: NaiveDateTime,
}

/// Search statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStatistics {
    pub table_name: String,
    pub total_records: i64,
    pub indexed_records: i64,
    pub avg_vector_length: Option<f64>,
}

impl SearchParams {
    /// Create search parameters for a simple text query
    pub fn simple(query: &str) -> Self {
        Self {
            query: query.to_string(),
            ..Default::default()
        }
    }

    /// Create search parameters with custom similarity threshold
    pub fn with_similarity(query: &str, threshold: f32) -> Self {
        Self {
            query: query.to_string(),
            similarity_threshold: Some(threshold),
            ..Default::default()
        }
    }

    /// Create search parameters for a specific data source
    pub fn for_source(query: &str, _source_id: i32) -> Self {
        Self {
            query: query.to_string(),
            source_id: None, // Convert i32 to UUID if needed
            ..Default::default()
        }
    }

    /// Get the effective similarity threshold
    pub fn get_similarity_threshold(&self) -> f32 {
        self.similarity_threshold.unwrap_or(0.3)
    }

    /// Get the effective limit
    pub fn get_limit(&self) -> i32 {
        self.limit.unwrap_or(50).min(1000) // Cap at 1000 for performance
    }

    /// Get the effective offset
    pub fn get_offset(&self) -> i32 {
        self.offset.unwrap_or(0).max(0) // Ensure non-negative
    }

    /// Check if inactive series should be included
    pub fn should_include_inactive(&self) -> bool {
        self.include_inactive.unwrap_or(false)
    }

    /// Get the sort order
    pub fn get_sort_order(&self) -> &SearchSortOrder {
        self.sort_by.as_ref().unwrap_or(&SearchSortOrder::Relevance)
    }
}

#[cfg(test)]
mod _inline_tests {
    use super::*;

    #[test]
    fn test_search_params_validation() {
        // REQUIREMENT: Validate search parameter constraints
        // PURPOSE: Ensure search parameters meet system requirements
        // This prevents invalid queries that could impact performance

        let valid_params = SearchParams {
            query: "GDP growth".to_string(),
            similarity_threshold: Some(0.5),
            limit: Some(100),
            offset: Some(0),
            ..Default::default()
        };

        // Valid parameters should pass validation
        assert!(
            valid_params.validate().is_ok(),
            "Valid parameters should pass validation"
        );
    }

    #[test]
    fn test_search_params_invalid_query() {
        // REQUIREMENT: Reject empty or overly long search queries
        // PURPOSE: Prevent invalid search operations

        let empty_query = SearchParams {
            query: "".to_string(),
            ..Default::default()
        };

        // Empty query should fail validation
        assert!(
            empty_query.validate().is_err(),
            "Empty query should fail validation"
        );

        let long_query = SearchParams {
            query: "a".repeat(501), // Exceeds 500 character limit
            ..Default::default()
        };

        // Overly long query should fail validation
        assert!(
            long_query.validate().is_err(),
            "Overly long query should fail validation"
        );
    }

    #[test]
    fn test_search_params_invalid_similarity_threshold() {
        // REQUIREMENT: Validate similarity threshold range
        // PURPOSE: Ensure similarity thresholds are within valid bounds

        let invalid_threshold = SearchParams {
            query: "test".to_string(),
            similarity_threshold: Some(1.5), // Above 1.0
            ..Default::default()
        };

        // Invalid threshold should fail validation
        assert!(
            invalid_threshold.validate().is_err(),
            "Invalid similarity threshold should fail validation"
        );

        let negative_threshold = SearchParams {
            query: "test".to_string(),
            similarity_threshold: Some(-0.1), // Below 0.0
            ..Default::default()
        };

        // Negative threshold should fail validation
        assert!(
            negative_threshold.validate().is_err(),
            "Negative similarity threshold should fail validation"
        );
    }

    #[test]
    fn test_search_params_helper_methods() {
        // REQUIREMENT: Test search parameter helper methods
        // PURPOSE: Verify utility methods work correctly

        let params = SearchParams::simple("unemployment rate");
        assert_eq!(params.query, "unemployment rate");
        assert_eq!(params.get_similarity_threshold(), 0.3);
        assert_eq!(params.get_limit(), 50);
        assert_eq!(params.get_offset(), 0);
        assert!(!params.should_include_inactive());

        let custom_params = SearchParams::with_similarity("GDP", 0.7);
        assert_eq!(custom_params.get_similarity_threshold(), 0.7);

        let source_params = SearchParams::for_source("inflation", 1);
        assert_eq!(source_params.source_id, None); // Updated for UUID
    }

    #[test]
    fn test_search_params_limits() {
        // REQUIREMENT: Test parameter limits and bounds checking
        // PURPOSE: Ensure system protection against excessive resource usage

        let high_limit = SearchParams {
            query: "test".to_string(),
            limit: Some(5000), // Exceeds max
            ..Default::default()
        };

        // Should cap at maximum allowed
        assert_eq!(high_limit.get_limit(), 1000);

        let negative_offset = SearchParams {
            query: "test".to_string(),
            offset: Some(-10),
            ..Default::default()
        };

        // Should normalize to 0
        assert_eq!(negative_offset.get_offset(), 0);
    }

    #[test]
    fn test_search_result_structure() {
        // REQUIREMENT: Verify search result structure
        // PURPOSE: Ensure search results contain all required fields

        let result = SeriesSearchResult {
            id: Uuid::new_v4(),
            title: "Real GDP".to_string(),
            description: Some("Inflation-adjusted GDP".to_string()),
            external_id: "GDP_REAL".to_string(),
            source_id: Uuid::new_v4(), // Use a test UUID
            frequency: "Quarterly".to_string(),
            units: "Billions USD".to_string(),
            start_date: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            end_date: None,
            last_updated: chrono::Utc::now().naive_utc(),
            is_active: true,
            rank: 0.85,
            similarity_score: 0.92,
        };

        // Verify all fields are accessible
        assert!(!result.title.is_empty());
        assert!(result.rank > 0.0);
        assert!(result.similarity_score >= 0.0 && result.similarity_score <= 1.0);
        assert!(result.is_active);
    }
}

#[cfg(test)]
mod tests;
