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
