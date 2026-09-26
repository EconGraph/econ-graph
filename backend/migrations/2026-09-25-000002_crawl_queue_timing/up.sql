-- Track when a queue item was last claimed and when it reached a terminal state,
-- so average processing time can be computed without relying on the (now cleared) lock columns.
--   started_at:  set by claim_next / claim_by_id (reset on every claim, so retries measure the last attempt)
--   finished_at: set by complete / fail (and by retry_later when retries are exhausted);
--                cleared on claim and on retry_later reschedule
ALTER TABLE crawl_queue
    ADD COLUMN started_at TIMESTAMPTZ,
    ADD COLUMN finished_at TIMESTAMPTZ;
