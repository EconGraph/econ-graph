-- Store observation values as unconstrained NUMERIC.
--
-- The v4.0 baseline declared data_points.value and global_indicator_data.value as
-- DECIMAL(20,6): at most 14 integer digits, and every input rounded to 6 fractional
-- digits. Valid World Bank aggregates (e.g. NY.GDP.MKTP.PP.CD for WLD, about 1.9e14)
-- overflow it, and sources publishing more than 6 fractional digits were rounded.
-- Unconstrained NUMERIC stores each value exactly as given.
--
-- Widening numeric(p,s) to unconstrained numeric is binary-compatible: PostgreSQL only
-- updates the catalog (no table rewrite, no index rebuild). ALTER COLUMN TYPE still takes
-- an ACCESS EXCLUSIVE lock on each table until the migration commits, so the statement
-- first waits for in-flight transactions on these tables, and readers and writers queue
-- behind it while it waits.
--
-- Values already stored keep the digits they were rounded to. The crawler's
-- refetch_series job kind, allowed below, re-fetches a series' full history and rewrites
-- them from what the source still serves (docs/technical/OBSERVATION_PRECISION_REFETCH.md).
-- Replacing the crawl_queue kind constraint scans crawl_queue (finished rows are purged
-- after 14 days, so it stays small) under the same ACCESS EXCLUSIVE lock.

ALTER TABLE data_points ALTER COLUMN value TYPE NUMERIC;

ALTER TABLE global_indicator_data ALTER COLUMN value TYPE NUMERIC;

COMMENT ON COLUMN data_points.value IS
    'Observation value, exactly as the source published it (unconstrained NUMERIC)';

COMMENT ON COLUMN global_indicator_data.value IS
    'Observation value, exactly as the source published it (unconstrained NUMERIC)';

ALTER TABLE crawl_queue DROP CONSTRAINT check_crawl_queue_kind;

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing', 'refetch_series'));
