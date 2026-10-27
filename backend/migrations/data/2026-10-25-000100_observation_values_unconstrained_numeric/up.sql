-- Store observation values as unconstrained NUMERIC.
--
-- The v4.0 baseline declared data_points.value and global_indicator_data.value as
-- DECIMAL(20,6): at most 14 integer digits, and every input rounded to 6 fractional
-- digits. Valid World Bank aggregates (e.g. NY.GDP.MKTP.PP.CD for WLD, about 1.9e14)
-- overflow it, and sources publishing more than 6 fractional digits were rounded.
-- Unconstrained NUMERIC stores each value exactly as given.
--
-- Widening numeric(p,s) to unconstrained numeric is binary-compatible: PostgreSQL does not
-- rewrite either table, and keeps data_points' indexes (idx_data_points_latest_revision
-- includes value). It does rebuild global_indicator_data's partial index
-- idx_global_data_date_value, whose predicate (value IS NOT NULL) reads the column.
--
-- Values already stored keep the digits they were rounded to. The crawler's
-- refetch_series job kind, allowed below, re-fetches a series' full history and rewrites
-- them from what the source still serves (docs/technical/OBSERVATION_PRECISION_REFETCH.md).
--
-- Locking: every statement here takes ACCESS EXCLUSIVE on its table until the migration
-- commits. The tables are locked up front in the order the crawler worker uses (its result
-- transaction locks its crawl_queue row, then writes data_points), so the migration waits
-- for a worker instead of deadlocking with it. While the migration waits for its locks,
-- reads of these tables queue behind it, so the whole lock step is bounded: LOCK TABLE takes
-- the three locks one after another, each under lock_timeout, and statement_timeout caps
-- the statement as a whole. If long transactions hold the tables, the migration fails after
-- about 10 seconds with nothing changed (and the backend retries it on its next start)
-- rather than stalling the API. Once the locks are held, statement_timeout is set to 0 (no
-- limit) for the rest of the migration, not reset to a role or database default.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '10s';

LOCK TABLE crawl_queue, data_points, global_indicator_data IN ACCESS EXCLUSIVE MODE;

SET LOCAL statement_timeout = 0;

ALTER TABLE crawl_queue DROP CONSTRAINT check_crawl_queue_kind;

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing', 'refetch_series'));

ALTER TABLE data_points ALTER COLUMN value TYPE NUMERIC;

ALTER TABLE global_indicator_data ALTER COLUMN value TYPE NUMERIC;

COMMENT ON COLUMN data_points.value IS
    'Observation value, exactly as the source published it (unconstrained NUMERIC)';

COMMENT ON COLUMN global_indicator_data.value IS
    'Observation value, exactly as the source published it (unconstrained NUMERIC)';
