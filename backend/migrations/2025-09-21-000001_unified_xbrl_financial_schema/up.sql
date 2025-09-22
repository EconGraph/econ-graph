-- Unified XBRL Financial Schema Migration
-- Created: 2025-09-21
-- Purpose: Complete XBRL financial data schema with DTS support
-- Consolidates: Financial schema, DTS support, and all related tables

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

-- Annotation types for collaborative features
CREATE TYPE annotation_type AS ENUM ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight');

-- Annotation status
CREATE TYPE annotation_status AS ENUM ('active', 'resolved', 'archived');

-- Assignment types
CREATE TYPE assignment_type AS ENUM ('review', 'analyze', 'verify', 'approve', 'investigate');

-- Assignment status
CREATE TYPE assignment_status AS ENUM ('pending', 'in_progress', 'completed', 'overdue', 'cancelled');

-- ============================================================================
-- CORE TABLES
-- ============================================================================

-- Companies table for storing company information
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Financial statements table
CREATE TABLE financial_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Financial line items table
CREATE TABLE financial_line_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Financial ratios table
CREATE TABLE financial_ratios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- ============================================================================
-- DTS (DISCOVERABLE TAXONOMY SET) SUPPORT TABLES
-- ============================================================================

-- XBRL taxonomy schemas table
CREATE TABLE xbrl_taxonomy_schemas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- XBRL taxonomy linkbases table
CREATE TABLE xbrl_taxonomy_linkbases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- DTS dependencies table
CREATE TABLE xbrl_dts_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- XBRL instance DTS references table
CREATE TABLE xbrl_instance_dts_references (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- XBRL taxonomy concepts table (extended)
CREATE TABLE xbrl_taxonomy_concepts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- ============================================================================
-- COLLABORATIVE ANNOTATION TABLES
-- ============================================================================

-- Financial annotations table
CREATE TABLE financial_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Annotation assignments table
CREATE TABLE annotation_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Annotation replies table
CREATE TABLE annotation_replies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
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

-- Annotation templates table
CREATE TABLE annotation_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

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

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Companies table indexes
CREATE INDEX idx_companies_cik ON companies(cik);
CREATE INDEX idx_companies_ticker ON companies(ticker);
CREATE INDEX idx_companies_name ON companies(name);
CREATE INDEX idx_companies_industry ON companies(industry);
CREATE INDEX idx_companies_sector ON companies(sector);
CREATE INDEX idx_companies_active ON companies(is_active);

-- Financial statements table indexes
CREATE INDEX idx_financial_statements_company_id ON financial_statements(company_id);
CREATE INDEX idx_financial_statements_filing_type ON financial_statements(filing_type);
CREATE INDEX idx_financial_statements_period_end_date ON financial_statements(period_end_date);
CREATE INDEX idx_financial_statements_fiscal_year ON financial_statements(fiscal_year);
CREATE INDEX idx_financial_statements_processing_status ON financial_statements(xbrl_processing_status);
CREATE INDEX idx_financial_statements_created_at ON financial_statements(created_at);

-- Financial line items table indexes
CREATE INDEX idx_financial_line_items_statement_id ON financial_line_items(statement_id);
CREATE INDEX idx_financial_line_items_taxonomy_concept ON financial_line_items(taxonomy_concept);
CREATE INDEX idx_financial_line_items_statement_type ON financial_line_items(statement_type);
CREATE INDEX idx_financial_line_items_statement_section ON financial_line_items(statement_section);
CREATE INDEX idx_financial_line_items_parent_concept ON financial_line_items(parent_concept);
CREATE INDEX idx_financial_line_items_level ON financial_line_items(level);
CREATE INDEX idx_financial_line_items_order_index ON financial_line_items(order_index);

-- Financial ratios table indexes
CREATE INDEX idx_financial_ratios_statement_id ON financial_ratios(statement_id);
CREATE INDEX idx_financial_ratios_category ON financial_ratios(ratio_category);
CREATE INDEX idx_financial_ratios_name ON financial_ratios(ratio_name);
CREATE INDEX idx_financial_ratios_calculated_at ON financial_ratios(calculated_at);

-- XBRL taxonomy schemas table indexes
CREATE INDEX idx_xbrl_taxonomy_schemas_namespace ON xbrl_taxonomy_schemas(schema_namespace);
CREATE INDEX idx_xbrl_taxonomy_schemas_source_type ON xbrl_taxonomy_schemas(source_type);
CREATE INDEX idx_xbrl_taxonomy_schemas_status ON xbrl_taxonomy_schemas(processing_status);
CREATE INDEX idx_xbrl_taxonomy_schemas_created_at ON xbrl_taxonomy_schemas(created_at);

-- XBRL taxonomy linkbases table indexes
CREATE INDEX idx_xbrl_taxonomy_linkbases_schema_id ON xbrl_taxonomy_linkbases(schema_id);
CREATE INDEX idx_xbrl_taxonomy_linkbases_type ON xbrl_taxonomy_linkbases(linkbase_type);
CREATE INDEX idx_xbrl_taxonomy_linkbases_status ON xbrl_taxonomy_linkbases(processing_status);

-- DTS dependencies table indexes
CREATE INDEX idx_xbrl_dts_dependencies_parent ON xbrl_dts_dependencies(parent_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_child ON xbrl_dts_dependencies(child_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_resolved ON xbrl_dts_dependencies(is_resolved);

-- XBRL instance DTS references table indexes
CREATE INDEX idx_xbrl_instance_dts_references_statement ON xbrl_instance_dts_references(statement_id);
CREATE INDEX idx_xbrl_instance_dts_references_type ON xbrl_instance_dts_references(reference_type);
CREATE INDEX idx_xbrl_instance_dts_references_resolved ON xbrl_instance_dts_references(is_resolved);

-- XBRL taxonomy concepts table indexes
CREATE INDEX idx_xbrl_taxonomy_concepts_schema_id ON xbrl_taxonomy_concepts(schema_id);
CREATE INDEX idx_xbrl_taxonomy_concepts_qname ON xbrl_taxonomy_concepts(concept_qname);
CREATE INDEX idx_xbrl_taxonomy_concepts_namespace ON xbrl_taxonomy_concepts(concept_namespace);

-- Financial annotations table indexes
CREATE INDEX idx_financial_annotations_statement_id ON financial_annotations(statement_id);
CREATE INDEX idx_financial_annotations_line_item_id ON financial_annotations(line_item_id);
CREATE INDEX idx_financial_annotations_type ON financial_annotations(annotation_type);
CREATE INDEX idx_financial_annotations_status ON financial_annotations(status);
CREATE INDEX idx_financial_annotations_created_by ON financial_annotations(created_by);

-- Annotation assignments table indexes
CREATE INDEX idx_annotation_assignments_annotation_id ON annotation_assignments(annotation_id);
CREATE INDEX idx_annotation_assignments_assigned_to ON annotation_assignments(assigned_to);
CREATE INDEX idx_annotation_assignments_status ON annotation_assignments(status);

-- Annotation replies table indexes
CREATE INDEX idx_annotation_replies_annotation_id ON annotation_replies(annotation_id);
CREATE INDEX idx_annotation_replies_parent_reply_id ON annotation_replies(parent_reply_id);
CREATE INDEX idx_annotation_replies_created_by ON annotation_replies(created_by);

-- Annotation templates table indexes
CREATE INDEX idx_annotation_templates_type ON annotation_templates(annotation_type);
CREATE INDEX idx_annotation_templates_active ON annotation_templates(is_active);

-- ============================================================================
-- TRIGGERS AND FUNCTIONS
-- ============================================================================

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for updated_at columns
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

-- ============================================================================
-- VIEWS FOR COMMON QUERIES
-- ============================================================================

-- View for company financial statements with basic info
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

-- View for financial line items with statement context
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

-- View for financial ratios with context
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
-- SAMPLE DATA
-- ============================================================================

-- Add columns to data_sources table if they don't exist (for compatibility with later migrations)
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS is_visible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS is_enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS requires_admin_approval BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS crawl_frequency_hours INTEGER NOT NULL DEFAULT 24;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS last_crawl_at TIMESTAMPTZ;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS crawl_status VARCHAR(50) DEFAULT 'pending';
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS crawl_error_message TEXT;
ALTER TABLE data_sources ADD COLUMN IF NOT EXISTS api_documentation_url VARCHAR(500);

-- Create indexes for data_sources if they don't exist
CREATE INDEX IF NOT EXISTS idx_data_sources_visible ON data_sources(is_visible);
CREATE INDEX IF NOT EXISTS idx_data_sources_enabled ON data_sources(is_enabled);
CREATE INDEX IF NOT EXISTS idx_data_sources_crawl_status ON data_sources(crawl_status);

-- Insert default data sources for SEC EDGAR (only if it doesn't exist)
INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, crawl_status)
SELECT 'SEC EDGAR', 'SEC Electronic Data Gathering, Analysis, and Retrieval system for XBRL financial filings', 'https://www.sec.gov/edgar', false, 10, true, true, false, 24, 'pending'
WHERE NOT EXISTS (SELECT 1 FROM data_sources WHERE name = 'SEC EDGAR');

-- Insert sample annotation templates
INSERT INTO annotation_templates (name, description, annotation_type, template_content, created_by) VALUES
    ('Revenue Analysis', 'Template for analyzing revenue trends and patterns', 'insight', 'Revenue Analysis:\n\n1. Trend Analysis:\n   - Period-over-period growth: [X]%\n   - Year-over-year growth: [Y]%\n\n2. Key Observations:\n   - [Observation 1]\n   - [Observation 2]\n\n3. Recommendations:\n   - [Recommendation 1]\n   - [Recommendation 2]', '00000000-0000-0000-0000-000000000000'),
    ('Risk Assessment', 'Template for identifying and assessing financial risks', 'risk', 'Risk Assessment:\n\n1. Identified Risks:\n   - [Risk 1]: [Description]\n   - [Risk 2]: [Description]\n\n2. Impact Assessment:\n   - [Risk 1]: [High/Medium/Low]\n   - [Risk 2]: [High/Medium/Low]\n\n3. Mitigation Strategies:\n   - [Strategy 1]\n   - [Strategy 2]', '00000000-0000-0000-0000-000000000000'),
    ('Data Quality Concern', 'Template for reporting data quality issues', 'concern', 'Data Quality Concern:\n\n1. Issue Description:\n   [Detailed description of the data quality issue]\n\n2. Affected Data:\n   - [Data point 1]\n   - [Data point 2]\n\n3. Recommended Actions:\n   - [Action 1]\n   - [Action 2]', '00000000-0000-0000-0000-000000000000');
