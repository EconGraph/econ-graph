-- Drop series_metadata table
DROP TRIGGER IF EXISTS trigger_update_series_metadata_updated_at ON series_metadata;
DROP FUNCTION IF EXISTS update_series_metadata_updated_at();
DROP TABLE IF EXISTS series_metadata;
