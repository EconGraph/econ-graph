-- Production data migrations are forward-only; rolling this back would lose captured metadata.
DO $$ BEGIN RAISE EXCEPTION 'observation attributes retain source provenance; use a forward migration'; END $$;
