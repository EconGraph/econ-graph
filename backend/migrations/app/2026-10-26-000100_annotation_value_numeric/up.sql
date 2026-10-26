-- App-plane migration (ECO-520): requires maintainer sign-off before deployment.
-- Preserve annotation coordinates exactly, including values above 10^14 and
-- fractional digits beyond the old DECIMAL(20, 6) scale.
ALTER TABLE chart_annotations
    ALTER COLUMN annotation_value TYPE NUMERIC;
