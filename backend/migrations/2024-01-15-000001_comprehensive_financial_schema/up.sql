-- Comprehensive Financial Data Schema Migration
-- Creates tables for SEC EDGAR XBRL financial data storage and analysis with enums
-- This extends the existing economic data schema with financial statement capabilities

-- Create enum types first
CREATE TYPE compression_type AS ENUM ('zstd', 'lz4', 'gzip', 'none');
CREATE TYPE processing_status AS ENUM ('pending', 'processing', 'completed', 'failed');
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
CREATE TYPE annotation_type AS ENUM ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight');
CREATE TYPE annotation_status AS ENUM ('active', 'resolved', 'archived');
CREATE TYPE assignment_type AS ENUM ('review', 'analyze', 'verify', 'approve', 'investigate');
CREATE TYPE assignment_status AS ENUM ('pending', 'in_progress', 'completed', 'overdue', 'cancelled');

-- Create companies table for storing company information
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

-- Create index on CIK for fast lookups
CREATE INDEX idx_companies_cik ON companies(cik);
CREATE INDEX idx_companies_ticker ON companies(ticker);
CREATE INDEX idx_companies_name ON companies(name);
CREATE INDEX idx_companies_industry ON companies(industry);
CREATE INDEX idx_companies_sector ON companies(sector);

-- Create financial statements table
CREATE TABLE financial_statements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    filing_type VARCHAR(10) NOT NULL, -- 10-K, 10-Q, 8-K, etc. (string - from external SEC sources)
    form_type VARCHAR(20) NOT NULL, -- 10-K, 10-Q, 8-K, etc. (string - from external SEC sources)
    accession_number VARCHAR(20) NOT NULL, -- SEC accession number
    filing_date DATE NOT NULL,
    period_end_date DATE NOT NULL,
    fiscal_year INTEGER NOT NULL,
    fiscal_quarter INTEGER, -- 1, 2, 3, 4 for quarterly filings (nullable - annual filings don't have quarters)
    document_type VARCHAR(50) NOT NULL, -- 10-K, 10-Q, etc. (string - from external SEC sources)
    document_url TEXT NOT NULL, -- URL to the filing document (required - always available from SEC)
    xbrl_file_oid OID, -- PostgreSQL Large Object OID for XBRL file (nullable - file may not be downloaded yet)
    xbrl_file_content BYTEA, -- Alternative: store as bytea for smaller files (nullable - file may not be downloaded yet)
    xbrl_file_size_bytes BIGINT, -- Size of XBRL file (nullable - file may not be downloaded yet)
    xbrl_file_compressed BOOLEAN NOT NULL DEFAULT TRUE, -- Whether file is compressed (always known)
    xbrl_file_compression_type compression_type NOT NULL DEFAULT 'zstd', -- Enum: zstd, lz4, gzip, none
    xbrl_file_hash VARCHAR(64), -- SHA-256 hash for integrity verification (nullable - file may not be downloaded yet)
    xbrl_processing_status processing_status NOT NULL DEFAULT 'pending', -- Enum: pending, processing, completed, failed
    xbrl_processing_error TEXT, -- Error message if processing failed (nullable - only present on failure)
    xbrl_processing_started_at TIMESTAMPTZ, -- When processing started (nullable - not started yet)
    xbrl_processing_completed_at TIMESTAMPTZ, -- When processing completed (nullable - not completed yet)
    is_amended BOOLEAN NOT NULL DEFAULT FALSE, -- True if this is an amended filing
    amendment_type VARCHAR(20), -- Type of amendment if applicable (string - from external SEC sources)
    original_filing_date DATE, -- Original filing date if amended (nullable - only present if amended)
    is_restated BOOLEAN NOT NULL DEFAULT FALSE, -- True if this filing contains restatements
    restatement_reason TEXT, -- Reason for restatement (nullable - only present if restated)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for financial statements
CREATE INDEX idx_financial_statements_company_id ON financial_statements(company_id);
CREATE INDEX idx_financial_statements_filing_type ON financial_statements(filing_type);
CREATE INDEX idx_financial_statements_filing_date ON financial_statements(filing_date);
CREATE INDEX idx_financial_statements_period_end_date ON financial_statements(period_end_date);
CREATE INDEX idx_financial_statements_fiscal_year ON financial_statements(fiscal_year);
CREATE INDEX idx_financial_statements_fiscal_quarter ON financial_statements(fiscal_quarter);
CREATE INDEX idx_financial_statements_accession_number ON financial_statements(accession_number);
CREATE INDEX idx_financial_statements_processing_status ON financial_statements(xbrl_processing_status);

-- Create financial line items table for storing individual financial statement line items
CREATE TABLE financial_line_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    taxonomy_concept VARCHAR(255) NOT NULL, -- XBRL taxonomy concept name
    standard_label VARCHAR(255), -- Standard label from taxonomy (nullable - may not be available)
    custom_label VARCHAR(255), -- Custom label if different from standard (nullable - may not be available)
    value DECIMAL(20,2), -- Financial value (nullable - some concepts may not have values)
    unit VARCHAR(50) NOT NULL, -- Unit of measurement (USD, shares, etc.) (required - always present in XBRL)
    context_ref VARCHAR(255) NOT NULL, -- XBRL context reference (required - always present in XBRL)
    segment_ref VARCHAR(255), -- XBRL segment reference for dimensional data (nullable - not all items have segments)
    scenario_ref VARCHAR(255), -- XBRL scenario reference (nullable - not all items have scenarios)
    precision INTEGER, -- Decimal precision of the value (nullable - may not be specified)
    decimals INTEGER, -- Number of decimal places (nullable - may not be specified)
    is_credit BOOLEAN, -- True if this is a credit balance item (nullable - may not be determinable)
    is_debit BOOLEAN, -- True if this is a debit balance item (nullable - may not be determinable)
    statement_type statement_type NOT NULL, -- Enum: income_statement, balance_sheet, cash_flow, equity
    statement_section statement_section NOT NULL, -- Enum: revenue, expenses, assets, liabilities, equity, operating, investing, financing
    parent_concept VARCHAR(255), -- Parent concept in hierarchy (nullable - top-level items don't have parents)
    level INTEGER NOT NULL DEFAULT 0, -- Hierarchy level (0 = top level)
    order_index INTEGER, -- Display order within statement (nullable - may not be specified)
    is_calculated BOOLEAN NOT NULL DEFAULT FALSE, -- True if this is a calculated value
    calculation_formula TEXT, -- Formula used for calculation (nullable - only present for calculated items)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for financial line items
CREATE INDEX idx_financial_line_items_statement_id ON financial_line_items(statement_id);
CREATE INDEX idx_financial_line_items_taxonomy_concept ON financial_line_items(taxonomy_concept);
CREATE INDEX idx_financial_line_items_statement_type ON financial_line_items(statement_type);
CREATE INDEX idx_financial_line_items_statement_section ON financial_line_items(statement_section);
CREATE INDEX idx_financial_line_items_parent_concept ON financial_line_items(parent_concept);
CREATE INDEX idx_financial_line_items_level ON financial_line_items(level);

-- Create financial ratios table for storing calculated financial ratios
CREATE TABLE financial_ratios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    ratio_category ratio_category NOT NULL, -- Enum: profitability, liquidity, leverage, efficiency, market, growth
    ratio_name VARCHAR(100) NOT NULL, -- Current Ratio, ROE, etc.
    ratio_value DECIMAL(10,4), -- Calculated ratio value
    ratio_formula TEXT, -- Formula used for calculation
    numerator_value DECIMAL(20,2), -- Numerator value
    denominator_value DECIMAL(20,2), -- Denominator value
    numerator_concept VARCHAR(255), -- XBRL concept for numerator
    denominator_concept VARCHAR(255), -- XBRL concept for denominator
    calculation_method calculation_method, -- Enum: simple, weighted_average, geometric_mean, median
    is_industry_standard BOOLEAN DEFAULT TRUE, -- True if this is a standard industry ratio
    benchmark_value DECIMAL(10,4), -- Industry benchmark value
    benchmark_percentile INTEGER, -- Percentile ranking (1-100)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for financial ratios
CREATE INDEX idx_financial_ratios_statement_id ON financial_ratios(statement_id);
CREATE INDEX idx_financial_ratios_ratio_category ON financial_ratios(ratio_category);
CREATE INDEX idx_financial_ratios_ratio_name ON financial_ratios(ratio_name);

-- Create company comparisons table for storing peer group comparisons
CREATE TABLE company_comparisons (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    peer_company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    comparison_type comparison_type NOT NULL, -- Enum: industry, sector, size, geographic, custom
    comparison_metrics JSONB, -- Metrics being compared
    comparison_period_start DATE NOT NULL,
    comparison_period_end DATE NOT NULL,
    similarity_score DECIMAL(5,4), -- Similarity score (0-1)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for company comparisons
CREATE INDEX idx_company_comparisons_company_id ON company_comparisons(company_id);
CREATE INDEX idx_company_comparisons_peer_company_id ON company_comparisons(peer_company_id);
CREATE INDEX idx_company_comparisons_comparison_type ON company_comparisons(comparison_type);

-- Create XBRL taxonomy concepts table for storing taxonomy metadata
CREATE TABLE xbrl_taxonomy_concepts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    concept_name VARCHAR(255) NOT NULL UNIQUE,
    standard_label VARCHAR(255),
    documentation VARCHAR(1000), -- Concept documentation
    data_type xbrl_data_type, -- Enum: monetaryItemType, sharesItemType, etc.
    period_type period_type, -- Enum: duration, instant
    balance_type balance_type, -- Enum: debit, credit
    substitution_group substitution_group, -- Enum: item, tuple
    abstract BOOLEAN DEFAULT FALSE, -- True if this is an abstract concept
    nillable BOOLEAN DEFAULT TRUE, -- True if concept can be nil
    taxonomy_version VARCHAR(50), -- US-GAAP-2023, etc.
    namespace_uri TEXT, -- XBRL namespace URI
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for XBRL taxonomy concepts
CREATE INDEX idx_xbrl_taxonomy_concepts_concept_name ON xbrl_taxonomy_concepts(concept_name);
CREATE INDEX idx_xbrl_taxonomy_concepts_taxonomy_version ON xbrl_taxonomy_concepts(taxonomy_version);
CREATE INDEX idx_xbrl_taxonomy_concepts_data_type ON xbrl_taxonomy_concepts(data_type);

-- Create XBRL processing logs table for tracking processing status
CREATE TABLE xbrl_processing_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    processing_step processing_step NOT NULL, -- Enum: download, parse, validate, store, extract, calculate
    status processing_status NOT NULL, -- Enum: pending, processing, completed, failed
    error_message TEXT,
    processing_time_ms INTEGER, -- Processing time in milliseconds
    records_processed INTEGER, -- Number of records processed
    records_failed INTEGER, -- Number of records that failed
    metadata JSONB, -- Additional processing metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for XBRL processing logs
CREATE INDEX idx_xbrl_processing_logs_statement_id ON xbrl_processing_logs(statement_id);
CREATE INDEX idx_xbrl_processing_logs_processing_step ON xbrl_processing_logs(processing_step);
CREATE INDEX idx_xbrl_processing_logs_status ON xbrl_processing_logs(status);
CREATE INDEX idx_xbrl_processing_logs_created_at ON xbrl_processing_logs(created_at);

-- Create financial annotations table for collaborative analysis
CREATE TABLE financial_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    line_item_id UUID REFERENCES financial_line_items(id) ON DELETE CASCADE, -- Optional for statement-level annotations
    author_id UUID NOT NULL, -- References user/analyst who created annotation
    content TEXT NOT NULL, -- Annotation content
    annotation_type annotation_type NOT NULL, -- Enum: comment, question, concern, insight, risk, opportunity, highlight
    tags TEXT[], -- Array of tags for categorization
    highlights JSONB, -- Highlight ranges and colors
    mentions UUID[], -- Array of user IDs mentioned in annotation
    parent_annotation_id UUID REFERENCES financial_annotations(id) ON DELETE CASCADE, -- For threaded discussions
    status annotation_status DEFAULT 'active', -- Enum: active, resolved, archived
    is_private BOOLEAN DEFAULT FALSE, -- Private annotations visible only to author
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create annotation replies table for threaded discussions
CREATE TABLE annotation_replies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    annotation_id UUID NOT NULL REFERENCES financial_annotations(id) ON DELETE CASCADE,
    author_id UUID NOT NULL, -- References user/analyst who created reply
    content TEXT NOT NULL, -- Reply content
    mentions UUID[], -- Array of user IDs mentioned in reply
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create annotation assignments table for team workflow
CREATE TABLE annotation_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,
    line_item_id UUID REFERENCES financial_line_items(id) ON DELETE CASCADE,
    assignee_id UUID NOT NULL, -- User assigned to analyze this item
    assigner_id UUID NOT NULL, -- User who made the assignment
    assignment_type assignment_type NOT NULL, -- Enum: review, analyze, verify, approve, investigate
    due_date TIMESTAMPTZ,
    status assignment_status DEFAULT 'pending', -- Enum: pending, in_progress, completed, overdue, cancelled
    notes TEXT, -- Assignment notes or instructions
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create annotation templates table for reusable annotation patterns
CREATE TABLE annotation_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    template_content TEXT NOT NULL, -- Template annotation content
    annotation_type annotation_type NOT NULL, -- Enum: comment, question, concern, insight, risk, opportunity, highlight
    tags TEXT[], -- Default tags for this template
    is_public BOOLEAN DEFAULT FALSE, -- Public templates available to all users
    created_by UUID NOT NULL, -- User who created the template
    usage_count INTEGER DEFAULT 0, -- How many times this template has been used
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for performance
CREATE INDEX idx_financial_annotations_statement_id ON financial_annotations(statement_id);
CREATE INDEX idx_financial_annotations_line_item_id ON financial_annotations(line_item_id);
CREATE INDEX idx_financial_annotations_author_id ON financial_annotations(author_id);
CREATE INDEX idx_financial_annotations_type ON financial_annotations(annotation_type);
CREATE INDEX idx_financial_annotations_status ON financial_annotations(status);
CREATE INDEX idx_financial_annotations_created_at ON financial_annotations(created_at);
CREATE INDEX idx_financial_annotations_parent_id ON financial_annotations(parent_annotation_id);

CREATE INDEX idx_annotation_replies_annotation_id ON annotation_replies(annotation_id);
CREATE INDEX idx_annotation_replies_author_id ON annotation_replies(author_id);
CREATE INDEX idx_annotation_replies_created_at ON annotation_replies(created_at);

CREATE INDEX idx_annotation_assignments_statement_id ON annotation_assignments(statement_id);
CREATE INDEX idx_annotation_assignments_line_item_id ON annotation_assignments(line_item_id);
CREATE INDEX idx_annotation_assignments_assignee_id ON annotation_assignments(assignee_id);
CREATE INDEX idx_annotation_assignments_status ON annotation_assignments(status);
CREATE INDEX idx_annotation_assignments_due_date ON annotation_assignments(due_date);

CREATE INDEX idx_annotation_templates_created_by ON annotation_templates(created_by);
CREATE INDEX idx_annotation_templates_is_public ON annotation_templates(is_public);
CREATE INDEX idx_annotation_templates_type ON annotation_templates(annotation_type);

-- Create updated_at triggers for all tables
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_companies_updated_at
    BEFORE UPDATE ON companies
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_statements_updated_at
    BEFORE UPDATE ON financial_statements
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_line_items_updated_at
    BEFORE UPDATE ON financial_line_items
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_ratios_updated_at
    BEFORE UPDATE ON financial_ratios
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_company_comparisons_updated_at
    BEFORE UPDATE ON company_comparisons
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_xbrl_taxonomy_concepts_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_concepts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_financial_annotations_updated_at
    BEFORE UPDATE ON financial_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_replies_updated_at
    BEFORE UPDATE ON annotation_replies
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_assignments_updated_at
    BEFORE UPDATE ON annotation_assignments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_templates_updated_at
    BEFORE UPDATE ON annotation_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default data sources for SEC EDGAR
INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute, is_visible, is_enabled, crawl_frequency_hours) VALUES
    ('SEC EDGAR', 'SEC Electronic Data Gathering, Analysis, and Retrieval system for XBRL financial filings', 'https://www.sec.gov/edgar', false, 10, true, true, 24);

-- Create views for common financial data queries

-- View for company financial statements with basic info
CREATE VIEW company_financial_statements AS
SELECT
    fs.id,
    fs.company_id,
    c.cik,
    c.ticker,
    c.name as company_name,
    fs.filing_type,
    fs.filing_date,
    fs.period_end_date,
    fs.fiscal_year,
    fs.fiscal_quarter,
    fs.xbrl_processing_status,
    fs.created_at
FROM financial_statements fs
JOIN companies c ON fs.company_id = c.id;

-- View for financial line items with company and statement info
CREATE VIEW financial_line_items_detailed AS
SELECT
    fli.id,
    fli.statement_id,
    fs.company_id,
    c.cik,
    c.ticker,
    c.name as company_name,
    fs.filing_type,
    fs.filing_date,
    fs.period_end_date,
    fs.fiscal_year,
    fs.fiscal_quarter,
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
    fli.created_at
FROM financial_line_items fli
JOIN financial_statements fs ON fli.statement_id = fs.id
JOIN companies c ON fs.company_id = c.id;

-- View for financial ratios with company and statement info
CREATE VIEW financial_ratios_detailed AS
SELECT
    fr.id,
    fr.statement_id,
    fs.company_id,
    c.cik,
    c.ticker,
    c.name as company_name,
    fs.filing_type,
    fs.filing_date,
    fs.period_end_date,
    fs.fiscal_year,
    fs.fiscal_quarter,
    fr.ratio_category,
    fr.ratio_name,
    fr.ratio_value,
    fr.ratio_formula,
    fr.numerator_value,
    fr.denominator_value,
    fr.benchmark_value,
    fr.benchmark_percentile,
    fr.created_at
FROM financial_ratios fr
JOIN financial_statements fs ON fr.statement_id = fs.id
JOIN companies c ON fs.company_id = c.id;
