-- Forward-only data migration. Fetch and catalog evidence must never be conflated again.
DO $$ BEGIN RAISE EXCEPTION 'fetch NotFound evidence migration is forward-only'; END $$;
