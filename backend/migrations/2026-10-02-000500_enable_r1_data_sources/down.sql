UPDATE data_sources SET
    is_visible = false,
    is_enabled = false,
    requires_admin_approval = true,
    crawl_status = 'disabled'
WHERE name IN (
    'World Bank Open Data',
    'Bureau of Economic Analysis (BEA)'
);
