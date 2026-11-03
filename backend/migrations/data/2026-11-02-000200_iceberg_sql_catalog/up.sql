-- The catalog lives in the data database. Immutable objects are staged before this pointer
-- moves, in the same caller-owned transaction as summaries, attempts and queue completion.
CREATE TABLE iceberg_catalog (
    dataset_id UUID PRIMARY KEY REFERENCES datasets(id) ON DELETE RESTRICT,
    metadata_location TEXT NOT NULL,
    previous_metadata_location TEXT,
    generation BIGINT NOT NULL DEFAULT 0 CHECK (generation >= 0),
    commit_id UUID NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE iceberg_commits (
    commit_id UUID PRIMARY KEY,
    dataset_id UUID NOT NULL REFERENCES datasets(id) ON DELETE RESTRICT,
    metadata_location TEXT NOT NULL,
    parent_metadata_location TEXT,
    generation BIGINT NOT NULL,
    content_sha256 TEXT,
    committed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(dataset_id, generation)
);
