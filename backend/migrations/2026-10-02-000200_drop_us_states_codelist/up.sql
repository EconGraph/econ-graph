-- The `us_states` code list is gone: the Census adapter now fetches the Census Bureau's state
-- file and stores the state names as the `bds` dataset's own `state` codes (ECO-336). Drop the
-- code-list name from stored dataset dimensions, so the API doesn't report an unknown code list
-- before the worker's next dataset sync rewrites the row.
UPDATE datasets
SET dimensions = (
    SELECT jsonb_agg(
        CASE WHEN d ->> 'codelist' = 'us_states' THEN d - 'codelist' ELSE d END
        ORDER BY ord
    )
    FROM jsonb_array_elements(dimensions) WITH ORDINALITY AS t(d, ord)
)
WHERE dimensions @> '[{"codelist": "us_states"}]';
