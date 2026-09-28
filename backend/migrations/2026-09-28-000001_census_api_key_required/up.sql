-- The Census Data API now rejects requests without a key, so the crawler requires
-- CENSUS_API_KEY. Record that on the existing data source row.
UPDATE data_sources
SET api_key_required = true,
    api_key_name = 'CENSUS_API_KEY'
WHERE name = 'U.S. Census Bureau';
