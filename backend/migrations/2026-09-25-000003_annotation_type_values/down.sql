-- Postgres can't drop enum values, so rebuild annotation_type with its original labels.
-- Rows that use one of the added values must be changed or removed first.
ALTER TYPE annotation_type RENAME TO annotation_type_old;
CREATE TYPE annotation_type AS ENUM ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight');

ALTER TABLE financial_annotations
    ALTER COLUMN annotation_type TYPE annotation_type USING annotation_type::text::annotation_type;
ALTER TABLE annotation_templates
    ALTER COLUMN annotation_type TYPE annotation_type USING annotation_type::text::annotation_type;

DROP TYPE annotation_type_old;
