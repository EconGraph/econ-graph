-- Every series belongs to a dataset: economic_series.dataset_id becomes NOT NULL.
--
-- Existing rows without one are backfilled first, using the same rules as the crawler's adapters
-- (econ-graph-crawler src/sources/*.rs and data/datasets/*.toml):
--   1. The dataset and dimensions discovery already recorded in series_metadata for the same
--      (source_id, external_id). Dimensions are left empty if another series already holds that
--      dataset key; the next crawl of the series rewrites them either way.
--   2. Otherwise the adapter's dataset, matched on the external id the way the adapter forms it:
--      FRED and the static catalogs have one dataset for every id; BLS ids that fit a survey's
--      layout (bls.rs SERIES_ID_LAYOUTS) go to that survey, and every other BLS id to the
--      catch-all `other`; BEA, Census BDS, WDI and FHFA ids carry their dataset as a prefix
--      (bea_nipa/, bea_regional/, bds/, wdi/, fhfa_hpi/). FHFA ids from before its datasets
--      (USHPI, {STATE}HPI, {METRO}HPI) are FHFA house price indexes too, so they also go to
--      fhfa_hpi. Ids no adapter forms (Census ACS, IMF) match nothing. Dimensions stay empty until
--      the next crawl; legacy FHFA ids are no longer crawled, so theirs stay empty.
-- A dataset that rule 2 needs but that has no row yet (sync_datasets runs at worker startup,
-- after migrations) gets one with just its code and name; the next sync fills in the rest.
--   3. A series no adapter forms an id for any more (before BEA, Census and WDI used dataset
--      prefixes: `GDP` from BEA, an ACS table id from Census, `NY.GDP.PCAP.CD` from the World
--      Bank, or any IMF or SEC EDGAR id) goes to its source's `legacy` dataset, so upgrading a
--      database that crawled them keeps their observations instead of stopping every backend
--      start. The scheduler never selects them (they are not crawled again); delete them or
--      leave them as history.
-- Any row still without a dataset (a series with no applicable rule) stops the migration with a
-- list of what is left.
--
-- series_metadata.dataset_id stays nullable: it is the discovery catalog, and the initial schema
-- seeds rows for sources the crawler has no adapter for (IMF) or under retired ids.

UPDATE economic_series es
SET dataset_id = sm.dataset_id,
    dimensions = CASE
        WHEN sm.dimensions = '{}'::jsonb OR EXISTS (
            SELECT 1 FROM economic_series o
            WHERE o.dataset_id = sm.dataset_id AND o.dimensions = sm.dimensions
        ) THEN '{}'::jsonb
        ELSE sm.dimensions
    END
FROM series_metadata sm
WHERE es.dataset_id IS NULL
    AND sm.dataset_id IS NOT NULL
    AND sm.source_id = es.source_id
    AND sm.external_id = es.external_id;

CREATE TEMPORARY TABLE series_dataset_rules (
    source_name VARCHAR(255) NOT NULL,
    code VARCHAR(100) NOT NULL,
    name VARCHAR(500) NOT NULL,
    -- A POSIX regular expression on the external id; NULL matches every external id of the source.
    id_pattern TEXT
);

INSERT INTO series_dataset_rules (source_name, code, name, id_pattern) VALUES
    ('Federal Reserve Economic Data (FRED)', 'FRED', 'FRED', NULL),
    ('Bureau of Labor Statistics (BLS)', 'CU', 'Consumer Price Index - All Urban Consumers', '^CU[A-Za-z0-9]{7,}$'),
    ('Bureau of Labor Statistics (BLS)', 'CE', 'Employment, Hours, and Earnings - National (CES)', '^CE[A-Za-z0-9]{11}$'),
    ('Bureau of Labor Statistics (BLS)', 'LN', 'Labor Force Statistics from the Current Population Survey', '^LN[A-Za-z0-9]{9}$'),
    ('Bureau of Labor Statistics (BLS)', 'LA', 'Local Area Unemployment Statistics', '^LA[A-Za-z0-9]{18}$'),
    ('Bureau of Labor Statistics (BLS)', 'other', 'Other BLS series', NULL),
    ('Bureau of Economic Analysis (BEA)', 'bea_nipa', 'BEA National Income and Product Accounts', '^bea_nipa/'),
    ('Bureau of Economic Analysis (BEA)', 'bea_regional', 'BEA Regional Economic Accounts', '^bea_regional/'),
    ('U.S. Census Bureau', 'bds', 'Business Dynamics Statistics', '^bds/'),
    ('World Bank Open Data', 'wdi', 'World Development Indicators', '^wdi/'),
    ('Federal Housing Finance Agency (FHFA)', 'fhfa_hpi', 'FHFA House Price Index', '^fhfa_hpi/'),
    -- Legacy ids of the FHFA adapter before DATA-7: USHPI, {STATE}HPI, {METRO}HPI (e.g. NYCHPI).
    ('Federal Housing Finance Agency (FHFA)', 'fhfa_hpi', 'FHFA House Price Index', '^[A-Z]{2,3}HPI$'),
    -- Series from before the datasets above (see rule 3 in the header).
    ('Bureau of Economic Analysis (BEA)', 'legacy', 'BEA series from before datasets', NULL),
    ('U.S. Census Bureau', 'legacy', 'Census series from before datasets', NULL),
    ('World Bank Open Data', 'legacy', 'World Bank series from before datasets', NULL),
    ('International Monetary Fund (IMF)', 'legacy', 'IMF series from before datasets', NULL),
    ('SEC EDGAR', 'legacy', 'SEC EDGAR series from before datasets', NULL),
    -- Development-only static catalogs (econ-graph-crawler src/sources/static_catalogs.rs).
    ('Bank of Canada (BoC)', 'catalog', 'Bank of Canada catalog', NULL),
    ('Bank of England (BoE)', 'catalog', 'Bank of England catalog', NULL),
    ('Bank of Japan (BoJ)', 'catalog', 'Bank of Japan catalog', NULL),
    ('European Central Bank (ECB)', 'catalog', 'European Central Bank catalog', NULL),
    ('International Labour Organization (ILO)', 'catalog', 'International Labour Organization catalog', NULL),
    ('OECD (Organisation for Economic Co-operation and Development)', 'catalog', 'OECD catalog', NULL),
    ('Reserve Bank of Australia (RBA)', 'catalog', 'Reserve Bank of Australia catalog', NULL),
    ('Swiss National Bank (SNB)', 'catalog', 'Swiss National Bank catalog', NULL),
    ('UN Statistics Division', 'catalog', 'UN Statistics Division catalog', NULL),
    ('World Trade Organization (WTO)', 'catalog', 'World Trade Organization catalog', NULL);

-- A pattern rule wins over its source's catch-all (NULL pattern) rule.
CREATE TEMPORARY TABLE series_dataset_matches AS
SELECT DISTINCT ON (es.id) es.id AS series_id, ds.id AS source_id, r.code, r.name
FROM economic_series es
JOIN data_sources ds ON ds.id = es.source_id
JOIN series_dataset_rules r ON r.source_name = ds.name
    AND (r.id_pattern IS NULL OR es.external_id ~ r.id_pattern)
WHERE es.dataset_id IS NULL
ORDER BY es.id, r.id_pattern IS NULL;

INSERT INTO datasets (source_id, code, name)
SELECT DISTINCT source_id, code, name FROM series_dataset_matches
ON CONFLICT (source_id, code) DO NOTHING;

UPDATE economic_series es
SET dataset_id = d.id
FROM series_dataset_matches m
JOIN datasets d ON d.source_id = m.source_id AND d.code = m.code
WHERE es.id = m.series_id;

DO $$
DECLARE
    missing TEXT;
BEGIN
    SELECT string_agg(
        format('%s: %s series (e.g. %s)', ds.name, n.count, n.example), '; ' ORDER BY ds.name
    )
    INTO missing
    FROM (
        SELECT source_id, count(*) AS count, min(external_id) AS example
        FROM economic_series
        WHERE dataset_id IS NULL
        GROUP BY source_id
    ) n
    JOIN data_sources ds ON ds.id = n.source_id;
    IF missing IS NOT NULL THEN
        RAISE EXCEPTION 'economic_series rows without a dataset: %', missing
            USING HINT = 'No adapter rule assigns these series a dataset. Set their dataset_id '
                'by hand (adding a datasets row if needed) or delete them, then rerun migrations.';
    END IF;
END
$$;

DROP TABLE series_dataset_matches, series_dataset_rules;

ALTER TABLE economic_series
    ALTER COLUMN dataset_id SET NOT NULL,
    -- Both always hold now that dataset_id is NOT NULL.
    DROP CONSTRAINT economic_series_dimensions_need_dataset,
    DROP CONSTRAINT economic_series_default_measure_needs_dataset;

DROP INDEX idx_economic_series_dataset_id;
CREATE INDEX idx_economic_series_dataset_id ON economic_series (dataset_id);
