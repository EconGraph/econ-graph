-- Data migrations are forward-only: the restriction review record must be retained.
DO $$ BEGIN RAISE EXCEPTION 'series lifecycle restriction review migration is forward-only'; END $$;
