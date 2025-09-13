-- Add api_key_name field to data_sources table
-- This field contains the name of the environment variable for the API key
-- NULL means no API key is required

ALTER TABLE data_sources
ADD COLUMN api_key_name VARCHAR(255);

-- Insert or update all data sources with correct API key requirements
-- FRED requires API key
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Federal Reserve Economic Data (FRED)',
    'Economic data from the Federal Reserve Bank of St. Louis',
    'https://api.stlouisfed.org/fred',
    true,
    'FRED_API_KEY',
    120,
    true,
    true,
    false,
    6,
    'https://fred.stlouisfed.org/docs/api/fred/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- BLS does NOT require API key (public API v2)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Bureau of Labor Statistics (BLS)',
    'Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics',
    'https://api.bls.gov/publicAPI/v2',
    false,
    NULL,
    500,
    true,
    true,
    false,
    12,
    'https://www.bls.gov/developers/api_signature_v2.htm',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- Census does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'U.S. Census Bureau',
    'Demographic and economic data from the U.S. Census Bureau',
    'https://api.census.gov/data',
    false,
    NULL,
    500,
    false,
    false,
    true,
    24,
    'https://www.census.gov/data/developers/data-sets.html',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- BEA does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Bureau of Economic Analysis (BEA)',
    'U.S. economic statistics including GDP, NIPA, ITA, and Regional data',
    'https://apps.bea.gov/api/data',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://apps.bea.gov/api/bea_web_service_api_user_guide.htm',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- World Bank does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'World Bank Open Data',
    'Global economic and development indicators from the World Bank',
    'https://api.worldbank.org/v2',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- IMF does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'International Monetary Fund (IMF)',
    'Global economic and financial data including IFS, BOP, GFS, and WEO',
    'https://dataservices.imf.org/REST/SDMX_JSON.svc',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://datahelp.imf.org/knowledgebase/articles/2016-01-19/how-to-access-imf-data-programmatically',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- ECB does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'European Central Bank (ECB)',
    'European economic and monetary data from the European Central Bank',
    'https://sdw-wsrest.ecb.europa.eu/service',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://sdw-wsrest.ecb.europa.eu/help/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- OECD does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Organisation for Economic Co-operation and Development (OECD)',
    'Global economic and social statistics from the OECD',
    'https://sdmx.oecd.org/public/rest/data',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://data-explorer.oecd.org/vis?fs[0]=Topic%2C1%7C1%7C*&lc=en',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- BOE does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Bank of England (BOE)',
    'UK economic and financial data from the Bank of England',
    'https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.bankofengland.co.uk/statistics/data',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- WTO does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'World Trade Organization (WTO)',
    'Global trade statistics and indicators from the WTO',
    'https://api.wto.org/timeseries/v1',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.wto.org/english/res_e/statis_e/data_services_e.htm',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- BOJ does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Bank of Japan (BOJ)',
    'Japanese economic and financial data from the Bank of Japan',
    'https://www.stat-search.boj.or.jp/ssi/mtshtml/csv/csv_download.html',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.boj.or.jp/en/statistics/index.htm',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- RBA does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Reserve Bank of Australia (RBA)',
    'Australian economic and financial data from the Reserve Bank of Australia',
    'https://www.rba.gov.au/statistics/',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.rba.gov.au/statistics/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- BOC does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Bank of Canada (BOC)',
    'Canadian economic and financial data from the Bank of Canada',
    'https://www.bankofcanada.ca/valet/observations',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.bankofcanada.ca/valet/docs',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- SNB does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Swiss National Bank (SNB)',
    'Swiss economic and financial data from the Swiss National Bank',
    'https://data.snb.ch/api/cube',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://data.snb.ch/en/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- UNStats does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'United Nations Statistics Division (UNStats)',
    'Global statistics from the United Nations Statistics Division',
    'https://unstats.un.org/SDGAPI/v1/sdg',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://unstats.un.org/sdgapi/swagger/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- ILO does NOT require API key (public API)
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'International Labour Organization (ILO)',
    'Global labor statistics from the International Labour Organization',
    'https://www.ilo.org/ilostat/api',
    false,
    NULL,
    1000,
    false,
    false,
    true,
    24,
    'https://www.ilo.org/ilostat/',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();

-- FHFA requires API key
INSERT INTO data_sources (id, name, description, base_url, api_key_required, api_key_name, rate_limit_per_minute, is_visible, is_enabled, requires_admin_approval, crawl_frequency_hours, api_documentation_url, created_at, updated_at)
VALUES (
    gen_random_uuid(),
    'Federal Housing Finance Agency (FHFA)',
    'U.S. housing finance data from the Federal Housing Finance Agency',
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    true,
    'FHFA_API_KEY',
    500,
    false,
    false,
    true,
    24,
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    NOW(),
    NOW()
)
ON CONFLICT (name) DO UPDATE SET
    api_key_name = EXCLUDED.api_key_name,
    api_key_required = EXCLUDED.api_key_required,
    updated_at = NOW();
