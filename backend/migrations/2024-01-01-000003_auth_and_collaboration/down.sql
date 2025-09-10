-- Down migration for auth and collaboration schema
-- Drops all auth and collaboration tables in reverse order

-- Drop chart collaborators table
DROP TABLE IF EXISTS chart_collaborators CASCADE;

-- Drop annotation comments table
DROP TABLE IF EXISTS annotation_comments CASCADE;

-- Drop chart annotations table
DROP TABLE IF EXISTS chart_annotations CASCADE;

-- Drop user sessions table
DROP TABLE IF EXISTS user_sessions CASCADE;

-- Drop users table
DROP TABLE IF EXISTS users CASCADE;
