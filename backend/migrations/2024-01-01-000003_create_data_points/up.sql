-- Create data_points table
CREATE TABLE data_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    revision_date DATE NOT NULL,
    is_original_release BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique combination of series, date, and revision
    UNIQUE(series_id, date, revision_date, is_original_release)
);

-- Create indexes for faster queries
CREATE INDEX idx_data_points_series_id ON data_points(series_id);
CREATE INDEX idx_data_points_date ON data_points(date);
CREATE INDEX idx_data_points_revision_date ON data_points(revision_date);
CREATE INDEX idx_data_points_is_original_release ON data_points(is_original_release);
CREATE INDEX idx_data_points_series_date ON data_points(series_id, date);
CREATE INDEX idx_data_points_series_date_revision ON data_points(series_id, date, revision_date);

-- Create updated_at trigger
CREATE TRIGGER update_data_points_updated_at
    BEFORE UPDATE ON data_points
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create index for latest revisions (performance optimization)
-- Note: Removed subquery from WHERE clause as PostgreSQL doesn't support it in index predicates
CREATE INDEX idx_data_points_latest_revision ON data_points(series_id, date, revision_date DESC, value);
