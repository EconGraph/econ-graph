-- Removing this migration while company notes exist would destroy their targets.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM financial_annotations WHERE research_anchor IS NOT NULL) THEN
        RAISE EXCEPTION 'Company notes exist; preserve or explicitly migrate their anchors before rollback';
    END IF;
END $$;
DROP INDEX financial_annotations_research_owner;
ALTER TABLE financial_annotations
    DROP CONSTRAINT financial_annotations_target,
    DROP COLUMN research_anchor,
    DROP COLUMN research_anchor_key,
    DROP COLUMN research_cik,
    ALTER COLUMN statement_id SET NOT NULL;
