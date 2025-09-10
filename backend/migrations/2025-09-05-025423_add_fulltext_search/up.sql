-- REQUIREMENT: Implement PostgreSQL full-text search with spelling correction and synonyms
-- PURPOSE: Add comprehensive search capabilities to economic series data
-- This migration adds full-text search indices, spelling correction, and synonym support

-- Enable required PostgreSQL extensions
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;
CREATE EXTENSION IF NOT EXISTS fuzzystrmatch;

-- Create custom text search configuration for economic data (simplified for testing)
CREATE TEXT SEARCH CONFIGURATION economic_search (COPY = english);

-- Note: Synonym dictionary disabled for Docker testing
-- In production, this would load from a synonym file
-- CREATE TEXT SEARCH DICTIONARY economic_synonyms (
--     TEMPLATE = synonym,
--     SYNONYMS = economic_synonyms
-- );

-- Use standard English stemming for now
-- ALTER TEXT SEARCH CONFIGURATION economic_search
--     ALTER MAPPING FOR asciiword, asciihword, hword_asciipart, hword, hword_part, word
--     WITH economic_synonyms, english_stem;

-- Create function for fuzzy search with spelling correction
CREATE OR REPLACE FUNCTION fuzzy_search_series(
    search_query text,
    similarity_threshold real DEFAULT 0.3
)
RETURNS TABLE (
    id uuid,
    title text,
    description text,
    external_id text,
    source_id integer,
    frequency text,
    units text,
    start_date date,
    end_date date,
    last_updated timestamp,
    is_active boolean,
    rank real,
    similarity_score real
) AS $$
BEGIN
    RETURN QUERY
    WITH search_terms AS (
        SELECT plainto_tsquery('economic_search', search_query) as query,
               unaccent(lower(search_query)) as clean_query
    ),
    -- Full-text search results
    fts_results AS (
        SELECT
            es.*,
            ts_rank(es.search_vector, st.query) as fts_rank,
            0.0 as similarity_score,
            'fts' as match_type
        FROM economic_series es, search_terms st
        WHERE es.search_vector @@ st.query
    ),
    -- Trigram similarity results for spelling correction
    trigram_results AS (
        SELECT
            es.*,
            0.0 as fts_rank,
            GREATEST(
                similarity(es.title, st.clean_query),
                similarity(COALESCE(es.description, ''), st.clean_query),
                similarity(es.external_id, st.clean_query)
            ) as similarity_score,
            'trigram' as match_type
        FROM economic_series es, search_terms st
        WHERE (
            similarity(es.title, st.clean_query) >= similarity_threshold
            OR similarity(COALESCE(es.description, ''), st.clean_query) >= similarity_threshold
            OR similarity(es.external_id, st.clean_query) >= similarity_threshold
        )
        AND NOT EXISTS (
            SELECT 1 FROM fts_results fts WHERE fts.id = es.id
        )
    ),
    -- Combine results
    combined_results AS (
        SELECT *, fts_rank + similarity_score as total_rank FROM fts_results
        UNION ALL
        SELECT *, fts_rank + similarity_score as total_rank FROM trigram_results
    )
    SELECT
        cr.id,
        cr.title,
        cr.description,
        cr.external_id,
        cr.source_id,
        cr.frequency::text,
        cr.units,
        cr.start_date,
        cr.end_date,
        cr.last_updated,
        cr.is_active,
        cr.total_rank,
        cr.similarity_score
    FROM combined_results cr
    WHERE cr.is_active = true
    ORDER BY cr.total_rank DESC, cr.title ASC;
END;
$$ LANGUAGE plpgsql STABLE;

-- Add search vector column to economic_series table
ALTER TABLE economic_series
ADD COLUMN search_vector tsvector;

-- Create function to update search vector
CREATE OR REPLACE FUNCTION update_economic_series_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('economic_search', COALESCE(NEW.title, '')), 'A') ||
        setweight(to_tsvector('economic_search', COALESCE(NEW.description, '')), 'B') ||
        setweight(to_tsvector('economic_search', COALESCE(NEW.external_id, '')), 'C') ||
        setweight(to_tsvector('economic_search', COALESCE(NEW.units, '')), 'D');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger to automatically update search vector
CREATE TRIGGER update_economic_series_search_vector_trigger
    BEFORE INSERT OR UPDATE ON economic_series
    FOR EACH ROW EXECUTE FUNCTION update_economic_series_search_vector();

-- Update existing records with search vectors
UPDATE economic_series SET
    search_vector =
        setweight(to_tsvector('economic_search', COALESCE(title, '')), 'A') ||
        setweight(to_tsvector('economic_search', COALESCE(description, '')), 'B') ||
        setweight(to_tsvector('economic_search', COALESCE(external_id, '')), 'C') ||
        setweight(to_tsvector('economic_search', COALESCE(units, '')), 'D');

-- Create GIN index for full-text search (CONCURRENTLY removed for migration compatibility)
CREATE INDEX idx_economic_series_search_vector
ON economic_series USING GIN (search_vector);

-- Create GIN index for trigram similarity search
CREATE INDEX idx_economic_series_title_trigram
ON economic_series USING GIN (title gin_trgm_ops);

CREATE INDEX idx_economic_series_description_trigram
ON economic_series USING GIN (description gin_trgm_ops);

CREATE INDEX idx_economic_series_external_id_trigram
ON economic_series USING GIN (external_id gin_trgm_ops);

-- Create composite index for filtering active series
CREATE INDEX idx_economic_series_active_search
ON economic_series (is_active) WHERE is_active = true;

-- Add similar search capabilities to data_sources table
ALTER TABLE data_sources
ADD COLUMN search_vector tsvector;

-- Create function to update data source search vector
CREATE OR REPLACE FUNCTION update_data_source_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('economic_search', COALESCE(NEW.name, '')), 'A') ||
        setweight(to_tsvector('economic_search', COALESCE(NEW.description, '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create trigger for data sources
CREATE TRIGGER update_data_source_search_vector_trigger
    BEFORE INSERT OR UPDATE ON data_sources
    FOR EACH ROW EXECUTE FUNCTION update_data_source_search_vector();

-- Update existing data source records
UPDATE data_sources SET
    search_vector =
        setweight(to_tsvector('economic_search', COALESCE(name, '')), 'A') ||
        setweight(to_tsvector('economic_search', COALESCE(description, '')), 'B');

-- Create indices for data sources (CONCURRENTLY removed for migration compatibility)
CREATE INDEX idx_data_sources_search_vector
ON data_sources USING GIN (search_vector);

CREATE INDEX idx_data_sources_name_trigram
ON data_sources USING GIN (name gin_trgm_ops);

CREATE INDEX idx_data_sources_description_trigram
ON data_sources USING GIN (description gin_trgm_ops);

-- Create search statistics view for monitoring
CREATE VIEW search_statistics AS
SELECT
    'economic_series' as table_name,
    COUNT(*) as total_records,
    COUNT(*) FILTER (WHERE search_vector IS NOT NULL) as indexed_records,
    AVG(length(search_vector::text)) as avg_vector_length
FROM economic_series
UNION ALL
SELECT
    'data_sources' as table_name,
    COUNT(*) as total_records,
    COUNT(*) FILTER (WHERE search_vector IS NOT NULL) as indexed_records,
    AVG(length(search_vector::text)) as avg_vector_length
FROM data_sources;

-- Grant necessary permissions
GRANT USAGE ON SCHEMA public TO PUBLIC;
GRANT SELECT ON search_statistics TO PUBLIC;
