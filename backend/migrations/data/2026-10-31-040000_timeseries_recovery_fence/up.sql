-- A durable pause/control plane for reversible observation-store transitions. No store is
-- selected by applying this migration: an empty state retains legacy PostgreSQL behavior.
CREATE TABLE timeseries_recovery_state (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    target_id uuid NOT NULL UNIQUE,
    target_label text NOT NULL CHECK (btrim(target_label) <> ''),
    backend text NOT NULL DEFAULT 'postgres' CHECK (backend IN ('postgres', 'iceberg')),
    epoch bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0),
    writes_paused boolean NOT NULL DEFAULT false,
    pause_token uuid,
    checkpoint jsonb,
    updated_at timestamptz NOT NULL DEFAULT now(),
    CHECK (writes_paused = (pause_token IS NOT NULL))
);
CREATE TABLE timeseries_recovery_transitions (
    id uuid PRIMARY KEY DEFAULT uuidv7(),
    target_id uuid NOT NULL REFERENCES timeseries_recovery_state(target_id),
    epoch bigint NOT NULL,
    action text NOT NULL CHECK (action IN ('initialize', 'pause', 'resume', 'cutover', 'rollback')),
    evidence jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (target_id, epoch)
);

-- Every crawl transaction calls this before its writes. Recovery operations take the
-- exclusive form of this same advisory lock; pause therefore waits for guarded writers
-- to commit and subsequent writers see the durable pause rather than racing a read flag.
CREATE FUNCTION assert_timeseries_write_allowed(expected_target uuid, expected_backend text)
RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE current_state timeseries_recovery_state%ROWTYPE;
BEGIN
    -- Advisory locks serialize commits but do not refresh an already established RR or
    -- serializable snapshot. Require RC so the SELECT after waiting observes the pause.
    IF current_setting('transaction_isolation') <> 'read committed' THEN
        RAISE EXCEPTION 'observation write guard requires READ COMMITTED isolation' USING ERRCODE = '25001';
    END IF;
    IF expected_backend NOT IN ('postgres', 'iceberg') OR expected_backend IS NULL THEN
        RAISE EXCEPTION 'invalid observation backend' USING ERRCODE = '22023';
    END IF;
    PERFORM pg_advisory_xact_lock_shared(1162303316);
    SELECT * INTO current_state FROM timeseries_recovery_state WHERE singleton;
    IF NOT FOUND THEN
        IF expected_target IS NULL AND expected_backend = 'postgres' THEN RETURN 0; END IF;
        RAISE EXCEPTION 'observation recovery target is not initialized' USING ERRCODE = '55000';
    END IF;
    IF expected_target IS DISTINCT FROM current_state.target_id THEN
        RAISE EXCEPTION 'observation write target does not match recovery state' USING ERRCODE = '22023';
    END IF;
    IF expected_backend <> current_state.backend THEN
        RAISE EXCEPTION 'observation write backend does not match recovery state' USING ERRCODE = '55000';
    END IF;
    IF current_state.writes_paused THEN
        RAISE EXCEPTION 'observation writes are paused for recovery' USING ERRCODE = '55P03';
    END IF;
    RETURN current_state.epoch;
END $$;
