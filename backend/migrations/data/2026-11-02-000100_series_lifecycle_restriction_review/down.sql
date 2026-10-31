DROP TABLE series_lifecycle_events;
ALTER TABLE series_lifecycle
    DROP CONSTRAINT series_lifecycle_restriction_review_check,
    DROP COLUMN restriction_clear_reason,
    DROP COLUMN restriction_cleared_at;
