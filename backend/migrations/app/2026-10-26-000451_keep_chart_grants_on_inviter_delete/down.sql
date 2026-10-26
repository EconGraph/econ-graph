ALTER TABLE chart_collaborators
    DROP CONSTRAINT chart_collaborators_invited_by_fkey;

ALTER TABLE chart_collaborators
    ADD CONSTRAINT chart_collaborators_invited_by_fkey
    FOREIGN KEY (invited_by) REFERENCES users(id);

COMMENT ON COLUMN chart_collaborators.invited_by IS NULL;
