-- Never discard configured targets or their transition evidence during rollback.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM timeseries_recovery_state)
       OR EXISTS (SELECT 1 FROM timeseries_recovery_transitions) THEN
        RAISE EXCEPTION 'configured recovery state is retained; use the verified store rollback procedure';
    END IF;
END $$;
DROP FUNCTION assert_timeseries_write_allowed(uuid, text);
DROP TABLE timeseries_recovery_transitions;
DROP TABLE timeseries_recovery_state;
