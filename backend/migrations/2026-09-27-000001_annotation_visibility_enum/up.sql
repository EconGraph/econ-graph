-- Replace the overloaded chart_annotations.is_visible boolean with a native Postgres enum,
-- so a nullable "maybe visible" column can't hide the private/public distinction the
-- application actually enforces.
CREATE TYPE annotation_visibility AS ENUM ('private', 'public');

ALTER TABLE chart_annotations
    ADD COLUMN visibility annotation_visibility NOT NULL DEFAULT 'private';

-- Only rows explicitly marked visible (is_visible = true) were ever served as public by the
-- application; NULL and false both read as private, so they keep the new column's default.
UPDATE chart_annotations SET visibility = 'public' WHERE is_visible = true;

ALTER TABLE chart_annotations DROP COLUMN is_visible;
