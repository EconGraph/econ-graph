-- Drop the public.uuidv7() shim and require PostgreSQL 18.
--
-- 2025-02-02-000002 created public.uuidv7() as a fallback returning gen_random_uuid(), which is
-- UUIDv4. On PostgreSQL 18 the built-in pg_catalog.uuidv7() wins name resolution, so the shim
-- was dead code that silently produced v4 ids if called schema-qualified or with a different
-- search_path. PostgreSQL 18 is now the minimum version, so the fallback can go.

DO $$
BEGIN
    IF current_setting('server_version_num')::int < 180000 THEN
        RAISE EXCEPTION 'EconGraph requires PostgreSQL 18 or newer (found %)', version();
    END IF;
END $$;

-- A database created on an older server may have defaults bound to the shim; rebind them to the
-- built-in function before dropping it.
DO $$
DECLARE
    col RECORD;
BEGIN
    FOR col IN
        SELECT d.adrelid::regclass AS table_name, a.attname AS column_name
        FROM pg_attrdef d
        JOIN pg_depend dep ON dep.classid = 'pg_attrdef'::regclass AND dep.objid = d.oid
        JOIN pg_proc p ON dep.refclassid = 'pg_proc'::regclass AND dep.refobjid = p.oid
        JOIN pg_namespace n ON n.oid = p.pronamespace
        JOIN pg_attribute a ON a.attrelid = d.adrelid AND a.attnum = d.adnum
        WHERE p.proname = 'uuidv7' AND n.nspname = 'public'
    LOOP
        EXECUTE format('ALTER TABLE %s ALTER COLUMN %I SET DEFAULT pg_catalog.uuidv7()',
                       col.table_name, col.column_name);
    END LOOP;
END $$;

DROP FUNCTION IF EXISTS public.uuidv7();
