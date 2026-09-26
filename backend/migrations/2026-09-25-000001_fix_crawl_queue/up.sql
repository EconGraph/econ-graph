-- Make crawl_queue a correct shared job queue.

-- 1. Job kind: one queue carries several kinds of work for the same (source, series_id).
ALTER TABLE crawl_queue
    ADD COLUMN kind VARCHAR(30) NOT NULL DEFAULT 'fetch_series';

ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_kind
    CHECK (kind IN ('fetch_series', 'discover_catalog', 'fetch_filing'));

-- 2. The old UNIQUE(source, series_id) covered ALL rows, so a series could only ever be enqueued once.
--    Uniqueness now applies only to active rows, per kind. A non-deferrable partial unique index also
--    lets INSERT ... ON CONFLICT DO NOTHING work (a DEFERRABLE constraint cannot be an arbiter).
ALTER TABLE crawl_queue DROP CONSTRAINT IF EXISTS unique_active_queue_item;

CREATE UNIQUE INDEX uq_crawl_queue_active_item
    ON crawl_queue (source, series_id, kind)
    WHERE status IN ('pending', 'processing', 'retrying');

-- 3. Claim index matching the claim query:
--      WHERE status IN ('pending','retrying') AND (scheduled_for IS NULL OR scheduled_for <= NOW())
--      ORDER BY priority DESC, created_at ASC  ... FOR UPDATE SKIP LOCKED LIMIT 1
DROP INDEX IF EXISTS idx_crawl_queue_processing;

CREATE INDEX idx_crawl_queue_claim
    ON crawl_queue (priority DESC, created_at ASC, scheduled_for)
    WHERE status IN ('pending', 'retrying');
