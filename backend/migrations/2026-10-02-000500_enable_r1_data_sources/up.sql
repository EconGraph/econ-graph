-- The consolidated initial migration seeded World Bank and BEA as is_enabled = false /
-- requires_admin_approval = true, pending an admin-approval flow that never shipped in release 1
-- (the admin frontend is out of scope). Census was seeded disabled the same way but the same
-- migration re-enables it further down (see "Update Census Bureau data source configuration");
-- World Bank and BEA never got that treatment.
--
-- Both ship in the release-1 crawler build (FRED, BLS, CENSUS, BEA, WORLD_BANK, FHFA), so a fresh
-- v4.0 install never crawled them: the scheduler's due/discovery queries and the coverage report
-- skip any source whose data_sources row has is_enabled = false.
--
-- Enable them the same way Census, FRED and BLS already are. This matches the
-- DataSource::world_bank() / bea() templates in econ-graph-core, which the crawler falls back to
-- only when no data_sources row already exists for that name -- a row the baseline migration
-- seeded is never revisited by that fallback, so the flags have to be corrected here too.
UPDATE data_sources SET
    is_visible = true,
    is_enabled = true,
    requires_admin_approval = false,
    crawl_status = 'pending'
WHERE name IN (
    'World Bank Open Data',
    'Bureau of Economic Analysis (BEA)'
);
