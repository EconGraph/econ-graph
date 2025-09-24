-- Unified XBRL Financial Schema Migration - Rollback
-- Created: 2025-09-21
-- Purpose: Rollback all XBRL financial data schema changes

-- ============================================================================
-- DROP VIEWS
-- ============================================================================

DROP VIEW IF EXISTS financial_ratios_with_context;
DROP VIEW IF EXISTS financial_line_items_with_context;
DROP VIEW IF EXISTS company_financial_statements;

-- ============================================================================
-- DROP TRIGGERS
-- ============================================================================

DROP TRIGGER IF EXISTS update_annotation_templates_updated_at ON annotation_templates;
DROP TRIGGER IF EXISTS update_annotation_replies_updated_at ON annotation_replies;
DROP TRIGGER IF EXISTS update_annotation_assignments_updated_at ON annotation_assignments;
DROP TRIGGER IF EXISTS update_financial_annotations_updated_at ON financial_annotations;
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_concepts_updated_at ON xbrl_taxonomy_concepts;
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_linkbases_updated_at ON xbrl_taxonomy_linkbases;
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_schemas_updated_at ON xbrl_taxonomy_schemas;
DROP TRIGGER IF EXISTS update_financial_ratios_updated_at ON financial_ratios;
DROP TRIGGER IF EXISTS update_financial_line_items_updated_at ON financial_line_items;
DROP TRIGGER IF EXISTS update_financial_statements_updated_at ON financial_statements;
DROP TRIGGER IF EXISTS update_companies_updated_at ON companies;

-- ============================================================================
-- DROP FUNCTIONS
-- ============================================================================

DROP FUNCTION IF EXISTS update_updated_at_column();

-- ============================================================================
-- DROP TABLES (in reverse dependency order)
-- ============================================================================

-- Drop collaborative annotation tables
DROP TABLE IF EXISTS annotation_templates CASCADE;
DROP TABLE IF EXISTS annotation_replies CASCADE;
DROP TABLE IF EXISTS annotation_assignments CASCADE;
DROP TABLE IF EXISTS financial_annotations CASCADE;

-- Drop DTS support tables
DROP TABLE IF EXISTS xbrl_taxonomy_concepts CASCADE;
DROP TABLE IF EXISTS xbrl_instance_dts_references CASCADE;
DROP TABLE IF EXISTS xbrl_dts_dependencies CASCADE;
DROP TABLE IF EXISTS xbrl_taxonomy_linkbases CASCADE;
DROP TABLE IF EXISTS xbrl_taxonomy_schemas CASCADE;

-- Drop financial data tables
DROP TABLE IF EXISTS financial_ratios CASCADE;
DROP TABLE IF EXISTS financial_line_items CASCADE;
DROP TABLE IF EXISTS financial_statements CASCADE;
DROP TABLE IF EXISTS companies CASCADE;

-- ============================================================================
-- CLEAN UP DATA_SOURCES TABLE
-- ============================================================================

-- Remove is_visible and is_enabled columns from data_sources table
DROP INDEX IF EXISTS idx_data_sources_visible;
ALTER TABLE data_sources DROP COLUMN IF EXISTS is_visible;
ALTER TABLE data_sources DROP COLUMN IF EXISTS is_enabled;

-- ============================================================================
-- DROP ENUM TYPES
-- ============================================================================

DROP TYPE IF EXISTS assignment_status CASCADE;
DROP TYPE IF EXISTS assignment_type CASCADE;
DROP TYPE IF EXISTS annotation_status CASCADE;
DROP TYPE IF EXISTS annotation_type CASCADE;
DROP TYPE IF EXISTS substitution_group CASCADE;
DROP TYPE IF EXISTS balance_type CASCADE;
DROP TYPE IF EXISTS period_type CASCADE;
DROP TYPE IF EXISTS xbrl_data_type CASCADE;
DROP TYPE IF EXISTS comparison_type CASCADE;
DROP TYPE IF EXISTS calculation_method CASCADE;
DROP TYPE IF EXISTS ratio_category CASCADE;
DROP TYPE IF EXISTS statement_section CASCADE;
DROP TYPE IF EXISTS statement_type CASCADE;
DROP TYPE IF EXISTS processing_status CASCADE;
DROP TYPE IF EXISTS compression_type CASCADE;
DROP TYPE IF EXISTS taxonomy_source_type CASCADE;
DROP TYPE IF EXISTS taxonomy_file_type CASCADE;
DROP TYPE IF EXISTS processing_step CASCADE;
