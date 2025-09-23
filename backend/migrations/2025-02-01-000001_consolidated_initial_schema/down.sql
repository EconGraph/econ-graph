-- Consolidated Initial Schema Migration - DOWN
-- This migration consolidates all migrations from 2024-01-01 through 2025-09-21
-- WARNING: This will drop all tables and data!

-- Drop all tables in reverse dependency order (children first, parents last)
-- XBRL Financial Schema tables
DROP TABLE IF EXISTS annotation_templates CASCADE;
DROP TABLE IF EXISTS annotation_replies CASCADE;
DROP TABLE IF EXISTS annotation_assignments CASCADE;
DROP TABLE IF EXISTS financial_annotations CASCADE;
DROP TABLE IF EXISTS xbrl_taxonomy_concepts CASCADE;
DROP TABLE IF EXISTS xbrl_instance_dts_references CASCADE;
DROP TABLE IF EXISTS xbrl_dts_dependencies CASCADE;
DROP TABLE IF EXISTS xbrl_taxonomy_linkbases CASCADE;
DROP TABLE IF EXISTS xbrl_taxonomy_schemas CASCADE;
DROP TABLE IF EXISTS financial_ratios CASCADE;
DROP TABLE IF EXISTS financial_line_items CASCADE;
DROP TABLE IF EXISTS financial_statements CASCADE;
DROP TABLE IF EXISTS companies CASCADE;

-- Core system tables
DROP TABLE IF EXISTS security_events CASCADE;
DROP TABLE IF EXISTS audit_logs CASCADE;
DROP TABLE IF EXISTS series_metadata CASCADE;
DROP TABLE IF EXISTS crawl_attempts CASCADE;
DROP TABLE IF EXISTS user_data_source_preferences CASCADE;
DROP TABLE IF EXISTS annotation_comments CASCADE;
DROP TABLE IF EXISTS chart_annotations CASCADE;
DROP TABLE IF EXISTS chart_collaborators CASCADE;
DROP TABLE IF EXISTS user_sessions CASCADE;
DROP TABLE IF EXISTS users CASCADE;
DROP TABLE IF EXISTS event_country_impacts CASCADE;
DROP TABLE IF EXISTS global_economic_events CASCADE;
DROP TABLE IF EXISTS leading_indicators CASCADE;
DROP TABLE IF EXISTS trade_relationships CASCADE;
DROP TABLE IF EXISTS country_correlations CASCADE;
DROP TABLE IF EXISTS global_indicator_data CASCADE;
DROP TABLE IF EXISTS global_economic_indicators CASCADE;
DROP TABLE IF EXISTS countries CASCADE;
DROP TABLE IF EXISTS data_points CASCADE;
DROP TABLE IF EXISTS economic_series CASCADE;
DROP TABLE IF EXISTS crawl_queue CASCADE;
DROP TABLE IF EXISTS data_sources CASCADE;

-- Drop custom types
DROP TYPE IF EXISTS compression_type CASCADE;
DROP TYPE IF EXISTS processing_status CASCADE;
DROP TYPE IF EXISTS statement_type CASCADE;
DROP TYPE IF EXISTS statement_section CASCADE;
DROP TYPE IF EXISTS ratio_category CASCADE;
DROP TYPE IF EXISTS calculation_method CASCADE;
DROP TYPE IF EXISTS comparison_type CASCADE;
DROP TYPE IF EXISTS xbrl_data_type CASCADE;
DROP TYPE IF EXISTS period_type CASCADE;
DROP TYPE IF EXISTS balance_type CASCADE;
DROP TYPE IF EXISTS substitution_group CASCADE;
DROP TYPE IF EXISTS processing_step CASCADE;
DROP TYPE IF EXISTS taxonomy_file_type CASCADE;
DROP TYPE IF EXISTS taxonomy_source_type CASCADE;
DROP TYPE IF EXISTS annotation_type CASCADE;
DROP TYPE IF EXISTS annotation_status CASCADE;
DROP TYPE IF EXISTS assignment_type CASCADE;
DROP TYPE IF EXISTS assignment_status CASCADE;

-- Drop functions
DROP FUNCTION IF EXISTS update_updated_at_column() CASCADE;
DROP FUNCTION IF EXISTS update_series_metadata_updated_at() CASCADE;

-- Drop views
DROP VIEW IF EXISTS company_financial_statements CASCADE;
DROP VIEW IF EXISTS financial_line_items_with_context CASCADE;
DROP VIEW IF EXISTS financial_ratios_with_context CASCADE;
