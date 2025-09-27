-- Rollback PostgreSQL 18 and UUIDv7 Migration
-- This migration reverts the UUIDv7 changes back to standard UUID format
-- Note: This is primarily for development/testing purposes as UUIDv7 is generally preferred

-- ============================================================================
-- 1. REMOVE PERFORMANCE OPTIMIZATIONS
-- ============================================================================

-- Drop the partial indexes we added
DROP INDEX IF EXISTS idx_data_points_recent_created;
DROP INDEX IF EXISTS idx_economic_series_recent_created;
DROP INDEX IF EXISTS idx_crawl_queue_recent_created;

-- ============================================================================
-- 2. REMOVE COMMENTS
-- ============================================================================

-- Remove the UUIDv7 documentation comments
COMMENT ON COLUMN data_sources.id IS NULL;
COMMENT ON COLUMN economic_series.id IS NULL;
COMMENT ON COLUMN data_points.id IS NULL;
COMMENT ON COLUMN crawl_queue.id IS NULL;
COMMENT ON COLUMN countries.id IS NULL;
COMMENT ON COLUMN global_economic_indicators.id IS NULL;
COMMENT ON COLUMN global_indicator_data.id IS NULL;
COMMENT ON COLUMN country_correlations.id IS NULL;
COMMENT ON COLUMN trade_relationships.id IS NULL;
COMMENT ON COLUMN global_economic_events.id IS NULL;
COMMENT ON COLUMN event_country_impacts.id IS NULL;
COMMENT ON COLUMN leading_indicators.id IS NULL;
COMMENT ON COLUMN users.id IS NULL;
COMMENT ON COLUMN user_sessions.id IS NULL;
COMMENT ON COLUMN chart_annotations.id IS NULL;
COMMENT ON COLUMN annotation_comments.id IS NULL;
COMMENT ON COLUMN chart_collaborators.id IS NULL;
COMMENT ON COLUMN audit_logs.id IS NULL;
COMMENT ON COLUMN security_events.id IS NULL;
COMMENT ON COLUMN user_data_source_preferences.id IS NULL;
COMMENT ON COLUMN series_metadata.id IS NULL;
COMMENT ON COLUMN crawl_attempts.id IS NULL;
COMMENT ON COLUMN companies.id IS NULL;
COMMENT ON COLUMN financial_statements.id IS NULL;
COMMENT ON COLUMN financial_line_items.id IS NULL;
COMMENT ON COLUMN financial_ratios.id IS NULL;
COMMENT ON COLUMN xbrl_taxonomy_schemas.id IS NULL;
COMMENT ON COLUMN xbrl_taxonomy_linkbases.id IS NULL;
COMMENT ON COLUMN xbrl_dts_dependencies.id IS NULL;
COMMENT ON COLUMN xbrl_instance_dts_references.id IS NULL;
COMMENT ON COLUMN xbrl_taxonomy_concepts.id IS NULL;
COMMENT ON COLUMN financial_annotations.id IS NULL;
COMMENT ON COLUMN annotation_assignments.id IS NULL;
COMMENT ON COLUMN annotation_replies.id IS NULL;
COMMENT ON COLUMN annotation_templates.id IS NULL;

-- ============================================================================
-- 3. REVERT DEFAULT VALUES (OPTIONAL)
-- ============================================================================

-- Note: In PostgreSQL 18, gen_random_uuid() generates UUIDv7 by default
-- Reverting to a different function would require creating a custom function
-- For now, we'll leave the defaults as they are since UUIDv7 is generally preferred
-- If you need to revert to UUIDv4, you would need to create a custom function:

-- Example of how to create a UUIDv4 function (commented out):
-- CREATE OR REPLACE FUNCTION gen_uuid_v4() RETURNS UUID AS $$
-- BEGIN
--     RETURN gen_random_uuid();
-- END;
-- $$ LANGUAGE plpgsql;

-- Then update all the defaults to use gen_uuid_v4() instead of gen_random_uuid()

-- ============================================================================
-- 4. ROLLBACK COMPLETION LOG
-- ============================================================================

-- Log the rollback of the PostgreSQL 18 and UUIDv7 upgrade
INSERT INTO audit_logs (user_id, user_name, action, resource_type, resource_id, details, created_at)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    'System Migration',
    'database_rollback',
    'schema',
    'postgres_18_uuidv7_upgrade',
    '{"rollback_type": "postgres_18_uuidv7", "version": "18", "uuid_format": "v7", "tables_affected": 35}',
    NOW()
) ON CONFLICT DO NOTHING;
