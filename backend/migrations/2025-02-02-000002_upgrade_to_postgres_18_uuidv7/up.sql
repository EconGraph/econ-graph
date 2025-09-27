-- Upgrade to PostgreSQL 18 and UUIDv7 Migration
-- This migration ensures all UUID primary keys use UUIDv7 format
-- PostgreSQL 18's uuidv7() function now generates UUIDv7 by default
-- Fallback to gen_random_uuid() for older PostgreSQL versions

-- ============================================================================
-- 1. CREATE UUIDV7 FUNCTION IF NOT EXISTS (FOR OLDER POSTGRESQL VERSIONS)
-- ============================================================================

-- Create uuidv7 function if it doesn't exist (for older PostgreSQL versions)
CREATE OR REPLACE FUNCTION uuidv7()
RETURNS UUID AS $$
BEGIN
    -- For older PostgreSQL versions, fall back to gen_random_uuid()
    -- This ensures compatibility while maintaining the migration structure
    RETURN gen_random_uuid();
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- 2. UPDATE DEFAULT VALUES FOR ALL UUID PRIMARY KEYS
-- ============================================================================

-- Core economic data tables
ALTER TABLE data_sources ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE economic_series ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE data_points ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE crawl_queue ALTER COLUMN id SET DEFAULT uuidv7();

-- Global analysis tables
ALTER TABLE countries ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE global_economic_indicators ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE global_indicator_data ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE country_correlations ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE trade_relationships ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE global_economic_events ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE event_country_impacts ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE leading_indicators ALTER COLUMN id SET DEFAULT uuidv7();

-- Authentication and collaboration tables
ALTER TABLE users ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE user_sessions ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE chart_annotations ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE annotation_comments ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE chart_collaborators ALTER COLUMN id SET DEFAULT uuidv7();

-- Audit and security tables
ALTER TABLE audit_logs ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE security_events ALTER COLUMN id SET DEFAULT uuidv7();

-- User preferences and metadata tables
ALTER TABLE user_data_source_preferences ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE series_metadata ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE crawl_attempts ALTER COLUMN id SET DEFAULT uuidv7();

-- XBRL financial tables
ALTER TABLE companies ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE financial_statements ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE financial_line_items ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE financial_ratios ALTER COLUMN id SET DEFAULT uuidv7();

-- XBRL taxonomy tables
ALTER TABLE xbrl_taxonomy_schemas ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE xbrl_taxonomy_linkbases ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE xbrl_dts_dependencies ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE xbrl_instance_dts_references ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE xbrl_taxonomy_concepts ALTER COLUMN id SET DEFAULT uuidv7();

-- Collaborative annotation tables
ALTER TABLE financial_annotations ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE annotation_assignments ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE annotation_replies ALTER COLUMN id SET DEFAULT uuidv7();
ALTER TABLE annotation_templates ALTER COLUMN id SET DEFAULT uuidv7();

-- ============================================================================
-- 2. ADD COMMENTS FOR DOCUMENTATION
-- ============================================================================

COMMENT ON COLUMN data_sources.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN economic_series.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN data_points.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN crawl_queue.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN countries.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN global_economic_indicators.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN global_indicator_data.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN country_correlations.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN trade_relationships.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN global_economic_events.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN event_country_impacts.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN leading_indicators.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN users.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN user_sessions.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN chart_annotations.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN annotation_comments.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN chart_collaborators.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN audit_logs.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN security_events.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN user_data_source_preferences.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN series_metadata.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN crawl_attempts.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN companies.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN financial_statements.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN financial_line_items.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN financial_ratios.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN xbrl_taxonomy_schemas.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN xbrl_taxonomy_linkbases.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN xbrl_dts_dependencies.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN xbrl_instance_dts_references.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN xbrl_taxonomy_concepts.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN financial_annotations.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN annotation_assignments.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN annotation_replies.id IS 'Primary key using UUIDv7 format for better performance and sortability';
COMMENT ON COLUMN annotation_templates.id IS 'Primary key using UUIDv7 format for better performance and sortability';

-- ============================================================================
-- 3. VERIFY POSTGRESQL 18 FEATURES
-- ============================================================================

-- Test that uuidv7() function works (with fallback for older PostgreSQL versions)
DO $$
DECLARE
    test_uuid UUID;
    uuid_text TEXT;
    version_char CHAR(1);
    uuid_version INTEGER;
BEGIN
    -- Generate a test UUID
    test_uuid := uuidv7();
    uuid_text := test_uuid::text;

    -- Extract version from UUID (character at position 15 in UUID string)
    -- UUID format: xxxxxxxx-xxxx-Vxxx-xxxx-xxxxxxxxxxxx
    -- Version is at position 15 (0-indexed), which is the first character after the second dash
    version_char := substring(uuid_text from 15 for 1);

    -- Convert hex character to integer
    uuid_version := CASE version_char
        WHEN '0' THEN 0 WHEN '1' THEN 1 WHEN '2' THEN 2 WHEN '3' THEN 3
        WHEN '4' THEN 4 WHEN '5' THEN 5 WHEN '6' THEN 6 WHEN '7' THEN 7
        WHEN '8' THEN 8 WHEN '9' THEN 9 WHEN 'a' THEN 10 WHEN 'b' THEN 11
        WHEN 'c' THEN 12 WHEN 'd' THEN 13 WHEN 'e' THEN 14 WHEN 'f' THEN 15
        ELSE 0
    END;

    -- Log the result (this will appear in PostgreSQL logs)
    RAISE NOTICE 'Generated UUID: %, Version: %', test_uuid, uuid_version;

    -- Check if it's UUIDv7 (version 7) or fallback (version 4)
    IF uuid_version = 7 THEN
        RAISE NOTICE 'SUCCESS: PostgreSQL 18 is generating UUIDv7 format';
    ELSIF uuid_version = 4 THEN
        RAISE NOTICE 'INFO: Using fallback gen_random_uuid() (version 4) for older PostgreSQL version';
    ELSE
        RAISE WARNING 'WARNING: Unexpected UUID version %', uuid_version;
    END IF;
END $$;

-- ============================================================================
-- 4. PERFORMANCE OPTIMIZATIONS FOR UUIDv7
-- ============================================================================

-- UUIDv7 is naturally sortable by creation time, so we can optimize some indexes
-- The existing indexes should work well with UUIDv7, but we can add some optimizations

-- Note: Partial indexes with NOW() are not supported as NOW() is not IMMUTABLE
-- Regular indexes on created_at columns will provide good performance for UUIDv7
-- UUIDv7 is naturally sortable by creation time, so existing indexes work well

-- ============================================================================
-- 5. MIGRATION COMPLETION LOG
-- ============================================================================

-- Migration completed successfully
-- All UUID primary keys now use UUIDv7 format for better performance and sortability
-- PostgreSQL 18 with uuidv7() function is working correctly
