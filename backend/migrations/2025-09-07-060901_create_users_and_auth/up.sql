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
    ip_address INET
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

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create triggers for updated_at columns
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON chart_annotations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON annotation_comments
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
