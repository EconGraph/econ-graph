-- Remove chunked catalog discovery: drop discovery_runs and discovery_run_chunks, and restore
-- the crawl_queue kind constraint without discover_chunk and finalize_discovery.
--
-- LIMITATIONS:
-- - The restored kind constraint rejects discover_chunk and finalize_discovery rows. If any
--   exist (finished ones are kept for the worker's queue retention, 14 days by default), this
--   rollback raises an error and changes nothing. They are job records, not crawled data:
--   delete them by hand first if the rollback must proceed.
-- - Run history and any stored chunk results are dropped. A run in progress is lost; the next
--   scheduled discovery of that source starts again.
--
-- crawl_queue is locked before the check, so no such row can be inserted between the check and
-- the constraint change. lock_timeout and statement_timeout bound the wait at about 10 seconds.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '10s';

LOCK TABLE crawl_queue IN ACCESS EXCLUSIVE MODE;

SET LOCAL statement_timeout = 0;

DO $$
DECLARE
    bad bigint;
BEGIN
    SELECT count(*) INTO bad FROM crawl_queue
    WHERE kind IN ('discover_chunk', 'finalize_discovery');
    IF bad > 0 THEN
        RAISE EXCEPTION
            'crawl_queue has % discover_chunk or finalize_discovery row(s), which the previous kind constraint rejects; not rolling back', bad;
    END IF;
END
$$;

DROP INDEX idx_crawl_queue_discovery_run;

ALTER TABLE crawl_queue DROP CONSTRAINT check_crawl_queue_kind;

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing', 'refetch_series'));

DROP TABLE discovery_run_chunks;

DROP TABLE discovery_runs;
