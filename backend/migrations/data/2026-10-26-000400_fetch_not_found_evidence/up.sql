-- Catalog membership and a fetch endpoint's availability are independent evidence.
-- A successful historical fetch resets fetch misses, never catalog disappearance.
ALTER TABLE series_lifecycle
    ADD COLUMN fetch_not_found_count INTEGER NOT NULL DEFAULT 0
        CHECK (fetch_not_found_count >= 0),
    ADD COLUMN fetch_retired BOOLEAN NOT NULL DEFAULT FALSE;
COMMENT ON COLUMN series_lifecycle.fetch_retired IS
    'Retired solely by consecutive fetch NotFound results; a successful fetch may recover it. Catalog, legal and human-confirmed publication states remain authoritative.';
