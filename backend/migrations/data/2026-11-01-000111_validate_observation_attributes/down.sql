-- Production data migrations are forward-only; keep the validated provenance invariant.
DO $$ BEGIN RAISE EXCEPTION 'attribute validation is forward-only; use a forward migration'; END $$;
