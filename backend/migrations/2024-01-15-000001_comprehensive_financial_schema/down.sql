-- Comprehensive Financial Data Schema Migration - Rollback
-- This migration removes all financial data tables and enum types

-- Drop views first
DROP VIEW IF EXISTS financial_ratios_detailed;
DROP VIEW IF EXISTS financial_line_items_detailed;
DROP VIEW IF EXISTS company_financial_statements;

-- Drop triggers
DROP TRIGGER IF EXISTS update_annotation_templates_updated_at ON annotation_templates;
DROP TRIGGER IF EXISTS update_annotation_assignments_updated_at ON annotation_assignments;
DROP TRIGGER IF EXISTS update_annotation_replies_updated_at ON annotation_replies;
DROP TRIGGER IF EXISTS update_financial_annotations_updated_at ON financial_annotations;
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_concepts_updated_at ON xbrl_taxonomy_concepts;
DROP TRIGGER IF EXISTS update_company_comparisons_updated_at ON company_comparisons;
DROP TRIGGER IF EXISTS update_financial_ratios_updated_at ON financial_ratios;
DROP TRIGGER IF EXISTS update_financial_line_items_updated_at ON financial_line_items;
DROP TRIGGER IF EXISTS update_financial_statements_updated_at ON financial_statements;
DROP TRIGGER IF EXISTS update_companies_updated_at ON companies;

-- Drop tables (in reverse dependency order)
DROP TABLE IF EXISTS annotation_templates;
DROP TABLE IF EXISTS annotation_assignments;
DROP TABLE IF EXISTS annotation_replies;
DROP TABLE IF EXISTS financial_annotations;
DROP TABLE IF EXISTS xbrl_processing_logs;
DROP TABLE IF EXISTS xbrl_taxonomy_concepts;
DROP TABLE IF EXISTS company_comparisons;
DROP TABLE IF EXISTS financial_ratios;
DROP TABLE IF EXISTS financial_line_items;
DROP TABLE IF EXISTS financial_statements;
DROP TABLE IF EXISTS companies;

-- Drop enum types
DROP TYPE IF EXISTS assignment_status;
DROP TYPE IF EXISTS assignment_type;
DROP TYPE IF EXISTS annotation_status;
DROP TYPE IF EXISTS annotation_type;
DROP TYPE IF EXISTS processing_step;
DROP TYPE IF EXISTS substitution_group;
DROP TYPE IF EXISTS balance_type;
DROP TYPE IF EXISTS period_type;
DROP TYPE IF EXISTS xbrl_data_type;
DROP TYPE IF EXISTS comparison_type;
DROP TYPE IF EXISTS calculation_method;
DROP TYPE IF EXISTS ratio_category;
DROP TYPE IF EXISTS statement_section;
DROP TYPE IF EXISTS statement_type;
DROP TYPE IF EXISTS processing_status;
DROP TYPE IF EXISTS compression_type;

-- Remove SEC EDGAR data source
DELETE FROM data_sources WHERE name = 'SEC EDGAR';
