-- Never discard quarantined source values, accepted corrections or run outcomes the old
-- schema cannot represent. Digests are derived and labels move back onto the facts.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM company_fact_conflicts)
       OR EXISTS (SELECT 1 FROM company_fact_revisions)
       OR EXISTS (SELECT 1 FROM company_fact_filing_revisions)
       OR EXISTS (SELECT 1 FROM company_fact_load_runs
                  WHERE archive_stale OR status = 'succeeded_with_conflicts') THEN
        RAISE EXCEPTION 'companyfacts conflicts, corrections or stale-archive runs are retained; resolve them before rolling back';
    END IF;
END $$;
DROP TABLE company_fact_company_digests;
DROP TABLE company_fact_filing_revisions;
DROP TABLE company_fact_revisions;
DROP TABLE company_fact_conflicts;

DROP INDEX company_fact_load_runs_completed;
CREATE INDEX company_fact_load_runs_completed ON company_fact_load_runs (completed_at DESC) WHERE status = 'succeeded';
ALTER TABLE company_fact_load_runs
    DROP CONSTRAINT company_fact_load_runs_source_provenance,
    DROP CONSTRAINT company_fact_load_runs_conflicts,
    DROP CONSTRAINT company_fact_load_runs_completion,
    DROP CONSTRAINT company_fact_load_runs_status,
    DROP COLUMN archive_stale,
    DROP COLUMN companies_skipped,
    DROP COLUMN filing_rows_written,
    DROP COLUMN fact_rows_written,
    DROP COLUMN conflicts_count,
    ADD CONSTRAINT company_fact_load_runs_status_check CHECK (status IN ('running', 'succeeded', 'failed')),
    ADD CONSTRAINT company_fact_load_runs_check CHECK (
        (status = 'running' AND completed_at IS NULL) OR
        (status IN ('succeeded', 'failed') AND completed_at IS NOT NULL)),
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

ALTER TABLE company_facts ADD COLUMN label TEXT, ADD COLUMN description TEXT;
UPDATE company_facts f SET label = c.label, description = c.description
FROM company_fact_concepts c
WHERE c.taxonomy = f.taxonomy AND c.concept = f.concept
  AND (c.label IS NOT NULL OR c.description IS NOT NULL);
DROP TABLE company_fact_concepts;
