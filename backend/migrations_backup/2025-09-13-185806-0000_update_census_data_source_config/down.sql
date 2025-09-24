-- Revert Census Bureau data source configuration changes
-- Note: This reverts to the original configuration which may not be accurate

UPDATE data_sources
SET
    api_key_required = true,
    is_visible = false,
    is_enabled = false,
    requires_admin_approval = true,
    rate_limit_per_minute = 100,
    crawl_frequency_hours = 12,
    updated_at = NOW()
WHERE name = 'U.S. Census Bureau';
