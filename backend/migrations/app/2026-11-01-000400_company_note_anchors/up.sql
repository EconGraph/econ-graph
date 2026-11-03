-- Extend the existing app-owned annotation model; data targets are never foreign keys.
ALTER TABLE financial_annotations
    ALTER COLUMN statement_id DROP NOT NULL,
    ADD COLUMN research_anchor jsonb,
    ADD COLUMN research_anchor_key text,
    ADD COLUMN research_cik text,
    ADD CONSTRAINT financial_annotations_target CHECK (
        (research_anchor IS NULL AND research_anchor_key IS NULL AND research_cik IS NULL AND statement_id IS NOT NULL)
        OR (research_anchor IS NOT NULL AND research_anchor_key IS NOT NULL AND research_cik IS NOT NULL AND research_cik ~ '^[0-9]{10}$'
            AND statement_id IS NULL AND line_item_id IS NULL)
    );
CREATE INDEX financial_annotations_research_owner ON financial_annotations(created_by, research_cik, created_at, id)
    WHERE research_anchor IS NOT NULL;
