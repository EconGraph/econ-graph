-- Down migration for initial schema
-- Drops all core economic data tables in reverse order

-- Drop crawl_queue table and its constraints
DROP TABLE IF EXISTS crawl_queue CASCADE;

-- Drop data_points table
DROP TABLE IF EXISTS data_points CASCADE;

-- Drop economic_series table
DROP TABLE IF EXISTS economic_series CASCADE;

-- Drop data_sources table
DROP TABLE IF EXISTS data_sources CASCADE;

-- Drop the updated_at trigger function
DROP FUNCTION IF EXISTS update_updated_at_column() CASCADE;
