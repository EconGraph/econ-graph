-- Initial Schema Migration
-- Creates the core economic data tables: data_sources, economic_series, data_points, and crawl_queue
-- This consolidates the original 4 separate migrations into one cohesive schema

-- Enable required PostgreSQL extensions
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Create data_sources table
CREATE TABLE data_sources (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    base_url VARCHAR(500) NOT NULL,
    api_key_required BOOLEAN NOT NULL DEFAULT FALSE,
    rate_limit_per_minute INTEGER NOT NULL DEFAULT 60,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index on name for faster lookups
CREATE INDEX idx_data_sources_name ON data_sources(name);

-- Create updated_at trigger function
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Create updated_at trigger for data_sources
CREATE TRIGGER update_data_sources_updated_at
    BEFORE UPDATE ON data_sources
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Insert default data sources
INSERT INTO data_sources (name, description, base_url, api_key_required, rate_limit_per_minute) VALUES
    ('Federal Reserve Economic Data (FRED)', 'Economic data from the Federal Reserve Bank of St. Louis', 'https://api.stlouisfed.org/fred', true, 120),
    ('Bureau of Labor Statistics (BLS)', 'Labor statistics and economic indicators from the U.S. Bureau of Labor Statistics', 'https://api.bls.gov/publicAPI/v2', true, 500),
    ('U.S. Census Bureau', 'Demographic and economic data from the U.S. Census Bureau', 'https://api.census.gov/data', true, 500),
    ('World Bank Open Data', 'Global economic and development indicators from the World Bank', 'https://api.worldbank.org/v2', false, 1000);

-- Create economic_series table
CREATE TABLE economic_series (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id VARCHAR(255) NOT NULL,
    title VARCHAR(500) NOT NULL,
    description TEXT,
    units VARCHAR(100),
    frequency VARCHAR(50) NOT NULL,
    seasonal_adjustment VARCHAR(100),
    last_updated TIMESTAMPTZ,
    start_date DATE,
    end_date DATE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique external_id per source
    UNIQUE(source_id, external_id)
);

-- Create indexes for faster queries
CREATE INDEX idx_economic_series_source_id ON economic_series(source_id);
CREATE INDEX idx_economic_series_external_id ON economic_series(external_id);
CREATE INDEX idx_economic_series_title ON economic_series USING GIN (to_tsvector('english', title));
CREATE INDEX idx_economic_series_description ON economic_series USING GIN (to_tsvector('english', description));
CREATE INDEX idx_economic_series_frequency ON economic_series(frequency);
CREATE INDEX idx_economic_series_is_active ON economic_series(is_active);
CREATE INDEX idx_economic_series_last_updated ON economic_series(last_updated);

-- Create updated_at trigger for economic_series
CREATE TRIGGER update_economic_series_updated_at
    BEFORE UPDATE ON economic_series
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create data_points table
CREATE TABLE data_points (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    series_id UUID NOT NULL REFERENCES economic_series(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    value DECIMAL(20,6),
    revision_date DATE NOT NULL,
    is_original_release BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Ensure unique combination of series, date, and revision
    UNIQUE(series_id, date, revision_date, is_original_release)
);

-- Create indexes for faster queries
CREATE INDEX idx_data_points_series_id ON data_points(series_id);
CREATE INDEX idx_data_points_date ON data_points(date);
CREATE INDEX idx_data_points_revision_date ON data_points(revision_date);
CREATE INDEX idx_data_points_is_original_release ON data_points(is_original_release);
CREATE INDEX idx_data_points_series_date ON data_points(series_id, date);
CREATE INDEX idx_data_points_series_date_revision ON data_points(series_id, date, revision_date);

-- Create updated_at trigger for data_points
CREATE TRIGGER update_data_points_updated_at
    BEFORE UPDATE ON data_points
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create index for latest revisions (performance optimization)
CREATE INDEX idx_data_points_latest_revision ON data_points(series_id, date, revision_date DESC, value);

-- Create crawl_queue table
CREATE TABLE crawl_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source VARCHAR(50) NOT NULL,
    series_id VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_for TIMESTAMPTZ,
    locked_by VARCHAR(100),
    locked_at TIMESTAMPTZ,

    -- Ensure unique combination of source and series_id for pending/processing items
    CONSTRAINT unique_active_queue_item UNIQUE(source, series_id) DEFERRABLE INITIALLY DEFERRED
);

-- Create indexes for queue processing
CREATE INDEX idx_crawl_queue_status ON crawl_queue(status);
CREATE INDEX idx_crawl_queue_priority ON crawl_queue(priority DESC);
CREATE INDEX idx_crawl_queue_scheduled_for ON crawl_queue(scheduled_for);
CREATE INDEX idx_crawl_queue_locked_by ON crawl_queue(locked_by);
CREATE INDEX idx_crawl_queue_source ON crawl_queue(source);
CREATE INDEX idx_crawl_queue_created_at ON crawl_queue(created_at);

-- Create composite index for queue processing (SKIP LOCKED optimization)
CREATE INDEX idx_crawl_queue_processing ON crawl_queue(status, priority DESC, scheduled_for, locked_by)
WHERE status IN ('pending', 'retrying');

-- Create updated_at trigger for crawl_queue
CREATE TRIGGER update_crawl_queue_updated_at
    BEFORE UPDATE ON crawl_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add constraint to validate status values
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_status
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'retrying', 'cancelled'));

-- Add constraint to validate priority range
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_priority
    CHECK (priority >= 1 AND priority <= 10);

-- Add constraint to ensure locked items have lock information
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_lock_consistency
    CHECK ((locked_by IS NULL AND locked_at IS NULL) OR (locked_by IS NOT NULL AND locked_at IS NOT NULL));
