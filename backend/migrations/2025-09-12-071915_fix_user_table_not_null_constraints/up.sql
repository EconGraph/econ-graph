-- Fix user table and user_sessions table NOT NULL constraints
-- Fields with default values should be NOT NULL to match Rust model expectations

-- Update existing NULL values to their defaults before adding NOT NULL constraints
UPDATE users SET theme = 'light' WHERE theme IS NULL;
UPDATE users SET default_chart_type = 'line' WHERE default_chart_type IS NULL;
UPDATE users SET notifications_enabled = true WHERE notifications_enabled IS NULL;
UPDATE users SET collaboration_enabled = true WHERE collaboration_enabled IS NULL;
UPDATE users SET is_active = true WHERE is_active IS NULL;
UPDATE users SET email_verified = false WHERE email_verified IS NULL;
UPDATE users SET created_at = NOW() WHERE created_at IS NULL;
UPDATE users SET updated_at = NOW() WHERE updated_at IS NULL;

-- Update user_sessions table
UPDATE user_sessions SET created_at = NOW() WHERE created_at IS NULL;
UPDATE user_sessions SET last_used_at = NOW() WHERE last_used_at IS NULL;

-- Add NOT NULL constraints to fields that have default values
ALTER TABLE users ALTER COLUMN theme SET NOT NULL;
ALTER TABLE users ALTER COLUMN default_chart_type SET NOT NULL;
ALTER TABLE users ALTER COLUMN notifications_enabled SET NOT NULL;
ALTER TABLE users ALTER COLUMN collaboration_enabled SET NOT NULL;
ALTER TABLE users ALTER COLUMN is_active SET NOT NULL;
ALTER TABLE users ALTER COLUMN email_verified SET NOT NULL;
ALTER TABLE users ALTER COLUMN created_at SET NOT NULL;
ALTER TABLE users ALTER COLUMN updated_at SET NOT NULL;

-- Add NOT NULL constraints to user_sessions table
ALTER TABLE user_sessions ALTER COLUMN created_at SET NOT NULL;
ALTER TABLE user_sessions ALTER COLUMN last_used_at SET NOT NULL;
