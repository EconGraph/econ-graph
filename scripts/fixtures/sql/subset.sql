-- Cuts the sanitized data database down to the CI subset, for
-- scripts/fixtures/build-sanitized-fixture.sh --ci-subset. Runs in the
-- scratch container only, after the database has passed verification.
--
-- psql variables: series_target (series to keep in all), max_points (a
-- series with more data_points rows than this, every vintage counted, is
-- never picked).
--
-- Kept:
--   - up to ceil(series_target / sources) series from every source that has
--     any, those with data points first, picked by md5 of their id (the same
--     database gives the same pick); with all their data_points rows (every
--     vintage: revision_date, is_original_release) and fetch validators;
--   - the series_metadata rows of the kept series, and the datasets that the
--     kept series and metadata rows belong to;
--   - every row of data_sources and countries (reference tables);
--   - every row of the app tables, when they share this database
--     (APP_DATABASE=shared), and the migration bookkeeping.
-- Every other table is emptied, and large objects nothing references any
-- more are removed.

\set ON_ERROR_STOP on
BEGIN;
SET LOCAL session_replication_role = replica;

CREATE SCHEMA fixture_subset;

CREATE TABLE fixture_subset.picked AS
WITH counts AS (
    SELECT series_id, count(*) AS n FROM data_points GROUP BY series_id
), candidates AS (
    SELECT s.id, s.source_id, coalesce(c.n, 0) AS n
    FROM economic_series s LEFT JOIN counts c ON c.series_id = s.id
    WHERE coalesce(c.n, 0) <= :max_points
), per_source AS (
    SELECT ceil(:series_target::numeric / greatest(count(DISTINCT source_id), 1))::int AS k
    FROM candidates
), ranked AS (
    SELECT candidates.*,
           row_number() OVER (PARTITION BY source_id ORDER BY (n = 0), md5(id::text)) AS rn
    FROM candidates
)
SELECT ranked.id, ranked.source_id FROM ranked, per_source WHERE ranked.rn <= per_source.k;

CREATE TABLE fixture_subset.economic_series AS
    SELECT * FROM economic_series WHERE id IN (SELECT id FROM fixture_subset.picked);
CREATE TABLE fixture_subset.data_points AS
    SELECT * FROM data_points WHERE series_id IN (SELECT id FROM fixture_subset.picked);
CREATE TABLE fixture_subset.series_fetch_validators AS
    SELECT * FROM series_fetch_validators WHERE series_id IN (SELECT id FROM fixture_subset.picked);
CREATE TABLE fixture_subset.series_metadata AS
    SELECT * FROM series_metadata
    WHERE (source_id, external_id) IN (SELECT source_id, external_id FROM fixture_subset.economic_series);
CREATE TABLE fixture_subset.datasets AS
    SELECT * FROM datasets
    WHERE id IN (SELECT dataset_id FROM fixture_subset.economic_series
                 UNION SELECT dataset_id FROM fixture_subset.series_metadata);

DO $$
DECLARE
    filtered constant text[] := ARRAY['economic_series', 'data_points', 'series_fetch_validators',
                                      'series_metadata', 'datasets'];
    whole constant text[] := ARRAY['data_sources', 'countries', '__diesel_schema_migrations',
        -- the app tables (backend/migrations/app)
        'users', 'user_sessions', 'chart_annotations', 'annotation_comments', 'chart_collaborators',
        'user_data_source_preferences', 'audit_logs', 'security_events', 'financial_annotations',
        'annotation_assignments', 'annotation_replies', 'annotation_templates'];
    emptied text[];
    t text;
    cols text;
    n bigint;
    lo oid;
BEGIN
    SELECT array_agg(c.relname::text ORDER BY c.relname) INTO emptied
    FROM pg_class c JOIN pg_namespace ns ON ns.oid = c.relnamespace
    WHERE ns.nspname = 'public' AND c.relkind IN ('r', 'p')
      AND NOT c.relispartition
      AND c.relname <> ALL (filtered) AND c.relname <> ALL (whole);
    RAISE NOTICE 'subset: emptied tables: %', coalesce(array_to_string(emptied, ', '), '(none)');

    EXECUTE (SELECT 'TRUNCATE ' || string_agg(format('public.%I', x), ', ') || ' CASCADE'
             FROM unnest(filtered || coalesce(emptied, '{}')) AS x);

    FOREACH t IN ARRAY filtered LOOP
        SELECT string_agg(format('%I', a.attname), ', ' ORDER BY a.attnum) INTO cols
        FROM pg_attribute a
        WHERE a.attrelid = format('public.%I', t)::regclass
          AND a.attnum > 0 AND NOT a.attisdropped AND a.attgenerated = '';
        EXECUTE format('INSERT INTO public.%I (%s) SELECT %s FROM fixture_subset.%I', t, cols, cols, t);
        GET DIAGNOSTICS n = ROW_COUNT;
        RAISE NOTICE 'subset: public.%: % rows', t, n;
    END LOOP;

    -- Large objects (XBRL files) that no oid column still references.
    CREATE TEMP TABLE fixture_lo_refs (o oid) ON COMMIT DROP;
    FOR t, cols IN
        SELECT format('%I.%I', ns.nspname, c.relname), a.attname
        FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace ns ON ns.oid = c.relnamespace
        WHERE c.relkind = 'r' AND a.atttypid = 'oid'::regtype AND a.attnum > 0 AND NOT a.attisdropped
          AND ns.nspname NOT IN ('pg_catalog', 'information_schema') AND ns.nspname NOT LIKE 'pg\_%'
    LOOP
        EXECUTE format('INSERT INTO fixture_lo_refs SELECT %I FROM %s WHERE %I IS NOT NULL', cols, t, cols);
    END LOOP;
    n := 0;
    FOR lo IN SELECT m.oid FROM pg_largeobject_metadata m
              WHERE m.oid NOT IN (SELECT o FROM fixture_lo_refs) LOOP
        PERFORM lo_unlink(lo);
        n := n + 1;
    END LOOP;
    RAISE NOTICE 'subset: large objects removed: %', n;
END $$;

DROP SCHEMA fixture_subset CASCADE;
COMMIT;

-- One line per source: how many series and data points the subset keeps.
SELECT 'SUBSET', d.name, count(DISTINCT s.id), count(p.id)
FROM data_sources d
LEFT JOIN economic_series s ON s.source_id = d.id
LEFT JOIN data_points p ON p.series_id = s.id
GROUP BY d.name ORDER BY d.name;
