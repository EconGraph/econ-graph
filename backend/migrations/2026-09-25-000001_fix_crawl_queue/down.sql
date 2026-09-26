DROP INDEX IF EXISTS idx_crawl_queue_claim;

CREATE INDEX idx_crawl_queue_processing ON crawl_queue(status, priority DESC, scheduled_for, locked_by)
WHERE status IN ('pending', 'retrying');

DROP INDEX IF EXISTS uq_crawl_queue_active_item;

-- The old constraint is UNIQUE over all rows: keep only the newest row per (source, series_id).
DELETE FROM crawl_queue a
    USING crawl_queue b
    WHERE a.source = b.source
      AND a.series_id = b.series_id
      AND (a.created_at, a.id) < (b.created_at, b.id);

ALTER TABLE crawl_queue ADD CONSTRAINT unique_active_queue_item
    UNIQUE (source, series_id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE crawl_queue DROP CONSTRAINT IF EXISTS check_crawl_queue_kind;
ALTER TABLE crawl_queue DROP COLUMN IF EXISTS kind;
