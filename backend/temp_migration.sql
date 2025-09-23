-- Consolidated Initial Schema Migration - UP (Semantically Merged)
-- This migration consolidates all migrations committed before 2025-09-14
-- All ALTER TABLE statements have been merged into CREATE TABLE definitions
-- Chronological order by commit date:
-- 2025-09-10: 2024-01-01-* (consolidated refactor)
-- 2025-09-12: 2025-09-12-104236_comprehensive_crawler_and_schema_fixes
-- 2025-09-12: 2025-09-12-124405_create_series_metadata_table
-- 2025-09-12: 2025-09-13-021105-0000_consolidated_admin_audit_security_tables
-- 2025-09-14: 2025-09-13-185806-0000_update_census_data_source_config
-- 2025-09-14: 2025-09-13-213800-0000_add_api_key_name_to_data_sources

-- Enable required PostgreSQL extensions
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create series_metadata_updated_at trigger function
CREATE OR REPLACE FUNCTION update_series_metadata_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- ============================================================================
-- CORE ECONOMIC DATA TABLES
-- ============================================================================

-- Create data_sources table (with all columns merged from ALTER TABLE statements)
CREATE TABLE data_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    base_url VARCHAR(500) NOT NULL,
    api_key_required BOOLEAN NOT NULL DEFAULT FALSE,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    -- Columns added in comprehensive crawler fixes
    is_visible BOOLEAN NOT NULL DEFAULT true,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    requires_admin_approval BOOLEAN NOT NULL DEFAULT false,
    crawl_frequency_hours INTEGER NOT NULL DEFAULT 24,
    last_crawl_at TIMESTAMPTZ,
    crawl_status VARCHAR(50) DEFAULT 'pending',
    crawl_error_message TEXT,
    api_documentation_url VARCHAR(500),
    -- Column added in api key name migration
    api_key_name VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes on data_sources
CREATE INDEX idx_data_sources_name ON data_sources(name);
CREATE INDEX idx_data_sources_is_enabled ON data_sources(is_enabled);
CREATE INDEX idx_data_sources_is_visible ON data_sources(is_visible);
CREATE INDEX idx_data_sources_crawl_status ON data_sources(crawl_status);
CREATE INDEX idx_data_sources_last_crawl_at ON data_sources(last_crawl_at);

-- Create updated_at trigger for data_sources
CREATE TRIGGER update_data_sources_updated_at
    BEFORE UPDATE ON data_sources
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert default data sources
INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute, api_key_name) VALUES
    ('Federal Reserve Economic Data (FRED)', 'Economic data from the Federal Reserve Bank of St. Louis', 'https://api.stlouisfed.org/fred', true, 120, 'FRED_API_KEY'),
    ('Bureau of Labor Statistics (BLS)', 'Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics', 'https://api.bls.gov/publicAPI/v2', true, 500, 'BLS_API_KEY'),
    ('U.S. Census Bureau', 'Demographic and economic data from the U.S. Census Bureau', 'https://api.census.gov/data', true, 500, 'CENSUS_API_KEY'),
    ('World Bank Open Data', 'Global economic and development indicators from the World Bank', 'https://api.worldbank.org/v2', false, 1000, NULL);

-- Create economic_series table (with all columns merged from ALTER TABLE statements)
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
    -- Columns added in comprehensive crawler fixes
    first_discovered_at TIMESTAMPTZ,
    last_crawled_at TIMESTAMPTZ,
    first_missing_date DATE,
    crawl_status VARCHAR(50),
    crawl_error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique external_id per source
    UNIQUE(source_id, external_id)
);

-- Create indexes for economic_series
CREATE INDEX idx_economic_series_source_id ON economic_series(source_id);
CREATE INDEX idx_economic_series_external_id ON economic_series(external_id);
CREATE INDEX idx_economic_series_title ON economic_series USING GIN (to_tsvector('english', title));
CREATE INDEX idx_economic_series_description ON economic_series USING GIN (to_tsvector('english', description));
CREATE INDEX idx_economic_series_frequency ON economic_series(frequency);
CREATE INDEX idx_economic_series_is_active ON economic_series(is_active);
CREATE INDEX idx_economic_series_last_updated ON economic_series(last_updated);
CREATE INDEX idx_economic_series_crawl_status ON economic_series(crawl_status);
CREATE INDEX idx_economic_series_last_crawled_at ON economic_series(last_crawled_at);
CREATE INDEX idx_economic_series_first_discovered_at ON economic_series(first_discovered_at);

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

    -- Ensure unique data point per series and date
    UNIQUE(series_id, date, revision_date)
);

-- Create indexes for data_points
CREATE INDEX idx_data_points_series_id ON data_points(series_id);
CREATE INDEX idx_data_points_date ON data_points(date);
CREATE INDEX idx_data_points_revision_date ON data_points(revision_date);
CREATE INDEX idx_data_points_value ON data_points(value);
CREATE INDEX idx_data_points_is_original_release ON data_points(is_original_release);

-- Create crawl_queue table (with all constraints merged)
CREATE TABLE crawl_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    priority INTEGER NOT NULL DEFAULT 5,
    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 3,
    last_attempt_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ,
    locked_by UUID,
    locked_at TIMESTAMPTZ,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints merged from ALTER TABLE statements
    CONSTRAINT check_crawl_queue_status CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'retrying', 'cancelled')),
    CONSTRAINT check_crawl_queue_priority CHECK (priority >= 1 AND priority <= 10),
    CONSTRAINT check_crawl_queue_lock_consistency CHECK ((locked_by IS NULL AND locked_at IS NULL) OR (locked_by IS NOT NULL AND locked_at IS NOT NULL))
);

-- Create indexes for crawl_queue
CREATE INDEX idx_crawl_queue_status ON crawl_queue(status);
CREATE INDEX idx_crawl_queue_priority ON crawl_queue(priority);
CREATE INDEX idx_crawl_queue_next_attempt_at ON crawl_queue(next_attempt_at);
CREATE INDEX idx_crawl_queue_locked_by ON crawl_queue(locked_by);
CREATE INDEX idx_crawl_queue_series_id ON crawl_queue(series_id);
CREATE INDEX idx_crawl_queue_created_at ON crawl_queue(created_at);

-- Create updated_at trigger for crawl_queue
CREATE TRIGGER update_crawl_queue_updated_at
    BEFORE UPDATE ON crawl_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- SERIES METADATA TABLE (from series metadata migration)
-- ============================================================================

-- Create series_metadata table
CREATE TABLE series_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    source_name VARCHAR(255) NOT NULL,
    source_url VARCHAR(500),
    data_type VARCHAR(100),
    geographic_area VARCHAR(255),
    geographic_type VARCHAR(100),
    seasonal_adjustment_type VARCHAR(100),
    base_period VARCHAR(100),
    units_short VARCHAR(50),
    units_label VARCHAR(100),
    observation_start DATE,
    observation_end DATE,
    frequency_short VARCHAR(10),
    frequency_long VARCHAR(50),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique metadata per series
    UNIQUE(series_id)
);

-- Create indexes for series_metadata
CREATE INDEX idx_series_metadata_series_id ON series_metadata(series_id);
CREATE INDEX idx_series_metadata_source_name ON series_metadata(source_name);
CREATE INDEX idx_series_metadata_geographic_area ON series_metadata(geographic_area);
CREATE INDEX idx_series_metadata_data_type ON series_metadata(data_type);
CREATE INDEX idx_series_metadata_frequency_short ON series_metadata(frequency_short);

-- Create updated_at trigger for series_metadata
CREATE TRIGGER update_series_metadata_updated_at
    BEFORE UPDATE ON series_metadata
    FOR EACH ROW
    EXECUTE FUNCTION update_series_metadata_updated_at();

-- ============================================================================
-- GLOBAL ECONOMIC NETWORK ANALYSIS TABLES
-- ============================================================================

-- Create countries table
CREATE TABLE countries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code_2 CHAR(2) NOT NULL UNIQUE,
    iso_code_3 CHAR(3) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL UNIQUE,
    region VARCHAR(100),
    subregion VARCHAR(100),
    population BIGINT,
    gdp_usd BIGINT,
    currency_code CHAR(3),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for countries
CREATE INDEX idx_countries_iso_code_2 ON countries(iso_code_2);
CREATE INDEX idx_countries_iso_code_3 ON countries(iso_code_3);
CREATE INDEX idx_countries_name ON countries(name);
CREATE INDEX idx_countries_region ON countries(region);
CREATE INDEX idx_countries_subregion ON countries(subregion);

-- Create updated_at trigger for countries
CREATE TRIGGER update_countries_updated_at
    BEFORE UPDATE ON countries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create global_economic_indicators table
CREATE TABLE global_economic_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    category VARCHAR(100) NOT NULL,
    units VARCHAR(100),
    source VARCHAR(255),
    frequency VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for global_economic_indicators
CREATE INDEX idx_global_economic_indicators_name ON global_economic_indicators(name);
CREATE INDEX idx_global_economic_indicators_category ON global_economic_indicators(category);
CREATE INDEX idx_global_economic_indicators_source ON global_economic_indicators(source);

-- Create updated_at trigger for global_economic_indicators
CREATE TRIGGER update_global_economic_indicators_updated_at
    BEFORE UPDATE ON global_economic_indicators
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create global_indicator_data table
CREATE TABLE global_indicator_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    indicator_id UUID NOT NULL REFERENCES global_economic_indicators(id) ON DELETE CASCADE,
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique data point per indicator, country, and date
    UNIQUE(indicator_id, country_id, date)
);

-- Create indexes for global_indicator_data
CREATE INDEX idx_global_indicator_data_indicator_id ON global_indicator_data(indicator_id);
CREATE INDEX idx_global_indicator_data_country_id ON global_indicator_data(country_id);
CREATE INDEX idx_global_indicator_data_date ON global_indicator_data(date);
CREATE INDEX idx_global_indicator_data_value ON global_indicator_data(value);

-- Create country_correlations table
CREATE TABLE country_correlations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country_1_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    country_2_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    correlation_coefficient DECIMAL(5,4) NOT NULL,
    calculation_period_start DATE NOT NULL,
    calculation_period_end DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique correlation per country pair and period
    UNIQUE(country_1_id, country_2_id, calculation_period_start, calculation_period_end)
);

-- Create indexes for country_correlations
CREATE INDEX idx_country_correlations_country_1_id ON country_correlations(country_1_id);
CREATE INDEX idx_country_correlations_country_2_id ON country_correlations(country_2_id);
CREATE INDEX idx_country_correlations_correlation_coefficient ON country_correlations(correlation_coefficient);
CREATE INDEX idx_country_correlations_calculation_period ON country_correlations(calculation_period_start, calculation_period_end);

-- Create trade_relationships table
CREATE TABLE trade_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    exporter_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    importer_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    trade_value_usd BIGINT NOT NULL,
    trade_volume_units VARCHAR(100),
    trade_year INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique trade relationship per country pair and year
    UNIQUE(exporter_country_id, importer_country_id, trade_year)
);

-- Create indexes for trade_relationships
CREATE INDEX idx_trade_relationships_exporter_country_id ON trade_relationships(exporter_country_id);
CREATE INDEX idx_trade_relationships_importer_country_id ON trade_relationships(importer_country_id);
CREATE INDEX idx_trade_relationships_trade_value_usd ON trade_relationships(trade_value_usd);
CREATE INDEX idx_trade_relationships_trade_year ON trade_relationships(trade_year);

-- Create leading_indicators table
CREATE TABLE leading_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    calculation_method TEXT,
    data_sources TEXT[],
    update_frequency VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for leading_indicators
CREATE INDEX idx_leading_indicators_name ON leading_indicators(name);
CREATE INDEX idx_leading_indicators_update_frequency ON leading_indicators(update_frequency);

-- Create updated_at trigger for leading_indicators
CREATE TRIGGER update_leading_indicators_updated_at
    BEFORE UPDATE ON leading_indicators
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create global_economic_events table
CREATE TABLE global_economic_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR(500) NOT NULL,
    description TEXT,
    event_date DATE NOT NULL,
    impact_level VARCHAR(50) NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    source VARCHAR(255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for global_economic_events
CREATE INDEX idx_global_economic_events_event_date ON global_economic_events(event_date);
CREATE INDEX idx_global_economic_events_impact_level ON global_economic_events(impact_level);
CREATE INDEX idx_global_economic_events_event_type ON global_economic_events(event_type);
CREATE INDEX idx_global_economic_events_source ON global_economic_events(source);

-- Create updated_at trigger for global_economic_events
CREATE TRIGGER update_global_economic_events_updated_at
    BEFORE UPDATE ON global_economic_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create event_country_impacts table
CREATE TABLE event_country_impacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES global_economic_events(id) ON DELETE CASCADE,
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    impact_score DECIMAL(3,2) NOT NULL,
    impact_description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique impact per event and country
    UNIQUE(event_id, country_id)
);

-- Create indexes for event_country_impacts
CREATE INDEX idx_event_country_impacts_event_id ON event_country_impacts(event_id);
CREATE INDEX idx_event_country_impacts_country_id ON event_country_impacts(country_id);
CREATE INDEX idx_event_country_impacts_impact_score ON event_country_impacts(impact_score);

-- ============================================================================
-- AUTHENTICATION AND COLLABORATION TABLES
-- ============================================================================

-- Create users table (with all NOT NULL constraints merged)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    theme VARCHAR(20) NOT NULL DEFAULT 'light',
    default_chart_type VARCHAR(50) NOT NULL DEFAULT 'line',
    notifications_enabled BOOLEAN NOT NULL DEFAULT true,
    collaboration_enabled BOOLEAN NOT NULL DEFAULT true,
    is_active BOOLEAN NOT NULL DEFAULT true,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for users
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_is_active ON users(is_active);
CREATE INDEX idx_users_email_verified ON users(email_verified);
CREATE INDEX idx_users_last_login_at ON users(last_login_at);

-- Create updated_at trigger for users
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create user_sessions table (with all NOT NULL constraints merged)
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token VARCHAR(255) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for user_sessions
CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id);
CREATE INDEX idx_user_sessions_session_token ON user_sessions(session_token);
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at);
CREATE INDEX idx_user_sessions_last_used_at ON user_sessions(last_used_at);

-- Create chart_collaborators table
CREATE TABLE chart_collaborators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chart_id VARCHAR(255) NOT NULL,
    permission_level VARCHAR(50) NOT NULL DEFAULT 'view',
    invited_by UUID REFERENCES users(id) ON DELETE SET NULL,
    invited_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accepted_at TIMESTAMPTZ,

    -- Ensure unique collaboration per user and chart
    UNIQUE(user_id, chart_id)
);

-- Create indexes for chart_collaborators
CREATE INDEX idx_chart_collaborators_user_id ON chart_collaborators(user_id);
CREATE INDEX idx_chart_collaborators_chart_id ON chart_collaborators(chart_id);
CREATE INDEX idx_chart_collaborators_permission_level ON chart_collaborators(permission_level);
CREATE INDEX idx_chart_collaborators_invited_by ON chart_collaborators(invited_by);

-- Create chart_annotations table
CREATE TABLE chart_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chart_id VARCHAR(255) NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    annotation_type VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    position_x DECIMAL(10,2),
    position_y DECIMAL(10,2),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for chart_annotations
CREATE INDEX idx_chart_annotations_chart_id ON chart_annotations(chart_id);
CREATE INDEX idx_chart_annotations_user_id ON chart_annotations(user_id);
CREATE INDEX idx_chart_annotations_annotation_type ON chart_annotations(annotation_type);
CREATE INDEX idx_chart_annotations_created_at ON chart_annotations(created_at);

-- Create updated_at trigger for chart_annotations
CREATE TRIGGER update_chart_annotations_updated_at
    BEFORE UPDATE ON chart_annotations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- ADMIN, AUDIT, AND SECURITY TABLES
-- ============================================================================

-- Create audit_logs table
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL,
    resource_type VARCHAR(100) NOT NULL,
    resource_id UUID,
    details JSONB,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for audit_logs
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource_type ON audit_logs(resource_type);
CREATE INDEX idx_audit_logs_resource_id ON audit_logs(resource_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);
CREATE INDEX idx_audit_logs_details ON audit_logs USING GIN (details);

-- Create security_events table
CREATE TABLE security_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type VARCHAR(100) NOT NULL,
    severity VARCHAR(50) NOT NULL DEFAULT 'medium',
    description TEXT,
    ip_address INET,
    user_agent TEXT,
    resolved_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for security_events
CREATE INDEX idx_security_events_user_id ON security_events(user_id);
CREATE INDEX idx_security_events_event_type ON security_events(event_type);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);
CREATE INDEX idx_security_events_resolved_at ON security_events(resolved_at);

-- Create annotation_templates table
CREATE TABLE annotation_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    template_content TEXT NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_public BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for annotation_templates
CREATE INDEX idx_annotation_templates_name ON annotation_templates(name);
CREATE INDEX idx_annotation_templates_created_by ON annotation_templates(created_by);
CREATE INDEX idx_annotation_templates_is_public ON annotation_templates(is_public);

-- Create updated_at trigger for annotation_templates
CREATE TRIGGER update_annotation_templates_updated_at
    BEFORE UPDATE ON annotation_templates
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create annotation_assignments table
CREATE TABLE annotation_assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES annotation_templates(id) ON DELETE CASCADE,
    chart_id VARCHAR(255) NOT NULL,
    assigned_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_to UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,

    -- Ensure unique assignment per template, chart, and user
    UNIQUE(template_id, chart_id, assigned_to)
);

-- Create indexes for annotation_assignments
CREATE INDEX idx_annotation_assignments_template_id ON annotation_assignments(template_id);
CREATE INDEX idx_annotation_assignments_chart_id ON annotation_assignments(chart_id);
CREATE INDEX idx_annotation_assignments_assigned_by ON annotation_assignments(assigned_by);
CREATE INDEX idx_annotation_assignments_assigned_to ON annotation_assignments(assigned_to);
CREATE INDEX idx_annotation_assignments_completed_at ON annotation_assignments(completed_at);

-- Create annotation_comments table
CREATE TABLE annotation_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    annotation_id UUID NOT NULL REFERENCES chart_annotations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for annotation_comments
CREATE INDEX idx_annotation_comments_annotation_id ON annotation_comments(annotation_id);
CREATE INDEX idx_annotation_comments_user_id ON annotation_comments(user_id);
CREATE INDEX idx_annotation_comments_created_at ON annotation_comments(created_at);

-- Create updated_at trigger for annotation_comments
CREATE TRIGGER update_annotation_comments_updated_at
    BEFORE UPDATE ON annotation_comments
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create annotation_replies table
CREATE TABLE annotation_replies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    comment_id UUID NOT NULL REFERENCES annotation_comments(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for annotation_replies
CREATE INDEX idx_annotation_replies_comment_id ON annotation_replies(comment_id);
CREATE INDEX idx_annotation_replies_user_id ON annotation_replies(user_id);
CREATE INDEX idx_annotation_replies_created_at ON annotation_replies(created_at);

-- Create updated_at trigger for annotation_replies
CREATE TRIGGER update_annotation_replies_updated_at
    BEFORE UPDATE ON annotation_replies
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create financial_annotations table
CREATE TABLE financial_annotations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    annotation_id UUID NOT NULL REFERENCES chart_annotations(id) ON DELETE CASCADE,
    financial_metric VARCHAR(100) NOT NULL,
    metric_value DECIMAL(20,6),
    confidence_level DECIMAL(3,2),
    calculation_method TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for financial_annotations
CREATE INDEX idx_financial_annotations_annotation_id ON financial_annotations(annotation_id);
CREATE INDEX idx_financial_annotations_financial_metric ON financial_annotations(financial_metric);
CREATE INDEX idx_financial_annotations_metric_value ON financial_annotations(metric_value);
CREATE INDEX idx_financial_annotations_confidence_level ON financial_annotations(confidence_level);

-- Create updated_at trigger for financial_annotations
CREATE TRIGGER update_financial_annotations_updated_at
    BEFORE UPDATE ON financial_annotations
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- CRAWLER ENHANCEMENT TABLES
-- ============================================================================

-- Create crawl_attempts table
CREATE TABLE crawl_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    status VARCHAR(50) NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    data_points_retrieved INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for crawl_attempts
CREATE INDEX idx_crawl_attempts_series_id ON crawl_attempts(series_id);
CREATE INDEX idx_crawl_attempts_attempt_number ON crawl_attempts(attempt_number);
CREATE INDEX idx_crawl_attempts_status ON crawl_attempts(status);
CREATE INDEX idx_crawl_attempts_started_at ON crawl_attempts(started_at);
CREATE INDEX idx_crawl_attempts_completed_at ON crawl_attempts(completed_at);

-- Create user_data_source_preferences table
CREATE TABLE user_data_source_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    data_source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    is_favorite BOOLEAN NOT NULL DEFAULT false,
    notification_enabled BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique preference per user and data source
    UNIQUE(user_id, data_source_id)
);

-- Create indexes for user_data_source_preferences
CREATE INDEX idx_user_data_source_preferences_user_id ON user_data_source_preferences(user_id);
CREATE INDEX idx_user_data_source_preferences_data_source_id ON user_data_source_preferences(data_source_id);
CREATE INDEX idx_user_data_source_preferences_is_favorite ON user_data_source_preferences(is_favorite);
CREATE INDEX idx_user_data_source_preferences_notification_enabled ON user_data_source_preferences(notification_enabled);

-- Create updated_at trigger for user_data_source_preferences
CREATE TRIGGER update_user_data_source_preferences_updated_at
    BEFORE UPDATE ON user_data_source_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- DATA UPDATES AND SEED DATA
-- ============================================================================

-- Update existing data sources with new columns
UPDATE data_sources
SET
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    crawl_frequency_hours = 24,
    crawl_status = 'pending',
    api_documentation_url = CASE
        WHEN name = 'Federal Reserve Economic Data (FRED)' THEN 'https://fred.stlouisfed.org/docs/api/fred/'
        WHEN name = 'Bureau of Labor Statistics (BLS)' THEN 'https://www.bls.gov/developers/api_signature_v2.htm'
        WHEN name = 'U.S. Census Bureau' THEN 'https://www.census.gov/data/developers/data-sets.html'
        WHEN name = 'World Bank Open Data' THEN 'https://datahelpdesk.worldbank.org/knowledgebase/articles/898581'
        ELSE NULL
    END,
    api_key_name = CASE
        WHEN name = 'Federal Reserve Economic Data (FRED)' THEN 'FRED_API_KEY'
        WHEN name = 'Bureau of Labor Statistics (BLS)' THEN 'BLS_API_KEY'
        WHEN name = 'U.S. Census Bureau' THEN 'CENSUS_API_KEY'
        ELSE NULL
    END
WHERE name IN ('Federal Reserve Economic Data (FRED)', 'Bureau of Labor Statistics (BLS)', 'U.S. Census Bureau', 'World Bank Open Data');

-- Set first_discovered_at to created_at for existing economic_series records
UPDATE economic_series
SET first_discovered_at = created_at
WHERE first_discovered_at IS NULL;

-- Insert sample global economic indicators
INSERT INTO global_economic_indicators (name, description, category, units, source, frequency) VALUES
    ('GDP Growth Rate', 'Annual percentage change in gross domestic product', 'Economic Growth', 'Percent', 'World Bank', 'Annual'),
    ('Inflation Rate', 'Annual percentage change in consumer price index', 'Price Stability', 'Percent', 'IMF', 'Monthly'),
    ('Unemployment Rate', 'Percentage of labor force that is unemployed', 'Labor Market', 'Percent', 'ILO', 'Monthly'),
    ('Interest Rate', 'Central bank policy interest rate', 'Monetary Policy', 'Percent', 'Central Banks', 'Monthly'),
    ('Trade Balance', 'Difference between exports and imports', 'Trade', 'USD', 'WTO', 'Monthly');

-- Insert sample countries
INSERT INTO countries (iso_code_2, iso_code_3, name, region, subregion, population, gdp_usd, currency_code) VALUES
    ('US', 'USA', 'United States', 'Americas', 'Northern America', 331900000, 22940000000000, 'USD'),
    ('CN', 'CHN', 'China', 'Asia', 'Eastern Asia', 1439000000, 17730000000000, 'CNY'),
    ('JP', 'JPN', 'Japan', 'Asia', 'Eastern Asia', 125800000, 4937000000000, 'JPY'),
    ('DE', 'DEU', 'Germany', 'Europe', 'Western Europe', 83200000, 4226000000000, 'EUR'),
    ('GB', 'GBR', 'United Kingdom', 'Europe', 'Northern Europe', 67000000, 3187000000000, 'GBP'),
    ('FR', 'FRA', 'France', 'Europe', 'Western Europe', 67400000, 2937000000000, 'EUR'),
    ('IT', 'ITA', 'Italy', 'Europe', 'Southern Europe', 59500000, 2108000000000, 'EUR'),
    ('BR', 'BRA', 'Brazil', 'Americas', 'South America', 213900000, 1609000000000, 'BRL'),
    ('CA', 'CAN', 'Canada', 'Americas', 'Northern America', 38200000, 1997000000000, 'CAD'),
    ('IN', 'IND', 'India', 'Asia', 'Southern Asia', 1380000000, 3176000000000, 'INR');
