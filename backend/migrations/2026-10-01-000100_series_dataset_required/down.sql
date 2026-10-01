-- Makes economic_series.dataset_id nullable again. The backfill is kept: series keep the
-- datasets (and dimensions) the up migration gave them.
DROP INDEX idx_economic_series_dataset_id;
CREATE INDEX idx_economic_series_dataset_id ON economic_series (dataset_id) WHERE dataset_id IS NOT NULL;

ALTER TABLE economic_series
    ALTER COLUMN dataset_id DROP NOT NULL,
    ADD CONSTRAINT economic_series_dimensions_need_dataset CHECK (
        dataset_id IS NOT NULL OR dimensions = '{}'::jsonb
    ),
    ADD CONSTRAINT economic_series_default_measure_needs_dataset CHECK (
        dataset_id IS NOT NULL OR default_measure IS NULL
    );
