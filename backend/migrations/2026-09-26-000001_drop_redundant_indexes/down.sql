CREATE INDEX IF NOT EXISTS idx_crawl_attempts_success ON crawl_attempts (success);
CREATE INDEX IF NOT EXISTS idx_crawl_attempts_series_id ON crawl_attempts (series_id);

CREATE INDEX IF NOT EXISTS idx_xbrl_dts_dependencies_parent ON xbrl_dts_dependencies (parent_schema_id);
CREATE INDEX IF NOT EXISTS idx_xbrl_taxonomy_schemas_namespace ON xbrl_taxonomy_schemas (schema_namespace);
CREATE INDEX IF NOT EXISTS idx_financial_statements_company_id ON financial_statements (company_id);
CREATE INDEX IF NOT EXISTS idx_companies_cik ON companies (cik);
CREATE INDEX IF NOT EXISTS idx_series_metadata_source_id ON series_metadata (source_id);
CREATE INDEX IF NOT EXISTS idx_user_data_source_preferences_user_id ON user_data_source_preferences (user_id);
CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);
CREATE INDEX IF NOT EXISTS idx_correlations_countries ON country_correlations (country_a_id, country_b_id);
CREATE INDEX IF NOT EXISTS idx_global_data_indicator_date ON global_indicator_data (indicator_id, date DESC);
CREATE INDEX IF NOT EXISTS idx_economic_series_source_id ON economic_series (source_id);
CREATE INDEX IF NOT EXISTS idx_data_sources_name ON data_sources (name);

CREATE INDEX IF NOT EXISTS idx_data_points_is_original_release ON data_points (is_original_release);
CREATE INDEX IF NOT EXISTS idx_data_points_series_date_revision ON data_points (series_id, date, revision_date);
CREATE INDEX IF NOT EXISTS idx_data_points_series_date ON data_points (series_id, date);
CREATE INDEX IF NOT EXISTS idx_data_points_series_id ON data_points (series_id);
