-- Update Census Bureau data source configuration to reflect that it doesn't require an API key
-- and should be enabled and visible by default

UPDATE data_sources
SET
    api_key_required = false,
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    rate_limit_per_minute = 500,
    crawl_frequency_hours = 24,
    updated_at = NOW()
WHERE name = 'U.S. Census Bureau';
