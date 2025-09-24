-- Down migration for global analysis schema
-- Drops all global analysis tables in reverse order

-- Drop leading indicators table
DROP TABLE IF EXISTS leading_indicators CASCADE;

-- Drop event country impacts table
DROP TABLE IF EXISTS event_country_impacts CASCADE;

-- Drop global economic events table
DROP TABLE IF EXISTS global_economic_events CASCADE;

-- Drop trade relationships table
DROP TABLE IF EXISTS trade_relationships CASCADE;

-- Drop country correlations table
DROP TABLE IF EXISTS country_correlations CASCADE;

-- Drop global indicator data table
DROP TABLE IF EXISTS global_indicator_data CASCADE;

-- Drop global economic indicators table
DROP TABLE IF EXISTS global_economic_indicators CASCADE;

-- Drop countries table
DROP TABLE IF EXISTS countries CASCADE;
