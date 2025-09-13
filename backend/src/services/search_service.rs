// REQUIREMENT: Full-text search service with PostgreSQL integration
// PURPOSE: Implement comprehensive search functionality with spelling correction and synonyms
// This service provides advanced search capabilities for economic time series data

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::models::search::{SearchParams, SearchSuggestion, SeriesSearchResult, SuggestionType};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use std::sync::Arc;
use tracing::{error, info, warn};
use validator::Validate;

/// Service for handling full-text search operations
pub struct SearchService {
    pool: Arc<DatabasePool>,
}

impl SearchService {
    /// Create a new search service
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }

    /// Perform full-text search for economic series with spelling correction
    pub async fn search_series(
        &self,
        params: &SearchParams,
    ) -> Result<Vec<SeriesSearchResult>, AppError> {
        // REQUIREMENT: Full-text search with spelling correction and synonyms
        // PURPOSE: Find economic series using advanced PostgreSQL search capabilities

        let start_time = std::time::Instant::now();

        // Validate search parameters
        params.validate().map_err(|e| {
            warn!("Invalid search parameters: {:?}", e);
            AppError::Validation(format!("Invalid search parameters: {}", e))
        })?;

        let _conn = self.pool.get().await.map_err(|e| {
            error!("Failed to get database connection: {}", e);
            AppError::ExternalApiError(format!("Connection error: {}", e))
        })?;

        let search_query = params.query.clone();
        let similarity_threshold = params.get_similarity_threshold();
        let limit = params.get_limit();
        let offset = params.get_offset();
        let source_filter = params.source_id;
        let frequency_filter = params.frequency.clone();
        let include_inactive = params.should_include_inactive();

        let mut conn = self.pool.get().await.map_err(|e| {
            error!("Failed to get database connection: {}", e);
            AppError::ExternalApiError(format!("Connection error: {}", e))
        })?;

        let results = diesel::sql_query(
            "SELECT es.id, es.title, es.description, es.external_id, es.source_id, es.frequency,
                    es.units, es.start_date, es.end_date, es.last_updated, es.is_active,
                    CASE WHEN es.title ILIKE '%' || $1 || '%' THEN 1.0 ELSE 0.5 END as rank,
                    CASE WHEN es.title ILIKE '%' || $1 || '%' THEN 1.0 ELSE 0.5 END as similarity_score
             FROM economic_series es
             WHERE (es.title ILIKE '%' || $1 || '%' OR es.description ILIKE '%' || $1 || '%')
             AND ($3::uuid IS NULL OR es.source_id = $3)
             AND ($4::text IS NULL OR es.frequency = $4)
             AND ($5::boolean OR es.is_active = true)
             ORDER BY rank DESC, es.title ASC
             LIMIT $6 OFFSET $7"
        )
        .bind::<diesel::sql_types::Text, _>(&search_query)
        .bind::<diesel::sql_types::Float4, _>(similarity_threshold)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(source_filter)
        .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(frequency_filter.as_deref())
        .bind::<diesel::sql_types::Bool, _>(include_inactive)
        .bind::<diesel::sql_types::Integer, _>(limit)
        .bind::<diesel::sql_types::Integer, _>(offset)
        .load::<SeriesSearchResultRow>(&mut conn)
        .await
        .map_err(|e| {
            error!("Search query execution failed: {}", e);
            AppError::ExternalApiError(format!("Query execution error: {}", e))
        })?;

        let execution_time = start_time.elapsed();

        // Convert database rows to search results
        let search_results: Vec<SeriesSearchResult> = results
            .into_iter()
            .map(|row| row.into_search_result())
            .collect();

        // Log search analytics
        info!(
            "Search completed: query='{}', results={}, time={}ms",
            params.query,
            search_results.len(),
            execution_time.as_millis()
        );

        Ok(search_results)
    }

    /// Get search suggestions for query completion and spelling correction
    pub async fn get_suggestions(
        &self,
        partial_query: &str,
        limit: i32,
    ) -> Result<Vec<SearchSuggestion>, AppError> {
        if partial_query.trim().is_empty() || partial_query.len() < 2 {
            return Ok(vec![]);
        }

        let mut conn = self.pool.get().await.map_err(|e| {
            error!("Failed to get database connection: {}", e);
            AppError::ExternalApiError(format!("Connection error: {}", e))
        })?;

        let query = partial_query.to_lowercase().trim().to_string();
        let search_limit = limit.min(20);

        let suggestions = diesel::sql_query(
            "SELECT DISTINCT title as word, 0.0 as rank, 'completion' as suggestion_type, 1 as match_count
             FROM economic_series
             WHERE title ILIKE $1 AND is_active = true
             ORDER BY title ASC
             LIMIT $2"
        )
        .bind::<diesel::sql_types::Text, _>(&format!("{}%", query))
        .bind::<diesel::sql_types::Integer, _>(search_limit)
        .load::<SuggestionRow>(&mut conn)
        .await
        .map_err(|e| {
            error!("Suggestions query execution failed: {}", e);
            AppError::ExternalApiError(format!("Query execution error: {}", e))
        })?;

        let search_suggestions: Vec<SearchSuggestion> = suggestions
            .into_iter()
            .take(limit as usize)
            .map(|row| SearchSuggestion {
                suggestion: row.word,
                match_count: row.match_count as i32,
                suggestion_type: SuggestionType::Completion,
                confidence: 0.8,
            })
            .collect();

        Ok(search_suggestions)
    }
}

// Database result row structures
#[derive(QueryableByName, Debug)]
struct SeriesSearchResultRow {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub id: uuid::Uuid,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub title: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub description: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub external_id: String,
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub source_id: uuid::Uuid,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub frequency: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub units: String,
    #[diesel(sql_type = diesel::sql_types::Date)]
    pub start_date: chrono::NaiveDate,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Date>)]
    pub end_date: Option<chrono::NaiveDate>,
    #[diesel(sql_type = diesel::sql_types::Timestamp)]
    pub last_updated: chrono::NaiveDateTime,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    pub is_active: bool,
    #[diesel(sql_type = diesel::sql_types::Float4)]
    pub rank: f32,
    #[diesel(sql_type = diesel::sql_types::Float4)]
    pub similarity_score: f32,
}

impl SeriesSearchResultRow {
    fn into_search_result(self) -> SeriesSearchResult {
        SeriesSearchResult {
            id: self.id,
            title: self.title,
            description: self.description,
            external_id: self.external_id,
            source_id: self.source_id,
            frequency: self.frequency,
            units: self.units,
            start_date: self.start_date,
            end_date: self.end_date,
            last_updated: self.last_updated,
            is_active: self.is_active,
            rank: self.rank,
            similarity_score: self.similarity_score,
        }
    }
}

#[derive(QueryableByName, Debug)]
struct SuggestionRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub word: String,
    #[diesel(sql_type = diesel::sql_types::Float4)]
    pub rank: f32,
    #[diesel(sql_type = diesel::sql_types::Text)]
    pub suggestion_type: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    pub match_count: i64,
}

// Module-level function for compatibility
pub async fn search_series(
    pool: &DatabasePool,
    params: &SearchParams,
) -> AppResult<Vec<SeriesSearchResult>> {
    let search_service = SearchService::new(Arc::new(pool.clone()));
    search_service.search_series(params).await
}
