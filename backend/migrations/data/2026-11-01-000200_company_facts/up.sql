-- Companyfacts preserves SEC taxonomy families and raw units. It does not change
-- existing filing/parser tables, research identities, or app-owned note storage.
CREATE TABLE company_facts (
    id UUID PRIMARY KEY,
    cik VARCHAR(10) NOT NULL CHECK (cik ~ '^[0-9]{10}$' AND cik <> '0000000000'),
    taxonomy TEXT NOT NULL CHECK (taxonomy <> '' AND taxonomy = btrim(taxonomy)),
    concept TEXT NOT NULL CHECK (concept <> '' AND concept = btrim(concept)),
    period_start DATE,
    period_end DATE NOT NULL,
    unit TEXT NOT NULL CHECK (unit <> '' AND unit = btrim(unit)),
    accession TEXT NOT NULL CHECK (accession ~ '^[0-9]{10}-[0-9]{2}-[0-9]{6}$'),
    filed DATE NOT NULL,
    value NUMERIC NOT NULL CHECK (value::text NOT IN ('NaN', 'Infinity', '-Infinity')),
    form TEXT NOT NULL CHECK (form <> '' AND form = btrim(form)),
    fiscal_year INTEGER,
    fiscal_period TEXT,
    frame TEXT,
    label TEXT,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (period_start IS NULL OR period_start <= period_end),
    UNIQUE NULLS NOT DISTINCT (cik, taxonomy, concept, period_start, period_end, unit, accession)
);
CREATE INDEX company_facts_company_period ON company_facts (cik, period_end DESC);
CREATE INDEX company_facts_company_concept ON company_facts (cik, taxonomy, concept, filed DESC, accession DESC);

-- Submissions metadata is separate from the raw-parser financial_statements.
-- Placeholder statements would make the legacy crawler skip unparsed filings.
CREATE TABLE company_fact_filings (
    id UUID PRIMARY KEY,
    cik VARCHAR(10) NOT NULL CHECK (cik ~ '^[0-9]{10}$' AND cik <> '0000000000'),
    accession TEXT NOT NULL CHECK (accession ~ '^[0-9]{10}-[0-9]{2}-[0-9]{6}$'),
    filed DATE NOT NULL,
    report_date DATE,
    form TEXT NOT NULL,
    primary_document TEXT,
    is_xbrl BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cik, accession)
);
CREATE INDEX company_fact_filings_company_filed ON company_fact_filings (cik, filed DESC, accession DESC);

CREATE TABLE company_fact_load_runs (
    id UUID PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    companies_count BIGINT NOT NULL DEFAULT 0 CHECK (companies_count >= 0),
    facts_count BIGINT NOT NULL DEFAULT 0 CHECK (facts_count >= 0),
    filings_count BIGINT NOT NULL DEFAULT 0 CHECK (filings_count >= 0),
    error TEXT,
    companyfacts_sha256 TEXT,
    submissions_sha256 TEXT,
    CHECK ((status = 'running' AND completed_at IS NULL) OR
           (status IN ('succeeded', 'failed') AND completed_at IS NOT NULL)),
    CHECK (completed_at IS NULL OR completed_at >= started_at)
);
CREATE INDEX company_fact_load_runs_completed ON company_fact_load_runs (completed_at DESC) WHERE status = 'succeeded';
