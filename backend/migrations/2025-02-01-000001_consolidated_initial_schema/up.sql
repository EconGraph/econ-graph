-- Initial Schema Migration
-- Creates the core economic data tables: data_sources, economic_series, data_points, and crawl_queue
-- This consolidates the original 4 separate migrations into one cohesive schema

-- Enable required PostgreSQL extensions
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create data_sources table
CREATE TABLE data_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    base_url VARCHAR(500) NOT NULL,
    api_key_required BOOLEAN NOT NULL DEFAULT FALSE,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index on name for faster lookups
CREATE INDEX idx_data_sources_name ON data_sources(name);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create updated_at trigger for data_sources
CREATE TRIGGER update_data_sources_updated_at
    BEFORE UPDATE ON data_sources
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert default data sources
INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute) VALUES
    ('Federal Reserve Economic Data (FRED)', 'Economic data from the Federal Reserve Bank of St. Louis', 'https://api.stlouisfed.org/fred', true, 120),
    ('Bureau of Labor Statistics (BLS)', 'Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics', 'https://api.bls.gov/publicAPI/v2', true, 500),
    ('U.S. Census Bureau', 'Demographic and economic data from the U.S. Census Bureau', 'https://api.census.gov/data', true, 500),
    ('World Bank Open Data', 'Global economic and development indicators from the World Bank', 'https://api.worldbank.org/v2', false, 1000);

-- Create economic_series table
CREATE TABLE economic_series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50) NOT NULL,
    seasonal_adjustment VARCHAR(100),
    last_updated TIMESTAMPTZ,
    start_date DATE,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique external_id per source
    UNIQUE(source_id, external_id)
);

-- Create indexes for faster queries
CREATE INDEX idx_economic_series_source_id ON economic_series(source_id);
CREATE INDEX idx_economic_series_external_id ON economic_series(external_id);
CREATE INDEX idx_economic_series_title ON economic_series USING GIN (to_tsvector('english', title));
CREATE INDEX idx_economic_series_description ON economic_series USING GIN (to_tsvector('english', description));
CREATE INDEX idx_economic_series_frequency ON economic_series(frequency);
CREATE INDEX idx_economic_series_is_active ON economic_series(is_active);
CREATE INDEX idx_economic_series_last_updated ON economic_series(last_updated);

-- Create updated_at trigger for economic_series
CREATE TRIGGER update_economic_series_updated_at
    BEFORE UPDATE ON economic_series
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create data_points table
CREATE TABLE data_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    revision_date DATE NOT NULL,
    is_original_release BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique combination of series, date, and revision
    UNIQUE(series_id, date, revision_date, is_original_release)
);

-- Create indexes for faster queries
CREATE INDEX idx_data_points_series_id ON data_points(series_id);
CREATE INDEX idx_data_points_date ON data_points(date);
CREATE INDEX idx_data_points_revision_date ON data_points(revision_date);
CREATE INDEX idx_data_points_is_original_release ON data_points(is_original_release);
CREATE INDEX idx_data_points_series_date ON data_points(series_id, date);
CREATE INDEX idx_data_points_series_date_revision ON data_points(series_id, date, revision_date);

-- Create updated_at trigger for data_points
CREATE TRIGGER update_data_points_updated_at
    BEFORE UPDATE ON data_points
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create index for latest revisions (performance optimization)
CREATE INDEX idx_data_points_latest_revision ON data_points(series_id, date, revision_date DESC, value);

-- Create crawl_queue table
CREATE TABLE crawl_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source VARCHAR(50) NOT NULL,
    series_id VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_for TIMESTAMPTZ,
    locked_by VARCHAR(100),
    locked_at TIMESTAMPTZ,

    -- Ensure unique combination of source and series_id for pending/processing items
    CONSTRAINT unique_active_queue_item UNIQUE(source, series_id) DEFERRABLE INITIALLY DEFERRED
);

-- Create indexes for queue processing
CREATE INDEX idx_crawl_queue_status ON crawl_queue(status);
CREATE INDEX idx_crawl_queue_priority ON crawl_queue(priority DESC);
CREATE INDEX idx_crawl_queue_scheduled_for ON crawl_queue(scheduled_for);
CREATE INDEX idx_crawl_queue_locked_by ON crawl_queue(locked_by);
CREATE INDEX idx_crawl_queue_source ON crawl_queue(source);
CREATE INDEX idx_crawl_queue_created_at ON crawl_queue(created_at);

-- Create composite index for queue processing (SKIP LOCKED optimization)
CREATE INDEX idx_crawl_queue_processing ON crawl_queue(status, priority DESC, scheduled_for, locked_by)
WHERE status IN ('pending', 'retrying');

-- Create updated_at trigger for crawl_queue
CREATE TRIGGER update_crawl_queue_updated_at
    BEFORE UPDATE ON crawl_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add constraint to validate status values
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_status
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'retrying', 'cancelled'));

-- Add constraint to validate priority range
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_priority
    CHECK (priority >= 1 AND priority <= 10);

-- Add constraint to ensure locked items have lock information
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_lock_consistency
    CHECK ((locked_by IS NULL AND locked_at IS NULL) OR (locked_by IS NOT NULL AND locked_at IS NOT NULL));
-- Global Economic Network Analysis Schema
-- Creates comprehensive tables for cross-country economic analysis
-- This consolidates the global analysis migration into a cohesive schema

-- Countries table with geographic and economic metadata
CREATE TABLE countries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) NOT NULL UNIQUE, -- ISO 3166-1 alpha-3 (USA, GBR, etc.)
    iso_code_2 VARCHAR(2) NOT NULL UNIQUE, -- ISO 3166-1 alpha-2 (US, GB, etc.)
    name VARCHAR(255) NOT NULL,
    region VARCHAR(100) NOT NULL, -- North America, Europe, Asia, etc.
    sub_region VARCHAR(100), -- Western Europe, Southeast Asia, etc.
    income_group VARCHAR(50), -- High income, Upper middle income, etc.
    population BIGINT,
    gdp_usd DECIMAL(20,2), -- GDP in USD
    gdp_per_capita_usd DECIMAL(15,2),
    latitude DECIMAL(10,8),
    longitude DECIMAL(11,8),
    currency_code VARCHAR(3), -- USD, EUR, GBP, etc.
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Economic indicators optimized for cross-country analysis
CREATE TABLE global_economic_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_code VARCHAR(50) NOT NULL, -- World Bank indicator codes (NY.GDP.MKTP.CD, etc.)
    indicator_name VARCHAR(500) NOT NULL,
    category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, Inflation, etc.
    subcategory VARCHAR(100), -- Real GDP, Nominal GDP, etc.
    unit VARCHAR(50), -- USD, Percent, Index, etc.
    frequency VARCHAR(20) NOT NULL, -- Annual, Quarterly, Monthly
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_id, indicator_code)
);

-- Time series data for global indicators
CREATE TABLE global_indicator_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    indicator_id UUID NOT NULL REFERENCES global_economic_indicators(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    is_preliminary BOOLEAN NOT NULL DEFAULT false,
    data_source VARCHAR(50) NOT NULL, -- World Bank, IMF, OECD, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(indicator_id, date)
);

-- Economic correlations between countries
CREATE TABLE country_correlations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country_a_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    country_b_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, etc.
    correlation_coefficient DECIMAL(5,4) NOT NULL, -- -1.0000 to 1.0000
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    sample_size INTEGER NOT NULL, -- Number of data points used
    p_value DECIMAL(10,8), -- Statistical significance
    is_significant BOOLEAN NOT NULL DEFAULT false,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_a_id, country_b_id, indicator_category, time_period_start, time_period_end),
    CHECK (country_a_id != country_b_id),
    CHECK (correlation_coefficient >= -1.0 AND correlation_coefficient <= 1.0)
);

-- Trade relationships between countries
CREATE TABLE trade_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    exporter_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    importer_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    trade_flow_type VARCHAR(20) NOT NULL, -- Goods, Services, Total
    year INTEGER NOT NULL,
    export_value_usd DECIMAL(20,2), -- Export value in USD
    import_value_usd DECIMAL(20,2), -- Import value in USD
    trade_balance_usd DECIMAL(20,2), -- Export - Import
    trade_intensity DECIMAL(8,6), -- Trade as % of GDP
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(exporter_country_id, importer_country_id, trade_flow_type, year),
    CHECK (exporter_country_id != importer_country_id)
);

-- Global economic events and their impacts
CREATE TABLE global_economic_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(500) NOT NULL,
    description TEXT,
    event_type VARCHAR(50) NOT NULL, -- Crisis, Policy, Natural Disaster, etc.
    severity VARCHAR(20) NOT NULL, -- Low, Medium, High, Critical
    start_date DATE NOT NULL,
    end_date DATE,
    primary_country_id UUID REFERENCES countries(id), -- Originating country
    affected_regions TEXT[], -- Array of affected regions
    economic_impact_score DECIMAL(5,2), -- 0-100 impact score
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Country impacts from global events
CREATE TABLE event_country_impacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES global_economic_events(id) ON DELETE CASCADE,
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    impact_type VARCHAR(50) NOT NULL, -- GDP, Employment, Trade, Financial
    impact_magnitude DECIMAL(8,4), -- Percentage change
    impact_duration_days INTEGER, -- How long the impact lasted
    recovery_time_days INTEGER, -- Time to return to pre-event levels
    confidence_score DECIMAL(3,2), -- 0-1 confidence in measurement
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(event_id, country_id, impact_type)
);

-- Economic leading indicators relationships
CREATE TABLE leading_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    leading_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    following_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL,
    lead_time_months INTEGER NOT NULL, -- How many months country A leads country B
    correlation_strength DECIMAL(5,4) NOT NULL, -- Correlation coefficient
    predictive_accuracy DECIMAL(5,4), -- Historical prediction accuracy (0-1)
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(leading_country_id, following_country_id, indicator_category),
    CHECK (leading_country_id != following_country_id),
    CHECK (lead_time_months >= 1 AND lead_time_months <= 24)
);

-- Create indexes for performance
CREATE INDEX idx_countries_region ON countries(region);
CREATE INDEX idx_countries_income_group ON countries(income_group);
CREATE INDEX idx_countries_iso_codes ON countries(iso_code, iso_code_2);

CREATE INDEX idx_global_indicators_country_category ON global_economic_indicators(country_id, category);
CREATE INDEX idx_global_indicators_code ON global_economic_indicators(indicator_code);

CREATE INDEX idx_global_data_indicator_date ON global_indicator_data(indicator_id, date DESC);
CREATE INDEX idx_global_data_date_value ON global_indicator_data(date, value) WHERE value IS NOT NULL;

CREATE INDEX idx_correlations_countries ON country_correlations(country_a_id, country_b_id);
CREATE INDEX idx_correlations_category ON country_correlations(indicator_category);
CREATE INDEX idx_correlations_strength ON country_correlations(correlation_coefficient DESC) WHERE is_significant = true;

CREATE INDEX idx_trade_exporter_year ON trade_relationships(exporter_country_id, year DESC);
CREATE INDEX idx_trade_importer_year ON trade_relationships(importer_country_id, year DESC);
CREATE INDEX idx_trade_value ON trade_relationships(export_value_usd DESC) WHERE export_value_usd IS NOT NULL;

CREATE INDEX idx_events_date ON global_economic_events(start_date DESC);
CREATE INDEX idx_events_severity ON global_economic_events(severity, economic_impact_score DESC);

CREATE INDEX idx_event_impacts_country ON event_country_impacts(country_id, impact_magnitude DESC);

CREATE INDEX idx_leading_indicators_strength ON leading_indicators(correlation_strength DESC, predictive_accuracy DESC);

-- Create updated_at triggers
CREATE TRIGGER update_countries_updated_at
    BEFORE UPDATE ON countries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_indicators_updated_at
    BEFORE UPDATE ON global_economic_indicators
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_events_updated_at
    BEFORE UPDATE ON global_economic_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert major countries for initial data
INSERT INTO countries (iso_code, iso_code_2, name, region, sub_region, income_group, latitude, longitude, currency_code) VALUES
    ('USA', 'US', 'United States', 'Americas', 'Northern America', 'High income', 39.8283, -98.5795, 'USD'),
    ('CHN', 'CN', 'China', 'Asia', 'Eastern Asia', 'Upper middle income', 35.8617, 104.1954, 'CNY'),
    ('JPN', 'JP', 'Japan', 'Asia', 'Eastern Asia', 'High income', 36.2048, 138.2529, 'JPY'),
    ('DEU', 'DE', 'Germany', 'Europe', 'Western Europe', 'High income', 51.1657, 10.4515, 'EUR'),
    ('GBR', 'GB', 'United Kingdom', 'Europe', 'Northern Europe', 'High income', 55.3781, -3.4360, 'GBP'),
    ('FRA', 'FR', 'France', 'Europe', 'Western Europe', 'High income', 46.2276, 2.2137, 'EUR'),
    ('IND', 'IN', 'India', 'Asia', 'Southern Asia', 'Lower middle income', 20.5937, 78.9629, 'INR'),
    ('ITA', 'IT', 'Italy', 'Europe', 'Southern Europe', 'High income', 41.8719, 12.5674, 'EUR'),
    ('BRA', 'BR', 'Brazil', 'Americas', 'South America', 'Upper middle income', -14.2350, -51.9253, 'BRL'),
    ('CAN', 'CA', 'Canada', 'Americas', 'Northern America', 'High income', 56.1304, -106.3468, 'CAD'),
    ('RUS', 'RU', 'Russian Federation', 'Europe', 'Eastern Europe', 'Upper middle income', 61.5240, 105.3188, 'RUB'),
    ('KOR', 'KR', 'South Korea', 'Asia', 'Eastern Asia', 'High income', 35.9078, 127.7669, 'KRW'),
    ('ESP', 'ES', 'Spain', 'Europe', 'Southern Europe', 'High income', 40.4637, -3.7492, 'EUR'),
    ('AUS', 'AU', 'Australia', 'Oceania', 'Australia and New Zealand', 'High income', -25.2744, 133.7751, 'AUD'),
    ('MEX', 'MX', 'Mexico', 'Americas', 'Central America', 'Upper middle income', 23.6345, -102.5528, 'MXN'),
    ('IDN', 'ID', 'Indonesia', 'Asia', 'South-Eastern Asia', 'Upper middle income', -0.7893, 113.9213, 'IDR'),
    ('NLD', 'NL', 'Netherlands', 'Europe', 'Western Europe', 'High income', 52.1326, 5.2913, 'EUR'),
    ('SAU', 'SA', 'Saudi Arabia', 'Asia', 'Western Asia', 'High income', 23.8859, 45.0792, 'SAR'),
    ('TUR', 'TR', 'Turkey', 'Asia', 'Western Asia', 'Upper middle income', 38.9637, 35.2433, 'TRY'),
    ('CHE', 'CH', 'Switzerland', 'Europe', 'Western Europe', 'High income', 46.8182, 8.2275, 'CHF');

-- Insert major global economic events for reference
INSERT INTO global_economic_events (name, description, event_type, severity, start_date, end_date, primary_country_id, economic_impact_score) VALUES
    ('2008 Global Financial Crisis', 'Global financial crisis originating from US subprime mortgage crisis', 'Crisis', 'Critical', '2007-12-01', '2009-06-01', (SELECT id FROM countries WHERE iso_code = 'USA'), 95.0),
    ('COVID-19 Pandemic', 'Global pandemic causing widespread economic disruption', 'Crisis', 'Critical', '2020-03-01', '2022-12-01', NULL, 98.0),
    ('European Debt Crisis', 'Sovereign debt crisis affecting eurozone countries', 'Crisis', 'High', '2010-01-01', '2012-12-01', (SELECT id FROM countries WHERE iso_code = 'DEU'), 75.0),
    ('Brexit', 'United Kingdom withdrawal from European Union', 'Policy', 'Medium', '2016-06-23', '2020-12-31', (SELECT id FROM countries WHERE iso_code = 'GBR'), 45.0),
    ('US-China Trade War', 'Trade dispute between United States and China', 'Policy', 'High', '2018-03-01', '2020-01-15', (SELECT id FROM countries WHERE iso_code = 'USA'), 65.0);

COMMENT ON TABLE countries IS 'Master table of countries with geographic and economic metadata for global analysis';
COMMENT ON TABLE global_economic_indicators IS 'Economic indicators available for cross-country analysis';
COMMENT ON TABLE global_indicator_data IS 'Time series data for global economic indicators';
COMMENT ON TABLE country_correlations IS 'Calculated correlations between countries for various economic indicators';
COMMENT ON TABLE trade_relationships IS 'Bilateral trade data between countries';
COMMENT ON TABLE global_economic_events IS 'Major global economic events and their characteristics';
COMMENT ON TABLE event_country_impacts IS 'Impact of global events on individual countries';
COMMENT ON TABLE leading_indicators IS 'Countries that lead others in economic indicators';
-- Authentication and Collaboration Schema
-- Creates user authentication, session management, and collaboration features
-- This consolidates the auth and collaboration migrations into a cohesive schema

-- Create users table for OAuth and email authentication
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    provider VARCHAR(50) NOT NULL DEFAULT 'email', -- 'google', 'facebook', 'email'
    provider_id VARCHAR(255), -- OAuth provider user ID
    password_hash VARCHAR(255), -- For email authentication
    role VARCHAR(50) NOT NULL DEFAULT 'viewer', -- 'admin', 'analyst', 'viewer'
    organization VARCHAR(255),

    -- User preferences
    theme VARCHAR(20) DEFAULT 'light',
    default_chart_type VARCHAR(50) DEFAULT 'line',
    notifications_enabled BOOLEAN DEFAULT true,
    collaboration_enabled BOOLEAN DEFAULT true,

    -- Metadata
    is_active BOOLEAN DEFAULT true,
    email_verified BOOLEAN DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_login_at TIMESTAMP WITH TIME ZONE
);

-- Create user sessions table for JWT token management
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_used_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    user_agent TEXT,
    ip_address TEXT -- Using TEXT instead of INET for better compatibility
);

-- Create chart annotations table for collaboration
CREATE TABLE chart_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    series_id VARCHAR(255), -- Reference to economic series
    chart_id UUID, -- For custom chart groupings

    -- Annotation data
    annotation_date DATE NOT NULL,
    annotation_value DECIMAL(20, 6), -- Optional Y-axis value
    title VARCHAR(255) NOT NULL,
    description TEXT,
    color VARCHAR(7) DEFAULT '#2196f3', -- Hex color code
    annotation_type VARCHAR(20) DEFAULT 'line', -- 'line', 'point', 'box', 'trend'

    -- Metadata
    is_visible BOOLEAN DEFAULT true,
    is_pinned BOOLEAN DEFAULT false,
    tags TEXT[], -- Array of tags

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create annotation comments table for collaboration discussions
CREATE TABLE annotation_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    annotation_id UUID NOT NULL REFERENCES chart_annotations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    content TEXT NOT NULL,
    is_resolved BOOLEAN DEFAULT false,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create chart collaborators table for sharing permissions
CREATE TABLE chart_collaborators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id UUID NOT NULL, -- Custom chart identifier
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID REFERENCES users(id),

    role VARCHAR(20) DEFAULT 'viewer', -- 'owner', 'editor', 'viewer'
    permissions JSONB DEFAULT '{"view": true, "annotate": false, "edit": false}',

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE
);

-- Create indexes for performance
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_provider ON users(provider, provider_id);
CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(token_hash);
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at);
CREATE INDEX idx_chart_annotations_user_id ON chart_annotations(user_id);
CREATE INDEX idx_chart_annotations_series_id ON chart_annotations(series_id);
CREATE INDEX idx_chart_annotations_date ON chart_annotations(annotation_date);
CREATE INDEX idx_annotation_comments_annotation_id ON annotation_comments(annotation_id);
CREATE INDEX idx_annotation_comments_user_id ON annotation_comments(user_id);
CREATE INDEX idx_chart_collaborators_chart_id ON chart_collaborators(chart_id);
CREATE INDEX idx_chart_collaborators_user_id ON chart_collaborators(user_id);

-- Create triggers for updated_at columns (reusing the function from initial schema)
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON chart_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON annotation_comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- Comprehensive Crawler Enhancements and Schema Fixes Migration
-- This migration consolidates all enhancements including:
-- 1. Data source visibility controls
-- 2. Series-level crawl tracking
-- 3. Crawl attempts tracking for predictive crawling
-- 4. User table NOT NULL constraint fixes

-- ============================================================================
-- 1. DATA SOURCE VISIBILITY CONTROLS
-- ============================================================================

-- Add visibility and admin controls to data_sources table
ALTER TABLE data_sources ADD COLUMN is_visible BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE data_sources ADD COLUMN is_enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE data_sources ADD COLUMN requires_admin_approval BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE data_sources ADD COLUMN crawl_frequency_hours INTEGER NOT NULL DEFAULT 24;
ALTER TABLE data_sources ADD COLUMN last_crawl_at TIMESTAMPTZ;
ALTER TABLE data_sources ADD COLUMN crawl_status VARCHAR(50) DEFAULT 'pending';
ALTER TABLE data_sources ADD COLUMN crawl_error_message TEXT;
ALTER TABLE data_sources ADD COLUMN api_documentation_url VARCHAR(500);

-- Add indexes for performance
CREATE INDEX idx_data_sources_visible ON data_sources(is_visible);
CREATE INDEX idx_data_sources_enabled ON data_sources(is_enabled);
CREATE INDEX idx_data_sources_crawl_status ON data_sources(crawl_status);

-- Update existing data sources with appropriate visibility settings
UPDATE data_sources SET
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    crawl_frequency_hours = 6,
    crawl_status = 'active'
WHERE name = 'Federal Reserve Economic Data (FRED)';

UPDATE data_sources SET
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    crawl_frequency_hours = 12,
    crawl_status = 'active'
WHERE name = 'Bureau of Labor Statistics (BLS)';

-- Set Census and World Bank as disabled by default (require admin approval)
UPDATE data_sources SET
    is_visible = false,
    is_enabled = false,
    requires_admin_approval = true,
    crawl_frequency_hours = 24,
    crawl_status = 'disabled'
WHERE name IN ('U.S. Census Bureau', 'World Bank Open Data');

-- Add user preferences table for data source visibility
CREATE TABLE user_data_source_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    data_source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    is_visible BOOLEAN NOT NULL DEFAULT true,
    is_favorite BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, data_source_id)
);

-- Add indexes for user preferences
CREATE INDEX idx_user_data_source_preferences_user_id ON user_data_source_preferences(user_id);
CREATE INDEX idx_user_data_source_preferences_data_source_id ON user_data_source_preferences(data_source_id);
CREATE INDEX idx_user_data_source_preferences_visible ON user_data_source_preferences(is_visible);

-- Add updated_at trigger for user preferences
CREATE TRIGGER update_user_data_source_preferences_updated_at
    BEFORE UPDATE ON user_data_source_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- 2. SERIES-LEVEL CRAWL TRACKING
-- ============================================================================

-- Add series-level crawl tracking fields to economic_series table
-- This allows us to track when each series was first discovered, last crawled,
-- and when we first detected missing data (without deleting the series)
ALTER TABLE economic_series ADD COLUMN first_discovered_at TIMESTAMPTZ;
ALTER TABLE economic_series ADD COLUMN last_crawled_at TIMESTAMPTZ;
ALTER TABLE economic_series ADD COLUMN first_missing_date DATE;
ALTER TABLE economic_series ADD COLUMN crawl_status VARCHAR(50);
ALTER TABLE economic_series ADD COLUMN crawl_error_message TEXT;

-- Set first_discovered_at to created_at for existing records
UPDATE economic_series SET first_discovered_at = created_at WHERE first_discovered_at IS NULL;

-- Add comments for documentation
COMMENT ON COLUMN economic_series.first_discovered_at IS 'When this series was first discovered by our crawler';
COMMENT ON COLUMN economic_series.last_crawled_at IS 'When we last attempted to crawl this specific series';
COMMENT ON COLUMN economic_series.first_missing_date IS 'First date we detected missing data (NULLable - series may still be active)';
COMMENT ON COLUMN economic_series.crawl_status IS 'Status of last crawl attempt (success, failed, pending, etc.)';
COMMENT ON COLUMN economic_series.crawl_error_message IS 'Error message from last failed crawl attempt';

-- ============================================================================
-- 3. CRAWL ATTEMPTS TRACKING FOR PREDICTIVE CRAWLING
-- ============================================================================

-- Create crawl_attempts table for tracking crawl history and success rates
CREATE TABLE crawl_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Crawl attempt details
    crawl_method VARCHAR(50) NOT NULL, -- 'api', 'ftp', 'web_scrape', etc.
    crawl_url TEXT, -- URL or endpoint attempted
    http_status_code INTEGER, -- HTTP response status

    -- Data freshness tracking
    data_found BOOLEAN NOT NULL DEFAULT FALSE, -- Whether we found any data
    new_data_points INTEGER DEFAULT 0, -- Number of new data points found
    latest_data_date DATE, -- Date of the most recent data point found
    data_freshness_hours INTEGER, -- How fresh the data was (hours since publication)

    -- Error tracking
    success BOOLEAN NOT NULL DEFAULT FALSE, -- Whether crawl succeeded
    error_type VARCHAR(50), -- 'network', 'api_limit', 'data_format', 'not_found', etc.
    error_message TEXT, -- Detailed error message
    retry_count INTEGER DEFAULT 0, -- Number of retries attempted

    -- Performance metrics
    response_time_ms INTEGER, -- Response time in milliseconds
    data_size_bytes INTEGER, -- Size of data retrieved
    rate_limit_remaining INTEGER, -- API rate limit remaining

    -- Metadata
    user_agent TEXT, -- User agent used for request
    request_headers JSONB, -- Request headers sent
    response_headers JSONB, -- Response headers received

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient querying
CREATE INDEX idx_crawl_attempts_series_id ON crawl_attempts(series_id);
CREATE INDEX idx_crawl_attempts_attempted_at ON crawl_attempts(attempted_at);
CREATE INDEX idx_crawl_attempts_success ON crawl_attempts(success);
CREATE INDEX idx_crawl_attempts_data_found ON crawl_attempts(data_found);
CREATE INDEX idx_crawl_attempts_error_type ON crawl_attempts(error_type);
CREATE INDEX idx_crawl_attempts_latest_data_date ON crawl_attempts(latest_data_date);

-- Create composite indexes for common queries
CREATE INDEX idx_crawl_attempts_series_success ON crawl_attempts(series_id, success);
CREATE INDEX idx_crawl_attempts_series_attempted ON crawl_attempts(series_id, attempted_at);
CREATE INDEX idx_crawl_attempts_success_attempted ON crawl_attempts(success, attempted_at);

-- Add trigger for updated_at
CREATE TRIGGER set_crawl_attempts_updated_at
    BEFORE UPDATE ON crawl_attempts
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- 4. USER TABLE NOT NULL CONSTRAINT FIXES
-- ============================================================================

-- Fix user table and user_sessions table NOT NULL constraints
-- Fields with default values should be NOT NULL to match Rust model expectations

-- Update existing NULL values to their defaults before adding NOT NULL constraints
UPDATE users SET theme = 'light' WHERE theme IS NULL;
UPDATE users SET default_chart_type = 'line' WHERE default_chart_type IS NULL;
UPDATE users SET notifications_enabled = true WHERE notifications_enabled IS NULL;
UPDATE users SET collaboration_enabled = true WHERE collaboration_enabled IS NULL;
UPDATE users SET is_active = true WHERE is_active IS NULL;
UPDATE users SET email_verified = false WHERE email_verified IS NULL;
UPDATE users SET created_at = NOW() WHERE created_at IS NULL;
UPDATE users SET updated_at = NOW() WHERE updated_at IS NULL;

-- Update user_sessions table
UPDATE user_sessions SET created_at = NOW() WHERE created_at IS NULL;
UPDATE user_sessions SET last_used_at = NOW() WHERE last_used_at IS NULL;

-- Add NOT NULL constraints to fields that have default values
ALTER TABLE users ALTER COLUMN theme SET NOT NULL;
ALTER TABLE users ALTER COLUMN default_chart_type SET NOT NULL;
ALTER TABLE users ALTER COLUMN notifications_enabled SET NOT NULL;
ALTER TABLE users ALTER COLUMN collaboration_enabled SET NOT NULL;
ALTER TABLE users ALTER COLUMN is_active SET NOT NULL;
ALTER TABLE users ALTER COLUMN email_verified SET NOT NULL;
ALTER TABLE users ALTER COLUMN created_at SET NOT NULL;
ALTER TABLE users ALTER COLUMN updated_at SET NOT NULL;

-- Add NOT NULL constraints to user_sessions table
ALTER TABLE user_sessions ALTER COLUMN created_at SET NOT NULL;
ALTER TABLE user_sessions ALTER COLUMN last_used_at SET NOT NULL;

-- ============================================================================
-- 5. ADDITIONAL DATA SOURCES
-- ============================================================================

-- Add IMF data source if it doesn't exist
INSERT INTO data_sources (id, name, description, base_url, api_documentation_url, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, crawl_status)
SELECT
    gen_random_uuid(),
    'International Monetary Fund (IMF)',
    'Global economic and financial data from the IMF',
    'http://dataservices.imf.org',
    'https://data.imf.org/en/Resource-Pages/IMF-API',
    false,
    false,
    true,
    24,
    'disabled'
WHERE NOT EXISTS (
    SELECT 1 FROM data_sources WHERE name = 'International Monetary Fund (IMF)'
);

-- Add BEA data source if it doesn't exist
INSERT INTO data_sources (id, name, description, base_url, api_documentation_url, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, crawl_status)
SELECT
    gen_random_uuid(),
    'Bureau of Economic Analysis (BEA)',
    'National economic accounts and GDP data from BEA',
    'https://apps.bea.gov',
    'https://apps.bea.gov/api/bea_web_service_api_user_guide.htm',
    false,
    false,
    true,
    24,
    'disabled'
WHERE NOT EXISTS (
    SELECT 1 FROM data_sources WHERE name = 'Bureau of Economic Analysis (BEA)'
);
-- Create series_metadata table to store discovered series information
CREATE TABLE series_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50),
    geographic_level VARCHAR(100),
    data_url TEXT,
    api_endpoint TEXT,
    last_discovered_at TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Ensure unique external_id per source
    UNIQUE(source_id, external_id)
);

-- Create indexes for efficient querying
CREATE INDEX idx_series_metadata_source_id ON series_metadata(source_id);
CREATE INDEX idx_series_metadata_external_id ON series_metadata(external_id);
CREATE INDEX idx_series_metadata_last_discovered ON series_metadata(last_discovered_at);
CREATE INDEX idx_series_metadata_active ON series_metadata(is_active);

-- Add trigger for updated_at
CREATE OR REPLACE FUNCTION update_series_metadata_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_series_metadata_updated_at
    BEFORE UPDATE ON series_metadata
    FOR EACH ROW
    EXECUTE FUNCTION update_series_metadata_updated_at();

-- Insert initial series metadata for existing data sources
-- FRED (Federal Reserve Economic Data)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'GDP',
    'Gross Domestic Product',
    'Real gross domestic product, seasonally adjusted annual rate',
    'Billions of Chained 2017 Dollars',
    'Quarterly',
    'United States',
    'https://fred.stlouisfed.org/series/GDP',
    'https://api.stlouisfed.org/fred/series/observations?series_id=GDP&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'UNRATE',
    'Unemployment Rate',
    'Unemployment rate, seasonally adjusted',
    'Percent',
    'Monthly',
    'United States',
    'https://fred.stlouisfed.org/series/UNRATE',
    'https://api.stlouisfed.org/fred/series/observations?series_id=UNRATE&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'CPIAUCSL',
    'Consumer Price Index for All Urban Consumers',
    'Consumer Price Index for All Urban Consumers: All Items in U.S. City Average',
    'Index 1982-1984=100',
    'Monthly',
    'United States',
    'https://fred.stlouisfed.org/series/CPIAUCSL',
    'https://api.stlouisfed.org/fred/series/observations?series_id=CPIAUCSL&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

-- BLS (Bureau of Labor Statistics)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'CES0000000001',
    'All Employees, Total Nonfarm',
    'Total nonfarm employment, seasonally adjusted',
    'Thousands of Persons',
    'Monthly',
    'United States',
    'https://data.bls.gov/timeseries/CES0000000001',
    'https://api.bls.gov/publicAPI/v2/timeseries/data/CES0000000001',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Labor Statistics (BLS)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'LNS14000000',
    'Unemployment Rate',
    'Unemployment rate, seasonally adjusted',
    'Percent',
    'Monthly',
    'United States',
    'https://data.bls.gov/timeseries/LNS14000000',
    'https://api.bls.gov/publicAPI/v2/timeseries/data/LNS14000000',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Labor Statistics (BLS)';

-- Census Bureau
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'B01001001',
    'Total Population',
    'Total population estimate',
    'Persons',
    'Annual',
    'United States',
    'https://api.census.gov/data/2023/pep/population',
    'https://api.census.gov/data/2023/pep/population?get=B01001_001E&for=us:1',
    true
FROM data_sources ds WHERE ds.name = 'U.S. Census Bureau';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'B19013_001E',
    'Median Household Income',
    'Median household income in the past 12 months',
    'Dollars',
    'Annual',
    'United States',
    'https://api.census.gov/data/2023/acs/acs5',
    'https://api.census.gov/data/2023/acs/acs5?get=B19013_001E&for=us:1',
    true
FROM data_sources ds WHERE ds.name = 'U.S. Census Bureau';

-- BEA (Bureau of Economic Analysis)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'GDP',
    'Gross Domestic Product',
    'Gross domestic product, current dollars',
    'Millions of Dollars',
    'Quarterly',
    'United States',
    'https://apps.bea.gov/api/data',
    'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=GDP&TableName=T10101&Frequency=Q&Year=2023&ResultFormat=JSON',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Economic Analysis (BEA)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'PCE',
    'Personal Consumption Expenditures',
    'Personal consumption expenditures, current dollars',
    'Millions of Dollars',
    'Quarterly',
    'United States',
    'https://apps.bea.gov/api/data',
    'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=NIPA&TableName=T20301&Frequency=Q&Year=2023&ResultFormat=JSON',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Economic Analysis (BEA)';

-- World Bank
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NY.GDP.MKTP.CD',
    'GDP (current US$)',
    'GDP at purchaser''s prices is the sum of gross value added by all resident producers in the economy plus any product taxes and minus any subsidies not included in the value of the products',
    'Current US$',
    'Annual',
    'Country',
    'https://data.worldbank.org/indicator/NY.GDP.MKTP.CD',
    'https://api.worldbank.org/v2/country/all/indicator/NY.GDP.MKTP.CD?format=json',
    true
FROM data_sources ds WHERE ds.name = 'World Bank';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'SP.POP.TOTL',
    'Population, total',
    'Total population is based on the de facto definition of population, which counts all residents regardless of legal status or citizenship',
    'Persons',
    'Annual',
    'Country',
    'https://data.worldbank.org/indicator/SP.POP.TOTL',
    'https://api.worldbank.org/v2/country/all/indicator/SP.POP.TOTL?format=json',
    true
FROM data_sources ds WHERE ds.name = 'World Bank';

-- IMF
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NGDP_R_SA_XDC',
    'Gross domestic product, real, seasonally adjusted',
    'Gross domestic product, real, seasonally adjusted, national currency',
    'National currency',
    'Quarterly',
    'Country',
    'https://data.imf.org/regular.aspx?key=61545850',
    'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_R_SA_XDC',
    true
FROM data_sources ds WHERE ds.name = 'International Monetary Fund (IMF)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NGDP_XDC',
    'Gross domestic product, current prices',
    'Gross domestic product, current prices, national currency',
    'National currency',
    'Quarterly',
    'Country',
    'https://data.imf.org/regular.aspx?key=61545850',
    'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_XDC',
    true
FROM data_sources ds WHERE ds.name = 'International Monetary Fund (IMF)';

-- FHFA
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'HPI_USA',
    'U.S. House Price Index',
    'U.S. House Price Index, seasonally adjusted',
    'Index (January 1991=100)',
    'Quarterly',
    'United States',
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    'https://api.fhfa.gov/house-price-index',
    true
FROM data_sources ds WHERE ds.name = 'Federal Housing Finance Agency (FHFA)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'HPI_STATE',
    'State House Price Index',
    'State House Price Index, seasonally adjusted',
    'Index (January 1991=100)',
    'Quarterly',
    'State',
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    'https://api.fhfa.gov/house-price-index/state',
    true
FROM data_sources ds WHERE ds.name = 'Federal Housing Finance Agency (FHFA)';

-- ECB (European Central Bank)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ICP.M.U2.N.000000.4.ANR',
    'Euro area - Main refinancing operations rate',
    'Interest rate for main refinancing operations in the euro area',
    'Percent per annum',
    'Monthly',
    'Euro area',
    'https://sdw-wsrest.ecb.europa.eu/service/data/ICP/M.U2.N.000000.4.ANR',
    'https://sdw-wsrest.ecb.europa.eu/service/data/ICP/M.U2.N.000000.4.ANR',
    true
FROM data_sources ds WHERE ds.name = 'European Central Bank (ECB)';

-- OECD
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'SNA_TABLE1.1.GDP.B1_GE.CPCAR_M',
    'OECD - GDP at current prices',
    'Gross Domestic Product at current prices for OECD countries',
    'Millions of national currency',
    'Annual',
    'Country',
    'https://data-explorer.oecd.org/vis?fs[0]=Topic%2C1%7C1%7C1%7C0',
    'https://sdmx.oecd.org/public/rest/data/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPCAR_M',
    true
FROM data_sources ds WHERE ds.name = 'OECD (Organisation for Economic Co-operation and Development)';

-- Bank of England
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'IUDBEDR',
    'UK - Bank Rate',
    'Official Bank Rate set by the Bank of England''s Monetary Policy Committee',
    'Percent per annum',
    'Monthly',
    'United Kingdom',
    'https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&SeriesCodes=IUDBEDR',
    'https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&SeriesCodes=IUDBEDR',
    true
FROM data_sources ds WHERE ds.name = 'Bank of England (BoE)';

-- WTO
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'MT_GOODS_EXP',
    'WTO - Merchandise exports',
    'Merchandise exports for WTO member countries',
    'Millions of US dollars',
    'Annual',
    'Country',
    'https://www.wto.org/english/res_e/statis_e/data_explorer_e.htm',
    'https://api.wto.org/timeseries/v1/data/MT_GOODS_EXP',
    true
FROM data_sources ds WHERE ds.name = 'World Trade Organization (WTO)';

-- Bank of Japan
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'BOJ_UNRATE',
    'Japan - Policy interest rate',
    'Policy interest rate set by the Bank of Japan',
    'Percent per annum',
    'Monthly',
    'Japan',
    'https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1',
    'https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1',
    true
FROM data_sources ds WHERE ds.name = 'Bank of Japan (BoJ)';

-- Reserve Bank of Australia
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'F1.1',
    'Australia - Cash Rate Target',
    'The cash rate target set by the Reserve Bank Board',
    'Percent per annum',
    'Monthly',
    'Australia',
    'https://www.rba.gov.au/statistics/f01-hist.html',
    'https://www.rba.gov.au/statistics/f01-hist.html',
    true
FROM data_sources ds WHERE ds.name = 'Reserve Bank of Australia (RBA)';

-- Bank of Canada
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'V39079',
    'Canada - Overnight Rate Target',
    'The target for the overnight rate set by the Bank of Canada',
    'Percent per annum',
    'Daily',
    'Canada',
    'https://www.bankofcanada.ca/valet/observations/V39079',
    'https://www.bankofcanada.ca/valet/observations/V39079',
    true
FROM data_sources ds WHERE ds.name = 'Bank of Canada (BoC)';

-- Swiss National Bank
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ir',
    'Switzerland - SNB Policy Rate',
    'Swiss National Bank policy rate',
    'Percent per annum',
    'Daily',
    'Switzerland',
    'https://data.snb.ch/en/ir',
    'https://data.snb.ch/en/ir',
    true
FROM data_sources ds WHERE ds.name = 'Swiss National Bank (SNB)';

-- UN Statistics Division
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'UN_GDP',
    'UN - GDP per capita',
    'Gross Domestic Product per capita from UN Statistics Division',
    'Current US dollars',
    'Annual',
    'Country',
    'https://unstats.un.org/unsd/snaama/Basic',
    'https://unstats.un.org/unsd/snaama/Basic',
    true
FROM data_sources ds WHERE ds.name = 'UN Statistics Division';

-- International Labour Organization
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ILO_UNEMPLOYMENT',
    'ILO - Unemployment Rate',
    'Unemployment rate from International Labour Organization',
    'Percent',
    'Annual',
    'Country',
    'https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm',
    'https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm',
    true
FROM data_sources ds WHERE ds.name = 'International Labour Organization (ILO)';
-- Consolidated Admin Audit and Security Tables Migration
-- This migration creates all admin-related tables and ensures proper nullability constraints

-- ============================================================================
-- 1. AUDIT LOGS TABLE
-- ============================================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    user_name VARCHAR(255) NOT NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    resource_id VARCHAR(255),
    ip_address TEXT,
    user_agent TEXT,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add indexes for audit logs
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource_type ON audit_logs(resource_type);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);

-- ============================================================================
-- 2. SECURITY EVENTS TABLE
-- ============================================================================

CREATE TABLE security_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type VARCHAR(50) NOT NULL,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    user_email VARCHAR(255),
    severity VARCHAR(20) NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    description TEXT NOT NULL,
    metadata JSONB,
    resolved BOOLEAN DEFAULT FALSE,
    resolved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add indexes for security events
CREATE INDEX idx_security_events_event_type ON security_events(event_type);
CREATE INDEX idx_security_events_user_id ON security_events(user_id);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_resolved ON security_events(resolved);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);

-- ============================================================================
-- 3. COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON TABLE audit_logs IS 'Comprehensive audit trail for all administrative and user actions';
COMMENT ON COLUMN audit_logs.user_name IS 'Name of user who performed the action (denormalized for performance)';
COMMENT ON COLUMN audit_logs.action IS 'Type of action performed (create, update, delete, login, etc.)';
COMMENT ON COLUMN audit_logs.resource_type IS 'Type of resource affected (user, series, chart, etc.)';
COMMENT ON COLUMN audit_logs.resource_id IS 'ID of the specific resource affected (nullable for global actions)';
COMMENT ON COLUMN audit_logs.details IS 'Additional context and metadata about the action';

COMMENT ON TABLE security_events IS 'Security-related events and incidents for monitoring and alerting';
COMMENT ON COLUMN security_events.event_type IS 'Type of security event (login_failure, suspicious_activity, etc.)';
COMMENT ON COLUMN security_events.severity IS 'Severity level (low, medium, high, critical)';
COMMENT ON COLUMN security_events.resolved IS 'Whether the security event has been resolved';
COMMENT ON COLUMN security_events.metadata IS 'Additional context and technical details about the security event';
-- Update Census Bureau data source configuration to reflect that it doesn't require an API key
-- and should be enabled and visible by default

UPDATE data_sources
SET
    api_key_required = false,
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    rate_limit_per_minute = 500,
    crawl_frequency_hours = 24,
    updated_at = NOW()
WHERE name = 'U.S. Census Bureau';
-- Add api_key_name column to data_sources table
ALTER TABLE data_sources ADD COLUMN api_key_name VARCHAR(255);

-- Add comment to explain the column
COMMENT ON COLUMN data_sources.api_key_name IS 'Name of the environment variable containing the API key for this data source. NULL if no API key is required.';
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
