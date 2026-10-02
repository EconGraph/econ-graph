-- Seeds a source's reference codes into a new database, from a file recorded with the ETag it was
-- downloaded with. Called by the generated `*_seed_{source}_reference_codes` migrations (see
-- backend/crates/econ-graph-crawler/src/reference_file.rs), one call per file.
--
-- Does nothing when `reference_file_cache` already has a row for the file: this database has
-- crawled it (or been seeded with it) and the crawl keeps it current. Otherwise the codes are
-- merged into the dataset's dimension (codes it already has keep their labels; a dimension with a
-- shared `codelist` is left alone and nothing is stored), creating the
-- dataset row or the dimension if needed, and the file's ETag is stored so the first crawl only
-- downloads the file again if the source changed it. `sync_datasets` later fills in the rest of
-- the dataset from its toml file and keeps these codes.
CREATE FUNCTION seed_reference_codes(
    p_source TEXT,
    p_dataset TEXT,
    p_dimension JSONB,
    p_url TEXT,
    p_etag TEXT,
    p_codes JSONB
) RETURNS VOID
LANGUAGE plpgsql AS $$
DECLARE
    v_source_id UUID;
    v_codes JSONB := (
        SELECT COALESCE(jsonb_agg(c ORDER BY c ->> 'code'), '[]'::jsonb)
        FROM (
            SELECT DISTINCT ON (c ->> 'code') c
            FROM jsonb_array_elements(p_codes) AS c
            ORDER BY c ->> 'code'
        ) AS u(c)
    );
    v_dimension JSONB := (p_dimension - 'codes' - 'codelist') || jsonb_build_object('codes', v_codes);
BEGIN
    SELECT id INTO v_source_id FROM data_sources WHERE name = p_source;
    IF v_source_id IS NULL THEN
        RAISE EXCEPTION 'seed_reference_codes: no data source named %', p_source;
    END IF;
    IF EXISTS (
        SELECT 1 FROM reference_file_cache WHERE source_id = v_source_id AND url = p_url
    ) THEN
        RETURN;
    END IF;
    -- A dimension labelled by a shared code list takes no inline codes (it may not have both),
    -- and the crawl's merge skips it the same way, so nothing is stored for the file.
    IF EXISTS (
        SELECT 1
        FROM datasets AS ds, jsonb_array_elements(ds.dimensions) AS d
        WHERE ds.source_id = v_source_id AND ds.code = p_dataset
            AND d ->> 'name' = p_dimension ->> 'name' AND d ? 'codelist'
    ) THEN
        RETURN;
    END IF;

    INSERT INTO datasets (source_id, code, name, dimensions)
    VALUES (v_source_id, p_dataset, p_dataset, jsonb_build_array(v_dimension))
    ON CONFLICT (source_id, code) DO UPDATE SET dimensions = CASE
        WHEN EXISTS (
            SELECT 1 FROM jsonb_array_elements(datasets.dimensions) AS d
            WHERE d ->> 'name' = p_dimension ->> 'name'
        ) THEN (
            SELECT jsonb_agg(
                CASE
                    WHEN d ->> 'name' = p_dimension ->> 'name' THEN
                        d || jsonb_build_object('codes', COALESCE(d -> 'codes', '[]'::jsonb) || (
                            SELECT COALESCE(jsonb_agg(c ORDER BY c ->> 'code'), '[]'::jsonb)
                            FROM jsonb_array_elements(v_codes) AS c
                            WHERE NOT EXISTS (
                                SELECT 1
                                FROM jsonb_array_elements(COALESCE(d -> 'codes', '[]'::jsonb)) AS e
                                WHERE e ->> 'code' = c ->> 'code'
                            )
                        ))
                    ELSE d
                END
                ORDER BY ord
            )
            FROM jsonb_array_elements(datasets.dimensions) WITH ORDINALITY AS t(d, ord)
        )
        ELSE datasets.dimensions || jsonb_build_array(v_dimension)
    END;

    INSERT INTO reference_file_cache (source_id, url, etag) VALUES (v_source_id, p_url, p_etag);
END
$$;
