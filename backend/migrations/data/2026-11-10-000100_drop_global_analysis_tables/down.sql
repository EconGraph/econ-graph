-- Rollback of 2026-11-10-000100_drop_global_analysis_tables.
--
-- Recreates the eight legacy global-analysis tables, empty, with the indexes, triggers and
-- comments they had after 2026-10-25-000100 (global_indicator_data.value is unconstrained
-- NUMERIC). The DDL is copied from the v4.0 data baseline.
--
-- Data loss: the baseline's seed rows (20 countries and 5 global_economic_events with invented
-- impact scores) are not restored. Nothing read them.
--
-- This restores the schema only. The code that used these tables was deleted in the same
-- change, so a real rollback also reverts that PR (ECO-898 global analysis).

-- Countries table with geographic and economic metadata
CREATE TABLE countries (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    indicator_id UUID NOT NULL REFERENCES global_economic_indicators(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value NUMERIC,
    is_preliminary BOOLEAN NOT NULL DEFAULT false,
    data_source VARCHAR(50) NOT NULL, -- World Bank, IMF, OECD, etc.
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(indicator_id, date)
);

-- Economic correlations between countries
CREATE TABLE country_correlations (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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
    id UUID PRIMARY KEY DEFAULT uuidv7(),
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

CREATE INDEX idx_countries_region ON countries(region);
CREATE INDEX idx_countries_income_group ON countries(income_group);
CREATE INDEX idx_countries_iso_codes ON countries(iso_code, iso_code_2);

CREATE INDEX idx_global_indicators_country_category ON global_economic_indicators(country_id, category);
CREATE INDEX idx_global_indicators_code ON global_economic_indicators(indicator_code);

CREATE INDEX idx_global_data_date_value ON global_indicator_data(date, value) WHERE value IS NOT NULL;

CREATE INDEX idx_correlations_category ON country_correlations(indicator_category);
CREATE INDEX idx_correlations_strength ON country_correlations(correlation_coefficient DESC) WHERE is_significant = true;

CREATE INDEX idx_trade_exporter_year ON trade_relationships(exporter_country_id, year DESC);
CREATE INDEX idx_trade_importer_year ON trade_relationships(importer_country_id, year DESC);
CREATE INDEX idx_trade_value ON trade_relationships(export_value_usd DESC) WHERE export_value_usd IS NOT NULL;

CREATE INDEX idx_events_date ON global_economic_events(start_date DESC);
CREATE INDEX idx_events_severity ON global_economic_events(severity, economic_impact_score DESC);

CREATE INDEX idx_event_impacts_country ON event_country_impacts(country_id, impact_magnitude DESC);

CREATE INDEX idx_leading_indicators_strength ON leading_indicators(correlation_strength DESC, predictive_accuracy DESC);

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

COMMENT ON TABLE countries IS 'Master table of countries with geographic and economic metadata for global analysis';
COMMENT ON TABLE global_economic_indicators IS 'Economic indicators available for cross-country analysis';
COMMENT ON TABLE global_indicator_data IS 'Time series data for global economic indicators';
COMMENT ON TABLE country_correlations IS 'Calculated correlations between countries for various economic indicators';
COMMENT ON TABLE trade_relationships IS 'Bilateral trade data between countries';
COMMENT ON TABLE global_economic_events IS 'Major global economic events and their characteristics';
COMMENT ON TABLE event_country_impacts IS 'Impact of global events on individual countries';
COMMENT ON TABLE leading_indicators IS 'Countries that lead others in economic indicators';

DO $$
DECLARE
    t TEXT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'countries', 'global_economic_indicators', 'global_indicator_data',
        'country_correlations', 'trade_relationships', 'global_economic_events',
        'event_country_impacts', 'leading_indicators'
    ] LOOP
        EXECUTE format('COMMENT ON COLUMN %I.id IS %L', t,
                       'Primary key using UUIDv7 format for better performance and sortability');
    END LOOP;
END $$;

COMMENT ON COLUMN global_indicator_data.value IS
    'Observation value, exactly as the source published it (unconstrained NUMERIC)';
