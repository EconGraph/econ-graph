-- Rollback is possible only while every annotation still fits DECIMAL(20, 6)
-- exactly. Refuse both overflow and rounding; never discard user coordinates.
-- Hold the same lock as ALTER TABLE so no writer can race the check.
LOCK TABLE chart_annotations IN ACCESS EXCLUSIVE MODE;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM chart_annotations
        WHERE annotation_value IS NOT NULL
          AND (annotation_value::text IN ('NaN', 'Infinity', '-Infinity')
               OR abs(annotation_value) >= 100000000000000
               OR annotation_value <> trunc(annotation_value, 6))
    ) THEN
        RAISE EXCEPTION 'Cannot restore DECIMAL(20,6): annotation values would overflow or lose precision';
    END IF;
END $$;

ALTER TABLE chart_annotations
    ALTER COLUMN annotation_value TYPE DECIMAL(20, 6);
