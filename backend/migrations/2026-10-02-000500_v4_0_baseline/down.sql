-- The baseline cannot be reverted. Reverting it would drop every table and all application data,
-- and on a database that ran the migrations it replaced, version 2026-10-02-000500 is also the
-- version of the last of them (enable_r1_data_sources), so `diesel migration revert` there would
-- look like undoing a one-line data fix. To start from an empty schema, drop and recreate the
-- database instead (docs/development/MIGRATIONS.md).
DO $$
BEGIN
    RAISE EXCEPTION 'The v4.0.0 baseline migration cannot be reverted: it would delete all application data'
        USING HINT = 'Drop and recreate the database to start from an empty schema.';
END $$;
