-- Data point vintages as temporal ranges (PostgreSQL 18 WITHOUT OVERLAPS).
--
-- Each data_points row is one vintage of one observation: the value published on revision_date,
-- valid until the next revision (superseded_on) or still current (superseded_on IS NULL).
-- `vintage` is that validity as a daterange, and a temporal UNIQUE constraint guarantees at most
-- one value per (series_id, date) on any day. Writers keep inserting (revision_date, value);
-- a trigger maintains superseded_on, including for revisions that arrive out of order.
--
--   current values:            WHERE superseded_on IS NULL
--   values as known on day X:  WHERE vintage @> X
--                              (or revision_date <= X AND (superseded_on IS NULL OR superseded_on > X))

CREATE EXTENSION IF NOT EXISTS btree_gist;

-- One row per (series_id, date, revision_date). Earlier data could hold the same revision twice
-- with different is_original_release flags; keep the most recently updated.
DELETE FROM data_points d
USING data_points newer
WHERE newer.series_id = d.series_id
  AND newer.date = d.date
  AND newer.revision_date = d.revision_date
  AND (newer.updated_at, newer.id) > (d.updated_at, d.id);

ALTER TABLE data_points ADD COLUMN superseded_on DATE;

UPDATE data_points d
SET superseded_on = nxt.next_revision
FROM (
    SELECT id, LEAD(revision_date) OVER (PARTITION BY series_id, date ORDER BY revision_date)
               AS next_revision
    FROM data_points
) nxt
WHERE nxt.id = d.id AND nxt.next_revision IS NOT NULL;

ALTER TABLE data_points
    ADD CONSTRAINT data_points_superseded_after_revision CHECK (superseded_on > revision_date),
    ADD COLUMN vintage DATERANGE NOT NULL
        GENERATED ALWAYS AS (daterange(revision_date, superseded_on, '[)')) STORED;

ALTER TABLE data_points
    DROP CONSTRAINT data_points_series_id_date_revision_date_is_original_releas_key,
    ADD CONSTRAINT data_points_series_date_revision_key UNIQUE (series_id, date, revision_date),
    ADD CONSTRAINT data_points_one_value_per_day
        UNIQUE (series_id, date, vintage WITHOUT OVERLAPS);

-- Current values, the common read path.
CREATE INDEX idx_data_points_current ON data_points (series_id, date) INCLUDE (value)
    WHERE superseded_on IS NULL;

-- Keep superseded_on consistent as revisions arrive, in any order.
CREATE OR REPLACE FUNCTION data_points_link_vintage() RETURNS TRIGGER AS $$
BEGIN
    -- The vintage this revision interrupts now ends where this one starts.
    UPDATE data_points
    SET superseded_on = NEW.revision_date
    WHERE series_id = NEW.series_id
      AND date = NEW.date
      AND revision_date < NEW.revision_date
      AND (superseded_on IS NULL OR superseded_on > NEW.revision_date);

    -- This vintage lasts until the next known revision, if any.
    SELECT MIN(revision_date) INTO NEW.superseded_on
    FROM data_points
    WHERE series_id = NEW.series_id
      AND date = NEW.date
      AND revision_date > NEW.revision_date;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER data_points_link_vintage
    BEFORE INSERT ON data_points
    FOR EACH ROW EXECUTE FUNCTION data_points_link_vintage();

-- Deleting a vintage extends the previous one over the gap.
CREATE OR REPLACE FUNCTION data_points_unlink_vintage() RETURNS TRIGGER AS $$
BEGIN
    UPDATE data_points
    SET superseded_on = OLD.superseded_on
    WHERE series_id = OLD.series_id
      AND date = OLD.date
      AND superseded_on = OLD.revision_date;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER data_points_unlink_vintage
    AFTER DELETE ON data_points
    FOR EACH ROW EXECUTE FUNCTION data_points_unlink_vintage();
