UPDATE data_sources
SET api_key_required = false,
    api_key_name = NULL
WHERE name = 'U.S. Census Bureau';
