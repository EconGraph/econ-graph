-- Create series_metadata table to store discovered series information
CREATE TABLE series_metadata (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50),
    geographic_level VARCHAR(100),
    data_url TEXT,
    api_endpoint TEXT,
    last_discovered_at TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Ensure unique external_id per source
    UNIQUE(source_id, external_id)
);

-- Create indexes for efficient querying
CREATE INDEX idx_series_metadata_source_id ON series_metadata(source_id);
CREATE INDEX idx_series_metadata_external_id ON series_metadata(external_id);
CREATE INDEX idx_series_metadata_last_discovered ON series_metadata(last_discovered_at);
CREATE INDEX idx_series_metadata_active ON series_metadata(is_active);

-- Add trigger for updated_at
CREATE OR REPLACE FUNCTION update_series_metadata_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_series_metadata_updated_at
    BEFORE UPDATE ON series_metadata
    FOR EACH ROW
    EXECUTE FUNCTION update_series_metadata_updated_at();

-- Insert initial series metadata for existing data sources
-- FRED (Federal Reserve Economic Data)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'GDP',
    'Gross Domestic Product',
    'Real gross domestic product, seasonally adjusted annual rate',
    'Billions of Chained 2017 Dollars',
    'Quarterly',
    'United States',
    'https://fred.stlouisfed.org/series/GDP',
    'https://api.stlouisfed.org/fred/series/observations?series_id=GDP&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'UNRATE',
    'Unemployment Rate',
    'Unemployment rate, seasonally adjusted',
    'Percent',
    'Monthly',
    'United States',
    'https://fred.stlouisfed.org/series/UNRATE',
    'https://api.stlouisfed.org/fred/series/observations?series_id=UNRATE&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'CPIAUCSL',
    'Consumer Price Index for All Urban Consumers',
    'Consumer Price Index for All Urban Consumers: All Items in U.S. City Average',
    'Index 1982-1984=100',
    'Monthly',
    'United States',
    'https://fred.stlouisfed.org/series/CPIAUCSL',
    'https://api.stlouisfed.org/fred/series/observations?series_id=CPIAUCSL&api_key=YOUR_API_KEY&file_type=json',
    true
FROM data_sources ds WHERE ds.name = 'Federal Reserve Economic Data (FRED)';

-- BLS (Bureau of Labor Statistics)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'CES0000000001',
    'All Employees, Total Nonfarm',
    'Total nonfarm employment, seasonally adjusted',
    'Thousands of Persons',
    'Monthly',
    'United States',
    'https://data.bls.gov/timeseries/CES0000000001',
    'https://api.bls.gov/publicAPI/v2/timeseries/data/CES0000000001',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Labor Statistics (BLS)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'LNS14000000',
    'Unemployment Rate',
    'Unemployment rate, seasonally adjusted',
    'Percent',
    'Monthly',
    'United States',
    'https://data.bls.gov/timeseries/LNS14000000',
    'https://api.bls.gov/publicAPI/v2/timeseries/data/LNS14000000',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Labor Statistics (BLS)';

-- Census Bureau
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'B01001001',
    'Total Population',
    'Total population estimate',
    'Persons',
    'Annual',
    'United States',
    'https://api.census.gov/data/2023/pep/population',
    'https://api.census.gov/data/2023/pep/population?get=B01001_001E&for=us:1',
    true
FROM data_sources ds WHERE ds.name = 'U.S. Census Bureau';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'B19013_001E',
    'Median Household Income',
    'Median household income in the past 12 months',
    'Dollars',
    'Annual',
    'United States',
    'https://api.census.gov/data/2023/acs/acs5',
    'https://api.census.gov/data/2023/acs/acs5?get=B19013_001E&for=us:1',
    true
FROM data_sources ds WHERE ds.name = 'U.S. Census Bureau';

-- BEA (Bureau of Economic Analysis)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'GDP',
    'Gross Domestic Product',
    'Gross domestic product, current dollars',
    'Millions of Dollars',
    'Quarterly',
    'United States',
    'https://apps.bea.gov/api/data',
    'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=GDP&TableName=T10101&Frequency=Q&Year=2023&ResultFormat=JSON',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Economic Analysis (BEA)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'PCE',
    'Personal Consumption Expenditures',
    'Personal consumption expenditures, current dollars',
    'Millions of Dollars',
    'Quarterly',
    'United States',
    'https://apps.bea.gov/api/data',
    'https://apps.bea.gov/api/data/?&UserID=YOUR_API_KEY&method=GetData&datasetname=NIPA&TableName=T20301&Frequency=Q&Year=2023&ResultFormat=JSON',
    true
FROM data_sources ds WHERE ds.name = 'Bureau of Economic Analysis (BEA)';

-- World Bank
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NY.GDP.MKTP.CD',
    'GDP (current US$)',
    'GDP at purchaser''s prices is the sum of gross value added by all resident producers in the economy plus any product taxes and minus any subsidies not included in the value of the products',
    'Current US$',
    'Annual',
    'Country',
    'https://data.worldbank.org/indicator/NY.GDP.MKTP.CD',
    'https://api.worldbank.org/v2/country/all/indicator/NY.GDP.MKTP.CD?format=json',
    true
FROM data_sources ds WHERE ds.name = 'World Bank';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'SP.POP.TOTL',
    'Population, total',
    'Total population is based on the de facto definition of population, which counts all residents regardless of legal status or citizenship',
    'Persons',
    'Annual',
    'Country',
    'https://data.worldbank.org/indicator/SP.POP.TOTL',
    'https://api.worldbank.org/v2/country/all/indicator/SP.POP.TOTL?format=json',
    true
FROM data_sources ds WHERE ds.name = 'World Bank';

-- IMF
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NGDP_R_SA_XDC',
    'Gross domestic product, real, seasonally adjusted',
    'Gross domestic product, real, seasonally adjusted, national currency',
    'National currency',
    'Quarterly',
    'Country',
    'https://data.imf.org/regular.aspx?key=61545850',
    'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_R_SA_XDC',
    true
FROM data_sources ds WHERE ds.name = 'International Monetary Fund (IMF)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'NGDP_XDC',
    'Gross domestic product, current prices',
    'Gross domestic product, current prices, national currency',
    'National currency',
    'Quarterly',
    'Country',
    'https://data.imf.org/regular.aspx?key=61545850',
    'https://dataservices.imf.org/REST/SDMX_JSON.svc/CompactData/IFS/Q.US.NGDP_XDC',
    true
FROM data_sources ds WHERE ds.name = 'International Monetary Fund (IMF)';

-- FHFA
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'HPI_USA',
    'U.S. House Price Index',
    'U.S. House Price Index, seasonally adjusted',
    'Index (January 1991=100)',
    'Quarterly',
    'United States',
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    'https://api.fhfa.gov/house-price-index',
    true
FROM data_sources ds WHERE ds.name = 'Federal Housing Finance Agency (FHFA)';

INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'HPI_STATE',
    'State House Price Index',
    'State House Price Index, seasonally adjusted',
    'Index (January 1991=100)',
    'Quarterly',
    'State',
    'https://www.fhfa.gov/DataTools/Downloads/Pages/House-Price-Index-Datasets.aspx',
    'https://api.fhfa.gov/house-price-index/state',
    true
FROM data_sources ds WHERE ds.name = 'Federal Housing Finance Agency (FHFA)';

-- ECB (European Central Bank)
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ICP.M.U2.N.000000.4.ANR',
    'Euro area - Main refinancing operations rate',
    'Interest rate for main refinancing operations in the euro area',
    'Percent per annum',
    'Monthly',
    'Euro area',
    'https://sdw-wsrest.ecb.europa.eu/service/data/ICP/M.U2.N.000000.4.ANR',
    'https://sdw-wsrest.ecb.europa.eu/service/data/ICP/M.U2.N.000000.4.ANR',
    true
FROM data_sources ds WHERE ds.name = 'European Central Bank (ECB)';

-- OECD
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'SNA_TABLE1.1.GDP.B1_GE.CPCAR_M',
    'OECD - GDP at current prices',
    'Gross Domestic Product at current prices for OECD countries',
    'Millions of national currency',
    'Annual',
    'Country',
    'https://data-explorer.oecd.org/vis?fs[0]=Topic%2C1%7C1%7C1%7C0',
    'https://sdmx.oecd.org/public/rest/data/OECD.SNA_TABLE1,DSD_SNA_TABLE1@DF_SNA_TABLE1,1.0/1.GDP.B1_GE.CPCAR_M',
    true
FROM data_sources ds WHERE ds.name = 'OECD (Organisation for Economic Co-operation and Development)';

-- Bank of England
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'IUDBEDR',
    'UK - Bank Rate',
    'Official Bank Rate set by the Bank of England''s Monetary Policy Committee',
    'Percent per annum',
    'Monthly',
    'United Kingdom',
    'https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&SeriesCodes=IUDBEDR',
    'https://www.bankofengland.co.uk/boeapps/database/_iadb-fromshowcolumns.asp?csv.x=yes&SeriesCodes=IUDBEDR',
    true
FROM data_sources ds WHERE ds.name = 'Bank of England (BoE)';

-- WTO
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'MT_GOODS_EXP',
    'WTO - Merchandise exports',
    'Merchandise exports for WTO member countries',
    'Millions of US dollars',
    'Annual',
    'Country',
    'https://www.wto.org/english/res_e/statis_e/data_explorer_e.htm',
    'https://api.wto.org/timeseries/v1/data/MT_GOODS_EXP',
    true
FROM data_sources ds WHERE ds.name = 'World Trade Organization (WTO)';

-- Bank of Japan
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'BOJ_UNRATE',
    'Japan - Policy interest rate',
    'Policy interest rate set by the Bank of Japan',
    'Percent per annum',
    'Monthly',
    'Japan',
    'https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1',
    'https://www.stat-search.boj.or.jp/ssi/mtshtml/cgi-bin/ssi/mtshtml.cgi?svr=ssi&lst=1&page=1&id=1',
    true
FROM data_sources ds WHERE ds.name = 'Bank of Japan (BoJ)';

-- Reserve Bank of Australia
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'F1.1',
    'Australia - Cash Rate Target',
    'The cash rate target set by the Reserve Bank Board',
    'Percent per annum',
    'Monthly',
    'Australia',
    'https://www.rba.gov.au/statistics/f01-hist.html',
    'https://www.rba.gov.au/statistics/f01-hist.html',
    true
FROM data_sources ds WHERE ds.name = 'Reserve Bank of Australia (RBA)';

-- Bank of Canada
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'V39079',
    'Canada - Overnight Rate Target',
    'The target for the overnight rate set by the Bank of Canada',
    'Percent per annum',
    'Daily',
    'Canada',
    'https://www.bankofcanada.ca/valet/observations/V39079',
    'https://www.bankofcanada.ca/valet/observations/V39079',
    true
FROM data_sources ds WHERE ds.name = 'Bank of Canada (BoC)';

-- Swiss National Bank
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ir',
    'Switzerland - SNB Policy Rate',
    'Swiss National Bank policy rate',
    'Percent per annum',
    'Daily',
    'Switzerland',
    'https://data.snb.ch/en/ir',
    'https://data.snb.ch/en/ir',
    true
FROM data_sources ds WHERE ds.name = 'Swiss National Bank (SNB)';

-- UN Statistics Division
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'UN_GDP',
    'UN - GDP per capita',
    'Gross Domestic Product per capita from UN Statistics Division',
    'Current US dollars',
    'Annual',
    'Country',
    'https://unstats.un.org/unsd/snaama/Basic',
    'https://unstats.un.org/unsd/snaama/Basic',
    true
FROM data_sources ds WHERE ds.name = 'UN Statistics Division';

-- International Labour Organization
INSERT INTO series_metadata (source_id, external_id, title, description, units, frequency, geographic_level, data_url, api_endpoint, is_active)
SELECT
    ds.id,
    'ILO_UNEMPLOYMENT',
    'ILO - Unemployment Rate',
    'Unemployment rate from International Labour Organization',
    'Percent',
    'Annual',
    'Country',
    'https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm',
    'https://www.ilo.org/global/statistics-and-databases/lang--en/index.htm',
    true
FROM data_sources ds WHERE ds.name = 'International Labour Organization (ILO)';
