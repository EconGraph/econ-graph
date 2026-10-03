-- Drops everything the baseline creates, leaving the database as 00000000000000_diesel_initial_setup
-- left it. All application data is lost.

DROP VIEW IF EXISTS financial_ratios_with_context;
DROP VIEW IF EXISTS financial_line_items_with_context;
DROP VIEW IF EXISTS company_financial_statements;

DROP TABLE IF EXISTS
    annotation_assignments,
    annotation_comments,
    annotation_replies,
    annotation_templates,
    audit_logs,
    chart_annotations,
    chart_collaborators,
    companies,
    countries,
    country_correlations,
    crawl_attempts,
    crawl_queue,
    data_points,
    data_sources,
    datasets,
    economic_series,
    event_country_impacts,
    financial_annotations,
    financial_line_items,
    financial_ratios,
    financial_statements,
    global_economic_events,
    global_economic_indicators,
    global_indicator_data,
    leading_indicators,
    security_events,
    series_metadata,
    trade_relationships,
    user_data_source_preferences,
    user_sessions,
    users,
    xbrl_dts_dependencies,
    xbrl_instance_dts_references,
    xbrl_taxonomy_concepts,
    xbrl_taxonomy_linkbases,
    xbrl_taxonomy_schemas;

DROP TYPE IF EXISTS
    annotation_status,
    annotation_type,
    annotation_visibility,
    assignment_status,
    assignment_type,
    balance_type,
    calculation_method,
    comparison_type,
    compression_type,
    period_type,
    processing_status,
    processing_step,
    ratio_category,
    statement_section,
    statement_type,
    substitution_group,
    taxonomy_file_type,
    taxonomy_source_type,
    xbrl_data_type;

DROP FUNCTION IF EXISTS dataset_components_valid(JSONB);
DROP FUNCTION IF EXISTS update_series_metadata_updated_at();
DROP FUNCTION IF EXISTS update_updated_at_column();

DROP EXTENSION IF EXISTS pg_trgm;
DROP EXTENSION IF EXISTS pgcrypto;
