-- Puts the `us_states` code list back on the `bds` dataset's `state` dimension where it has no
-- codes of its own (a dimension may not have both).
UPDATE datasets
SET dimensions = COALESCE((
    SELECT jsonb_agg(
        CASE
            WHEN d ->> 'name' = 'state' AND NOT d ? 'codes'
                THEN d || '{"codelist": "us_states"}'::jsonb
            ELSE d
        END
        ORDER BY ord
    )
    FROM jsonb_array_elements(dimensions) WITH ORDINALITY AS t(d, ord)
), '[]'::jsonb)
WHERE code = 'bds';
