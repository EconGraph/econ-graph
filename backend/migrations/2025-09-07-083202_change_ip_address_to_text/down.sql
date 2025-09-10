-- Revert IP address field back to INET type
ALTER TABLE user_sessions ALTER COLUMN ip_address TYPE INET USING ip_address::INET;
