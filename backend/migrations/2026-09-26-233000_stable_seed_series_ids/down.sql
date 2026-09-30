-- The seeded rows had generated ids before; any id is as good as another, so no id is restored.
COMMENT ON COLUMN economic_series.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN series_metadata.id IS 'Primary key using UUIDv7 format for better performance and sortability';
