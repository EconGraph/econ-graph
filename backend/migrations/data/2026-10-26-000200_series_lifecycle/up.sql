-- Catalog publication state is independent of fetch health. Retained observations,
-- metadata and user references are never removed by a catalog refresh.
CREATE TABLE series_lifecycle (
    source_id UUID NOT NULL REFERENCES data_sources(id) ON DELETE CASCADE,
    external_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','discontinued','missing_from_catalog','retired','restricted','superseded')),
    since TIMESTAMPTZ,
    missing_refreshes INTEGER NOT NULL DEFAULT 0 CHECK (missing_refreshes >= 0),
    -- Successful history fetches reset fetch misses independently of catalog absence.
    fetch_not_found_count INTEGER NOT NULL DEFAULT 0 CHECK (fetch_not_found_count >= 0),
    -- Consecutive complete whole-table responses that omitted the series: weaker evidence.
    fetch_absent_count INTEGER NOT NULL DEFAULT 0 CHECK (fetch_absent_count >= 0),
    fetch_retired BOOLEAN NOT NULL DEFAULT FALSE,
    last_seen_at TIMESTAMPTZ,
    proposed_successor_id UUID,
    successor_id UUID,
    scope_fingerprint TEXT NOT NULL,
    -- Listed by the last accepted (unguarded) catalog of scope_fingerprint. Only these rows
    -- form the mass-disappearance guard's denominator.
    in_accepted_catalog BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (source_id, external_id),
    CHECK (status <> 'superseded' OR successor_id IS NOT NULL)
);
CREATE INDEX series_lifecycle_status_idx ON series_lifecycle(source_id, status);
CREATE TABLE catalog_lifecycle_scopes (
    source_id UUID PRIMARY KEY REFERENCES data_sources(id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    last_refresh_at TIMESTAMPTZ NOT NULL,
    -- The newest snapshot rejected by the mass-disappearance guard, kept so an operator can
    -- accept it after review. Cleared by any accepted snapshot.
    guarded_snapshot JSONB,
    guarded_at TIMESTAMPTZ
);
COMMENT ON COLUMN series_lifecycle.successor_id IS
    'Human-confirmed successor only; no observation histories are merged.';
COMMENT ON COLUMN series_lifecycle.fetch_retired IS
    'Retired solely by consecutive fetch NotFound (or omitted-from-response) results; a successful fetch may recover it. Catalog, legal and human-confirmed publication states remain authoritative.';

-- Exact, immutable membership snapshots. Dataset labels intentionally keep historical codes;
-- a conditional GET must not mistake those accumulated labels for the current upstream list.
CREATE TABLE catalog_reference_snapshots (
    source varchar(32) NOT NULL,
    url text NOT NULL,
    content_sha256 varchar(64) NOT NULL,
    codes jsonb NOT NULL CHECK (jsonb_typeof(codes) = 'array' AND jsonb_array_length(codes) > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source, url, content_sha256)
);
