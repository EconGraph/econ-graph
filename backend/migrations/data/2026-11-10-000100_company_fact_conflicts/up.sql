-- ECO-890: the nightly companyfacts load writes only new or changed evidence. A source
-- value that differs from a stored one is quarantined in company_fact_conflicts and the
-- stored row stays unchanged; an operator may accept it as a new revision. Stored facts
-- are never replaced (docs/adr/0001-sec-companyfacts-identity.md).

-- Label and description are taxonomy metadata, not per-fact evidence.
CREATE TABLE company_fact_concepts (
    taxonomy TEXT NOT NULL CHECK (taxonomy <> '' AND taxonomy = btrim(taxonomy)),
    concept TEXT NOT NULL CHECK (concept <> '' AND concept = btrim(concept)),
    label TEXT,
    description TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (taxonomy, concept)
);
INSERT INTO company_fact_concepts (taxonomy, concept, label, description)
SELECT DISTINCT ON (taxonomy, concept) taxonomy, concept, label, description
FROM company_facts
ORDER BY taxonomy, concept, (label IS NULL), filed DESC, accession DESC;
ALTER TABLE company_facts DROP COLUMN label, DROP COLUMN description;

-- Run outcome: conflicts no longer fail the run, archive staleness is recorded rather than
-- fatal, and rows written show whether a run rewrote unchanged evidence.
DO $$
DECLARE c record;
BEGIN
    FOR c IN SELECT conname FROM pg_constraint
             WHERE conrelid = 'company_fact_load_runs'::regclass AND contype = 'c'
               AND conname <> 'company_fact_load_runs_source_provenance'
               AND pg_get_constraintdef(oid) LIKE '%succeeded%'
    LOOP
        EXECUTE format('ALTER TABLE company_fact_load_runs DROP CONSTRAINT %I', c.conname);
    END LOOP;
END $$;
ALTER TABLE company_fact_load_runs
    DROP CONSTRAINT company_fact_load_runs_source_provenance,
    ADD COLUMN conflicts_count BIGINT NOT NULL DEFAULT 0 CHECK (conflicts_count >= 0),
    ADD COLUMN fact_rows_written BIGINT NOT NULL DEFAULT 0 CHECK (fact_rows_written >= 0),
    ADD COLUMN filing_rows_written BIGINT NOT NULL DEFAULT 0 CHECK (filing_rows_written >= 0),
    ADD COLUMN companies_skipped BIGINT NOT NULL DEFAULT 0 CHECK (companies_skipped >= 0),
    ADD COLUMN archive_stale BOOLEAN NOT NULL DEFAULT false,
    ADD CONSTRAINT company_fact_load_runs_status CHECK (
        status IN ('running', 'succeeded', 'succeeded_with_conflicts', 'failed')),
    ADD CONSTRAINT company_fact_load_runs_completion CHECK (
        (status = 'running' AND completed_at IS NULL) OR
        (status <> 'running' AND completed_at IS NOT NULL)),
    ADD CONSTRAINT company_fact_load_runs_conflicts CHECK (
        (status = 'succeeded_with_conflicts') = (status <> 'failed' AND status <> 'running' AND conflicts_count > 0)),
    -- A successful download keeps both receipts. archive_stale states whether either was
    -- outside 24 hours before to 5 minutes after completion; staged imports are never stale.
    ADD CONSTRAINT company_fact_load_runs_source_provenance CHECK (
        (source_mode = 'staged'
            AND companyfacts_last_modified IS NULL
            AND submissions_last_modified IS NULL
            AND NOT archive_stale)
        OR
        (source_mode = 'downloaded' AND (
            status NOT IN ('succeeded', 'succeeded_with_conflicts')
            OR (
                companyfacts_last_modified IS NOT NULL
                AND submissions_last_modified IS NOT NULL
                AND archive_stale = NOT (
                    companyfacts_last_modified >= completed_at - INTERVAL '24 hours'
                    AND submissions_last_modified >= completed_at - INTERVAL '24 hours'
                    AND companyfacts_last_modified <= completed_at + INTERVAL '5 minutes'
                    AND submissions_last_modified <= completed_at + INTERVAL '5 minutes')
            )
        ))
    );
DROP INDEX company_fact_load_runs_completed;
CREATE INDEX company_fact_load_runs_completed ON company_fact_load_runs (completed_at DESC)
    WHERE status IN ('succeeded', 'succeeded_with_conflicts');

-- A source row that differs from the stored evidence (kind fact/filing), or two source rows
-- for one revision key that disagree within one archive (*_source_duplicate). One row per
-- distinct proposal; later runs that see it again update last_seen_* only.
CREATE TABLE company_fact_conflicts (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('fact', 'filing', 'fact_source_duplicate', 'filing_source_duplicate')),
    cik VARCHAR(10) NOT NULL CHECK (cik ~ '^[0-9]{10}$' AND cik <> '0000000000'),
    accession TEXT NOT NULL CHECK (accession ~ '^[0-9]{10}-[0-9]{2}-[0-9]{6}$'),
    -- company_facts.id for fact kinds, the accession for filing kinds.
    target_key TEXT NOT NULL,
    stored JSONB NOT NULL,
    proposed JSONB NOT NULL,
    proposed_digest TEXT NOT NULL CHECK (proposed_digest ~ '^[0-9a-f]{32}$'),
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'rejected')),
    first_seen_run_id UUID NOT NULL REFERENCES company_fact_load_runs (id),
    last_seen_run_id UUID NOT NULL REFERENCES company_fact_load_runs (id),
    seen_count BIGINT NOT NULL DEFAULT 1 CHECK (seen_count >= 1),
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    resolved_by TEXT CHECK (resolved_by <> ''),
    resolution_note TEXT,
    CHECK ((status = 'open') = (resolved_at IS NULL)),
    CHECK ((status = 'open') = (resolved_by IS NULL)),
    CHECK (status <> 'accepted' OR kind IN ('fact', 'filing')),
    UNIQUE (kind, cik, target_key, proposed_digest)
);
CREATE INDEX company_fact_conflicts_open ON company_fact_conflicts (cik, last_seen_at DESC) WHERE status = 'open';

-- Operator-accepted source corrections. The original company_facts / company_fact_filings
-- row is revision 1 and is never updated; each accepted correction adds revision n+1.
CREATE TABLE company_fact_revisions (
    id UUID PRIMARY KEY,
    fact_id UUID NOT NULL REFERENCES company_facts (id),
    revision INTEGER NOT NULL CHECK (revision >= 2),
    value NUMERIC NOT NULL CHECK (value::text NOT IN ('NaN', 'Infinity', '-Infinity')),
    filed DATE NOT NULL,
    form TEXT NOT NULL CHECK (form <> '' AND form = btrim(form)),
    conflict_id UUID NOT NULL UNIQUE REFERENCES company_fact_conflicts (id),
    accepted_by TEXT NOT NULL CHECK (accepted_by <> ''),
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    note TEXT,
    UNIQUE (fact_id, revision)
);
CREATE TABLE company_fact_filing_revisions (
    id UUID PRIMARY KEY,
    filing_id UUID NOT NULL REFERENCES company_fact_filings (id),
    revision INTEGER NOT NULL CHECK (revision >= 2),
    filed DATE NOT NULL,
    report_date DATE,
    form TEXT NOT NULL,
    conflict_id UUID NOT NULL UNIQUE REFERENCES company_fact_conflicts (id),
    accepted_by TEXT NOT NULL CHECK (accepted_by <> ''),
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    note TEXT,
    UNIQUE (filing_id, revision)
);

-- Digest of one company's archive entries (companyfacts, submissions and its history files)
-- when it last loaded with no open conflicts. A later run skips the company while the digest
-- matches and the recording run succeeded; the counts keep run totals for skipped companies.
CREATE TABLE company_fact_company_digests (
    cik VARCHAR(10) PRIMARY KEY CHECK (cik ~ '^[0-9]{10}$' AND cik <> '0000000000'),
    entry_sha256 TEXT NOT NULL CHECK (entry_sha256 ~ '^[0-9a-f]{64}$'),
    facts_count BIGINT NOT NULL CHECK (facts_count >= 0),
    filings_count BIGINT NOT NULL CHECK (filings_count >= 0),
    run_id UUID NOT NULL REFERENCES company_fact_load_runs (id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
