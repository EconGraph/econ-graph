-- Indexed series search: weighted full-text vector plus trigram fuzzy matching.
--
-- search_vector ranks title and external id above description. The trigram index serves
-- similarity() / % (typo tolerance) and ILIKE on titles. The two expression tsvector indexes it
-- replaces were never used by the search query, which did ILIKE '%q%' full scans instead.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

ALTER TABLE economic_series
    ADD COLUMN search_vector TSVECTOR NOT NULL GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', coalesce(external_id, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'B')
    ) STORED;

CREATE INDEX idx_economic_series_search_vector ON economic_series USING GIN (search_vector);
CREATE INDEX idx_economic_series_title_trgm ON economic_series USING GIN (title gin_trgm_ops);

DROP INDEX IF EXISTS idx_economic_series_title;
DROP INDEX IF EXISTS idx_economic_series_description;
