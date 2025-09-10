-- REQUIREMENT: Rollback migration for PostgreSQL full-text search
-- PURPOSE: Clean removal of full-text search capabilities
-- This migration removes all full-text search components safely

-- Drop search statistics view
DROP VIEW IF EXISTS search_statistics;

-- Drop indices (CONCURRENTLY for production safety)
DROP INDEX CONCURRENTLY IF EXISTS idx_data_sources_description_trigram;
DROP INDEX CONCURRENTLY IF EXISTS idx_data_sources_name_trigram;
DROP INDEX CONCURRENTLY IF EXISTS idx_data_sources_search_vector;
DROP INDEX CONCURRENTLY IF EXISTS idx_economic_series_active_search;
DROP INDEX CONCURRENTLY IF EXISTS idx_economic_series_external_id_trigram;
DROP INDEX CONCURRENTLY IF EXISTS idx_economic_series_description_trigram;
DROP INDEX CONCURRENTLY IF EXISTS idx_economic_series_title_trigram;
DROP INDEX CONCURRENTLY IF EXISTS idx_economic_series_search_vector;

-- Drop triggers
DROP TRIGGER IF EXISTS update_data_source_search_vector_trigger ON data_sources;
DROP TRIGGER IF EXISTS update_economic_series_search_vector_trigger ON economic_series;

-- Drop functions
DROP FUNCTION IF EXISTS update_data_source_search_vector();
DROP FUNCTION IF EXISTS update_economic_series_search_vector();
DROP FUNCTION IF EXISTS fuzzy_search_series(text, real);

-- Remove search vector columns
ALTER TABLE data_sources DROP COLUMN IF EXISTS search_vector;
ALTER TABLE economic_series DROP COLUMN IF EXISTS search_vector;

-- Drop custom text search configuration
DROP TEXT SEARCH CONFIGURATION IF EXISTS economic_search;
DROP TEXT SEARCH DICTIONARY IF EXISTS economic_synonyms;

-- Note: We don't drop extensions as they might be used by other parts of the system
-- Extensions that would be dropped if safe:
-- DROP EXTENSION IF EXISTS fuzzystrmatch;
-- DROP EXTENSION IF EXISTS unaccent;
-- DROP EXTENSION IF EXISTS pg_trgm;
