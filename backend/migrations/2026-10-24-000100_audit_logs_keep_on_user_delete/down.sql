-- Restore the v4.0 baseline: audit_logs.user_id NOT NULL, ON DELETE CASCADE.
--
-- LIMITATION: SET NOT NULL fails if any audit row's user has been deleted since the up
-- migration ran (user_id IS NULL). This rollback deliberately does not delete those
-- rows, because that would destroy audit history. Before rolling back, decide what to
-- do with them by hand, for example archive them and then
--     DELETE FROM audit_logs WHERE user_id IS NULL;
-- The rollback runs in one transaction, so a failure leaves the schema unchanged.

ALTER TABLE audit_logs DROP CONSTRAINT audit_logs_user_id_fkey;

ALTER TABLE audit_logs ALTER COLUMN user_id SET NOT NULL;

ALTER TABLE audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

COMMENT ON COLUMN audit_logs.user_id IS NULL;
