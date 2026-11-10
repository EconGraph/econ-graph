-- Drop the legacy global-analysis tables (ECO-898, ECO-286, ECO-127).
--
-- These eight tables backed global_analysis_service and GlobalAnalysisQuery, both deleted in
-- the same change. Nothing in production ever wrote to them, and the GraphQL query was never
-- mounted in the served schema. The world map reads data_points through cross_section_service.
-- Its `countries` code list is the reference CSV (econ-graph-core/data/countries.csv), not
-- this table. The only rows are the v4.0 baseline seeds: 20 countries and 5
-- global_economic_events with made-up impact scores.
--
-- Guard: the migration refuses to drop anything that is not a baseline seed row, so it can never
-- discard crawled or hand-entered data. If it refuses, nothing has changed; follow the HINT.
--
-- Rollback: down.sql recreates the eight tables, empty, with their indexes, triggers and
-- comments as they stood after 2026-10-25-000100. It does not restore the seed rows. They were
-- invented placeholders, so nothing of value is lost. No code reads these tables after this
-- change, so to bring the feature back you must also revert the code (the ECO-898
-- global-analysis PR).
--
-- Locking: nothing reads or writes these tables. lock_timeout still bounds the wait, so a
-- stray session can't stall the crawler-migrate Job.

SET LOCAL lock_timeout = '10s';

DO $$
DECLARE
    t TEXT;
    n BIGINT;
BEGIN
    FOREACH t IN ARRAY ARRAY[
        'global_economic_indicators', 'global_indicator_data', 'country_correlations',
        'trade_relationships', 'event_country_impacts', 'leading_indicators'
    ] LOOP
        EXECUTE format('SELECT count(*) FROM %I', t) INTO n;
        IF n > 0 THEN
            RAISE EXCEPTION
                'Not dropping the global-analysis tables: % has % row(s), but the v4.0 baseline seeded none. Nothing has changed.', t, n
                USING HINT = 'Find out what wrote these rows and whether anything needs them. If nothing does, pg_dump the table, DELETE its rows and run crawler migrate again. If something does, stop and raise it on ECO-898 before this migration ships.';
        END IF;
    END LOOP;

    SELECT count(*) INTO n FROM countries
    WHERE iso_code NOT IN ('USA', 'CHN', 'JPN', 'DEU', 'GBR', 'FRA', 'IND', 'ITA', 'BRA', 'CAN',
                           'RUS', 'KOR', 'ESP', 'AUS', 'MEX', 'IDN', 'NLD', 'SAU', 'TUR', 'CHE');
    IF n > 0 THEN
        RAISE EXCEPTION
            'Not dropping the global-analysis tables: countries has % row(s) that the v4.0 baseline did not seed. Nothing has changed.', n
            USING HINT = 'Find out what wrote these rows and whether anything needs them. If nothing does, pg_dump countries, DELETE the rows that are not seeds and run crawler migrate again. If something does, stop and raise it on ECO-898.';
    END IF;

    SELECT count(*) INTO n FROM global_economic_events
    WHERE name NOT IN ('2008 Global Financial Crisis', 'COVID-19 Pandemic', 'European Debt Crisis',
                       'Brexit', 'US-China Trade War');
    IF n > 0 THEN
        RAISE EXCEPTION
            'Not dropping the global-analysis tables: global_economic_events has % row(s) that the v4.0 baseline did not seed. Nothing has changed.', n
            USING HINT = 'Find out what wrote these rows and whether anything needs them. If nothing does, pg_dump global_economic_events, DELETE the rows that are not seeds and run crawler migrate again. If something does, stop and raise it on ECO-898 (and ECO-129, which plans a curated events file).';
    END IF;
END $$;

-- Children first. No CASCADE: if something unexpected depends on one of these tables, the
-- migration fails and changes nothing, rather than silently dropping the dependent object.
DROP TABLE event_country_impacts;
DROP TABLE global_economic_events;
DROP TABLE leading_indicators;
DROP TABLE trade_relationships;
DROP TABLE country_correlations;
DROP TABLE global_indicator_data;
DROP TABLE global_economic_indicators;
DROP TABLE countries;
