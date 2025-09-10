-- Drop triggers
DROP TRIGGER IF EXISTS update_users_updated_at ON users;
DROP TRIGGER IF EXISTS update_chart_annotations_updated_at ON chart_annotations;
DROP TRIGGER IF EXISTS update_annotation_comments_updated_at ON annotation_comments;

-- Drop trigger function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop indexes
DROP INDEX IF EXISTS idx_users_email;
DROP INDEX IF EXISTS idx_users_provider;
DROP INDEX IF EXISTS idx_user_sessions_user_id;
DROP INDEX IF EXISTS idx_user_sessions_token_hash;
DROP INDEX IF EXISTS idx_user_sessions_expires_at;
DROP INDEX IF EXISTS idx_chart_annotations_user_id;
DROP INDEX IF EXISTS idx_chart_annotations_series_id;
DROP INDEX IF EXISTS idx_chart_annotations_date;
DROP INDEX IF EXISTS idx_annotation_comments_annotation_id;
DROP INDEX IF EXISTS idx_annotation_comments_user_id;
DROP INDEX IF EXISTS idx_chart_collaborators_chart_id;
DROP INDEX IF EXISTS idx_chart_collaborators_user_id;

-- Drop tables in reverse order (due to foreign key constraints)
DROP TABLE IF EXISTS chart_collaborators;
DROP TABLE IF EXISTS annotation_comments;
DROP TABLE IF EXISTS chart_annotations;
DROP TABLE IF EXISTS user_sessions;
DROP TABLE IF EXISTS users;
