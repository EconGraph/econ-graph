-- Change IP address field from INET to TEXT for better compatibility
ALTER TABLE user_sessions ALTER COLUMN ip_address TYPE TEXT;
