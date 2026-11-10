-- Durable progress of the Massive (market data vendor) historical backfill (ECO-214).
-- A run's window and mode are fixed when it starts, so a rerun after a crash resumes the same
-- window. Each unit row commits in the same transaction as the Iceberg pointer compare-and-swap
-- that published its rows, so a unit is either loaded and recorded or neither.
-- The commit ids also list every Iceberg commit the backfill made, for a later purge of
-- Massive data when the licence ends.
CREATE TABLE massive_backfill_runs (
    id UUID PRIMARY KEY,
    window_start DATE NOT NULL,
    window_end DATE NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('ticker_range', 'grouped_daily')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    CHECK (window_start <= window_end)
);

-- At most one unfinished run.
CREATE UNIQUE INDEX massive_backfill_runs_one_open
    ON massive_backfill_runs ((true)) WHERE completed_at IS NULL;

-- One row per finished unit: 'figi:<share class FIGI>' (per-ticker mode) or 'date:<YYYY-MM-DD>'
-- (grouped-daily mode). commit_id is NULL when the unit had no rows to publish.
CREATE TABLE massive_backfill_units (
    run_id UUID NOT NULL REFERENCES massive_backfill_runs(id) ON DELETE RESTRICT,
    unit TEXT NOT NULL CHECK (unit ~ '^(figi|date):.+$'),
    rows_written INTEGER NOT NULL CHECK (rows_written >= 0),
    commit_id UUID REFERENCES iceberg_commits(commit_id) ON DELETE RESTRICT,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, unit),
    CHECK ((rows_written = 0) = (commit_id IS NULL))
);

CREATE INDEX massive_backfill_units_commit ON massive_backfill_units (commit_id);
