-- Data migrations are forward-only: publication history must be retained.
DO $$ BEGIN RAISE EXCEPTION 'series lifecycle migration is forward-only'; END $$;
