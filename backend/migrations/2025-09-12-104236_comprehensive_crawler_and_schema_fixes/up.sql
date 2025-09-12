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
