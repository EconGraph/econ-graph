-- Postgres can't drop enum values, so rebuild annotation_type with its original labels.
-- Rows that use one of the added values are mapped to the closest original label.
ALTER TYPE annotation_type RENAME TO annotation_type_old;
CREATE TYPE annotation_type AS ENUM ('comment', 'question', 'concern', 'insight', 'risk', 'opportunity', 'highlight');

CREATE FUNCTION pg_temp.legacy_annotation_type(v annotation_type_old) RETURNS annotation_type
LANGUAGE sql IMMUTABLE AS $$
    SELECT (CASE v::text
        WHEN 'revenue_growth' THEN 'opportunity'
        WHEN 'cost_concern' THEN 'concern'
        WHEN 'cash_flow' THEN 'insight'
        WHEN 'balance_sheet' THEN 'insight'
        WHEN 'one_time_item' THEN 'highlight'
        WHEN 'industry_context' THEN 'insight'
        ELSE v::text
    END)::annotation_type
$$;

ALTER TABLE financial_annotations
    ALTER COLUMN annotation_type TYPE annotation_type
    USING pg_temp.legacy_annotation_type(annotation_type);
ALTER TABLE annotation_templates
    ALTER COLUMN annotation_type TYPE annotation_type
    USING pg_temp.legacy_annotation_type(annotation_type);

DROP FUNCTION pg_temp.legacy_annotation_type(annotation_type_old);
DROP TYPE annotation_type_old;
