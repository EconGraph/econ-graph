-- Restore the static-catalog template's values on the WTO data source row.
UPDATE data_sources
SET api_key_required = false,
    api_key_name = NULL,
    rate_limit_per_minute = 1000,
    api_documentation_url = 'https://www.wto.org/english/res_e/statis_e/data_explorer_e.htm'
WHERE name = 'World Trade Organization (WTO)';
