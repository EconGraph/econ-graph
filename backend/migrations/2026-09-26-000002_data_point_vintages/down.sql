DROP TRIGGER IF EXISTS data_points_unlink_vintage ON data_points;
DROP FUNCTION IF EXISTS data_points_unlink_vintage();
DROP TRIGGER IF EXISTS data_points_link_vintage ON data_points;
DROP FUNCTION IF EXISTS data_points_link_vintage();

DROP INDEX IF EXISTS idx_data_points_current;

ALTER TABLE data_points
    DROP CONSTRAINT IF EXISTS data_points_one_value_per_day,
    DROP CONSTRAINT IF EXISTS data_points_series_date_revision_key,
    DROP COLUMN IF EXISTS vintage,
    DROP COLUMN IF EXISTS superseded_on;

ALTER TABLE data_points
    ADD CONSTRAINT data_points_series_id_date_revision_date_is_original_releas_key
        UNIQUE (series_id, date, revision_date, is_original_release);
