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

-- Defaults stay on uuidv7(). gen_random_uuid() still generates UUIDv4 in PostgreSQL 18; the
-- built-in uuidv7() is what generates UUIDv7.
