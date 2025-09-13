-- Consolidated Admin Audit and Security Tables Migration - Rollback
-- This migration removes all admin-related tables

-- Drop indexes first
DROP INDEX IF EXISTS idx_security_events_created_at;
DROP INDEX IF EXISTS idx_security_events_resolved;
DROP INDEX IF EXISTS idx_security_events_severity;
DROP INDEX IF EXISTS idx_security_events_user_id;
DROP INDEX IF EXISTS idx_security_events_event_type;

DROP INDEX IF EXISTS idx_audit_logs_created_at;
DROP INDEX IF EXISTS idx_audit_logs_resource_type;
DROP INDEX IF EXISTS idx_audit_logs_action;
DROP INDEX IF EXISTS idx_audit_logs_user_id;

-- Drop tables
DROP TABLE IF EXISTS security_events;
DROP TABLE IF EXISTS audit_logs;
