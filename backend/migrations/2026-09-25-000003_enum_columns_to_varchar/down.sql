-- Restore the Postgres ENUM types and column types.
-- annotation_type keeps the extra values the VARCHAR CHECK admitted, so rows using them survive
-- the rollback.

CREATE TYPE compression_type AS ENUM ('zstd', 'lz4', 'gzip', 'none');
CREATE TYPE processing_status AS ENUM ('pending', 'downloaded', 'processing', 'completed', 'failed');
CREATE TYPE statement_type AS ENUM ('income_statement', 'balance_sheet', 'cash_flow', 'equity');
CREATE TYPE statement_section AS ENUM ('revenue', 'expenses', 'assets', 'liabilities', 'equity', 'operating', 'investing', 'financing');
CREATE TYPE ratio_category AS ENUM ('profitability', 'liquidity', 'leverage', 'efficiency', 'market', 'growth');
CREATE TYPE calculation_method AS ENUM ('simple', 'weighted_average', 'geometric_mean', 'median');
CREATE TYPE comparison_type AS ENUM ('industry', 'sector', 'size', 'geographic', 'custom');
CREATE TYPE xbrl_data_type AS ENUM ('monetaryItemType', 'sharesItemType', 'stringItemType', 'decimalItemType', 'integerItemType', 'booleanItemType', 'dateItemType', 'timeItemType');
CREATE TYPE period_type AS ENUM ('duration', 'instant');
CREATE TYPE balance_type AS ENUM ('debit', 'credit');
CREATE TYPE substitution_group AS ENUM ('item', 'tuple');
CREATE TYPE processing_step AS ENUM ('download', 'parse', 'validate', 'store', 'extract', 'calculate');
CREATE TYPE taxonomy_file_type AS ENUM ('schema', 'label_linkbase', 'presentation_linkbase', 'calculation_linkbase', 'definition_linkbase', 'reference_linkbase', 'formula_linkbase');
CREATE TYPE taxonomy_source_type AS ENUM ('company_specific', 'us_gaap', 'sec_dei', 'fasb_srt', 'ifrs', 'other_standard', 'custom');
CREATE TYPE annotation_type AS ENUM ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight', 'revenue_growth', 'cost_concern', 'cash_flow', 'balance_sheet', 'one_time_item', 'industry_context');
CREATE TYPE annotation_status AS ENUM ('active', 'resolved', 'archived');
CREATE TYPE assignment_type AS ENUM ('review', 'analyze', 'verify', 'approve', 'investigate');
CREATE TYPE assignment_status AS ENUM ('pending', 'in_progress', 'completed', 'overdue', 'cancelled');

DROP VIEW IF EXISTS company_financial_statements;
DROP VIEW IF EXISTS financial_line_items_with_context;
DROP VIEW IF EXISTS financial_ratios_with_context;

ALTER TABLE financial_statements
    DROP CONSTRAINT chk_financial_statements_xbrl_file_compression_type,
    DROP CONSTRAINT chk_financial_statements_xbrl_processing_status,
    ALTER COLUMN xbrl_file_compression_type DROP DEFAULT,
    ALTER COLUMN xbrl_file_compression_type TYPE compression_type USING xbrl_file_compression_type::compression_type,
    ALTER COLUMN xbrl_file_compression_type SET DEFAULT 'zstd',
    ALTER COLUMN xbrl_processing_status DROP DEFAULT,
    ALTER COLUMN xbrl_processing_status TYPE processing_status USING xbrl_processing_status::processing_status,
    ALTER COLUMN xbrl_processing_status SET DEFAULT 'pending';

ALTER TABLE financial_line_items
    DROP CONSTRAINT chk_financial_line_items_statement_type,
    DROP CONSTRAINT chk_financial_line_items_statement_section,
    ALTER COLUMN statement_type TYPE statement_type USING statement_type::statement_type,
    ALTER COLUMN statement_section TYPE statement_section USING statement_section::statement_section;

ALTER TABLE financial_ratios
    DROP CONSTRAINT chk_financial_ratios_ratio_category,
    DROP CONSTRAINT chk_financial_ratios_calculation_method,
    ALTER COLUMN ratio_category TYPE ratio_category USING ratio_category::ratio_category,
    ALTER COLUMN calculation_method DROP DEFAULT,
    ALTER COLUMN calculation_method TYPE calculation_method USING calculation_method::calculation_method,
    ALTER COLUMN calculation_method SET DEFAULT 'simple';

ALTER TABLE xbrl_taxonomy_schemas
    DROP CONSTRAINT chk_xbrl_taxonomy_schemas_file_type,
    DROP CONSTRAINT chk_xbrl_taxonomy_schemas_source_type,
    DROP CONSTRAINT chk_xbrl_taxonomy_schemas_compression_type,
    DROP CONSTRAINT chk_xbrl_taxonomy_schemas_processing_status,
    ALTER COLUMN file_type DROP DEFAULT,
    ALTER COLUMN file_type TYPE taxonomy_file_type USING file_type::taxonomy_file_type,
    ALTER COLUMN file_type SET DEFAULT 'schema',
    ALTER COLUMN source_type TYPE taxonomy_source_type USING source_type::taxonomy_source_type,
    ALTER COLUMN compression_type DROP DEFAULT,
    ALTER COLUMN compression_type TYPE compression_type USING compression_type::compression_type,
    ALTER COLUMN compression_type SET DEFAULT 'zstd',
    ALTER COLUMN processing_status DROP DEFAULT,
    ALTER COLUMN processing_status TYPE processing_status USING processing_status::processing_status,
    ALTER COLUMN processing_status SET DEFAULT 'downloaded';

ALTER TABLE xbrl_taxonomy_linkbases
    DROP CONSTRAINT chk_xbrl_taxonomy_linkbases_linkbase_type,
    DROP CONSTRAINT chk_xbrl_taxonomy_linkbases_compression_type,
    DROP CONSTRAINT chk_xbrl_taxonomy_linkbases_processing_status,
    ALTER COLUMN linkbase_type TYPE taxonomy_file_type USING linkbase_type::taxonomy_file_type,
    ALTER COLUMN compression_type DROP DEFAULT,
    ALTER COLUMN compression_type TYPE compression_type USING compression_type::compression_type,
    ALTER COLUMN compression_type SET DEFAULT 'zstd',
    ALTER COLUMN processing_status DROP DEFAULT,
    ALTER COLUMN processing_status TYPE processing_status USING processing_status::processing_status,
    ALTER COLUMN processing_status SET DEFAULT 'downloaded';

ALTER TABLE financial_annotations
    DROP CONSTRAINT chk_financial_annotations_annotation_type,
    DROP CONSTRAINT chk_financial_annotations_status,
    ALTER COLUMN annotation_type TYPE annotation_type USING annotation_type::annotation_type,
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE annotation_status USING status::annotation_status,
    ALTER COLUMN status SET DEFAULT 'active';

ALTER TABLE annotation_replies
    DROP CONSTRAINT chk_annotation_replies_status,
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE annotation_status USING status::annotation_status,
    ALTER COLUMN status SET DEFAULT 'active';

ALTER TABLE annotation_templates
    DROP CONSTRAINT chk_annotation_templates_annotation_type,
    ALTER COLUMN annotation_type TYPE annotation_type USING annotation_type::annotation_type;

ALTER TABLE annotation_assignments
    DROP CONSTRAINT chk_annotation_assignments_assignment_type,
    DROP CONSTRAINT chk_annotation_assignments_status,
    ALTER COLUMN assignment_type TYPE assignment_type USING assignment_type::assignment_type,
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE assignment_status USING status::assignment_status,
    ALTER COLUMN status SET DEFAULT 'pending';

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
