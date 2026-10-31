-- Old attempts did not distinguish staged files from upstream downloads. Treat
-- them conservatively as staged: none can prove production data freshness.
ALTER TABLE company_fact_load_runs
    ADD COLUMN source_mode TEXT NOT NULL DEFAULT 'staged'
        CHECK (source_mode IN ('downloaded', 'staged')),
    ADD COLUMN companyfacts_last_modified TIMESTAMPTZ,
    ADD COLUMN submissions_last_modified TIMESTAMPTZ,
    ADD CONSTRAINT company_fact_load_runs_source_provenance CHECK (
        (source_mode = 'staged'
            AND companyfacts_last_modified IS NULL
            AND submissions_last_modified IS NULL)
        OR
        (source_mode = 'downloaded' AND (
            status <> 'succeeded'
            OR (
                companyfacts_last_modified IS NOT NULL
                AND submissions_last_modified IS NOT NULL
                AND companyfacts_last_modified >= completed_at - INTERVAL '24 hours'
                AND submissions_last_modified >= completed_at - INTERVAL '24 hours'
                AND companyfacts_last_modified <= completed_at + INTERVAL '5 minutes'
                AND submissions_last_modified <= completed_at + INTERVAL '5 minutes'
            )
        ))
    );
