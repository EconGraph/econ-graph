-- Remove api_key_name field from data_sources table

ALTER TABLE data_sources
DROP COLUMN api_key_name;
