-- Drop the legacy global-analysis tables (ECO-898, ECO-286, ECO-127).
--
-- These eight tables backed global_analysis_service and GlobalAnalysisQuery, both deleted in
-- the same change. Nothing in production ever wrote to them, and the GraphQL query was never
-- mounted in the served schema. The world map reads data_points through cross_section_service.
-- Its `countries` code list is the reference CSV (econ-graph-core/data/countries.csv), not
-- this table. The only rows are the v4.0 baseline seeds: 20 countries and 5
-- global_economic_events with made-up impact scores.
--
-- Guard: the migration refuses to drop anything that is not an unchanged baseline seed row (every
-- seeded column compared, duplicates counted), so it can never discard crawled, hand-entered or
-- edited data. If it refuses, nothing has changed; follow the HINT.
--
-- Rollback: down.sql recreates the eight tables, empty, with their indexes, triggers and
-- comments as they stood after 2026-10-25-000100. It does not restore the seed rows. They were
-- invented placeholders, so nothing of value is lost. No code reads these tables after this
-- change, so to bring the feature back you must also revert the code (the ECO-898
-- global-analysis PR).
--
-- Locking: nothing should read or write these tables, but the migration still locks all eight
-- in ACCESS EXCLUSIVE mode before the guard, so no row can be added between the guard's check
-- and the DROPs. The lock step is bounded like 2026-10-25-000100's: lock_timeout caps each lock
-- and statement_timeout caps the whole LOCK TABLE at about 10 seconds. If another session holds
-- a table, the migration fails with nothing changed and crawler-migrate can simply be rerun.
-- Once the locks are held, statement_timeout is set to 0 (no limit) for the rest.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '10s';

LOCK TABLE countries, global_economic_indicators, global_indicator_data, country_correlations,
    trade_relationships, global_economic_events, event_country_impacts, leading_indicators
    IN ACCESS EXCLUSIVE MODE;

SET LOCAL statement_timeout = 0;

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

    -- Every countries row must be one of the 20 baseline seed rows, unchanged in every column
    -- the seed sets or leaves at its default. EXCEPT ALL also catches a duplicate.
    SELECT count(*) INTO n FROM (
        SELECT iso_code, iso_code_2, name, region, sub_region, income_group, population, gdp_usd,
               gdp_per_capita_usd, latitude, longitude, currency_code, is_active
        FROM countries
        EXCEPT ALL
        SELECT v.iso_code, v.iso_code_2, v.name, v.region, v.sub_region, v.income_group,
               NULL::bigint, NULL::numeric, NULL::numeric, v.latitude, v.longitude, v.currency_code, true
        FROM (VALUES
            ('USA', 'US', 'United States', 'Americas', 'Northern America', 'High income', 39.8283, -98.5795, 'USD'),
            ('CHN', 'CN', 'China', 'Asia', 'Eastern Asia', 'Upper middle income', 35.8617, 104.1954, 'CNY'),
            ('JPN', 'JP', 'Japan', 'Asia', 'Eastern Asia', 'High income', 36.2048, 138.2529, 'JPY'),
            ('DEU', 'DE', 'Germany', 'Europe', 'Western Europe', 'High income', 51.1657, 10.4515, 'EUR'),
            ('GBR', 'GB', 'United Kingdom', 'Europe', 'Northern Europe', 'High income', 55.3781, -3.4360, 'GBP'),
            ('FRA', 'FR', 'France', 'Europe', 'Western Europe', 'High income', 46.2276, 2.2137, 'EUR'),
            ('IND', 'IN', 'India', 'Asia', 'Southern Asia', 'Lower middle income', 20.5937, 78.9629, 'INR'),
            ('ITA', 'IT', 'Italy', 'Europe', 'Southern Europe', 'High income', 41.8719, 12.5674, 'EUR'),
            ('BRA', 'BR', 'Brazil', 'Americas', 'South America', 'Upper middle income', -14.2350, -51.9253, 'BRL'),
            ('CAN', 'CA', 'Canada', 'Americas', 'Northern America', 'High income', 56.1304, -106.3468, 'CAD'),
            ('RUS', 'RU', 'Russian Federation', 'Europe', 'Eastern Europe', 'Upper middle income', 61.5240, 105.3188, 'RUB'),
            ('KOR', 'KR', 'South Korea', 'Asia', 'Eastern Asia', 'High income', 35.9078, 127.7669, 'KRW'),
            ('ESP', 'ES', 'Spain', 'Europe', 'Southern Europe', 'High income', 40.4637, -3.7492, 'EUR'),
            ('AUS', 'AU', 'Australia', 'Oceania', 'Australia and New Zealand', 'High income', -25.2744, 133.7751, 'AUD'),
            ('MEX', 'MX', 'Mexico', 'Americas', 'Central America', 'Upper middle income', 23.6345, -102.5528, 'MXN'),
            ('IDN', 'ID', 'Indonesia', 'Asia', 'South-Eastern Asia', 'Upper middle income', -0.7893, 113.9213, 'IDR'),
            ('NLD', 'NL', 'Netherlands', 'Europe', 'Western Europe', 'High income', 52.1326, 5.2913, 'EUR'),
            ('SAU', 'SA', 'Saudi Arabia', 'Asia', 'Western Asia', 'High income', 23.8859, 45.0792, 'SAR'),
            ('TUR', 'TR', 'Turkey', 'Asia', 'Western Asia', 'Upper middle income', 38.9637, 35.2433, 'TRY'),
            ('CHE', 'CH', 'Switzerland', 'Europe', 'Western Europe', 'High income', 46.8182, 8.2275, 'CHF')
        ) AS v(iso_code, iso_code_2, name, region, sub_region, income_group, latitude, longitude, currency_code)
    ) AS not_seeded;
    IF n > 0 THEN
        RAISE EXCEPTION
            'Not dropping the global-analysis tables: countries has % row(s) that are not an unchanged v4.0 baseline seed. Nothing has changed.', n
            USING HINT = 'Find out what added or changed these rows and whether anything needs them. If nothing does, pg_dump countries, DELETE the rows that are not unchanged seeds and run crawler migrate again. If something does, stop and raise it on ECO-898.';
    END IF;

    -- Likewise every global_economic_events row must be one of the 5 baseline seeds, unchanged;
    -- the primary country is compared by its ISO code.
    SELECT count(*) INTO n FROM (
        SELECT e.name, e.description, e.event_type, e.severity, e.start_date, e.end_date,
               c.iso_code, e.affected_regions, e.economic_impact_score
        FROM global_economic_events e LEFT JOIN countries c ON c.id = e.primary_country_id
        EXCEPT ALL
        SELECT v.name, v.description, v.event_type, v.severity, v.start_date::date,
               v.end_date::date, v.iso_code, NULL::text[], v.economic_impact_score
        FROM (VALUES
            ('2008 Global Financial Crisis', 'Global financial crisis originating from US subprime mortgage crisis', 'Crisis', 'Critical', '2007-12-01', '2009-06-01', 'USA', 95.0),
            ('COVID-19 Pandemic', 'Global pandemic causing widespread economic disruption', 'Crisis', 'Critical', '2020-03-01', '2022-12-01', NULL, 98.0),
            ('European Debt Crisis', 'Sovereign debt crisis affecting eurozone countries', 'Crisis', 'High', '2010-01-01', '2012-12-01', 'DEU', 75.0),
            ('Brexit', 'United Kingdom withdrawal from European Union', 'Policy', 'Medium', '2016-06-23', '2020-12-31', 'GBR', 45.0),
            ('US-China Trade War', 'Trade dispute between United States and China', 'Policy', 'High', '2018-03-01', '2020-01-15', 'USA', 65.0)
        ) AS v(name, description, event_type, severity, start_date, end_date, iso_code, economic_impact_score)
    ) AS not_seeded;
    IF n > 0 THEN
        RAISE EXCEPTION
            'Not dropping the global-analysis tables: global_economic_events has % row(s) that are not an unchanged v4.0 baseline seed. Nothing has changed.', n
            USING HINT = 'Find out what added or changed these rows and whether anything needs them. If nothing does, pg_dump global_economic_events, DELETE the rows that are not unchanged seeds and run crawler migrate again. If something does, stop and raise it on ECO-898 (and ECO-129, which plans a curated events file).';
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
