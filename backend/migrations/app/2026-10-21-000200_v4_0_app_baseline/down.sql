-- The app baseline cannot be reverted. Reverting it would drop every user and everything users
-- created, and on a database built by the single v4.0.0 baseline this version was recorded without
-- running (econ_graph_core::database::run_migrations), so reverting it there would drop tables
-- this migration never created. To start from an empty schema, drop and recreate the database
-- instead (docs/development/MIGRATIONS.md).
DO $$
BEGIN
    RAISE EXCEPTION 'The v4.0.0 app baseline migration cannot be reverted: it would delete all user data'
        USING HINT = 'Drop and recreate the database to start from an empty schema.';
END $$;
