-- A populated catalog is a durable part of observation history. Roll back configuration
-- or restore a verified empty target instead of dropping its pointers and receipts.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM iceberg_catalog) OR EXISTS (SELECT 1 FROM iceberg_commits) THEN
        RAISE EXCEPTION 'refusing to remove populated Iceberg catalog/history';
    END IF;
END $$;
DROP TABLE iceberg_commits;
DROP TABLE iceberg_catalog;
