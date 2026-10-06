-- EconGraph v4.0.0 baseline schema.
--
-- Everything the migrations before v4.0.0 built, written as one schema. No database was ever
-- deployed from them, so a new database is created in one step instead of replaying in-place
-- upgrades. Migrations after v4.0.0 are incremental again. See docs/development/MIGRATIONS.md.
--
-- This migration has the version of the last migration it replaces (2026-10-02-000500), so a
-- database that had already run that whole chain treats it as applied and only runs what comes
-- after. A database part way through the old chain stops at the guard below. The chain it
-- replaces is every migration on release/v4.0 before v4.0.0 was tagged, including the first
-- baseline (2026-10-01-000100) and the six migrations merged after it.
--
-- scripts/compare_migrations.sh checks that this file builds the same schema and seed rows as the
-- chain it replaces.

DO $$
BEGIN
    IF current_setting('server_version_num')::int < 180000 THEN
        RAISE EXCEPTION 'EconGraph requires PostgreSQL 18 or newer (found %)', version();
    END IF;
    IF to_regclass('public.data_sources') IS NOT NULL THEN
        RAISE EXCEPTION 'This database was built by migrations from before the v4.0.0 baseline and has not run all of them'
            USING HINT = 'Recreate the database, or run the old chain to its end first (git checkout '
                'the commit before the baseline was last squashed, then start the backend once) and switch back.';
    END IF;
END $$;

CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE OR REPLACE FUNCTION update_series_metadata_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

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

-- Seeds a source's reference codes into a new database, from a file recorded with the ETag it was
-- downloaded with. Called by the generated `*_seed_{source}_reference_codes` migrations (see
-- backend/crates/econ-graph-crawler/src/reference_file.rs), one call per file.
--
-- Does nothing when `reference_file_cache` already has a row for the file: this database has
-- crawled it (or been seeded with it) and the crawl keeps it current. Otherwise the codes are
-- merged into the dataset's dimension (codes it already has keep their labels; a dimension with a
-- shared `codelist` is left alone and nothing is stored), creating the
-- dataset row or the dimension if needed, and the file's ETag is stored so the first crawl only
-- downloads the file again if the source changed it. `sync_datasets` later fills in the rest of
-- the dataset from its toml file and keeps these codes.
CREATE FUNCTION seed_reference_codes(
    p_source TEXT,
    p_dataset TEXT,
    p_dimension JSONB,
    p_url TEXT,
    p_etag TEXT,
    p_codes JSONB
) RETURNS VOID
LANGUAGE plpgsql AS $$
DECLARE
    v_source_id UUID;
    v_codes JSONB := (
        SELECT COALESCE(jsonb_agg(c ORDER BY c ->> 'code'), '[]'::jsonb)
        FROM (
            SELECT DISTINCT ON (c ->> 'code') c
            FROM jsonb_array_elements(p_codes) AS c
            ORDER BY c ->> 'code'
        ) AS u(c)
    );
    v_dimension JSONB := (p_dimension - 'codes' - 'codelist') || jsonb_build_object('codes', v_codes);
BEGIN
    SELECT id INTO v_source_id FROM data_sources WHERE name = p_source;
    IF v_source_id IS NULL THEN
        RAISE EXCEPTION 'seed_reference_codes: no data source named %', p_source;
    END IF;
    IF EXISTS (
        SELECT 1 FROM reference_file_cache WHERE source_id = v_source_id AND url = p_url
    ) THEN
        RETURN;
    END IF;
    -- A dimension labelled by a shared code list takes no inline codes (it may not have both),
    -- and the crawl's merge skips it the same way, so nothing is stored for the file.
    IF EXISTS (
        SELECT 1
        FROM datasets AS ds, jsonb_array_elements(ds.dimensions) AS d
        WHERE ds.source_id = v_source_id AND ds.code = p_dataset
            AND d ->> 'name' = p_dimension ->> 'name' AND d ? 'codelist'
    ) THEN
        RETURN;
    END IF;

    INSERT INTO datasets (source_id, code, name, dimensions)
    VALUES (v_source_id, p_dataset, p_dataset, jsonb_build_array(v_dimension))
    ON CONFLICT (source_id, code) DO UPDATE SET dimensions = CASE
        WHEN EXISTS (
            SELECT 1 FROM jsonb_array_elements(datasets.dimensions) AS d
            WHERE d ->> 'name' = p_dimension ->> 'name'
        ) THEN (
            SELECT jsonb_agg(
                CASE
                    WHEN d ->> 'name' = p_dimension ->> 'name' THEN
                        d || jsonb_build_object('codes', COALESCE(d -> 'codes', '[]'::jsonb) || (
                            SELECT COALESCE(jsonb_agg(c ORDER BY c ->> 'code'), '[]'::jsonb)
                            FROM jsonb_array_elements(v_codes) AS c
                            WHERE NOT EXISTS (
                                SELECT 1
                                FROM jsonb_array_elements(COALESCE(d -> 'codes', '[]'::jsonb)) AS e
                                WHERE e ->> 'code' = c ->> 'code'
                            )
                        ))
                    ELSE d
                END
                ORDER BY ord
            )
            FROM jsonb_array_elements(datasets.dimensions) WITH ORDINALITY AS t(d, ord)
        )
        ELSE datasets.dimensions || jsonb_build_array(v_dimension)
    END;

    INSERT INTO reference_file_cache (source_id, url, etag) VALUES (v_source_id, p_url, p_etag);
END
$$;

-- ============================================================================
-- ENUM TYPES
-- ============================================================================

-- Compression types for XBRL file storage
CREATE TYPE compression_type AS ENUM ('zstd', 'lz4', 'gzip', 'none');

-- Processing status for XBRL files
CREATE TYPE processing_status AS ENUM ('pending', 'downloaded', 'processing', 'completed', 'failed');

-- Financial statement types
CREATE TYPE statement_type AS ENUM ('income_statement', 'balance_sheet', 'cash_flow', 'equity');

-- Financial statement sections
CREATE TYPE statement_section AS ENUM ('revenue', 'expenses', 'assets', 'liabilities', 'equity', 'operating', 'investing', 'financing');

-- Financial ratio categories
CREATE TYPE ratio_category AS ENUM ('profitability', 'liquidity', 'leverage', 'efficiency', 'market', 'growth');

-- Calculation methods for ratios
CREATE TYPE calculation_method AS ENUM ('simple', 'weighted_average', 'geometric_mean', 'median');

-- Comparison types for benchmarking
CREATE TYPE comparison_type AS ENUM ('industry', 'sector', 'size', 'geographic', 'custom');

-- XBRL data types
CREATE TYPE xbrl_data_type AS ENUM ('monetaryItemType', 'sharesItemType', 'stringItemType', 'decimalItemType', 'integerItemType', 'booleanItemType', 'dateItemType', 'timeItemType');

-- Period types for XBRL facts
CREATE TYPE period_type AS ENUM ('duration', 'instant');

-- Balance types for accounting
CREATE TYPE balance_type AS ENUM ('debit', 'credit');

-- XBRL substitution groups
CREATE TYPE substitution_group AS ENUM ('item', 'tuple');

-- Processing steps for XBRL files
CREATE TYPE processing_step AS ENUM ('download', 'parse', 'validate', 'store', 'extract', 'calculate');

-- Taxonomy file types for DTS support
CREATE TYPE taxonomy_file_type AS ENUM (
    'schema',
    'label_linkbase',
    'presentation_linkbase',
    'calculation_linkbase',
    'definition_linkbase',
    'reference_linkbase',
    'formula_linkbase'
);

-- Taxonomy source types
CREATE TYPE taxonomy_source_type AS ENUM (
    'company_specific',
    'us_gaap',
    'sec_dei',
    'fasb_srt',
    'ifrs',
    'other_standard',
    'custom'
);

-- Annotation types for collaborative features. Every variant of the Rust AnnotationType enum
-- (econ-graph-core/src/enums.rs) can be stored.
CREATE TYPE annotation_type AS ENUM (
    'comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight',
    'revenue_growth', 'cost_concern', 'cash_flow', 'balance_sheet', 'one_time_item',
    'industry_context'
);

-- Annotation status
CREATE TYPE annotation_status AS ENUM ('active', 'resolved', 'archived');

-- Assignment types
CREATE TYPE assignment_type AS ENUM ('review', 'analyze', 'verify', 'approve', 'investigate');

-- Assignment status
CREATE TYPE assignment_status AS ENUM ('pending', 'in_progress', 'completed', 'overdue', 'cancelled');

-- Who can see a chart annotation.
CREATE TYPE annotation_visibility AS ENUM ('private', 'public');

-- ============================================================================
-- DATA SOURCES, DATASETS AND SERIES
-- ============================================================================

CREATE TABLE data_sources (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    base_url VARCHAR(500) NOT NULL,
    api_key_required BOOLEAN NOT NULL DEFAULT FALSE,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_visible BOOLEAN NOT NULL DEFAULT true,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    requires_admin_approval BOOLEAN NOT NULL DEFAULT false,
    crawl_frequency_hours INTEGER NOT NULL DEFAULT 24,
    last_crawl_at TIMESTAMPTZ,
    crawl_status VARCHAR(50) DEFAULT 'pending',
    crawl_error_message TEXT,
    api_documentation_url VARCHAR(500),
    api_key_name VARCHAR(255)
);

CREATE INDEX idx_data_sources_visible ON data_sources(is_visible);
CREATE INDEX idx_data_sources_enabled ON data_sources(is_enabled);
CREATE INDEX idx_data_sources_crawl_status ON data_sources(crawl_status);

CREATE TRIGGER update_data_sources_updated_at
    BEFORE UPDATE ON data_sources
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Datasets: groups of related series that share a schema (SDMX-style dimensions, measures and
-- attributes). See docs/roadmap/federation.md, "Data model: datasets and series".
--
-- Observations stay in data_points. In train 1 every dataset is stored long: each measure is
-- its own series, so default_measure is 'value' until wide datasets arrive with Iceberg.
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
        -- `<> '[]'` rather than jsonb_array_length: AND has no evaluation order, and
        -- jsonb_array_length raises on a non-array instead of failing the check.
        dataset_components_valid(measures) AND measures <> '[]'::jsonb
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

-- A series' id is UUIDv5 of "{SOURCE}:{external_id}" in a fixed namespace
-- (econ-graph-crawler src/series_id.rs), so every database gives a series the same id.
--
-- Every series belongs to a dataset. dimensions holds the series' dimension values as a flat
-- object of strings, e.g. {"geo_level": "state", "state": "06", "variable": "ESTAB"}; '{}' when
-- it has none. Values must be strings (strict mode stops the path from unwrapping arrays such as
-- {"state": ["06"]}). default_measure overrides the dataset's; NULL means use the dataset's.
--
-- The composite foreign key makes a series' dataset belong to the series' own source. It has no
-- ON DELETE action: a dataset cannot be deleted while series use it, while deleting a data
-- source cascades to both.
CREATE TABLE economic_series (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50) NOT NULL,
    seasonal_adjustment VARCHAR(100),
    last_updated TIMESTAMPTZ,
    start_date DATE,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    first_discovered_at TIMESTAMPTZ,
    last_crawled_at TIMESTAMPTZ,
    first_missing_date DATE,
    crawl_status VARCHAR(50),
    crawl_error_message TEXT,
    -- Search ranks title and external id above description.
    search_vector TSVECTOR NOT NULL GENERATED ALWAYS AS (
        setweight(to_tsvector('simple', coalesce(external_id, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(title, '')), 'A') ||
        setweight(to_tsvector('english', coalesce(description, '')), 'B')
    ) STORED,
    dataset_id UUID NOT NULL,
    dimensions JSONB NOT NULL DEFAULT '{}'::jsonb,
    default_measure VARCHAR(100),

    UNIQUE(source_id, external_id),
    CONSTRAINT economic_series_dataset_source_fkey
        FOREIGN KEY (dataset_id, source_id) REFERENCES datasets(id, source_id),
    CONSTRAINT economic_series_dimensions_is_string_object CHECK (
        jsonb_typeof(dimensions) = 'object'
        AND NOT jsonb_path_exists(dimensions, 'strict $.* ? (@.type() != "string")', '{}', true)
    )
);

CREATE INDEX idx_economic_series_external_id ON economic_series(external_id);
CREATE INDEX idx_economic_series_frequency ON economic_series(frequency);
CREATE INDEX idx_economic_series_is_active ON economic_series(is_active);
CREATE INDEX idx_economic_series_last_updated ON economic_series(last_updated);
-- Full-text search, and trigram matching for similarity() / % (typo tolerance) and ILIKE on titles.
CREATE INDEX idx_economic_series_search_vector ON economic_series USING GIN (search_vector);
CREATE INDEX idx_economic_series_title_trgm ON economic_series USING GIN (title gin_trgm_ops);
-- Cross-section filters such as dimensions @> '{"indicator": "NY.GDP.PCAP.CD"}'.
CREATE INDEX idx_economic_series_dimensions ON economic_series USING GIN (dimensions jsonb_path_ops);
CREATE INDEX idx_economic_series_dataset_id ON economic_series (dataset_id);
-- A dimension key names at most one series per dataset. Dimensionless datasets (e.g. FRED,
-- where the series id alone identifies a series) hold many series with '{}', so they are exempt.
CREATE UNIQUE INDEX uq_economic_series_dataset_dimensions ON economic_series (dataset_id, dimensions)
    WHERE dataset_id IS NOT NULL AND dimensions <> '{}'::jsonb;

CREATE TRIGGER update_economic_series_updated_at
    BEFORE UPDATE ON economic_series
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE data_points (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    revision_date DATE NOT NULL,
    is_original_release BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Also serves every lookup by series_id, (series_id, date) and (series_id, date, revision_date).
    UNIQUE(series_id, date, revision_date, is_original_release)
);

CREATE INDEX idx_data_points_date ON data_points(date);
CREATE INDEX idx_data_points_revision_date ON data_points(revision_date);
-- Latest revisions
CREATE INDEX idx_data_points_latest_revision ON data_points(series_id, date, revision_date DESC, value);

CREATE TRIGGER update_data_points_updated_at
    BEFORE UPDATE ON data_points
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- The crawler's shared job queue. One queue carries several kinds of work for the same
-- (source, series_id).
--   started_at:  set by claim_next / claim_by_id (reset on every claim, so retries measure the last attempt)
--   finished_at: set by complete / fail (and by retry_later when retries are exhausted);
--                cleared on claim and on retry_later reschedule
--   claim_token: a fresh UUID on every claim, cleared when the lease ends. Lease-guarded
--                transitions must present it, so a stale task cannot finish a newer claim of the
--                same item even when both claims used the same worker id.
CREATE TABLE crawl_queue (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source VARCHAR(50) NOT NULL,
    series_id VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_for TIMESTAMPTZ,
    locked_by VARCHAR(100),
    locked_at TIMESTAMPTZ,
    kind VARCHAR(30) NOT NULL DEFAULT 'fetch_series',
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    claim_token UUID,

    CONSTRAINT check_crawl_queue_status
        CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'retrying', 'cancelled')),
    CONSTRAINT check_crawl_queue_priority
        CHECK (priority >= 1 AND priority <= 10),
    -- Locked items have lock information.
    CONSTRAINT check_crawl_queue_lock_consistency
        CHECK ((locked_by IS NULL AND locked_at IS NULL) OR (locked_by IS NOT NULL AND locked_at IS NOT NULL)),
    CONSTRAINT check_crawl_queue_kind
        CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing'))
);

CREATE INDEX idx_crawl_queue_status ON crawl_queue(status);
CREATE INDEX idx_crawl_queue_priority ON crawl_queue(priority DESC);
CREATE INDEX idx_crawl_queue_scheduled_for ON crawl_queue(scheduled_for);
CREATE INDEX idx_crawl_queue_locked_by ON crawl_queue(locked_by);
CREATE INDEX idx_crawl_queue_source ON crawl_queue(source);
CREATE INDEX idx_crawl_queue_created_at ON crawl_queue(created_at);
-- One active item per (source, series_id, kind). A non-deferrable partial unique index lets
-- INSERT ... ON CONFLICT DO NOTHING work (a DEFERRABLE constraint cannot be an arbiter).
CREATE UNIQUE INDEX uq_crawl_queue_active_item
    ON crawl_queue (source, series_id, kind)
    WHERE status IN ('pending', 'processing', 'retrying');
-- Matches the claim query:
--   WHERE status IN ('pending','retrying') AND (scheduled_for IS NULL OR scheduled_for <= NOW())
--   ORDER BY priority DESC, created_at ASC  ... FOR UPDATE SKIP LOCKED LIMIT 1
CREATE INDEX idx_crawl_queue_claim
    ON crawl_queue (priority DESC, created_at ASC, scheduled_for)
    WHERE status IN ('pending', 'retrying');

CREATE TRIGGER update_crawl_queue_updated_at
    BEFORE UPDATE ON crawl_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Discovered series, the discovery catalog. dataset_id stays nullable here: the seed rows below
-- include sources the crawler has no adapter for (IMF) and retired ids. Only a row with a
-- dataset may have dimensions or a default_measure.
CREATE TABLE series_metadata (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50),
    geographic_level VARCHAR(100),
    data_url TEXT,
    api_endpoint TEXT,
    last_discovered_at TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    dataset_id UUID,
    dimensions JSONB NOT NULL DEFAULT '{}'::jsonb,
    default_measure VARCHAR(100),

    UNIQUE(source_id, external_id),
    CONSTRAINT series_metadata_dataset_source_fkey
        FOREIGN KEY (dataset_id, source_id) REFERENCES datasets(id, source_id),
    CONSTRAINT series_metadata_dimensions_is_string_object CHECK (
        jsonb_typeof(dimensions) = 'object'
        AND NOT jsonb_path_exists(dimensions, 'strict $.* ? (@.type() != "string")', '{}', true)
    ),
    CONSTRAINT series_metadata_dimensions_need_dataset CHECK (
        dataset_id IS NOT NULL OR dimensions = '{}'::jsonb
    ),
    CONSTRAINT series_metadata_default_measure_needs_dataset CHECK (
        dataset_id IS NOT NULL OR default_measure IS NULL
    )
);

CREATE INDEX idx_series_metadata_external_id ON series_metadata(external_id);
CREATE INDEX idx_series_metadata_last_discovered ON series_metadata(last_discovered_at);
CREATE INDEX idx_series_metadata_active ON series_metadata(is_active);
CREATE INDEX idx_series_metadata_dimensions ON series_metadata USING GIN (dimensions jsonb_path_ops);
CREATE INDEX idx_series_metadata_dataset_id ON series_metadata (dataset_id) WHERE dataset_id IS NOT NULL;
CREATE UNIQUE INDEX uq_series_metadata_dataset_dimensions ON series_metadata (dataset_id, dimensions)
    WHERE dataset_id IS NOT NULL AND dimensions <> '{}'::jsonb;

CREATE TRIGGER trigger_update_series_metadata_updated_at
    BEFORE UPDATE ON series_metadata
    FOR EACH ROW
    EXECUTE FUNCTION update_series_metadata_updated_at();

-- The validators of a file or catalog resource an adapter fetches from its source (reference code
-- lists such as BLS's cu.item, and catalog resources discovery checks before re-listing every
-- series), so a scheduled refresh can send a conditional GET or compare a hash or version and
-- skip unchanged data (see HttpFetcher::get_text_if_changed). Not source-specific.
-- payload: what an adapter kept from the last copy it fetched, so a 304 can still be used without
-- re-downloading the file. BLS keeps the title, frequency and index base of each curated series
-- from its survey `.series` files here. NULL for files whose contents are merged elsewhere (e.g.
-- dataset dimension codes).
CREATE TABLE reference_file_cache (
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    etag TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB,
    last_modified TEXT,
    content_sha256 TEXT,
    version TEXT,

    PRIMARY KEY (source_id, url),
    CONSTRAINT reference_file_cache_url_not_blank CHECK (btrim(url) <> '')
);

-- Per-series validators, written in the same transaction as the series' points, so a validator
-- never outlives a failed write of the data it describes. No row means the next fetch is a full one.
CREATE TABLE series_fetch_validators (
    series_id UUID PRIMARY KEY REFERENCES economic_series(id) ON DELETE CASCADE,
    etag TEXT,
    last_modified TEXT,
    content_sha256 TEXT,
    version TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- GLOBAL ANALYSIS
-- ============================================================================

-- Countries table with geographic and economic metadata
CREATE TABLE countries (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    iso_code VARCHAR(3) NOT NULL UNIQUE, -- ISO 3166-1 alpha-3 (USA, GBR, etc.)
    iso_code_2 VARCHAR(2) NOT NULL UNIQUE, -- ISO 3166-1 alpha-2 (US, GB, etc.)
    name VARCHAR(255) NOT NULL,
    region VARCHAR(100) NOT NULL, -- North America, Europe, Asia, etc.
    sub_region VARCHAR(100), -- Western Europe, Southeast Asia, etc.
    income_group VARCHAR(50), -- High income, Upper middle income, etc.
    population BIGINT,
    gdp_usd DECIMAL(20,2), -- GDP in USD
    gdp_per_capita_usd DECIMAL(15,2),
    latitude DECIMAL(10,8),
    longitude DECIMAL(11,8),
    currency_code VARCHAR(3), -- USD, EUR, GBP, etc.
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Economic indicators optimized for cross-country analysis
CREATE TABLE global_economic_indicators (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_code VARCHAR(50) NOT NULL, -- World Bank indicator codes (NY.GDP.MKTP.CD, etc.)
    indicator_name VARCHAR(500) NOT NULL,
    category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, Inflation, etc.
    subcategory VARCHAR(100), -- Real GDP, Nominal GDP, etc.
    unit VARCHAR(50), -- USD, Percent, Index, etc.
    frequency VARCHAR(20) NOT NULL, -- Annual, Quarterly, Monthly
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_id, indicator_code)
);

-- Time series data for global indicators
CREATE TABLE global_indicator_data (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    indicator_id UUID NOT NULL REFERENCES global_economic_indicators(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    is_preliminary BOOLEAN NOT NULL DEFAULT false,
    data_source VARCHAR(50) NOT NULL, -- World Bank, IMF, OECD, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(indicator_id, date)
);

-- Economic correlations between countries
CREATE TABLE country_correlations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    country_a_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    country_b_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, etc.
    correlation_coefficient DECIMAL(5,4) NOT NULL, -- -1.0000 to 1.0000
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    sample_size INTEGER NOT NULL, -- Number of data points used
    p_value DECIMAL(10,8), -- Statistical significance
    is_significant BOOLEAN NOT NULL DEFAULT false,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_a_id, country_b_id, indicator_category, time_period_start, time_period_end),
    CHECK (country_a_id != country_b_id),
    CHECK (correlation_coefficient >= -1.0 AND correlation_coefficient <= 1.0)
);

-- Trade relationships between countries
CREATE TABLE trade_relationships (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    exporter_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    importer_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    trade_flow_type VARCHAR(20) NOT NULL, -- Goods, Services, Total
    year INTEGER NOT NULL,
    export_value_usd DECIMAL(20,2), -- Export value in USD
    import_value_usd DECIMAL(20,2), -- Import value in USD
    trade_balance_usd DECIMAL(20,2), -- Export - Import
    trade_intensity DECIMAL(8,6), -- Trade as % of GDP
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(exporter_country_id, importer_country_id, trade_flow_type, year),
    CHECK (exporter_country_id != importer_country_id)
);

-- Global economic events and their impacts
CREATE TABLE global_economic_events (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    name VARCHAR(500) NOT NULL,
    description TEXT,
    event_type VARCHAR(50) NOT NULL, -- Crisis, Policy, Natural Disaster, etc.
    severity VARCHAR(20) NOT NULL, -- Low, Medium, High, Critical
    start_date DATE NOT NULL,
    end_date DATE,
    primary_country_id UUID REFERENCES countries(id), -- Originating country
    affected_regions TEXT[], -- Array of affected regions
    economic_impact_score DECIMAL(5,2), -- 0-100 impact score
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Country impacts from global events
CREATE TABLE event_country_impacts (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    event_id UUID NOT NULL REFERENCES global_economic_events(id) ON DELETE CASCADE,
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    impact_type VARCHAR(50) NOT NULL, -- GDP, Employment, Trade, Financial
    impact_magnitude DECIMAL(8,4), -- Percentage change
    impact_duration_days INTEGER, -- How long the impact lasted
    recovery_time_days INTEGER, -- Time to return to pre-event levels
    confidence_score DECIMAL(3,2), -- 0-1 confidence in measurement
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(event_id, country_id, impact_type)
);

-- Economic leading indicators relationships
CREATE TABLE leading_indicators (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    leading_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    following_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL,
    lead_time_months INTEGER NOT NULL, -- How many months country A leads country B
    correlation_strength DECIMAL(5,4) NOT NULL, -- Correlation coefficient
    predictive_accuracy DECIMAL(5,4), -- Historical prediction accuracy (0-1)
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(leading_country_id, following_country_id, indicator_category),
    CHECK (leading_country_id != following_country_id),
    CHECK (lead_time_months >= 1 AND lead_time_months <= 24)
);

CREATE INDEX idx_countries_region ON countries(region);
CREATE INDEX idx_countries_income_group ON countries(income_group);
CREATE INDEX idx_countries_iso_codes ON countries(iso_code, iso_code_2);

CREATE INDEX idx_global_indicators_country_category ON global_economic_indicators(country_id, category);
CREATE INDEX idx_global_indicators_code ON global_economic_indicators(indicator_code);

CREATE INDEX idx_global_data_date_value ON global_indicator_data(date, value) WHERE value IS NOT NULL;

CREATE INDEX idx_correlations_category ON country_correlations(indicator_category);
CREATE INDEX idx_correlations_strength ON country_correlations(correlation_coefficient DESC) WHERE is_significant = true;

CREATE INDEX idx_trade_exporter_year ON trade_relationships(exporter_country_id, year DESC);
CREATE INDEX idx_trade_importer_year ON trade_relationships(importer_country_id, year DESC);
CREATE INDEX idx_trade_value ON trade_relationships(export_value_usd DESC) WHERE export_value_usd IS NOT NULL;

CREATE INDEX idx_events_date ON global_economic_events(start_date DESC);
CREATE INDEX idx_events_severity ON global_economic_events(severity, economic_impact_score DESC);

CREATE INDEX idx_event_impacts_country ON event_country_impacts(country_id, impact_magnitude DESC);

CREATE INDEX idx_leading_indicators_strength ON leading_indicators(correlation_strength DESC, predictive_accuracy DESC);

CREATE TRIGGER update_countries_updated_at
    BEFORE UPDATE ON countries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_indicators_updated_at
    BEFORE UPDATE ON global_economic_indicators
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_events_updated_at
    BEFORE UPDATE ON global_economic_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- USERS AND COLLABORATION
-- ============================================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    provider VARCHAR(50) NOT NULL DEFAULT 'email', -- 'google', 'facebook', 'email'
    provider_id VARCHAR(255), -- OAuth provider user ID
    password_hash VARCHAR(255), -- For email authentication
    role VARCHAR(50) NOT NULL DEFAULT 'viewer', -- 'admin', 'analyst', 'viewer'
    organization VARCHAR(255),

    -- User preferences
    theme VARCHAR(20) NOT NULL DEFAULT 'light',
    default_chart_type VARCHAR(50) NOT NULL DEFAULT 'line',
    notifications_enabled BOOLEAN NOT NULL DEFAULT true,
    collaboration_enabled BOOLEAN NOT NULL DEFAULT true,

    -- Metadata
    is_active BOOLEAN NOT NULL DEFAULT true,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    user_agent TEXT,
    ip_address TEXT -- Using TEXT instead of INET for better compatibility
);

CREATE TABLE chart_annotations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    series_id VARCHAR(255), -- Reference to economic series
    chart_id UUID, -- For custom chart groupings

    -- Annotation data
    annotation_date DATE NOT NULL,
    annotation_value DECIMAL(20, 6), -- Optional Y-axis value
    title VARCHAR(255) NOT NULL,
    description TEXT,
    color VARCHAR(7) DEFAULT '#2196f3', -- Hex color code
    annotation_type VARCHAR(20) DEFAULT 'line', -- 'line', 'point', 'box', 'trend'

    -- Metadata
    is_pinned BOOLEAN DEFAULT false,
    tags TEXT[], -- Array of tags

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    visibility annotation_visibility NOT NULL DEFAULT 'private'
);

CREATE TABLE annotation_comments (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    annotation_id UUID NOT NULL REFERENCES chart_annotations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    content TEXT NOT NULL,
    is_resolved BOOLEAN DEFAULT false,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Sharing permissions: one grant per user per chart.
CREATE TABLE chart_collaborators (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    chart_id UUID NOT NULL, -- Custom chart identifier
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID REFERENCES users(id),

    role VARCHAR(20) DEFAULT 'viewer', -- 'owner', 'editor', 'viewer'
    permissions JSONB DEFAULT '{"view": true, "annotate": false, "edit": false}',

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE,

    CONSTRAINT chart_collaborators_chart_user_unique UNIQUE (chart_id, user_id)
);

CREATE TABLE user_data_source_preferences (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    data_source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    is_visible BOOLEAN NOT NULL DEFAULT true,
    is_favorite BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, data_source_id)
);

CREATE INDEX idx_users_provider ON users(provider, provider_id);
CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(token_hash);
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at);
CREATE INDEX idx_chart_annotations_user_id ON chart_annotations(user_id);
CREATE INDEX idx_chart_annotations_series_id ON chart_annotations(series_id);
CREATE INDEX idx_chart_annotations_date ON chart_annotations(annotation_date);
CREATE INDEX idx_annotation_comments_annotation_id ON annotation_comments(annotation_id);
CREATE INDEX idx_annotation_comments_user_id ON annotation_comments(user_id);
CREATE INDEX idx_chart_collaborators_chart_id ON chart_collaborators(chart_id);
CREATE INDEX idx_chart_collaborators_user_id ON chart_collaborators(user_id);
CREATE INDEX idx_user_data_source_preferences_data_source_id ON user_data_source_preferences(data_source_id);
CREATE INDEX idx_user_data_source_preferences_visible ON user_data_source_preferences(is_visible);

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON chart_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON annotation_comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_user_data_source_preferences_updated_at
    BEFORE UPDATE ON user_data_source_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- CRAWL ATTEMPTS (PREDICTIVE CRAWLING)
-- ============================================================================

CREATE TABLE crawl_attempts (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Crawl attempt details
    crawl_method VARCHAR(50) NOT NULL, -- 'api', 'ftp', 'web_scrape', etc.
    crawl_url TEXT, -- URL or endpoint attempted
    http_status_code INTEGER, -- HTTP response status

    -- Data freshness tracking
    data_found BOOLEAN NOT NULL DEFAULT FALSE, -- Whether we found any data
    new_data_points INTEGER DEFAULT 0, -- Number of new data points found
    latest_data_date DATE, -- Date of the most recent data point found
    data_freshness_hours INTEGER, -- How fresh the data was (hours since publication)

    -- Error tracking
    success BOOLEAN NOT NULL DEFAULT FALSE, -- Whether crawl succeeded
    error_type VARCHAR(50), -- 'network', 'api_limit', 'data_format', 'not_found', etc.
    error_message TEXT, -- Detailed error message
    retry_count INTEGER DEFAULT 0, -- Number of retries attempted

    -- Performance metrics
    response_time_ms INTEGER, -- Response time in milliseconds
    data_size_bytes INTEGER, -- Size of data retrieved
    rate_limit_remaining INTEGER, -- API rate limit remaining

    -- Metadata
    user_agent TEXT, -- User agent used for request
    request_headers JSONB, -- Request headers sent
    response_headers JSONB, -- Response headers received

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_crawl_attempts_attempted_at ON crawl_attempts(attempted_at);
CREATE INDEX idx_crawl_attempts_data_found ON crawl_attempts(data_found);
CREATE INDEX idx_crawl_attempts_error_type ON crawl_attempts(error_type);
CREATE INDEX idx_crawl_attempts_latest_data_date ON crawl_attempts(latest_data_date);
CREATE INDEX idx_crawl_attempts_series_success ON crawl_attempts(series_id, success);
CREATE INDEX idx_crawl_attempts_series_attempted ON crawl_attempts(series_id, attempted_at);
CREATE INDEX idx_crawl_attempts_success_attempted ON crawl_attempts(success, attempted_at);

CREATE TRIGGER set_crawl_attempts_updated_at
    BEFORE UPDATE ON crawl_attempts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- AUDIT AND SECURITY
-- ============================================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_name VARCHAR(255) NOT NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255),
    ip_address TEXT,
    user_agent TEXT,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource_type ON audit_logs(resource_type);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);

CREATE TABLE security_events (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    event_type VARCHAR(50) NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    user_email VARCHAR(255),
    severity VARCHAR(20) NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    description TEXT NOT NULL,
    metadata JSONB,
    resolved BOOLEAN DEFAULT FALSE,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_security_events_event_type ON security_events(event_type);
CREATE INDEX idx_security_events_user_id ON security_events(user_id);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_resolved ON security_events(resolved);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);

-- ============================================================================
-- XBRL FINANCIAL DATA
-- ============================================================================

CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    cik VARCHAR(10) NOT NULL UNIQUE, -- SEC Central Index Key
    ticker VARCHAR(10), -- Stock ticker symbol (nullable - not all companies have public tickers)
    name VARCHAR(255) NOT NULL,
    legal_name VARCHAR(500), -- Full legal company name (nullable - may not always be available)
    sic_code VARCHAR(4), -- Standard Industrial Classification code (nullable - not always available)
    sic_description VARCHAR(255), -- SIC description (nullable - depends on sic_code)
    industry VARCHAR(100), -- Industry classification (nullable - derived field)
    sector VARCHAR(100), -- Sector classification (nullable - derived field)
    business_address JSONB, -- Company business address (nullable - not always available)
    mailing_address JSONB, -- Company mailing address (nullable - not always available)
    phone VARCHAR(50), -- Phone number (nullable - not always available)
    website VARCHAR(255), -- Website URL (nullable - not always available)
    state_of_incorporation VARCHAR(2), -- US state code (nullable - not always available)
    state_of_incorporation_description VARCHAR(100), -- State description (nullable - depends on state_of_incorporation)
    fiscal_year_end VARCHAR(4), -- MM-DD format (nullable - not always available)
    entity_type VARCHAR(50), -- Corporation, LLC, etc. (nullable - not always available)
    entity_size VARCHAR(20), -- Large Accelerated Filer, Accelerated Filer, etc. (nullable - not always available)
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE financial_statements (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,

    -- Filing information
    filing_type VARCHAR(10) NOT NULL, -- 10-K, 10-Q, 8-K, etc.
    form_type VARCHAR(10) NOT NULL, -- Same as filing_type for consistency
    accession_number VARCHAR(20) NOT NULL, -- SEC accession number
    filing_date DATE NOT NULL,
    period_end_date DATE NOT NULL,
    fiscal_year INTEGER NOT NULL,
    fiscal_quarter INTEGER, -- Nullable for annual reports

    -- Document information
    document_type VARCHAR(20) NOT NULL DEFAULT 'XBRL',
    document_url TEXT NOT NULL,

    -- XBRL processing status
    xbrl_processing_status processing_status NOT NULL DEFAULT 'pending',
    is_amended BOOLEAN NOT NULL DEFAULT FALSE,
    is_restated BOOLEAN NOT NULL DEFAULT FALSE,
    amendment_type VARCHAR(50), -- Type of amendment if applicable
    original_filing_date DATE, -- Original filing date if amended
    restatement_reason TEXT, -- Reason for restatement if applicable

    -- XBRL file storage
    xbrl_file_oid OID, -- PostgreSQL large object for XBRL file
    xbrl_file_content BYTEA, -- Alternative storage as bytea
    xbrl_file_size_bytes BIGINT, -- File size in bytes
    xbrl_file_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    xbrl_file_compression_type compression_type NOT NULL DEFAULT 'zstd',
    xbrl_file_hash VARCHAR(64), -- SHA-256 hash of the file

    -- Processing metadata
    xbrl_processing_error TEXT,
    xbrl_processing_started_at TIMESTAMPTZ,
    xbrl_processing_completed_at TIMESTAMPTZ,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_company_filing UNIQUE (company_id, accession_number),
    CONSTRAINT valid_file_storage CHECK (
        (xbrl_file_content IS NOT NULL AND xbrl_file_oid IS NULL) OR
        (xbrl_file_content IS NULL AND xbrl_file_oid IS NOT NULL) OR
        (xbrl_file_content IS NULL AND xbrl_file_oid IS NULL)
    )
);

CREATE TABLE financial_line_items (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,

    -- XBRL concept information
    taxonomy_concept VARCHAR(255) NOT NULL, -- XBRL concept name
    standard_label VARCHAR(255), -- Standard label from taxonomy
    custom_label VARCHAR(255), -- Company-specific label

    -- Financial data
    value NUMERIC(20,6), -- Financial value (nullable for calculated items)
    unit VARCHAR(20) NOT NULL DEFAULT 'USD', -- Unit of measurement
    context_ref VARCHAR(100) NOT NULL, -- XBRL context reference
    segment_ref VARCHAR(100), -- XBRL segment reference (nullable)
    scenario_ref VARCHAR(100), -- XBRL scenario reference (nullable)

    -- Precision and formatting
    precision INTEGER, -- XBRL precision attribute
    decimals INTEGER, -- XBRL decimals attribute

    -- Statement organization
    statement_type statement_type NOT NULL,
    statement_section statement_section NOT NULL,
    parent_concept VARCHAR(255), -- Parent concept for hierarchical organization
    level INTEGER NOT NULL DEFAULT 1, -- Hierarchy level
    order_index INTEGER, -- Display order within section

    -- Calculation information
    is_calculated BOOLEAN NOT NULL DEFAULT FALSE,
    calculation_formula TEXT, -- Formula for calculated items

    -- Accounting information
    is_credit BOOLEAN, -- Whether this is a credit item
    is_debit BOOLEAN, -- Whether this is a debit item

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE financial_ratios (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,

    -- Ratio information
    ratio_category ratio_category NOT NULL,
    ratio_name VARCHAR(100) NOT NULL, -- e.g., 'return_on_equity'
    ratio_value NUMERIC(10,6), -- Calculated ratio value
    ratio_formula TEXT, -- Formula used for calculation

    -- Calculation components
    numerator_value NUMERIC(20,6), -- Numerator value
    denominator_value NUMERIC(20,6), -- Denominator value

    -- Benchmarking data
    industry_average NUMERIC(10,6), -- Industry average
    sector_average NUMERIC(10,6), -- Sector average
    peer_median NUMERIC(10,6), -- Peer group median

    -- Analysis metadata
    calculation_method calculation_method NOT NULL DEFAULT 'simple',
    confidence_score NUMERIC(3,2), -- Confidence in calculation (0.00-1.00)
    data_quality_score NUMERIC(3,2), -- Data quality score (0.00-1.00)

    -- Timestamps
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- DTS (discoverable taxonomy set) support

CREATE TABLE xbrl_taxonomy_schemas (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Schema identification
    schema_namespace VARCHAR(255) NOT NULL,
    schema_filename VARCHAR(255) NOT NULL,
    schema_version VARCHAR(50),
    schema_date DATE,

    -- File metadata
    file_type taxonomy_file_type NOT NULL DEFAULT 'schema',
    source_type taxonomy_source_type NOT NULL,

    -- Storage information
    file_content BYTEA,
    file_oid OID,
    file_size_bytes BIGINT NOT NULL,
    file_hash VARCHAR(64) NOT NULL, -- SHA-256 hash

    -- Compression
    is_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    compression_type compression_type NOT NULL DEFAULT 'zstd',

    -- Source information
    source_url TEXT,
    download_url TEXT,
    original_filename VARCHAR(255),

    -- Processing status
    processing_status processing_status NOT NULL DEFAULT 'downloaded',
    processing_error TEXT,
    processing_started_at TIMESTAMPTZ,
    processing_completed_at TIMESTAMPTZ,

    -- Metadata
    concepts_extracted INTEGER DEFAULT 0,
    relationships_extracted INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_schema_namespace_version UNIQUE (schema_namespace, schema_version),
    CONSTRAINT valid_file_storage CHECK (
        (file_content IS NOT NULL AND file_oid IS NULL) OR
        (file_content IS NULL AND file_oid IS NOT NULL)
    )
);

CREATE TABLE xbrl_taxonomy_linkbases (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Linkbase identification
    linkbase_filename VARCHAR(255) NOT NULL,
    linkbase_type taxonomy_file_type NOT NULL,
    target_namespace VARCHAR(255),

    -- Related schema
    schema_id UUID REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,

    -- File metadata
    file_content BYTEA,
    file_oid OID,
    file_size_bytes BIGINT NOT NULL,
    file_hash VARCHAR(64) NOT NULL, -- SHA-256 hash

    -- Compression
    is_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    compression_type compression_type NOT NULL DEFAULT 'zstd',

    -- Source information
    source_url TEXT,
    download_url TEXT,
    original_filename VARCHAR(255),

    -- Processing status
    processing_status processing_status NOT NULL DEFAULT 'downloaded',
    processing_error TEXT,
    processing_started_at TIMESTAMPTZ,
    processing_completed_at TIMESTAMPTZ,

    -- Metadata
    relationships_extracted INTEGER DEFAULT 0,
    labels_extracted INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_linkbase_file_storage CHECK (
        (file_content IS NOT NULL AND file_oid IS NULL) OR
        (file_content IS NULL AND file_oid IS NOT NULL)
    )
);

CREATE TABLE xbrl_dts_dependencies (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Dependency relationship
    parent_schema_id UUID NOT NULL REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,
    child_schema_id UUID REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,
    child_namespace VARCHAR(255) NOT NULL, -- For external dependencies not yet downloaded

    -- Dependency metadata
    dependency_type VARCHAR(50) NOT NULL, -- 'import', 'include', 'reference'
    dependency_location TEXT, -- URL or path where dependency was found
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE, -- Whether child_schema_id is populated

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_dependency UNIQUE (parent_schema_id, child_namespace),
    CONSTRAINT valid_dependency CHECK (
        (is_resolved = TRUE AND child_schema_id IS NOT NULL) OR
        (is_resolved = FALSE AND child_schema_id IS NULL)
    )
);

CREATE TABLE xbrl_instance_dts_references (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Related financial statement
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,

    -- DTS reference information
    reference_type VARCHAR(20) NOT NULL, -- 'schemaRef', 'linkbaseRef'
    reference_role VARCHAR(255), -- xlink:role attribute
    reference_href TEXT NOT NULL, -- xlink:href attribute
    reference_arcrole VARCHAR(255), -- xlink:arcrole attribute (for linkbaseRef)

    -- Resolved taxonomy
    resolved_schema_id UUID REFERENCES xbrl_taxonomy_schemas(id),
    resolved_linkbase_id UUID REFERENCES xbrl_taxonomy_linkbases(id),

    -- Status
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE,
    resolution_error TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT valid_resolution CHECK (
        (is_resolved = TRUE AND (resolved_schema_id IS NOT NULL OR resolved_linkbase_id IS NOT NULL)) OR
        (is_resolved = FALSE AND resolved_schema_id IS NULL AND resolved_linkbase_id IS NULL)
    )
);

CREATE TABLE xbrl_taxonomy_concepts (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Basic concept information
    concept_name VARCHAR(255) NOT NULL,
    concept_qname VARCHAR(255),
    concept_namespace VARCHAR(255),
    concept_local_name VARCHAR(255),

    -- Related schema
    schema_id UUID REFERENCES xbrl_taxonomy_schemas(id),

    -- XBRL concept properties
    is_abstract BOOLEAN DEFAULT FALSE,
    is_nillable BOOLEAN DEFAULT TRUE,
    min_occurs INTEGER DEFAULT 1,
    max_occurs INTEGER DEFAULT 1,
    base_type VARCHAR(100),
    facet_constraints JSONB,

    -- Documentation and labels
    documentation_url TEXT,
    label_roles JSONB, -- Multiple label roles and their values

    -- Relationships
    calculation_relationships JSONB, -- Calculation linkbase relationships
    presentation_relationships JSONB, -- Presentation linkbase relationships
    definition_relationships JSONB, -- Definition linkbase relationships

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Collaborative annotations on financial statements

CREATE TABLE financial_annotations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    line_item_id UUID REFERENCES financial_line_items(id) ON DELETE CASCADE,

    -- Annotation content
    annotation_type annotation_type NOT NULL,
    title VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    status annotation_status NOT NULL DEFAULT 'active',

    -- User information
    created_by UUID NOT NULL, -- References users table (when available)
    assigned_to UUID, -- References users table (when available)

    -- Metadata
    priority INTEGER DEFAULT 1, -- 1-5 priority scale
    tags TEXT[], -- Array of tags for categorization

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE TABLE annotation_assignments (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    annotation_id UUID NOT NULL REFERENCES financial_annotations(id) ON DELETE CASCADE,
    assigned_to UUID NOT NULL, -- References users table (when available)
    assigned_by UUID NOT NULL, -- References users table (when available)

    -- Assignment details
    assignment_type assignment_type NOT NULL,
    status assignment_status NOT NULL DEFAULT 'pending',
    due_date TIMESTAMPTZ,
    instructions TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE TABLE annotation_replies (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    annotation_id UUID NOT NULL REFERENCES financial_annotations(id) ON DELETE CASCADE,
    parent_reply_id UUID REFERENCES annotation_replies(id) ON DELETE CASCADE,

    -- Reply content
    content TEXT NOT NULL,
    status annotation_status NOT NULL DEFAULT 'active',

    -- User information
    created_by UUID NOT NULL, -- References users table (when available)

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE annotation_templates (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

    -- Template information
    name VARCHAR(255) NOT NULL,
    description TEXT,
    annotation_type annotation_type NOT NULL,
    template_content TEXT NOT NULL,

    -- Usage metadata
    usage_count INTEGER DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,

    -- User information
    created_by UUID NOT NULL, -- References users table (when available)

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_companies_ticker ON companies(ticker);
CREATE INDEX idx_companies_name ON companies(name);
CREATE INDEX idx_companies_industry ON companies(industry);
CREATE INDEX idx_companies_sector ON companies(sector);
CREATE INDEX idx_companies_active ON companies(is_active);

CREATE INDEX idx_financial_statements_filing_type ON financial_statements(filing_type);
CREATE INDEX idx_financial_statements_period_end_date ON financial_statements(period_end_date);
CREATE INDEX idx_financial_statements_fiscal_year ON financial_statements(fiscal_year);
CREATE INDEX idx_financial_statements_processing_status ON financial_statements(xbrl_processing_status);
CREATE INDEX idx_financial_statements_created_at ON financial_statements(created_at);

CREATE INDEX idx_financial_line_items_statement_id ON financial_line_items(statement_id);
CREATE INDEX idx_financial_line_items_taxonomy_concept ON financial_line_items(taxonomy_concept);
CREATE INDEX idx_financial_line_items_statement_type ON financial_line_items(statement_type);
CREATE INDEX idx_financial_line_items_statement_section ON financial_line_items(statement_section);
CREATE INDEX idx_financial_line_items_parent_concept ON financial_line_items(parent_concept);
CREATE INDEX idx_financial_line_items_level ON financial_line_items(level);
CREATE INDEX idx_financial_line_items_order_index ON financial_line_items(order_index);

CREATE INDEX idx_financial_ratios_statement_id ON financial_ratios(statement_id);
CREATE INDEX idx_financial_ratios_category ON financial_ratios(ratio_category);
CREATE INDEX idx_financial_ratios_name ON financial_ratios(ratio_name);
CREATE INDEX idx_financial_ratios_calculated_at ON financial_ratios(calculated_at);

CREATE INDEX idx_xbrl_taxonomy_schemas_source_type ON xbrl_taxonomy_schemas(source_type);
CREATE INDEX idx_xbrl_taxonomy_schemas_status ON xbrl_taxonomy_schemas(processing_status);
CREATE INDEX idx_xbrl_taxonomy_schemas_created_at ON xbrl_taxonomy_schemas(created_at);

CREATE INDEX idx_xbrl_taxonomy_linkbases_schema_id ON xbrl_taxonomy_linkbases(schema_id);
CREATE INDEX idx_xbrl_taxonomy_linkbases_type ON xbrl_taxonomy_linkbases(linkbase_type);
CREATE INDEX idx_xbrl_taxonomy_linkbases_status ON xbrl_taxonomy_linkbases(processing_status);

CREATE INDEX idx_xbrl_dts_dependencies_child ON xbrl_dts_dependencies(child_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_resolved ON xbrl_dts_dependencies(is_resolved);

CREATE INDEX idx_xbrl_instance_dts_references_statement ON xbrl_instance_dts_references(statement_id);
CREATE INDEX idx_xbrl_instance_dts_references_type ON xbrl_instance_dts_references(reference_type);
CREATE INDEX idx_xbrl_instance_dts_references_resolved ON xbrl_instance_dts_references(is_resolved);

CREATE INDEX idx_xbrl_taxonomy_concepts_schema_id ON xbrl_taxonomy_concepts(schema_id);
CREATE INDEX idx_xbrl_taxonomy_concepts_qname ON xbrl_taxonomy_concepts(concept_qname);
CREATE INDEX idx_xbrl_taxonomy_concepts_namespace ON xbrl_taxonomy_concepts(concept_namespace);

CREATE INDEX idx_financial_annotations_statement_id ON financial_annotations(statement_id);
CREATE INDEX idx_financial_annotations_line_item_id ON financial_annotations(line_item_id);
CREATE INDEX idx_financial_annotations_type ON financial_annotations(annotation_type);
CREATE INDEX idx_financial_annotations_status ON financial_annotations(status);
CREATE INDEX idx_financial_annotations_created_by ON financial_annotations(created_by);

CREATE INDEX idx_annotation_assignments_annotation_id ON annotation_assignments(annotation_id);
CREATE INDEX idx_annotation_assignments_assigned_to ON annotation_assignments(assigned_to);
CREATE INDEX idx_annotation_assignments_status ON annotation_assignments(status);

CREATE INDEX idx_annotation_replies_annotation_id ON annotation_replies(annotation_id);
CREATE INDEX idx_annotation_replies_parent_reply_id ON annotation_replies(parent_reply_id);
CREATE INDEX idx_annotation_replies_created_by ON annotation_replies(created_by);

CREATE INDEX idx_annotation_templates_type ON annotation_templates(annotation_type);
CREATE INDEX idx_annotation_templates_active ON annotation_templates(is_active);

CREATE TRIGGER update_companies_updated_at
    BEFORE UPDATE ON companies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_statements_updated_at
    BEFORE UPDATE ON financial_statements
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_line_items_updated_at
    BEFORE UPDATE ON financial_line_items
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_ratios_updated_at
    BEFORE UPDATE ON financial_ratios
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_xbrl_taxonomy_schemas_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_schemas
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_xbrl_taxonomy_linkbases_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_linkbases
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_xbrl_taxonomy_concepts_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_concepts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_annotations_updated_at
    BEFORE UPDATE ON financial_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_assignments_updated_at
    BEFORE UPDATE ON annotation_assignments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_replies_updated_at
    BEFORE UPDATE ON annotation_replies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_templates_updated_at
    BEFORE UPDATE ON annotation_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Company financial statements with basic info
CREATE VIEW company_financial_statements AS
SELECT
    fs.id,
    fs.company_id,
    c.name as company_name,
    c.ticker,
    fs.filing_type,
    fs.period_end_date,
    fs.fiscal_year,
    fs.fiscal_quarter,
    fs.xbrl_processing_status,
    fs.is_amended,
    fs.is_restated,
    fs.created_at
FROM financial_statements fs
JOIN companies c ON fs.company_id = c.id
WHERE c.is_active = TRUE;

-- Financial line items with statement context
CREATE VIEW financial_line_items_with_context AS
SELECT
    fli.id,
    fli.statement_id,
    fli.taxonomy_concept,
    fli.standard_label,
    fli.custom_label,
    fli.value,
    fli.unit,
    fli.statement_type,
    fli.statement_section,
    fli.parent_concept,
    fli.level,
    fli.order_index,
    fli.is_calculated,
    fs.company_id,
    c.name as company_name,
    fs.period_end_date,
    fs.fiscal_year
FROM financial_line_items fli
JOIN financial_statements fs ON fli.statement_id = fs.id
JOIN companies c ON fs.company_id = c.id
WHERE c.is_active = TRUE;

-- Financial ratios with context
CREATE VIEW financial_ratios_with_context AS
SELECT
    fr.id,
    fr.statement_id,
    fr.ratio_category,
    fr.ratio_name,
    fr.ratio_value,
    fr.industry_average,
    fr.sector_average,
    fr.peer_median,
    fr.confidence_score,
    fr.data_quality_score,
    fr.calculated_at,
    fs.company_id,
    c.name as company_name,
    fs.period_end_date,
    fs.fiscal_year
FROM financial_ratios fr
JOIN financial_statements fs ON fr.statement_id = fs.id
JOIN companies c ON fs.company_id = c.id
WHERE c.is_active = TRUE;

-- ============================================================================
-- COMMENTS
-- ============================================================================

COMMENT ON TABLE countries IS 'Master table of countries with geographic and economic metadata for global analysis';
COMMENT ON TABLE global_economic_indicators IS 'Economic indicators available for cross-country analysis';
COMMENT ON TABLE global_indicator_data IS 'Time series data for global economic indicators';
COMMENT ON TABLE country_correlations IS 'Calculated correlations between countries for various economic indicators';
COMMENT ON TABLE trade_relationships IS 'Bilateral trade data between countries';
COMMENT ON TABLE global_economic_events IS 'Major global economic events and their characteristics';
COMMENT ON TABLE event_country_impacts IS 'Impact of global events on individual countries';
COMMENT ON TABLE leading_indicators IS 'Countries that lead others in economic indicators';

COMMENT ON COLUMN data_sources.api_key_name IS 'Name of the environment variable containing the API key for this data source. NULL if no API key is required.';

COMMENT ON COLUMN economic_series.id IS
    'UUIDv5 of SOURCE:external_id for crawled series (econ-graph-crawler series_id.rs); uuidv7() default otherwise';
COMMENT ON COLUMN economic_series.first_discovered_at IS 'When this series was first discovered by our crawler';
COMMENT ON COLUMN economic_series.last_crawled_at IS 'When we last attempted to crawl this specific series';
COMMENT ON COLUMN economic_series.first_missing_date IS 'First date we detected missing data (NULLable - series may still be active)';
COMMENT ON COLUMN economic_series.crawl_status IS 'Status of last crawl attempt (success, failed, pending, etc.)';
COMMENT ON COLUMN economic_series.crawl_error_message IS 'Error message from last failed crawl attempt';

COMMENT ON COLUMN series_metadata.id IS
    'UUIDv5 of SOURCE:external_id (econ-graph-crawler series_id.rs); uuidv7() default otherwise';

COMMENT ON TABLE audit_logs IS 'Comprehensive audit trail for all administrative and user actions';
COMMENT ON COLUMN audit_logs.user_name IS 'Name of user who performed the action (denormalized for performance)';
COMMENT ON COLUMN audit_logs.action IS 'Type of action performed (create, update, delete, login, etc.)';
COMMENT ON COLUMN audit_logs.resource_type IS 'Type of resource affected (user, series, chart, etc.)';
COMMENT ON COLUMN audit_logs.resource_id IS 'ID of the specific resource affected (nullable for global actions)';
COMMENT ON COLUMN audit_logs.details IS 'Additional context and metadata about the action';

COMMENT ON TABLE security_events IS 'Security-related events and incidents for monitoring and alerting';
COMMENT ON COLUMN security_events.event_type IS 'Type of security event (login_failure, suspicious_activity, etc.)';
COMMENT ON COLUMN security_events.severity IS 'Severity level (low, medium, high, critical)';
COMMENT ON COLUMN security_events.resolved IS 'Whether the security event has been resolved';
COMMENT ON COLUMN security_events.metadata IS 'Additional context and technical details about the security event';

DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'data_sources', 'data_points', 'crawl_queue', 'countries', 'global_economic_indicators',
        'global_indicator_data', 'country_correlations', 'trade_relationships',
        'global_economic_events', 'event_country_impacts', 'leading_indicators', 'users',
        'user_sessions', 'chart_annotations', 'annotation_comments', 'chart_collaborators',
        'audit_logs', 'security_events', 'user_data_source_preferences', 'crawl_attempts',
        'companies', 'financial_statements', 'financial_line_items', 'financial_ratios',
        'xbrl_taxonomy_schemas', 'xbrl_taxonomy_linkbases', 'xbrl_dts_dependencies',
        'xbrl_instance_dts_references', 'xbrl_taxonomy_concepts', 'financial_annotations',
        'annotation_assignments', 'annotation_replies', 'annotation_templates'
    ] LOOP
        EXECUTE format('COMMENT ON COLUMN %I.id IS %L', t,
                       'Primary key using UUIDv7 format for better performance and sortability');
    END LOOP;
END $$;

-- ============================================================================
-- SEED DATA
-- ============================================================================

INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute,
                          api_key_name, api_documentation_url, is_visible, is_enabled,
                          requires_admin_approval, crawl_frequency_hours, crawl_status) VALUES
    ('Federal Reserve Economic Data (FRED)', 'Economic data from the Federal Reserve Bank of St. Louis', 'https://api.stlouisfed.org/fred', true, 120, NULL, NULL, true, true, false, 6, 'active'),
    ('Bureau of Labor Statistics (BLS)', 'Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics', 'https://api.bls.gov/publicAPI/v2', true, 500, NULL, NULL, true, true, false, 12, 'active'),
    ('U.S. Census Bureau', 'Demographic and economic data from the U.S. Census Bureau', 'https://api.census.gov/data', true, 500, 'CENSUS_API_KEY', NULL, true, true, false, 24, 'disabled'),
    ('World Bank Open Data', 'Global economic and development indicators from the World Bank', 'https://api.worldbank.org/v2', false, 1000, NULL, NULL, true, true, false, 24, 'pending'),
    ('International Monetary Fund (IMF)', 'Global economic and financial data from the IMF', 'http://dataservices.imf.org', false, 60, NULL, 'https://data.imf.org/en/Resource-Pages/IMF-API', false, false, true, 24, 'disabled'),
    ('Bureau of Economic Analysis (BEA)', 'National economic accounts and GDP data from BEA', 'https://apps.bea.gov', false, 60, NULL, 'https://apps.bea.gov/api/bea_web_service_api_user_guide.htm', true, true, false, 24, 'pending'),
    ('SEC EDGAR', 'SEC Electronic Data Gathering, Analysis, and Retrieval system for XBRL financial filings', 'https://www.sec.gov/edgar', false, 10, NULL, NULL, true, true, false, 24, 'pending');

-- Discovery catalog seeds, on their stable ids. The crawler test `seeded_series_metadata_has_stable_ids`
-- checks these literals against the Rust function.
INSERT INTO series_metadata (id, source_id, external_id, title, description, units, frequency,
                             geographic_level, data_url, api_endpoint, is_active)
SELECT v.id, ds.id, v.external_id, v.title, v.description, v.units, v.frequency,
       v.geographic_level, v.data_url, v.api_endpoint, true
FROM (VALUES
    ('d8124fe6-ef1c-52dd-8d22-c1976625064c'::uuid, 'Federal Reserve Economic Data (FRED)', 'GDP', 'Gross Domestic Product', 'Real gross domestic product, seasonally adjusted annual rate', 'Billions of Chained 2017 Dollars', 'Quarterly', 'United States', 'https://fred.stlouisfed.org/series/GDP', 'https://api.stlouisfed.org/fred/series/observations?series_id=GDP&api_key=YOUR_API_KEY&file_type=json'),
    ('d2ac622b-7d71-5884-9994-0f939bd9407f'::uuid, 'Federal Reserve Economic Data (FRED)', 'UNRATE', 'Unemployment Rate', 'Unemployment rate, seasonally adjusted', 'Percent', 'Monthly', 'United States', 'https://fred.stlouisfed.org/series/UNRATE', 'https://api.stlouisfed.org/fred/series/observations?series_id=UNRATE&api_key=YOUR_API_KEY&file_type=json'),
    ('432b1389-3872-5ab3-b699-955bc6fec997'::uuid, 'Federal Reserve Economic Data (FRED)', 'CPIAUCSL', 'Consumer Price Index for All Urban Consumers', 'Consumer Price Index for All Urban Consumers: All Items in U.S. City Average', 'Index 1982-1984=100', 'Monthly', 'United States', 'https://fred.stlouisfed.org/series/CPIAUCSL', 'https://api.stlouisfed.org/fred/series/observations?series_id=CPIAUCSL&api_key=YOUR_API_KEY&file_type=json'),
    ('201c52b2-7f26-533b-a4d6-54e3141ee87b'::uuid, 'Bureau of Labor Statistics (BLS)', 'CES0000000001', 'All Employees, Total Nonfarm', 'Total nonfarm employment, seasonally adjusted', 'Thousands of Persons', 'Monthly', 'United States', 'https://data.bls.gov/timeseries/CES0000000001', 'https://api.bls.gov/publicAPI/v2/timeseries/data/CES0000000001'),
    ('90c186a5-6afa-524b-83e3-c15e6985e2f6'::uuid, 'Bureau of Labor Statistics (BLS)', 'LNS14000000', 'Unemployment Rate', 'Unemployment rate, seasonally adjusted', 'Percent', 'Monthly', 'United States', 'https://data.bls.gov/timeseries/LNS14000000', 'https://api.bls.gov/publicAPI/v2/timeseries/data/LNS14000000'),
    ('089deb5f-9ed3-55f4-9782-aab27b353620'::uuid, 'U.S. Census Bureau', 'B01001001', 'Total Population', 'Total population estimate', 'Persons', 'Annual', 'United States', 'https://api.census.gov/data/2023/pep/population', 'https://api.census.gov/data/2023/pep/population?get=B01001_001E&for=us:1'),
    ('dc6cab7c-d45e-56bd-8f76-d0943035703c'::uuid, 'U.S. Census Bureau', 'B19013_001E', 'Median Household Income', 'Median household income in the past 12 months', 'Dollars', 'Annual', 'United States', 'https://api.census.gov/data/2023/acs/acs5', 'https://api.census.gov/data/2023/acs/acs5?get=B19013_001E&for=us:1'),
    ('95ebb92e-0ab9-505a-82e9-89993c5d2217'::uuid, 'Bureau of Economic Analysis (BEA)', 'GDP', 'Gross Domestic Product', 'Gross domestic product, current dollars', 'Millions of Dollars', 'Quarterly', 'United States', 'https://apps.bea.gov/api/data', 'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=GDP&TableName=T10101&Frequency=Q&Year=2023&ResultFormat=JSON'),
    ('35f7aa40-693f-53fe-9494-ca4000dcb24c'::uuid, 'Bureau of Economic Analysis (BEA)', 'PCE', 'Personal Consumption Expenditures', 'Personal consumption expenditures, current dollars', 'Millions of Dollars', 'Quarterly', 'United States', 'https://apps.bea.gov/api/data', 'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=NIPA&TableName=T20301&Frequency=Q&Year=2023&ResultFormat=JSON'),
    ('dcf10e81-73ad-56b5-82a8-8e6371da8401'::uuid, 'International Monetary Fund (IMF)', 'NGDP_R_SA_XDC', 'Gross domestic product, real, seasonally adjusted', 'Gross domestic product, real, seasonally adjusted, national currency', 'National currency', 'Quarterly', 'Country', 'https://data.imf.org/regular.aspx?key=61545850', 'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_R_SA_XDC'),
    ('c9d09457-0aed-5ca9-88f6-87f2a0545916'::uuid, 'International Monetary Fund (IMF)', 'NGDP_XDC', 'Gross domestic product, current prices', 'Gross domestic product, current prices, national currency', 'National currency', 'Quarterly', 'Country', 'https://data.imf.org/regular.aspx?key=61545850', 'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_XDC')
) AS v(id, source_name, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint)
JOIN data_sources ds ON ds.name = v.source_name;

INSERT INTO countries (iso_code, iso_code_2, name, region, sub_region, income_group, latitude, longitude, currency_code) VALUES
    ('USA', 'US', 'United States', 'Americas', 'Northern America', 'High income', 39.8283, -98.5795, 'USD'),
    ('CHN', 'CN', 'China', 'Asia', 'Eastern Asia', 'Upper middle income', 35.8617, 104.1954, 'CNY'),
    ('JPN', 'JP', 'Japan', 'Asia', 'Eastern Asia', 'High income', 36.2048, 138.2529, 'JPY'),
    ('DEU', 'DE', 'Germany', 'Europe', 'Western Europe', 'High income', 51.1657, 10.4515, 'EUR'),
    ('GBR', 'GB', 'United Kingdom', 'Europe', 'Northern Europe', 'High income', 55.3781, -3.4360, 'GBP'),
    ('FRA', 'FR', 'France', 'Europe', 'Western Europe', 'High income', 46.2276, 2.2137, 'EUR'),
    ('IND', 'IN', 'India', 'Asia', 'Southern Asia', 'Lower middle income', 20.5937, 78.9629, 'INR'),
    ('ITA', 'IT', 'Italy', 'Europe', 'Southern Europe', 'High income', 41.8719, 12.5674, 'EUR'),
    ('BRA', 'BR', 'Brazil', 'Americas', 'South America', 'Upper middle income', -14.2350, -51.9253, 'BRL'),
    ('CAN', 'CA', 'Canada', 'Americas', 'Northern America', 'High income', 56.1304, -106.3468, 'CAD'),
    ('RUS', 'RU', 'Russian Federation', 'Europe', 'Eastern Europe', 'Upper middle income', 61.5240, 105.3188, 'RUB'),
    ('KOR', 'KR', 'South Korea', 'Asia', 'Eastern Asia', 'High income', 35.9078, 127.7669, 'KRW'),
    ('ESP', 'ES', 'Spain', 'Europe', 'Southern Europe', 'High income', 40.4637, -3.7492, 'EUR'),
    ('AUS', 'AU', 'Australia', 'Oceania', 'Australia and New Zealand', 'High income', -25.2744, 133.7751, 'AUD'),
    ('MEX', 'MX', 'Mexico', 'Americas', 'Central America', 'Upper middle income', 23.6345, -102.5528, 'MXN'),
    ('IDN', 'ID', 'Indonesia', 'Asia', 'South-Eastern Asia', 'Upper middle income', -0.7893, 113.9213, 'IDR'),
    ('NLD', 'NL', 'Netherlands', 'Europe', 'Western Europe', 'High income', 52.1326, 5.2913, 'EUR'),
    ('SAU', 'SA', 'Saudi Arabia', 'Asia', 'Western Asia', 'High income', 23.8859, 45.0792, 'SAR'),
    ('TUR', 'TR', 'Turkey', 'Asia', 'Western Asia', 'Upper middle income', 38.9637, 35.2433, 'TRY'),
    ('CHE', 'CH', 'Switzerland', 'Europe', 'Western Europe', 'High income', 46.8182, 8.2275, 'CHF');

INSERT INTO global_economic_events (name, description, event_type, severity, start_date, end_date, primary_country_id, economic_impact_score) VALUES
    ('2008 Global Financial Crisis', 'Global financial crisis originating from US subprime mortgage crisis', 'Crisis', 'Critical', '2007-12-01', '2009-06-01', (SELECT id FROM countries WHERE iso_code = 'USA'), 95.0),
    ('COVID-19 Pandemic', 'Global pandemic causing widespread economic disruption', 'Crisis', 'Critical', '2020-03-01', '2022-12-01', NULL, 98.0),
    ('European Debt Crisis', 'Sovereign debt crisis affecting eurozone countries', 'Crisis', 'High', '2010-01-01', '2012-12-01', (SELECT id FROM countries WHERE iso_code = 'DEU'), 75.0),
    ('Brexit', 'United Kingdom withdrawal from European Union', 'Policy', 'Medium', '2016-06-23', '2020-12-31', (SELECT id FROM countries WHERE iso_code = 'GBR'), 45.0),
    ('US-China Trade War', 'Trade dispute between United States and China', 'Policy', 'High', '2018-03-01', '2020-01-15', (SELECT id FROM countries WHERE iso_code = 'USA'), 65.0);

INSERT INTO annotation_templates (name, description, annotation_type, template_content, created_by) VALUES
    ('Revenue Analysis', 'Template for analyzing revenue trends and patterns', 'insight', 'Revenue Analysis:\n\n1. Trend Analysis:\n   - Period-over-period growth: [X]%\n   - Year-over-year growth: [Y]%\n\n2. Key Observations:\n   - [Observation 1]\n   - [Observation 2]\n\n3. Recommendations:\n   - [Recommendation 1]\n   - [Recommendation 2]', '00000000-0000-0000-0000-000000000000'),
    ('Risk Assessment', 'Template for identifying and assessing financial risks', 'risk', 'Risk Assessment:\n\n1. Identified Risks:\n   - [Risk 1]: [Description]\n   - [Risk 2]: [Description]\n\n2. Impact Assessment:\n   - [Risk 1]: [High/Medium/Low]\n   - [Risk 2]: [High/Medium/Low]\n\n3. Mitigation Strategies:\n   - [Strategy 1]\n   - [Strategy 2]', '00000000-0000-0000-0000-000000000000'),
    ('Data Quality Concern', 'Template for reporting data quality issues', 'concern', 'Data Quality Concern:\n\n1. Issue Description:\n   [Detailed description of the data quality issue]\n\n2. Affected Data:\n   - [Data point 1]\n   - [Data point 2]\n\n3. Recommended Actions:\n   - [Action 1]\n   - [Action 2]', '00000000-0000-0000-0000-000000000000');
