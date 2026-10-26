-- Restore the v4.0 baseline: DECIMAL(20,6) observation values.
--
-- LIMITATIONS:
-- - Narrowing refuses to lose data. If any stored value needs more than 14 integer digits
--   or more than 6 fractional digits (anything stored since the up migration, such as World
--   Bank aggregates near 1e14 or exact source decimals), this rollback raises an error
--   naming the table and count, and changes nothing (diesel runs it in one transaction).
--   It does not delete or round those values. To roll back anyway, decide what to do with
--   them by hand first, for example back them up and then round or remove them.
-- - The baseline's crawl_queue kind constraint has no 'refetch_series'. If any
--   refetch_series queue row exists (finished ones are kept for 14 days), this rollback
--   raises an error and changes nothing. Those rows are job records, not observations:
--   cancel or delete them by hand first if the rollback must proceed.
-- - Unlike the up migration, narrowing rewrites both tables and rebuilds their indexes
--   under an ACCESS EXCLUSIVE lock, so it takes time proportional to the row count.
--
-- The tables are locked before the checks, in the crawler worker's order (crawl_queue, then
-- data_points), so no value can be written between the checks and the narrowing (where the
-- cast would round it), and the rollback waits for a worker instead of deadlocking with it.
-- lock_timeout (per lock) and statement_timeout (for the whole LOCK statement) make it fail
-- after about 10 seconds rather than stall reads while long transactions hold the tables;
-- statement_timeout is reset once the locks are held, so the checks and the rewrite aren't
-- cut short.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '10s';

LOCK TABLE crawl_queue, data_points, global_indicator_data IN ACCESS EXCLUSIVE MODE;

RESET statement_timeout;

DO $$
DECLARE
    bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM crawl_queue WHERE kind = 'refetch_series';
    IF bad > 0 THEN
        RAISE EXCEPTION
            'crawl_queue has % refetch_series row(s), which the baseline kind constraint rejects; not rolling back', bad;
    END IF;

    SELECT count(*) INTO bad FROM data_points
    WHERE value IS NOT NULL AND (value <> round(value, 6) OR abs(value) >= 1e14);
    IF bad > 0 THEN
        RAISE EXCEPTION
            'data_points has % value(s) that DECIMAL(20,6) cannot hold exactly; not narrowing', bad;
    END IF;

    SELECT count(*) INTO bad FROM global_indicator_data
    WHERE value IS NOT NULL AND (value <> round(value, 6) OR abs(value) >= 1e14);
    IF bad > 0 THEN
        RAISE EXCEPTION
            'global_indicator_data has % value(s) that DECIMAL(20,6) cannot hold exactly; not narrowing', bad;
    END IF;
END
$$;

ALTER TABLE crawl_queue DROP CONSTRAINT check_crawl_queue_kind;

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing'));

ALTER TABLE data_points ALTER COLUMN value TYPE DECIMAL(20,6);

ALTER TABLE global_indicator_data ALTER COLUMN value TYPE DECIMAL(20,6);

COMMENT ON COLUMN data_points.value IS NULL;

COMMENT ON COLUMN global_indicator_data.value IS NULL;
