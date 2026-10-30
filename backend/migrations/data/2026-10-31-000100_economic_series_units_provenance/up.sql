-- Where each series' units came from, the source's own units text, and the units in the
-- crawler's fixed vocabulary (econ-graph-crawler src/units.rs).
--
-- units_source: 'structured' when the source states units in a field of their own (FRED's
-- units, BEA's CL_UNIT, a curated indicator list), 'parsed_from_text' when the crawler parsed
-- them out of free text such as a description, 'unknown' when there are none.
-- units_raw: the units text exactly as the source gave it, before any parsing or rewording.
-- units_parsed: the units as the crawler stored them, as JSON
-- ({"kind": "currency", "currency": "USD", "scale": "billions"}); NULL when the crawler did
-- not recognise them or has not written them.
--
-- Existing rows: adding the column with a constant default records the default in the catalog
-- and rewrites no row, so every existing row reads 'structured'. Then only the rows without units
-- are updated to 'unknown' (one read of the table, writing just those rows). units_raw and
-- units_parsed stay NULL on existing rows until a fetch writes their units again.
-- The default then becomes 'unknown', so a writer that sets units also sets units_source.
--
-- lock_timeout makes the migration fail, changing nothing, rather than queue behind long
-- transactions on economic_series while blocking its readers.

SET LOCAL lock_timeout = '10s';

ALTER TABLE economic_series
    ADD COLUMN units_source VARCHAR(20) NOT NULL DEFAULT 'structured'
        CONSTRAINT economic_series_units_source_check
            CHECK (units_source IN ('structured', 'parsed_from_text', 'unknown')),
    ADD COLUMN units_raw TEXT,
    ADD COLUMN units_parsed JSONB;

UPDATE economic_series SET units_source = 'unknown' WHERE units IS NULL OR btrim(units) = '';

ALTER TABLE economic_series ALTER COLUMN units_source SET DEFAULT 'unknown';
