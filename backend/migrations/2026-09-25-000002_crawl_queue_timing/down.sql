ALTER TABLE crawl_queue
    DROP COLUMN IF EXISTS finished_at,
    DROP COLUMN IF EXISTS started_at;
