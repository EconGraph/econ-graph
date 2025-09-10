-- Add back search_vector columns (for rollback)
ALTER TABLE data_sources ADD COLUMN search_vector tsvector;
ALTER TABLE economic_series ADD COLUMN search_vector tsvector;

-- Recreate the trigger functions
CREATE OR REPLACE FUNCTION update_data_source_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('economic_search', COALESCE(NEW.name, '')), 'A') ||
        setweight(to_tsvector('economic_search', COALESCE(NEW.description, '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

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

-- Recreate the triggers
CREATE TRIGGER update_data_source_search_vector_trigger
    BEFORE INSERT OR UPDATE ON data_sources
    FOR EACH ROW EXECUTE FUNCTION update_data_source_search_vector();

CREATE TRIGGER update_economic_series_search_vector_trigger
    BEFORE INSERT OR UPDATE ON economic_series
    FOR EACH ROW EXECUTE FUNCTION update_economic_series_search_vector();
