-- Validators that let a scheduled re-crawl skip data its source hasn't changed (conditional GET
-- headers, a body hash, or a source-reported version; see HttpFetcher::get_text_if_changed).

-- reference_file_cache becomes the per-URL validator cache: besides reference files, it holds the
-- validators of a catalog resource discovery checks before re-listing every series.
ALTER TABLE reference_file_cache
    ADD COLUMN last_modified TEXT,
    ADD COLUMN content_sha256 TEXT,
    ADD COLUMN version TEXT;

-- Per-series validators, written in the same transaction as the series' points, so a validator
-- never outlives a failed write of the data it describes. No row means the next fetch is a full one.
CREATE TABLE series_fetch_validators (
    series_id UUID PRIMARY KEY REFERENCES economic_series(id) ON DELETE CASCADE,
    etag TEXT,
    last_modified TEXT,
    content_sha256 TEXT,
    version TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
