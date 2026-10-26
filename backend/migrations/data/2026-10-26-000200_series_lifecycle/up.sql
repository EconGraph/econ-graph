-- Catalog publication state is independent of fetch health. Retained observations,
-- metadata and user references are never removed by a catalog refresh.
CREATE TABLE series_lifecycle (
    source_id UUID NOT NULL REFERENCES data_sources(id),
    external_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','discontinued','missing_from_catalog','retired','restricted','superseded')),
    since TIMESTAMPTZ,
    missing_refreshes INTEGER NOT NULL DEFAULT 0 CHECK (missing_refreshes >= 0),
    last_seen_at TIMESTAMPTZ,
    proposed_successor_id UUID,
    successor_id UUID,
    scope_fingerprint TEXT NOT NULL,
    PRIMARY KEY (source_id, external_id),
    CHECK (status <> 'superseded' OR successor_id IS NOT NULL)
);
CREATE INDEX series_lifecycle_status_idx ON series_lifecycle(source_id, status);
CREATE TABLE catalog_lifecycle_scopes (
    source_id UUID PRIMARY KEY REFERENCES data_sources(id),
    fingerprint TEXT NOT NULL,
    last_refresh_at TIMESTAMPTZ NOT NULL
);
COMMENT ON COLUMN series_lifecycle.successor_id IS
    'Human-confirmed successor only; no observation histories are merged.';
