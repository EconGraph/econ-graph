-- Create crawl_queue table
CREATE TABLE crawl_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source VARCHAR(50) NOT NULL,
    series_id VARCHAR(255) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    scheduled_for TIMESTAMPTZ,
    locked_by VARCHAR(100),
    locked_at TIMESTAMPTZ,

    -- Ensure unique combination of source and series_id for pending/processing items
    CONSTRAINT unique_active_queue_item UNIQUE(source, series_id) DEFERRABLE INITIALLY DEFERRED
);

-- Create indexes for queue processing
CREATE INDEX idx_crawl_queue_status ON crawl_queue(status);
CREATE INDEX idx_crawl_queue_priority ON crawl_queue(priority DESC);
CREATE INDEX idx_crawl_queue_scheduled_for ON crawl_queue(scheduled_for);
CREATE INDEX idx_crawl_queue_locked_by ON crawl_queue(locked_by);
CREATE INDEX idx_crawl_queue_source ON crawl_queue(source);
CREATE INDEX idx_crawl_queue_created_at ON crawl_queue(created_at);

-- Create composite index for queue processing (SKIP LOCKED optimization)
CREATE INDEX idx_crawl_queue_processing ON crawl_queue(status, priority DESC, scheduled_for, locked_by)
WHERE status IN ('pending', 'retrying');

-- Create updated_at trigger
CREATE TRIGGER update_crawl_queue_updated_at
    BEFORE UPDATE ON crawl_queue
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Add constraint to validate status values
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_status
    CHECK (status IN ('pending', 'processing', 'completed', 'failed', 'retrying', 'cancelled'));

-- Add constraint to validate priority range
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_priority
    CHECK (priority >= 1 AND priority <= 10);

-- Add constraint to ensure locked items have lock information
ALTER TABLE crawl_queue ADD CONSTRAINT check_crawl_queue_lock_consistency
    CHECK ((locked_by IS NULL AND locked_at IS NULL) OR (locked_by IS NOT NULL AND locked_at IS NOT NULL));
