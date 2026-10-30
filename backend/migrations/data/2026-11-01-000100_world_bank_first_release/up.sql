-- Mark only the first World Bank release of each observation as its original release.
--
-- Before compare-and-skip (ECO-360), every World Bank release stored each observation as a
-- new row with is_original_release = true, so a date can hold one "original" row per
-- release, and originalOnly reads returned every one of them (or, with asOf, the newest).
-- Compare-and-skip stores a changed value as a revision (is_original_release = false), so
-- the dates it wrote already have one original row.
--
-- For each World Bank (series, date), this keeps is_original_release = true on the row with
-- the earliest revision_date (then the lowest id) and sets it to false on the later ones.
-- It deletes no row and changes no value or revision_date.
--
-- Synthetic legacy rows (revision_date = date, original, alongside an original with a real
-- vintage; the predicate of econ_graph_core::models::exclude_synthetic_legacy_rows) are left
-- out of the ranking and left as they are: their date is not a release, and reads already
-- skip them whenever a real original exists.
--
-- Checks, each raising an error with nothing changed (diesel runs the migration in one
-- transaction):
-- - exactly one data_sources row is named 'World Bank Open Data' (data_sources has no
--   source code column; the crawler finds its row by this name);
-- - no row to relabel already has a revision twin, the same (series_id, date,
--   revision_date) with is_original_release = false, which the unique key forbids;
-- - afterwards, no other source's rows changed, the row count is unchanged, and every
--   relabelled World Bank (series, date) has exactly one real original left.
--
-- Locking: the UPDATE takes row locks on the rows it relabels only, so API reads continue.
-- Run it with the crawler worker stopped (the release's pre-deploy check stops it), so no
-- World Bank fetch writes the same rows meanwhile. lock_timeout bounds the wait for any row a
-- straggling transaction holds; statement_timeout is lifted because the UPDATE's run time
-- grows with the number of legacy rows.
--
-- Forward-only: there is no down migration that could restore the old labels, and none is
-- needed; the old labels were wrong.

SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = 0;

DO $$
DECLARE
    world_bank_rows INTEGER;
    world_bank UUID;
    rows_before BIGINT;
    other_originals_before BIGINT;
    other_originals_after BIGINT;
    to_relabel BIGINT;
    twins BIGINT;
    relabelled BIGINT;
    rows_after BIGINT;
    dates_without_one_original BIGINT;
BEGIN
    SELECT COUNT(*) INTO world_bank_rows FROM data_sources WHERE name = 'World Bank Open Data';
    IF world_bank_rows <> 1 THEN
        RAISE EXCEPTION 'expected one data_sources row named World Bank Open Data, found %',
            world_bank_rows;
    END IF;
    SELECT id INTO world_bank FROM data_sources WHERE name = 'World Bank Open Data';

    SELECT COUNT(*) INTO rows_before FROM data_points;
    SELECT COUNT(*) INTO other_originals_before
    FROM data_points dp
    JOIN economic_series es ON es.id = dp.series_id
    WHERE es.source_id <> world_bank AND dp.is_original_release;

    -- The later real originals of every World Bank (series, date).
    CREATE TEMP TABLE world_bank_later_originals ON COMMIT DROP AS
    SELECT ranked.id, ranked.series_id, ranked.date, ranked.revision_date
    FROM (
        SELECT dp.id, dp.series_id, dp.date, dp.revision_date,
            ROW_NUMBER() OVER (
                PARTITION BY dp.series_id, dp.date ORDER BY dp.revision_date, dp.id
            ) AS release_number
        FROM data_points dp
        JOIN economic_series es ON es.id = dp.series_id
        WHERE es.source_id = world_bank
            AND dp.is_original_release
            AND NOT (
                dp.revision_date = dp.date
                AND EXISTS (
                    SELECT 1 FROM data_points other_original
                    WHERE other_original.series_id = dp.series_id
                        AND other_original.date = dp.date
                        AND other_original.is_original_release
                        AND other_original.revision_date <> dp.date
                )
            )
    ) ranked
    WHERE ranked.release_number > 1;
    SELECT COUNT(*) INTO to_relabel FROM world_bank_later_originals;

    SELECT COUNT(*) INTO twins
    FROM world_bank_later_originals later
    WHERE EXISTS (
        SELECT 1 FROM data_points revision
        WHERE revision.series_id = later.series_id
            AND revision.date = later.date
            AND revision.revision_date = later.revision_date
            AND NOT revision.is_original_release
    );
    IF twins > 0 THEN
        RAISE EXCEPTION '% World Bank rows to relabel already have a revision with the same '
            '(series_id, date, revision_date); nothing changed', twins;
    END IF;

    UPDATE data_points dp
    SET is_original_release = FALSE
    FROM world_bank_later_originals later
    WHERE dp.id = later.id;
    GET DIAGNOSTICS relabelled = ROW_COUNT;

    SELECT COUNT(*) INTO rows_after FROM data_points;
    SELECT COUNT(*) INTO other_originals_after
    FROM data_points dp
    JOIN economic_series es ON es.id = dp.series_id
    WHERE es.source_id <> world_bank AND dp.is_original_release;
    SELECT COUNT(*) INTO dates_without_one_original
    FROM (
        SELECT COUNT(*) FILTER (
                WHERE dp.is_original_release AND dp.revision_date <> dp.date
            ) AS originals
        FROM (SELECT DISTINCT series_id, date FROM world_bank_later_originals) relabelled_date
        JOIN data_points dp
            ON dp.series_id = relabelled_date.series_id AND dp.date = relabelled_date.date
        GROUP BY relabelled_date.series_id, relabelled_date.date
    ) per_date
    WHERE per_date.originals <> 1;

    IF relabelled <> to_relabel
        OR rows_after <> rows_before
        OR other_originals_after <> other_originals_before
        OR dates_without_one_original <> 0
    THEN
        RAISE EXCEPTION 'World Bank relabel check failed: relabelled % of %, rows % -> %, '
            'other sources'' originals % -> %, relabelled dates without exactly one original %',
            relabelled, to_relabel, rows_before, rows_after,
            other_originals_before, other_originals_after, dates_without_one_original;
    END IF;

    RAISE NOTICE 'World Bank: % later releases relabelled as revisions', relabelled;
END
$$;
