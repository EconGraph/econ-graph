-- Revert Comprehensive Crawler Enhancements and Schema Fixes Migration
-- This migration reverts all changes in reverse order

-- ============================================================================
-- 5. REMOVE ADDITIONAL DATA SOURCES
-- ============================================================================

-- Remove BEA data source
DELETE FROM data_sources WHERE name = 'Bureau of Economic Analysis (BEA)';

-- Remove IMF data source
DELETE FROM data_sources WHERE name = 'International Monetary Fund (IMF)';

-- ============================================================================
-- 4. REVERT USER TABLE NOT NULL CONSTRAINT FIXES
-- ============================================================================

-- Revert user table and user_sessions table NOT NULL constraints
-- Remove NOT NULL constraints to allow NULL values again

ALTER TABLE users ALTER COLUMN theme DROP NOT NULL;
ALTER TABLE users ALTER COLUMN default_chart_type DROP NOT NULL;
ALTER TABLE users ALTER COLUMN notifications_enabled DROP NOT NULL;
ALTER TABLE users ALTER COLUMN collaboration_enabled DROP NOT NULL;
ALTER TABLE users ALTER COLUMN is_active DROP NOT NULL;
ALTER TABLE users ALTER COLUMN email_verified DROP NOT NULL;
ALTER TABLE users ALTER COLUMN created_at DROP NOT NULL;
ALTER TABLE users ALTER COLUMN updated_at DROP NOT NULL;

-- Revert user_sessions table constraints
ALTER TABLE user_sessions ALTER COLUMN created_at DROP NOT NULL;
ALTER TABLE user_sessions ALTER COLUMN last_used_at DROP NOT NULL;

-- ============================================================================
-- 3. REMOVE CRAWL ATTEMPTS TRACKING
-- ============================================================================

-- Drop trigger for updated_at
DROP TRIGGER IF EXISTS set_crawl_attempts_updated_at ON crawl_attempts;

-- Drop indexes for crawl_attempts
DROP INDEX IF EXISTS idx_crawl_attempts_series_success;
DROP INDEX IF EXISTS idx_crawl_attempts_series_attempted;
DROP INDEX IF EXISTS idx_crawl_attempts_success_attempted;
DROP INDEX IF EXISTS idx_crawl_attempts_latest_data_date;
DROP INDEX IF EXISTS idx_crawl_attempts_error_type;
DROP INDEX IF EXISTS idx_crawl_attempts_data_found;
DROP INDEX IF EXISTS idx_crawl_attempts_success;
DROP INDEX IF EXISTS idx_crawl_attempts_attempted_at;
DROP INDEX IF EXISTS idx_crawl_attempts_series_id;

-- Drop crawl_attempts table
DROP TABLE IF EXISTS crawl_attempts;

-- ============================================================================
-- 2. REMOVE SERIES-LEVEL CRAWL TRACKING
-- ============================================================================

-- Remove series-level crawl tracking fields from economic_series table
ALTER TABLE economic_series DROP COLUMN IF EXISTS crawl_error_message;
ALTER TABLE economic_series DROP COLUMN IF EXISTS crawl_status;
ALTER TABLE economic_series DROP COLUMN IF EXISTS first_missing_date;
ALTER TABLE economic_series DROP COLUMN IF EXISTS last_crawled_at;
ALTER TABLE economic_series DROP COLUMN IF EXISTS first_discovered_at;

-- ============================================================================
-- 1. REMOVE DATA SOURCE VISIBILITY CONTROLS
-- ============================================================================

-- Drop trigger for user preferences
DROP TRIGGER IF EXISTS update_user_data_source_preferences_updated_at ON user_data_source_preferences;

-- Drop indexes for user preferences
DROP INDEX IF EXISTS idx_user_data_source_preferences_visible;
DROP INDEX IF EXISTS idx_user_data_source_preferences_data_source_id;
DROP INDEX IF EXISTS idx_user_data_source_preferences_user_id;

-- Drop user preferences table
DROP TABLE IF EXISTS user_data_source_preferences;

-- Drop indexes for data sources
DROP INDEX IF EXISTS idx_data_sources_crawl_status;
DROP INDEX IF EXISTS idx_data_sources_enabled;
DROP INDEX IF EXISTS idx_data_sources_visible;

-- Remove visibility and admin controls from data_sources table
ALTER TABLE data_sources DROP COLUMN IF EXISTS api_documentation_url;
ALTER TABLE data_sources DROP COLUMN IF EXISTS crawl_error_message;
ALTER TABLE data_sources DROP COLUMN IF EXISTS crawl_status;
ALTER TABLE data_sources DROP COLUMN IF EXISTS last_crawl_at;
ALTER TABLE data_sources DROP COLUMN IF EXISTS crawl_frequency_hours;
ALTER TABLE data_sources DROP COLUMN IF EXISTS requires_admin_approval;
ALTER TABLE data_sources DROP COLUMN IF EXISTS is_enabled;
ALTER TABLE data_sources DROP COLUMN IF EXISTS is_visible;
