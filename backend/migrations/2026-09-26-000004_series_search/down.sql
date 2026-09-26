CREATE INDEX IF NOT EXISTS idx_economic_series_title
    ON economic_series USING GIN (to_tsvector('english', title));
CREATE INDEX IF NOT EXISTS idx_economic_series_description
    ON economic_series USING GIN (to_tsvector('english', description));

DROP INDEX IF EXISTS idx_economic_series_title_trgm;
DROP INDEX IF EXISTS idx_economic_series_search_vector;
ALTER TABLE economic_series DROP COLUMN IF EXISTS search_vector;

DROP EXTENSION IF EXISTS pg_trgm;
