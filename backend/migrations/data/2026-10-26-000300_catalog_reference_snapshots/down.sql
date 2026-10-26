-- Forward-only data migration. Keep exact catalog snapshots and their historical evidence.
DO $$ BEGIN RAISE EXCEPTION 'catalog reference snapshots migration is forward-only'; END $$;
