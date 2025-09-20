-- Financial Data Schema Migration
-- Creates tables for SEC EDGAR XBRL financial data storage and analysis
-- This extends the existing economic data schema with financial statement capabilities

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
    filing_type VARCHAR(10) NOT NULL, -- 10-K, 10-Q, 8-K, etc.
    form_type VARCHAR(20) NOT NULL, -- 10-K, 10-Q, 8-K, etc. (required - always available from SEC)
    accession_number VARCHAR(20) NOT NULL, -- SEC accession number
    filing_date DATE NOT NULL,
    period_end_date DATE NOT NULL,
    fiscal_year INTEGER NOT NULL,
    fiscal_quarter INTEGER, -- 1, 2, 3, 4 for quarterly filings (nullable - annual filings don't have quarters)
    document_type VARCHAR(50) NOT NULL, -- 10-K, 10-Q, etc. (required - always available)
    document_url TEXT NOT NULL, -- URL to the filing document (required - always available from SEC)
    xbrl_file_oid OID, -- PostgreSQL Large Object OID for XBRL file (nullable - file may not be downloaded yet)
    xbrl_file_content BYTEA, -- Alternative: store as bytea for smaller files (nullable - file may not be downloaded yet)
    xbrl_file_size_bytes BIGINT, -- Size of XBRL file (nullable - file may not be downloaded yet)
    xbrl_file_compressed BOOLEAN NOT NULL DEFAULT TRUE, -- Whether file is compressed (always known)
    xbrl_file_compression_type VARCHAR(10) NOT NULL DEFAULT 'zstd', -- zstd, lz4, or none (always known)
    xbrl_file_hash VARCHAR(64), -- SHA-256 hash for integrity verification (nullable - file may not be downloaded yet)
    xbrl_processing_status VARCHAR(20) NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    xbrl_processing_error TEXT, -- Error message if processing failed (nullable - only present on failure)
    xbrl_processing_started_at TIMESTAMPTZ, -- When processing started (nullable - not started yet)
    xbrl_processing_completed_at TIMESTAMPTZ, -- When processing completed (nullable - not completed yet)
    is_amended BOOLEAN NOT NULL DEFAULT FALSE, -- True if this is an amended filing
    amendment_type VARCHAR(20), -- Type of amendment if applicable (nullable - only present if amended)
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
    statement_type VARCHAR(20) NOT NULL, -- income_statement, balance_sheet, cash_flow, equity (required - must be categorized)
    statement_section VARCHAR(50) NOT NULL, -- revenue, expenses, assets, liabilities, etc. (required - must be categorized)
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
    ratio_category VARCHAR(50) NOT NULL, -- profitability, liquidity, leverage, efficiency
    ratio_name VARCHAR(100) NOT NULL, -- Current Ratio, ROE, etc.
    ratio_value DECIMAL(10,4), -- Calculated ratio value
    ratio_formula TEXT, -- Formula used for calculation
    numerator_value DECIMAL(20,2), -- Numerator value
    denominator_value DECIMAL(20,2), -- Denominator value
    numerator_concept VARCHAR(255), -- XBRL concept for numerator
    denominator_concept VARCHAR(255), -- XBRL concept for denominator
    calculation_method VARCHAR(50), -- simple, weighted_average, etc.
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
    comparison_type VARCHAR(50) NOT NULL, -- industry, sector, size, custom
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
    data_type VARCHAR(50), -- monetaryItemType, sharesItemType, etc.
    period_type VARCHAR(20), -- duration, instant
    balance_type VARCHAR(20), -- debit, credit
    substitution_group VARCHAR(50), -- item, tuple
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
    processing_step VARCHAR(50) NOT NULL, -- download, parse, validate, store
    status VARCHAR(20) NOT NULL, -- started, completed, failed
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

-- Create updated_at triggers for all tables
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
