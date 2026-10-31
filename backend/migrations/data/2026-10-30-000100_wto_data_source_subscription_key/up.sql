-- The WTO data source row describes the live WTO Timeseries adapter.
--
-- The crawler creates the row from DataSource::wto() only when none exists, so a database that
-- already has one (written for the former development-only static WTO catalog) keeps that
-- template's values. Bring them in line with the live API: it requires a subscription key
-- (WTO_API_KEY), allows one call per second on its data endpoint, and is documented on the
-- WTO API portal. A database without the row is unchanged; the crawler creates it with these
-- values.
UPDATE data_sources
SET api_key_required = true,
    api_key_name = 'WTO_API_KEY',
    rate_limit_per_minute = 60,
    api_documentation_url = 'https://apiportal.wto.org/api-details#api=version1'
WHERE name = 'World Trade Organization (WTO)';
