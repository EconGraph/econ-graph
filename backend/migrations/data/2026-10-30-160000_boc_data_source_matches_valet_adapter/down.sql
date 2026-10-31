-- Restore the earlier DataSource::boc() description and rate on the Bank of Canada row.

UPDATE data_sources
SET description = 'Canadian monetary policy, inflation, GDP, employment, and financial stability data',
    rate_limit_per_minute = 1000,
    updated_at = NOW()
WHERE name = 'Bank of Canada (BoC)';
