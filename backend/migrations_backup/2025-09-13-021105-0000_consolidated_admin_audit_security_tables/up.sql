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
