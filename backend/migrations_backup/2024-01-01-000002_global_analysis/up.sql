-- Global Economic Network Analysis Schema
-- Creates comprehensive tables for cross-country economic analysis
-- This consolidates the global analysis migration into a cohesive schema

-- Countries table with geographic and economic metadata
CREATE TABLE countries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    iso_code VARCHAR(3) NOT NULL UNIQUE, -- ISO 3166-1 alpha-3 (USA, GBR, etc.)
    iso_code_2 VARCHAR(2) NOT NULL UNIQUE, -- ISO 3166-1 alpha-2 (US, GB, etc.)
    name VARCHAR(255) NOT NULL,
    region VARCHAR(100) NOT NULL, -- North America, Europe, Asia, etc.
    sub_region VARCHAR(100), -- Western Europe, Southeast Asia, etc.
    income_group VARCHAR(50), -- High income, Upper middle income, etc.
    population BIGINT,
    gdp_usd DECIMAL(20,2), -- GDP in USD
    gdp_per_capita_usd DECIMAL(15,2),
    latitude DECIMAL(10,8),
    longitude DECIMAL(11,8),
    currency_code VARCHAR(3), -- USD, EUR, GBP, etc.
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Economic indicators optimized for cross-country analysis
CREATE TABLE global_economic_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_code VARCHAR(50) NOT NULL, -- World Bank indicator codes (NY.GDP.MKTP.CD, etc.)
    indicator_name VARCHAR(500) NOT NULL,
    category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, Inflation, etc.
    subcategory VARCHAR(100), -- Real GDP, Nominal GDP, etc.
    unit VARCHAR(50), -- USD, Percent, Index, etc.
    frequency VARCHAR(20) NOT NULL, -- Annual, Quarterly, Monthly
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_id, indicator_code)
);

-- Time series data for global indicators
CREATE TABLE global_indicator_data (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    indicator_id UUID NOT NULL REFERENCES global_economic_indicators(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    is_preliminary BOOLEAN NOT NULL DEFAULT false,
    data_source VARCHAR(50) NOT NULL, -- World Bank, IMF, OECD, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(indicator_id, date)
);

-- Economic correlations between countries
CREATE TABLE country_correlations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    country_a_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    country_b_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL, -- GDP, Trade, Employment, etc.
    correlation_coefficient DECIMAL(5,4) NOT NULL, -- -1.0000 to 1.0000
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    sample_size INTEGER NOT NULL, -- Number of data points used
    p_value DECIMAL(10,8), -- Statistical significance
    is_significant BOOLEAN NOT NULL DEFAULT false,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(country_a_id, country_b_id, indicator_category, time_period_start, time_period_end),
    CHECK (country_a_id != country_b_id),
    CHECK (correlation_coefficient >= -1.0 AND correlation_coefficient <= 1.0)
);

-- Trade relationships between countries
CREATE TABLE trade_relationships (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    exporter_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    importer_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    trade_flow_type VARCHAR(20) NOT NULL, -- Goods, Services, Total
    year INTEGER NOT NULL,
    export_value_usd DECIMAL(20,2), -- Export value in USD
    import_value_usd DECIMAL(20,2), -- Import value in USD
    trade_balance_usd DECIMAL(20,2), -- Export - Import
    trade_intensity DECIMAL(8,6), -- Trade as % of GDP
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(exporter_country_id, importer_country_id, trade_flow_type, year),
    CHECK (exporter_country_id != importer_country_id)
);

-- Global economic events and their impacts
CREATE TABLE global_economic_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(500) NOT NULL,
    description TEXT,
    event_type VARCHAR(50) NOT NULL, -- Crisis, Policy, Natural Disaster, etc.
    severity VARCHAR(20) NOT NULL, -- Low, Medium, High, Critical
    start_date DATE NOT NULL,
    end_date DATE,
    primary_country_id UUID REFERENCES countries(id), -- Originating country
    affected_regions TEXT[], -- Array of affected regions
    economic_impact_score DECIMAL(5,2), -- 0-100 impact score
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Country impacts from global events
CREATE TABLE event_country_impacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL REFERENCES global_economic_events(id) ON DELETE CASCADE,
    country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    impact_type VARCHAR(50) NOT NULL, -- GDP, Employment, Trade, Financial
    impact_magnitude DECIMAL(8,4), -- Percentage change
    impact_duration_days INTEGER, -- How long the impact lasted
    recovery_time_days INTEGER, -- Time to return to pre-event levels
    confidence_score DECIMAL(3,2), -- 0-1 confidence in measurement
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(event_id, country_id, impact_type)
);

-- Economic leading indicators relationships
CREATE TABLE leading_indicators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    leading_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    following_country_id UUID NOT NULL REFERENCES countries(id) ON DELETE CASCADE,
    indicator_category VARCHAR(100) NOT NULL,
    lead_time_months INTEGER NOT NULL, -- How many months country A leads country B
    correlation_strength DECIMAL(5,4) NOT NULL, -- Correlation coefficient
    predictive_accuracy DECIMAL(5,4), -- Historical prediction accuracy (0-1)
    time_period_start DATE NOT NULL,
    time_period_end DATE NOT NULL,
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(leading_country_id, following_country_id, indicator_category),
    CHECK (leading_country_id != following_country_id),
    CHECK (lead_time_months >= 1 AND lead_time_months <= 24)
);

-- Create indexes for performance
CREATE INDEX idx_countries_region ON countries(region);
CREATE INDEX idx_countries_income_group ON countries(income_group);
CREATE INDEX idx_countries_iso_codes ON countries(iso_code, iso_code_2);

CREATE INDEX idx_global_indicators_country_category ON global_economic_indicators(country_id, category);
CREATE INDEX idx_global_indicators_code ON global_economic_indicators(indicator_code);

CREATE INDEX idx_global_data_indicator_date ON global_indicator_data(indicator_id, date DESC);
CREATE INDEX idx_global_data_date_value ON global_indicator_data(date, value) WHERE value IS NOT NULL;

CREATE INDEX idx_correlations_countries ON country_correlations(country_a_id, country_b_id);
CREATE INDEX idx_correlations_category ON country_correlations(indicator_category);
CREATE INDEX idx_correlations_strength ON country_correlations(correlation_coefficient DESC) WHERE is_significant = true;

CREATE INDEX idx_trade_exporter_year ON trade_relationships(exporter_country_id, year DESC);
CREATE INDEX idx_trade_importer_year ON trade_relationships(importer_country_id, year DESC);
CREATE INDEX idx_trade_value ON trade_relationships(export_value_usd DESC) WHERE export_value_usd IS NOT NULL;

CREATE INDEX idx_events_date ON global_economic_events(start_date DESC);
CREATE INDEX idx_events_severity ON global_economic_events(severity, economic_impact_score DESC);

CREATE INDEX idx_event_impacts_country ON event_country_impacts(country_id, impact_magnitude DESC);

CREATE INDEX idx_leading_indicators_strength ON leading_indicators(correlation_strength DESC, predictive_accuracy DESC);

-- Create updated_at triggers
CREATE TRIGGER update_countries_updated_at
    BEFORE UPDATE ON countries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_indicators_updated_at
    BEFORE UPDATE ON global_economic_indicators
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_global_events_updated_at
    BEFORE UPDATE ON global_economic_events
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert major countries for initial data
INSERT INTO countries (iso_code, iso_code_2, name, region, sub_region, income_group, latitude, longitude, currency_code) VALUES
    ('USA', 'US', 'United States', 'Americas', 'Northern America', 'High income', 39.8283, -98.5795, 'USD'),
    ('CHN', 'CN', 'China', 'Asia', 'Eastern Asia', 'Upper middle income', 35.8617, 104.1954, 'CNY'),
    ('JPN', 'JP', 'Japan', 'Asia', 'Eastern Asia', 'High income', 36.2048, 138.2529, 'JPY'),
    ('DEU', 'DE', 'Germany', 'Europe', 'Western Europe', 'High income', 51.1657, 10.4515, 'EUR'),
    ('GBR', 'GB', 'United Kingdom', 'Europe', 'Northern Europe', 'High income', 55.3781, -3.4360, 'GBP'),
    ('FRA', 'FR', 'France', 'Europe', 'Western Europe', 'High income', 46.2276, 2.2137, 'EUR'),
    ('IND', 'IN', 'India', 'Asia', 'Southern Asia', 'Lower middle income', 20.5937, 78.9629, 'INR'),
    ('ITA', 'IT', 'Italy', 'Europe', 'Southern Europe', 'High income', 41.8719, 12.5674, 'EUR'),
    ('BRA', 'BR', 'Brazil', 'Americas', 'South America', 'Upper middle income', -14.2350, -51.9253, 'BRL'),
    ('CAN', 'CA', 'Canada', 'Americas', 'Northern America', 'High income', 56.1304, -106.3468, 'CAD'),
    ('RUS', 'RU', 'Russian Federation', 'Europe', 'Eastern Europe', 'Upper middle income', 61.5240, 105.3188, 'RUB'),
    ('KOR', 'KR', 'South Korea', 'Asia', 'Eastern Asia', 'High income', 35.9078, 127.7669, 'KRW'),
    ('ESP', 'ES', 'Spain', 'Europe', 'Southern Europe', 'High income', 40.4637, -3.7492, 'EUR'),
    ('AUS', 'AU', 'Australia', 'Oceania', 'Australia and New Zealand', 'High income', -25.2744, 133.7751, 'AUD'),
    ('MEX', 'MX', 'Mexico', 'Americas', 'Central America', 'Upper middle income', 23.6345, -102.5528, 'MXN'),
    ('IDN', 'ID', 'Indonesia', 'Asia', 'South-Eastern Asia', 'Upper middle income', -0.7893, 113.9213, 'IDR'),
    ('NLD', 'NL', 'Netherlands', 'Europe', 'Western Europe', 'High income', 52.1326, 5.2913, 'EUR'),
    ('SAU', 'SA', 'Saudi Arabia', 'Asia', 'Western Asia', 'High income', 23.8859, 45.0792, 'SAR'),
    ('TUR', 'TR', 'Turkey', 'Asia', 'Western Asia', 'Upper middle income', 38.9637, 35.2433, 'TRY'),
    ('CHE', 'CH', 'Switzerland', 'Europe', 'Western Europe', 'High income', 46.8182, 8.2275, 'CHF');

-- Insert major global economic events for reference
INSERT INTO global_economic_events (name, description, event_type, severity, start_date, end_date, primary_country_id, economic_impact_score) VALUES
    ('2008 Global Financial Crisis', 'Global financial crisis originating from US subprime mortgage crisis', 'Crisis', 'Critical', '2007-12-01', '2009-06-01', (SELECT id FROM countries WHERE iso_code = 'USA'), 95.0),
    ('COVID-19 Pandemic', 'Global pandemic causing widespread economic disruption', 'Crisis', 'Critical', '2020-03-01', '2022-12-01', NULL, 98.0),
    ('European Debt Crisis', 'Sovereign debt crisis affecting eurozone countries', 'Crisis', 'High', '2010-01-01', '2012-12-01', (SELECT id FROM countries WHERE iso_code = 'DEU'), 75.0),
    ('Brexit', 'United Kingdom withdrawal from European Union', 'Policy', 'Medium', '2016-06-23', '2020-12-31', (SELECT id FROM countries WHERE iso_code = 'GBR'), 45.0),
    ('US-China Trade War', 'Trade dispute between United States and China', 'Policy', 'High', '2018-03-01', '2020-01-15', (SELECT id FROM countries WHERE iso_code = 'USA'), 65.0);

COMMENT ON TABLE countries IS 'Master table of countries with geographic and economic metadata for global analysis';
COMMENT ON TABLE global_economic_indicators IS 'Economic indicators available for cross-country analysis';
COMMENT ON TABLE global_indicator_data IS 'Time series data for global economic indicators';
COMMENT ON TABLE country_correlations IS 'Calculated correlations between countries for various economic indicators';
COMMENT ON TABLE trade_relationships IS 'Bilateral trade data between countries';
COMMENT ON TABLE global_economic_events IS 'Major global economic events and their characteristics';
COMMENT ON TABLE event_country_impacts IS 'Impact of global events on individual countries';
COMMENT ON TABLE leading_indicators IS 'Countries that lead others in economic indicators';
