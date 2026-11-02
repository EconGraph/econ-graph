ALTER TABLE company_fact_load_runs
    DROP CONSTRAINT company_fact_load_runs_source_provenance,
    DROP COLUMN submissions_last_modified,
    DROP COLUMN companyfacts_last_modified,
    DROP COLUMN source_mode;
