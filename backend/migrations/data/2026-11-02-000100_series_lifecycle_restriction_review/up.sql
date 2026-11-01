-- An operator's review of a `restricted` series (`crawler clear-restriction`). The series stays
-- restricted, and hidden, until a fetch of a source that checks restriction markers, claimed after
-- this time, finds none; any restriction evidence first cancels the review.
ALTER TABLE series_lifecycle
    ADD COLUMN restriction_cleared_at TIMESTAMPTZ,
    ADD COLUMN restriction_clear_reason TEXT,
    ADD CONSTRAINT series_lifecycle_restriction_review_check CHECK (
        (restriction_cleared_at IS NULL) = (restriction_clear_reason IS NULL)
        AND (restriction_cleared_at IS NULL OR status = 'restricted'));

-- Durable record of restriction reviews: requested, lifted by a fetch, or cancelled by evidence.
CREATE TABLE series_lifecycle_events (
    id BIGSERIAL PRIMARY KEY,
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event TEXT NOT NULL CHECK (event IN (
        'restriction_clearance_requested', 'restriction_lifted', 'restriction_clearance_cancelled')),
    detail TEXT NOT NULL
);
CREATE INDEX series_lifecycle_events_series_idx ON series_lifecycle_events(source_id, external_id, at);
