DROP INDEX IF EXISTS uq_series_metadata_dataset_dimensions;
DROP INDEX IF EXISTS uq_economic_series_dataset_dimensions;
DROP INDEX IF EXISTS idx_series_metadata_dataset_id;
DROP INDEX IF EXISTS idx_economic_series_dataset_id;
DROP INDEX IF EXISTS idx_series_metadata_dimensions;
DROP INDEX IF EXISTS idx_economic_series_dimensions;

ALTER TABLE series_metadata
    DROP CONSTRAINT IF EXISTS series_metadata_dimensions_need_dataset,
    DROP CONSTRAINT IF EXISTS series_metadata_dimensions_is_string_object,
    DROP CONSTRAINT IF EXISTS series_metadata_dataset_source_fkey,
    DROP COLUMN IF EXISTS default_measure,
    DROP COLUMN IF EXISTS dimensions,
    DROP COLUMN IF EXISTS dataset_id;

ALTER TABLE economic_series
    DROP CONSTRAINT IF EXISTS economic_series_dimensions_need_dataset,
    DROP CONSTRAINT IF EXISTS economic_series_dimensions_is_string_object,
    DROP CONSTRAINT IF EXISTS economic_series_dataset_source_fkey,
    DROP COLUMN IF EXISTS default_measure,
    DROP COLUMN IF EXISTS dimensions,
    DROP COLUMN IF EXISTS dataset_id;

DROP TABLE IF EXISTS datasets;
