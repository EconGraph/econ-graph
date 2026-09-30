-- Stable series ids (see econ-graph-crawler/src/series_id.rs): a series' id is UUIDv5 of
-- "{SOURCE}:{external_id}" in a fixed namespace, so every database gives a series the same id.
-- The crawler assigns it to every row it creates. This moves the 11 series_metadata rows the
-- initial migration seeds onto their stable ids, so a fresh database has them too (its other seed
-- rows name data sources that don't exist yet, so they are never inserted). Nothing references
-- series_metadata.id. The crawler test `seeded_series_metadata_has_stable_ids`
-- checks these literals against the Rust function.
UPDATE series_metadata sm
SET id = v.id
FROM (VALUES
    ('Federal Reserve Economic Data (FRED)', 'GDP', 'd8124fe6-ef1c-52dd-8d22-c1976625064c'::uuid),
    ('Federal Reserve Economic Data (FRED)', 'UNRATE', 'd2ac622b-7d71-5884-9994-0f939bd9407f'::uuid),
    ('Federal Reserve Economic Data (FRED)', 'CPIAUCSL', '432b1389-3872-5ab3-b699-955bc6fec997'::uuid),
    ('Bureau of Labor Statistics (BLS)', 'CES0000000001', '201c52b2-7f26-533b-a4d6-54e3141ee87b'::uuid),
    ('Bureau of Labor Statistics (BLS)', 'LNS14000000', '90c186a5-6afa-524b-83e3-c15e6985e2f6'::uuid),
    ('U.S. Census Bureau', 'B01001001', '089deb5f-9ed3-55f4-9782-aab27b353620'::uuid),
    ('U.S. Census Bureau', 'B19013_001E', 'dc6cab7c-d45e-56bd-8f76-d0943035703c'::uuid),
    ('Bureau of Economic Analysis (BEA)', 'GDP', '95ebb92e-0ab9-505a-82e9-89993c5d2217'::uuid),
    ('Bureau of Economic Analysis (BEA)', 'PCE', '35f7aa40-693f-53fe-9494-ca4000dcb24c'::uuid),
    ('International Monetary Fund (IMF)', 'NGDP_R_SA_XDC', 'dcf10e81-73ad-56b5-82a8-8e6371da8401'::uuid),
    ('International Monetary Fund (IMF)', 'NGDP_XDC', 'c9d09457-0aed-5ca9-88f6-87f2a0545916'::uuid)
) AS v(source_name, external_id, id)
JOIN data_sources ds ON ds.name = v.source_name
WHERE sm.source_id = ds.id AND sm.external_id = v.external_id;

COMMENT ON COLUMN economic_series.id IS
    'UUIDv5 of SOURCE:external_id for crawled series (econ-graph-crawler series_id.rs); uuidv7() default otherwise';
COMMENT ON COLUMN series_metadata.id IS
    'UUIDv5 of SOURCE:external_id (econ-graph-crawler series_id.rs); uuidv7() default otherwise';
