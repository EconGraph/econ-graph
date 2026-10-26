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
