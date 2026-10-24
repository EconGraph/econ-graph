-- EconGraph v4.0.0 baseline schema: the app half.
--
-- The app tables (users and everything they create: sessions, chart annotations and comments,
-- collaborators, data source preferences, audit and security logs, financial annotations) and
-- their seed rows. They were part of 2026-10-21-000100_v4_0_baseline until that baseline was split
-- into migrations/data and migrations/app so the two can live in separate databases
-- (docs/development/MIGRATIONS.md). The split dropped the three foreign
-- keys from app tables to data tables; the columns and their indexes stay.
--
-- On a database that ran the single v4.0.0 baseline these tables already exist.
-- econ_graph_core::database::run_migrations records this migration there without running it; run
-- any other way, it stops at the guard below. Both migration sets use the same
-- __diesel_schema_migrations table when they share a database, so this version must not be
-- reused in migrations/data.
--
-- scripts/compare_migrations.sh checks that the data baseline followed by this file builds the
-- same schema and seed rows as the single v4.0.0 baseline, less the three dropped foreign keys.

DO $$
BEGIN
    IF current_setting('server_version_num')::int < 180000 THEN
        RAISE EXCEPTION 'EconGraph requires PostgreSQL 18 or newer (found %)', version();
    END IF;
    IF to_regclass('public.users') IS NOT NULL THEN
        RAISE EXCEPTION 'This database already has the app tables, but not the v4.0.0 app baseline recorded'
            USING HINT = 'A database built by the single v4.0.0 baseline is brought forward by the backend '
                '(econ_graph_core::database::run_migrations), which records this migration without running it; '
                'see docs/development/MIGRATIONS.md.';
    END IF;
END $$;

-- Both migration sets create these, so either can be applied to an empty database on its own.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- ============================================================================
-- ENUM TYPES
-- ============================================================================

-- Annotation types for collaborative features. Every variant of the Rust AnnotationType enum
-- (econ-graph-core/src/enums.rs) can be stored.
CREATE TYPE annotation_type AS ENUM (
    'comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight',
    'revenue_growth', 'cost_concern', 'cash_flow', 'balance_sheet', 'one_time_item',
    'industry_context'
);

-- Annotation status
CREATE TYPE annotation_status AS ENUM ('active', 'resolved', 'archived');

-- Assignment types
CREATE TYPE assignment_type AS ENUM ('review', 'analyze', 'verify', 'approve', 'investigate');

-- Assignment status
CREATE TYPE assignment_status AS ENUM ('pending', 'in_progress', 'completed', 'overdue', 'cancelled');

-- Who can see a chart annotation.
CREATE TYPE annotation_visibility AS ENUM ('private', 'public');

-- ============================================================================
-- USERS AND COLLABORATION
-- ============================================================================

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    email VARCHAR(255) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    avatar_url TEXT,
    provider VARCHAR(50) NOT NULL DEFAULT 'email', -- 'google', 'facebook', 'email'
    provider_id VARCHAR(255), -- OAuth provider user ID
    password_hash VARCHAR(255), -- For email authentication
    role VARCHAR(50) NOT NULL DEFAULT 'viewer', -- 'admin', 'analyst', 'viewer'
    organization VARCHAR(255),

    -- User preferences
    theme VARCHAR(20) NOT NULL DEFAULT 'light',
    default_chart_type VARCHAR(50) NOT NULL DEFAULT 'line',
    notifications_enabled BOOLEAN NOT NULL DEFAULT true,
    collaboration_enabled BOOLEAN NOT NULL DEFAULT true,

    -- Metadata
    is_active BOOLEAN NOT NULL DEFAULT true,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_login_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    user_agent TEXT,
    ip_address TEXT -- Using TEXT instead of INET for better compatibility
);

CREATE TABLE chart_annotations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    is_pinned BOOLEAN DEFAULT false,
    tags TEXT[], -- Array of tags

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    visibility annotation_visibility NOT NULL DEFAULT 'private'
);

CREATE TABLE annotation_comments (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    annotation_id UUID NOT NULL REFERENCES chart_annotations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    content TEXT NOT NULL,
    is_resolved BOOLEAN DEFAULT false,

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Sharing permissions: one grant per user per chart.
CREATE TABLE chart_collaborators (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    chart_id UUID NOT NULL, -- Custom chart identifier
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invited_by UUID REFERENCES users(id),

    role VARCHAR(20) DEFAULT 'viewer', -- 'owner', 'editor', 'viewer'
    permissions JSONB DEFAULT '{"view": true, "annotate": false, "edit": false}',

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE,

    CONSTRAINT chart_collaborators_chart_user_unique UNIQUE (chart_id, user_id)
);

CREATE TABLE user_data_source_preferences (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- A data_sources id in the data database. Not a foreign key: the app and data tables can be
    -- in separate databases.
    data_source_id UUID NOT NULL,
    is_visible BOOLEAN NOT NULL DEFAULT true,
    is_favorite BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, data_source_id)
);

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
CREATE INDEX idx_user_data_source_preferences_data_source_id ON user_data_source_preferences(data_source_id);
CREATE INDEX idx_user_data_source_preferences_visible ON user_data_source_preferences(is_visible);

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON chart_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON annotation_comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_user_data_source_preferences_updated_at
    BEFORE UPDATE ON user_data_source_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- AUDIT AND SECURITY
-- ============================================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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

CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_resource_type ON audit_logs(resource_type);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at);

CREATE TABLE security_events (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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

CREATE INDEX idx_security_events_event_type ON security_events(event_type);
CREATE INDEX idx_security_events_user_id ON security_events(user_id);
CREATE INDEX idx_security_events_severity ON security_events(severity);
CREATE INDEX idx_security_events_resolved ON security_events(resolved);
CREATE INDEX idx_security_events_created_at ON security_events(created_at);

-- Collaborative annotations on financial statements

CREATE TABLE financial_annotations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    -- financial_statements and financial_line_items ids in the data database. Not foreign keys:
    -- the app and data tables can be in separate databases.
    statement_id UUID NOT NULL,
    line_item_id UUID,

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

CREATE TABLE annotation_assignments (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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

CREATE TABLE annotation_replies (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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

CREATE TABLE annotation_templates (
    id UUID PRIMARY KEY DEFAULT uuidv7(),

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

CREATE INDEX idx_financial_annotations_statement_id ON financial_annotations(statement_id);
CREATE INDEX idx_financial_annotations_line_item_id ON financial_annotations(line_item_id);
CREATE INDEX idx_financial_annotations_type ON financial_annotations(annotation_type);
CREATE INDEX idx_financial_annotations_status ON financial_annotations(status);
CREATE INDEX idx_financial_annotations_created_by ON financial_annotations(created_by);

CREATE INDEX idx_annotation_assignments_annotation_id ON annotation_assignments(annotation_id);
CREATE INDEX idx_annotation_assignments_assigned_to ON annotation_assignments(assigned_to);
CREATE INDEX idx_annotation_assignments_status ON annotation_assignments(status);

CREATE INDEX idx_annotation_replies_annotation_id ON annotation_replies(annotation_id);
CREATE INDEX idx_annotation_replies_parent_reply_id ON annotation_replies(parent_reply_id);
CREATE INDEX idx_annotation_replies_created_by ON annotation_replies(created_by);

CREATE INDEX idx_annotation_templates_type ON annotation_templates(annotation_type);
CREATE INDEX idx_annotation_templates_active ON annotation_templates(is_active);

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

DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'users', 'user_sessions', 'chart_annotations', 'annotation_comments', 'chart_collaborators',
        'audit_logs', 'security_events', 'user_data_source_preferences', 'financial_annotations',
        'annotation_assignments', 'annotation_replies', 'annotation_templates'
    ] LOOP
        EXECUTE format('COMMENT ON COLUMN %I.id IS %L', t,
                       'Primary key using UUIDv7 format for better performance and sortability');
    END LOOP;
END $$;

-- ============================================================================
-- SEED DATA
-- ============================================================================

INSERT INTO annotation_templates (name, description, annotation_type, template_content, created_by) VALUES
    ('Revenue Analysis', 'Template for analyzing revenue trends and patterns', 'insight', 'Revenue Analysis:\n\n1. Trend Analysis:\n   - Period-over-period growth: [X]%\n   - Year-over-year growth: [Y]%\n\n2. Key Observations:\n   - [Observation 1]\n   - [Observation 2]\n\n3. Recommendations:\n   - [Recommendation 1]\n   - [Recommendation 2]', '00000000-0000-0000-0000-000000000000'),
    ('Risk Assessment', 'Template for identifying and assessing financial risks', 'risk', 'Risk Assessment:\n\n1. Identified Risks:\n   - [Risk 1]: [Description]\n   - [Risk 2]: [Description]\n\n2. Impact Assessment:\n   - [Risk 1]: [High/Medium/Low]\n   - [Risk 2]: [High/Medium/Low]\n\n3. Mitigation Strategies:\n   - [Strategy 1]\n   - [Strategy 2]', '00000000-0000-0000-0000-000000000000'),
    ('Data Quality Concern', 'Template for reporting data quality issues', 'concern', 'Data Quality Concern:\n\n1. Issue Description:\n   [Detailed description of the data quality issue]\n\n2. Affected Data:\n   - [Data point 1]\n   - [Data point 2]\n\n3. Recommended Actions:\n   - [Action 1]\n   - [Action 2]', '00000000-0000-0000-0000-000000000000');
