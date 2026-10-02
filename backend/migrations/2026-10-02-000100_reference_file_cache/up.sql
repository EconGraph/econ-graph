-- Caches the ETag of a reference file an adapter fetches from its source (e.g. BLS's cu.item,
-- ce.industry, la.area, ln.series code lists), so a scheduled refresh can send a conditional GET
-- and skip re-parsing when the source hasn't changed it. Not source-specific: any adapter with
-- its own reference files can use this table.
CREATE TABLE reference_file_cache (
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    etag TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (source_id, url),
    CONSTRAINT reference_file_cache_url_not_blank CHECK (btrim(url) <> '')
);
