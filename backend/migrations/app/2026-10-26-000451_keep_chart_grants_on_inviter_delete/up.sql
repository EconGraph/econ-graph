-- A grant belongs to its recipient. Removing the inviter must not revoke
-- another user's access or prevent deleting the inviter's account.
ALTER TABLE chart_collaborators
    DROP CONSTRAINT chart_collaborators_invited_by_fkey;

ALTER TABLE chart_collaborators
    ADD CONSTRAINT chart_collaborators_invited_by_fkey
    FOREIGN KEY (invited_by) REFERENCES users(id) ON DELETE SET NULL;

COMMENT ON COLUMN chart_collaborators.invited_by IS
    'User who issued the grant; NULL when that user has been deleted';
