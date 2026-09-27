-- Datasets: groups of related series that share a schema (SDMX-style dimensions, measures and
-- attributes). See docs/roadmap/federation.md, "Data model: datasets and series".
--
-- Observations stay in data_points. In train 1 every dataset is stored long: each measure is
-- its own series, so default_measure is 'value' until wide datasets arrive with Iceberg.

-- Whether a components list is an array of {name, label, type, unit?, codes?, codelist?}
-- objects (codes being {code, label, unit?, description?} objects) that the Rust
-- model (econ_graph_core::models::dataset::DatasetComponent) can read.
CREATE FUNCTION dataset_components_valid(components JSONB) RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE AS $$
    SELECT CASE WHEN jsonb_typeof(components) = 'array' THEN NOT EXISTS (
        SELECT 1
        FROM jsonb_array_elements(components) AS c
        WHERE jsonb_typeof(c) <> 'object'
            OR jsonb_typeof(c -> 'name') IS DISTINCT FROM 'string'
            OR jsonb_typeof(c -> 'label') IS DISTINCT FROM 'string'
            OR jsonb_typeof(c -> 'type') IS DISTINCT FROM 'string'
            OR c ->> 'type' NOT IN ('string', 'integer', 'decimal', 'date', 'boolean')
            OR COALESCE(jsonb_typeof(c -> 'unit'), 'null') NOT IN ('string', 'null')
            OR COALESCE(jsonb_typeof(c -> 'codelist'), 'null') NOT IN ('string', 'null')
            OR CASE COALESCE(jsonb_typeof(c -> 'codes'), 'null')
                WHEN 'null' THEN FALSE
                WHEN 'array' THEN EXISTS (
                    SELECT 1
                    FROM jsonb_array_elements(c -> 'codes') AS code
                    WHERE jsonb_typeof(code) <> 'object'
                        OR jsonb_typeof(code -> 'code') IS DISTINCT FROM 'string'
                        OR jsonb_typeof(code -> 'label') IS DISTINCT FROM 'string'
                        OR COALESCE(jsonb_typeof(code -> 'unit'), 'null') NOT IN ('string', 'null')
                        OR COALESCE(jsonb_typeof(code -> 'description'), 'null')
                            NOT IN ('string', 'null')
                )
                ELSE TRUE
            END
    ) ELSE FALSE END
$$;

CREATE TABLE datasets (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    -- Source-scoped dataset code, e.g. 'BDS' for Census BDS or 'WDI' for the World Bank.
    code VARCHAR(100) NOT NULL,
    name VARCHAR(500) NOT NULL,
    description TEXT,
    -- Each of these is a JSON array of components:
    --   {"name", "label", "type", "unit"?, "codes"?, "codelist"?}
    -- where "codes" is an inline list of {"code", "label", "unit"?, "description"?} and
    -- "codelist" names a shared code list loaded at runtime (e.g. "countries").
    -- Dimensions are listed in key order; the canonical series key follows that order.
    dimensions JSONB NOT NULL DEFAULT '[]'::jsonb,
    measures JSONB NOT NULL DEFAULT '[{"name": "value", "label": "Value", "type": "decimal"}]'::jsonb,
    attributes JSONB NOT NULL DEFAULT '[]'::jsonb,
    -- The measure a chart plots when the user has not picked one.
    default_measure VARCHAR(100) NOT NULL DEFAULT 'value',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT datasets_source_code_key UNIQUE (source_id, code),
    -- Target of the composite foreign keys below, so a series and its dataset share a source.
    CONSTRAINT datasets_id_source_key UNIQUE (id, source_id),
    CONSTRAINT datasets_code_not_blank CHECK (btrim(code) <> ''),
    CONSTRAINT datasets_dimensions_valid CHECK (dataset_components_valid(dimensions)),
    CONSTRAINT datasets_measures_valid CHECK (
        dataset_components_valid(measures) AND jsonb_array_length(measures) > 0
    ),
    CONSTRAINT datasets_attributes_valid CHECK (dataset_components_valid(attributes)),
    CONSTRAINT datasets_default_measure_declared CHECK (
        measures @> jsonb_build_array(jsonb_build_object('name', default_measure))
    )
);

CREATE TRIGGER update_datasets_updated_at
    BEFORE UPDATE ON datasets
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Series columns. dataset_id stays nullable until every train 1 adapter declares its datasets.
-- dimensions holds the series' dimension values as a flat object of strings,
-- e.g. {"geo_level": "state", "state": "06", "variable": "ESTAB"}; '{}' when it has none.
-- Values must be strings (strict mode stops the path from unwrapping arrays such as
-- {"state": ["06"]}), and only a series with a dataset may have any, or a default_measure.
-- default_measure overrides the dataset's; NULL means use the dataset's.
--
-- The composite foreign key also makes a series' dataset belong to the series' own source
-- (source_id is NOT NULL, so it is checked whenever dataset_id is set). It has no ON DELETE
-- action: a dataset cannot be deleted while series use it, while deleting a data source
-- cascades to both.
ALTER TABLE economic_series
    ADD COLUMN dataset_id UUID,
    ADD COLUMN dimensions JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN default_measure VARCHAR(100),
    ADD CONSTRAINT economic_series_dataset_source_fkey
        FOREIGN KEY (dataset_id, source_id) REFERENCES datasets(id, source_id),
    ADD CONSTRAINT economic_series_dimensions_is_string_object CHECK (
        jsonb_typeof(dimensions) = 'object'
        AND NOT jsonb_path_exists(dimensions, 'strict $.* ? (@.type() != "string")', '{}', true)
    ),
    ADD CONSTRAINT economic_series_dimensions_need_dataset CHECK (
        dataset_id IS NOT NULL OR dimensions = '{}'::jsonb
    ),
    ADD CONSTRAINT economic_series_default_measure_needs_dataset CHECK (
        dataset_id IS NOT NULL OR default_measure IS NULL
    );

ALTER TABLE series_metadata
    ADD COLUMN dataset_id UUID,
    ADD COLUMN dimensions JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN default_measure VARCHAR(100),
    ADD CONSTRAINT series_metadata_dataset_source_fkey
        FOREIGN KEY (dataset_id, source_id) REFERENCES datasets(id, source_id),
    ADD CONSTRAINT series_metadata_dimensions_is_string_object CHECK (
        jsonb_typeof(dimensions) = 'object'
        AND NOT jsonb_path_exists(dimensions, 'strict $.* ? (@.type() != "string")', '{}', true)
    ),
    ADD CONSTRAINT series_metadata_dimensions_need_dataset CHECK (
        dataset_id IS NOT NULL OR dimensions = '{}'::jsonb
    ),
    ADD CONSTRAINT series_metadata_default_measure_needs_dataset CHECK (
        dataset_id IS NOT NULL OR default_measure IS NULL
    );

-- Cross-section filters such as dimensions @> '{"indicator": "NY.GDP.PCAP.CD"}'.
CREATE INDEX idx_economic_series_dimensions ON economic_series USING GIN (dimensions jsonb_path_ops);
CREATE INDEX idx_series_metadata_dimensions ON series_metadata USING GIN (dimensions jsonb_path_ops);

CREATE INDEX idx_economic_series_dataset_id ON economic_series (dataset_id) WHERE dataset_id IS NOT NULL;
CREATE INDEX idx_series_metadata_dataset_id ON series_metadata (dataset_id) WHERE dataset_id IS NOT NULL;

-- A dimension key names at most one series per dataset. Dimensionless datasets (e.g. FRED,
-- where the series id alone identifies a series) hold many series with '{}', so they are exempt.
CREATE UNIQUE INDEX uq_economic_series_dataset_dimensions ON economic_series (dataset_id, dimensions)
    WHERE dataset_id IS NOT NULL AND dimensions <> '{}'::jsonb;
CREATE UNIQUE INDEX uq_series_metadata_dataset_dimensions ON series_metadata (dataset_id, dimensions)
    WHERE dataset_id IS NOT NULL AND dimensions <> '{}'::jsonb;
