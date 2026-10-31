-- Where each series' units came from, the source's own units text, and the units in the
-- crawler's fixed vocabulary (econ-graph-crawler src/units.rs).
--
-- units_source: 'structured' when the source states units in a field of their own (FRED's
-- units, BEA's CL_UNIT, a curated indicator list), 'parsed_from_text' when the crawler parsed
-- them out of free text such as a description, 'unknown' when there are none. NULL on a row no
-- writer has set it on (every row that existed before this migration): readers derive it from
-- units, 'unknown' when units is NULL or only whitespace and 'structured' otherwise, and the
-- next fetch of the series writes it.
-- units_raw: the units text exactly as the source gave it, before any parsing or rewording.
-- NULL when not recorded.
-- units_parsed: the units as the crawler stored them, as JSON
-- ({"kind": "currency", "currency": "USD", "scale": "billions"}); NULL when the crawler did
-- not recognise them or has not written them.
--
-- Nothing is backfilled. Adding nullable columns without a default only changes the catalog,
-- and the check constraint is added NOT VALID, so Postgres doesn't scan the table. The
-- migration holds the ACCESS EXCLUSIVE lock for milliseconds (no scan, no rewrite), whatever
-- the table's size. lock_timeout makes the migration
-- fail, changing nothing, rather than queue behind long transactions on economic_series while
-- blocking its readers.

SET LOCAL lock_timeout = '10s';

ALTER TABLE economic_series
    ADD COLUMN units_source VARCHAR(20),
    ADD COLUMN units_raw TEXT,
    ADD COLUMN units_parsed JSONB,
    -- Existing rows are NULL, which satisfies the check; every new write is checked.
    ADD CONSTRAINT economic_series_units_source_check
        CHECK (units_source IN ('structured', 'parsed_from_text', 'unknown')) NOT VALID;
