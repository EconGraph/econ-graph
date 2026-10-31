-- The up migration relabels later World Bank releases as revisions without recording which
-- rows it changed, and the old labels were wrong, so there is nothing to restore. Rolling it
-- back raises an error and changes nothing; to undo it, restore the pre-migration dump.

DO $$
BEGIN
    RAISE EXCEPTION '2026-11-01-000100_world_bank_first_release cannot be rolled back; '
        'restore the pre-migration dump instead';
END
$$;
