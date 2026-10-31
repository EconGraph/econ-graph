-- SQL NULL means observation metadata was never captured. Do not invent historical flags.
ALTER TABLE data_points ADD COLUMN attributes JSONB;
ALTER TABLE data_points ADD CONSTRAINT data_points_attributes_object
    CHECK (attributes IS NULL OR jsonb_typeof(attributes) = 'object');
COMMENT ON COLUMN data_points.attributes IS
    'Provider observation metadata. SQL NULL=not captured; object fields retain explicit absence and verbatim provenance.';
