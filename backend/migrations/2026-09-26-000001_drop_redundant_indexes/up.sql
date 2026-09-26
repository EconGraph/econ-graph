-- Drop indexes that can never beat an existing index.
--
-- Each index below is a leading prefix of a UNIQUE constraint or a wider index on the same table,
-- so the planner can use the wider one for every query the narrow one serves. Keeping them only
-- costs write amplification and disk, which matters most on data_points.
-- crawl_queue indexes are left alone here because the crawler consolidation reworks them.

-- data_points: all covered by UNIQUE (series_id, date, revision_date, is_original_release)
DROP INDEX IF EXISTS idx_data_points_series_id;
DROP INDEX IF EXISTS idx_data_points_series_date;
DROP INDEX IF EXISTS idx_data_points_series_date_revision;
-- Two-valued column: the planner won't use this index for a real filter.
DROP INDEX IF EXISTS idx_data_points_is_original_release;

-- Covered by a UNIQUE constraint whose leading columns match
DROP INDEX IF EXISTS idx_data_sources_name;                     -- UNIQUE (name)
DROP INDEX IF EXISTS idx_economic_series_source_id;             -- UNIQUE (source_id, external_id)
DROP INDEX IF EXISTS idx_global_data_indicator_date;            -- UNIQUE (indicator_id, date)
DROP INDEX IF EXISTS idx_correlations_countries;                -- UNIQUE (country_a_id, country_b_id, ...)
DROP INDEX IF EXISTS idx_users_email;                           -- UNIQUE (email)
DROP INDEX IF EXISTS idx_user_data_source_preferences_user_id;  -- UNIQUE (user_id, data_source_id)
DROP INDEX IF EXISTS idx_series_metadata_source_id;             -- UNIQUE (source_id, external_id)
DROP INDEX IF EXISTS idx_companies_cik;                         -- UNIQUE (cik)
DROP INDEX IF EXISTS idx_financial_statements_company_id;       -- unique_company_filing
DROP INDEX IF EXISTS idx_xbrl_taxonomy_schemas_namespace;       -- unique_schema_namespace_version
DROP INDEX IF EXISTS idx_xbrl_dts_dependencies_parent;          -- unique_dependency

-- Covered by wider composite indexes on the same table
DROP INDEX IF EXISTS idx_crawl_attempts_series_id;              -- (series_id, attempted_at)
DROP INDEX IF EXISTS idx_crawl_attempts_success;                -- (success, attempted_at)
