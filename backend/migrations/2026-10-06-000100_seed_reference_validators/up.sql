-- seed_reference_codes also stores the file's Last-Modified and body SHA-256, which
-- reference_file_cache holds since 2026-10-02-000400_fetch_validators. With them the first crawl
-- answers a file the source hasn't changed as unchanged even when the source sends no ETag
-- (If-Modified-Since, or the same body hash). Otherwise as in 2026-10-02-000250.
DROP FUNCTION seed_reference_codes(TEXT, TEXT, JSONB, TEXT, TEXT, JSONB);

CREATE FUNCTION seed_reference_codes(
    p_source TEXT,
    p_dataset TEXT,
    p_dimension JSONB,
    p_url TEXT,
    p_etag TEXT,
    p_last_modified TEXT,
    p_content_sha256 TEXT,
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

    INSERT INTO reference_file_cache (source_id, url, etag, last_modified, content_sha256)
    VALUES (v_source_id, p_url, p_etag, p_last_modified, p_content_sha256);
END
$$;
