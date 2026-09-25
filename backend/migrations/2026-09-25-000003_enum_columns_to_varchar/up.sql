-- Convert every Postgres ENUM column to VARCHAR + CHECK.
--
-- schema.rs declares these columns as Text/Varchar and the Rust enums in
-- econ-graph-core/src/enums.rs serialize as Text, so every diesel insert/update failed with
-- "column ... is of type compression_type but expression is of type text"
-- (e.g. XbrlStorage::store_xbrl_file). VARCHAR + CHECK keeps the value validation and
-- matches how crawl_queue.status works. Existing data is preserved (USING col::text).
--
-- annotation_type's CHECK also admits the extra values the Rust AnnotationType enum can
-- write (revenue_growth, cost_concern, cash_flow, balance_sheet, one_time_item,
-- industry_context); all other CHECK lists equal the former enum labels.

-- Views depend on some of these columns; drop and recreate them unchanged.
DROP VIEW IF EXISTS company_financial_statements;
DROP VIEW IF EXISTS financial_line_items_with_context;
DROP VIEW IF EXISTS financial_ratios_with_context;

-- financial_statements
ALTER TABLE financial_statements
    ALTER COLUMN xbrl_file_compression_type DROP DEFAULT,
    ALTER COLUMN xbrl_file_compression_type TYPE VARCHAR(20) USING xbrl_file_compression_type::text,
    ALTER COLUMN xbrl_file_compression_type SET DEFAULT 'zstd',
    ADD CONSTRAINT chk_financial_statements_xbrl_file_compression_type
        CHECK (xbrl_file_compression_type IN ('zstd', 'lz4', 'gzip', 'none')),
    ALTER COLUMN xbrl_processing_status DROP DEFAULT,
    ALTER COLUMN xbrl_processing_status TYPE VARCHAR(20) USING xbrl_processing_status::text,
    ALTER COLUMN xbrl_processing_status SET DEFAULT 'pending',
    ADD CONSTRAINT chk_financial_statements_xbrl_processing_status
        CHECK (xbrl_processing_status IN ('pending', 'downloaded', 'processing', 'completed', 'failed'));

-- financial_line_items
ALTER TABLE financial_line_items
    ALTER COLUMN statement_type TYPE VARCHAR(30) USING statement_type::text,
    ADD CONSTRAINT chk_financial_line_items_statement_type
        CHECK (statement_type IN ('income_statement', 'balance_sheet', 'cash_flow', 'equity')),
    ALTER COLUMN statement_section TYPE VARCHAR(30) USING statement_section::text,
    ADD CONSTRAINT chk_financial_line_items_statement_section
        CHECK (statement_section IN ('revenue', 'expenses', 'assets', 'liabilities', 'equity',
                                     'operating', 'investing', 'financing'));

-- financial_ratios
ALTER TABLE financial_ratios
    ALTER COLUMN ratio_category TYPE VARCHAR(30) USING ratio_category::text,
    ADD CONSTRAINT chk_financial_ratios_ratio_category
        CHECK (ratio_category IN ('profitability', 'liquidity', 'leverage', 'efficiency', 'market', 'growth')),
    ALTER COLUMN calculation_method DROP DEFAULT,
    ALTER COLUMN calculation_method TYPE VARCHAR(30) USING calculation_method::text,
    ALTER COLUMN calculation_method SET DEFAULT 'simple',
    ADD CONSTRAINT chk_financial_ratios_calculation_method
        CHECK (calculation_method IN ('simple', 'weighted_average', 'geometric_mean', 'median'));

-- xbrl_taxonomy_schemas
ALTER TABLE xbrl_taxonomy_schemas
    ALTER COLUMN file_type DROP DEFAULT,
    ALTER COLUMN file_type TYPE VARCHAR(30) USING file_type::text,
    ALTER COLUMN file_type SET DEFAULT 'schema',
    ADD CONSTRAINT chk_xbrl_taxonomy_schemas_file_type
        CHECK (file_type IN ('schema', 'label_linkbase', 'presentation_linkbase', 'calculation_linkbase',
                             'definition_linkbase', 'reference_linkbase', 'formula_linkbase')),
    ALTER COLUMN source_type TYPE VARCHAR(30) USING source_type::text,
    ADD CONSTRAINT chk_xbrl_taxonomy_schemas_source_type
        CHECK (source_type IN ('company_specific', 'us_gaap', 'sec_dei', 'fasb_srt', 'ifrs',
                               'other_standard', 'custom')),
    ALTER COLUMN compression_type DROP DEFAULT,
    ALTER COLUMN compression_type TYPE VARCHAR(20) USING compression_type::text,
    ALTER COLUMN compression_type SET DEFAULT 'zstd',
    ADD CONSTRAINT chk_xbrl_taxonomy_schemas_compression_type
        CHECK (compression_type IN ('zstd', 'lz4', 'gzip', 'none')),
    ALTER COLUMN processing_status DROP DEFAULT,
    ALTER COLUMN processing_status TYPE VARCHAR(20) USING processing_status::text,
    ALTER COLUMN processing_status SET DEFAULT 'downloaded',
    ADD CONSTRAINT chk_xbrl_taxonomy_schemas_processing_status
        CHECK (processing_status IN ('pending', 'downloaded', 'processing', 'completed', 'failed'));

-- xbrl_taxonomy_linkbases
ALTER TABLE xbrl_taxonomy_linkbases
    ALTER COLUMN linkbase_type TYPE VARCHAR(30) USING linkbase_type::text,
    ADD CONSTRAINT chk_xbrl_taxonomy_linkbases_linkbase_type
        CHECK (linkbase_type IN ('schema', 'label_linkbase', 'presentation_linkbase', 'calculation_linkbase',
                                 'definition_linkbase', 'reference_linkbase', 'formula_linkbase')),
    ALTER COLUMN compression_type DROP DEFAULT,
    ALTER COLUMN compression_type TYPE VARCHAR(20) USING compression_type::text,
    ALTER COLUMN compression_type SET DEFAULT 'zstd',
    ADD CONSTRAINT chk_xbrl_taxonomy_linkbases_compression_type
        CHECK (compression_type IN ('zstd', 'lz4', 'gzip', 'none')),
    ALTER COLUMN processing_status DROP DEFAULT,
    ALTER COLUMN processing_status TYPE VARCHAR(20) USING processing_status::text,
    ALTER COLUMN processing_status SET DEFAULT 'downloaded',
    ADD CONSTRAINT chk_xbrl_taxonomy_linkbases_processing_status
        CHECK (processing_status IN ('pending', 'downloaded', 'processing', 'completed', 'failed'));

-- financial_annotations
ALTER TABLE financial_annotations
    ALTER COLUMN annotation_type TYPE VARCHAR(30) USING annotation_type::text,
    ADD CONSTRAINT chk_financial_annotations_annotation_type
        CHECK (annotation_type IN ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity',
                                   'highlight', 'revenue_growth', 'cost_concern', 'cash_flow',
                                   'balance_sheet', 'one_time_item', 'industry_context')),
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE VARCHAR(20) USING status::text,
    ALTER COLUMN status SET DEFAULT 'active',
    ADD CONSTRAINT chk_financial_annotations_status
        CHECK (status IN ('active', 'resolved', 'archived'));

-- annotation_replies
ALTER TABLE annotation_replies
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE VARCHAR(20) USING status::text,
    ALTER COLUMN status SET DEFAULT 'active',
    ADD CONSTRAINT chk_annotation_replies_status
        CHECK (status IN ('active', 'resolved', 'archived'));

-- annotation_templates
ALTER TABLE annotation_templates
    ALTER COLUMN annotation_type TYPE VARCHAR(30) USING annotation_type::text,
    ADD CONSTRAINT chk_annotation_templates_annotation_type
        CHECK (annotation_type IN ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity',
                                   'highlight', 'revenue_growth', 'cost_concern', 'cash_flow',
                                   'balance_sheet', 'one_time_item', 'industry_context'));

-- annotation_assignments
ALTER TABLE annotation_assignments
    ALTER COLUMN assignment_type TYPE VARCHAR(20) USING assignment_type::text,
    ADD CONSTRAINT chk_annotation_assignments_assignment_type
        CHECK (assignment_type IN ('review', 'analyze', 'verify', 'approve', 'investigate')),
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE VARCHAR(20) USING status::text,
    ALTER COLUMN status SET DEFAULT 'pending',
    ADD CONSTRAINT chk_annotation_assignments_status
        CHECK (status IN ('pending', 'in_progress', 'completed', 'overdue', 'cancelled'));

-- Recreate the views exactly as in the consolidated initial schema.
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

-- No column uses these types any more.
DROP TYPE compression_type;
DROP TYPE processing_status;
DROP TYPE statement_type;
DROP TYPE statement_section;
DROP TYPE ratio_category;
DROP TYPE calculation_method;
DROP TYPE comparison_type;
DROP TYPE xbrl_data_type;
DROP TYPE period_type;
DROP TYPE balance_type;
DROP TYPE substitution_group;
DROP TYPE processing_step;
DROP TYPE taxonomy_file_type;
DROP TYPE taxonomy_source_type;
DROP TYPE annotation_type;
DROP TYPE annotation_status;
DROP TYPE assignment_type;
DROP TYPE assignment_status;
