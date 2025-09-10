-- Remove search_vector columns from data_sources and economic_series tables
ALTER TABLE data_sources DROP COLUMN IF EXISTS search_vector CASCADE;
ALTER TABLE economic_series DROP COLUMN IF EXISTS search_vector CASCADE;

-- Drop the trigger functions (CASCADE should handle triggers, but let's be explicit)
DROP FUNCTION IF EXISTS update_data_source_search_vector() CASCADE;
DROP FUNCTION IF EXISTS update_economic_series_search_vector() CASCADE;
