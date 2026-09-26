// REQUIREMENT: Full-text search service with PostgreSQL integration
// PURPOSE: Implement comprehensive search functionality with spelling correction and synonyms
// This service provides advanced search capabilities for economic time series data

use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::search::{
    SearchParams, SearchSuggestion, SeriesSearchResult, SuggestionType,
};
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

        // Matches on the weighted full-text vector (title and external id rank above description)
        // or on trigram similarity to the title, which tolerates typos. Both conditions are served
        // by GIN indexes. `%` uses pg_trgm.similarity_threshold, set for this transaction only.
        let results = conn
            .transaction::<Vec<SeriesSearchResultRow>, diesel::result::Error, _>(
                async move |conn| {
                    diesel::sql_query(
                        "SELECT set_config('pg_trgm.similarity_threshold', $1::text, true)",
                    )
                    .bind::<diesel::sql_types::Float4, _>(similarity_threshold)
                    .execute(conn)
                    .await?;

                    diesel::sql_query(
                        "WITH q AS (SELECT websearch_to_tsquery('english', $1) AS tsq)
                         SELECT es.id, es.title, es.description, es.external_id, es.source_id,
                                es.frequency, es.units, es.start_date, es.end_date,
                                es.last_updated, es.is_active,
                                (ts_rank_cd(es.search_vector, q.tsq, 32)
                                    + similarity(es.title, $1))::float4 AS rank,
                                similarity(es.title, $1)::float4 AS similarity_score
                         FROM economic_series es, q
                         WHERE (es.search_vector @@ q.tsq OR es.title % $1)
                         AND ($2::uuid IS NULL OR es.source_id = $2)
                         AND ($3::text IS NULL OR es.frequency = $3)
                         AND ($4::boolean OR es.is_active = true)
                         ORDER BY rank DESC, es.title ASC
                         LIMIT $5 OFFSET $6",
                    )
                    .bind::<diesel::sql_types::Text, _>(&search_query)
                    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Uuid>, _>(source_filter)
                    .bind::<diesel::sql_types::Nullable<diesel::sql_types::Text>, _>(
                        frequency_filter.as_deref(),
                    )
                    .bind::<diesel::sql_types::Bool, _>(include_inactive)
                    .bind::<diesel::sql_types::Integer, _>(limit)
                    .bind::<diesel::sql_types::Integer, _>(offset)
                    .load::<SeriesSearchResultRow>(conn)
                    .await
                },
            )
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

        // Prefix matches complete the query; otherwise the closest titles by trigram similarity
        // act as spelling corrections.
        let suggestions = diesel::sql_query(
            "SELECT title AS word, similarity(title, $1)::float4 AS rank,
                    CASE WHEN title ILIKE $1 || '%' THEN 'completion' ELSE 'correction' END
                        AS suggestion_type,
                    COUNT(*) AS match_count
             FROM economic_series
             WHERE is_active = true AND (title ILIKE $1 || '%' OR title % $1)
             GROUP BY title
             ORDER BY (title ILIKE $1 || '%') DESC, rank DESC, title ASC
             LIMIT $2",
        )
        .bind::<diesel::sql_types::Text, _>(&query)
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
                suggestion_type: if row.suggestion_type == "completion" {
                    SuggestionType::Completion
                } else {
                    SuggestionType::Correction
                },
                confidence: row.rank,
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
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    pub units: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Date>)]
    pub start_date: Option<chrono::NaiveDate>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Date>)]
    pub end_date: Option<chrono::NaiveDate>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Timestamptz>)]
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
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

/// DB-backed search tests. They need `DATABASE_URL` (a migrated or migratable database) and are
/// skipped without it.
#[cfg(test)]
mod db_tests {
    use super::*;
    use serial_test::serial;

    async fn pool() -> Option<DatabasePool> {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed search test");
            return None;
        };
        econ_graph_core::run_migrations(&url)
            .await
            .expect("running migrations");
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        let mut conn = pool.get().await.unwrap();
        for sql in [
            "DELETE FROM economic_series WHERE external_id LIKE 'srch\\_%'",
            "INSERT INTO data_sources (name, description, base_url)
             VALUES ('search-test', 'search tests', 'http://localhost')
             ON CONFLICT (name) DO NOTHING",
            "INSERT INTO economic_series (source_id, external_id, title, description, frequency)
             SELECT id, v.e, v.t, v.d, 'Monthly' FROM data_sources,
             (VALUES ('srch_UNRATE', 'Unemployment Rate',
                      'Unemployed persons as a share of the labor force'),
                     ('srch_CPI', 'Consumer Price Index for All Urban Consumers',
                      'Measure of inflation in prices paid by urban consumers')) v(e, t, d)
             WHERE name = 'search-test'",
        ] {
            diesel::sql_query(sql).execute(&mut conn).await.unwrap();
        }
        drop(conn);
        Some(pool)
    }

    fn titles(results: &[SeriesSearchResult]) -> Vec<&str> {
        results.iter().map(|r| r.title.as_str()).collect()
    }

    #[tokio::test]
    #[serial]
    async fn search_matches_words_descriptions_ids_and_typos() {
        let Some(pool) = pool().await else { return };
        let service = SearchService::new(Arc::new(pool));

        let exact = service
            .search_series(&SearchParams::simple("unemployment"))
            .await
            .unwrap();
        assert_eq!(titles(&exact).into_iter().next(), Some("Unemployment Rate"));
        assert!(exact[0].rank > 0.0 && exact[0].similarity_score > 0.0);

        // Stemmed match on the description only.
        let described = service
            .search_series(&SearchParams::simple("inflation"))
            .await
            .unwrap();
        assert!(titles(&described).contains(&"Consumer Price Index for All Urban Consumers"));

        // Typo: no full-text match, found by trigram similarity.
        let typo = service
            .search_series(&SearchParams::simple("unemploymnt"))
            .await
            .unwrap();
        assert_eq!(titles(&typo).into_iter().next(), Some("Unemployment Rate"));

        // External id.
        let by_id = service
            .search_series(&SearchParams::simple("srch_CPI"))
            .await
            .unwrap();
        assert!(by_id.iter().any(|r| r.external_id == "srch_CPI"));
    }

    #[tokio::test]
    #[serial]
    async fn suggestions_complete_prefixes_and_correct_typos() {
        let Some(pool) = pool().await else { return };
        let service = SearchService::new(Arc::new(pool));

        let completions = service.get_suggestions("unemp", 5).await.unwrap();
        let first = <[_]>::first(&completions).expect("a completion");
        assert_eq!(first.suggestion, "Unemployment Rate");
        assert!(matches!(first.suggestion_type, SuggestionType::Completion));

        let corrections = service
            .get_suggestions("unemploymnt rate", 5)
            .await
            .unwrap();
        let first = <[_]>::first(&corrections).expect("a correction");
        assert_eq!(first.suggestion, "Unemployment Rate");
        assert!(matches!(first.suggestion_type, SuggestionType::Correction));
    }
}
