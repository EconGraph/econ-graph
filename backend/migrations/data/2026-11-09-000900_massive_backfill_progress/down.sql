-- Never discard the record of what the backfill wrote: a purge of Massive data needs it.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM massive_backfill_runs) THEN
        RAISE EXCEPTION 'massive_backfill_runs is not empty; its commit ids locate Massive data in Iceberg';
    END IF;
END $$;
DROP TABLE massive_backfill_units;
DROP TABLE massive_backfill_runs;
