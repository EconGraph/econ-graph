-- Add api_key_name column to data_sources table
ALTER TABLE data_sources ADD COLUMN api_key_name VARCHAR(255);

-- Add comment to explain the column
COMMENT ON COLUMN data_sources.api_key_name IS 'Name of the environment variable containing the API key for this data source. NULL if no API key is required.';
