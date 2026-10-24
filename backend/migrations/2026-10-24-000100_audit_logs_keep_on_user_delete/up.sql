-- Keep audit history when a user is deleted.
--
-- The v4.0 baseline declared audit_logs.user_id NOT NULL with ON DELETE CASCADE, so
-- deleting a user erased every audit row they had written. Make user_id nullable and
-- set it to NULL instead; user_name (denormalized at write time) keeps the row
-- attributable after the user is gone.
--
-- The baseline declares the foreign key inline, so PostgreSQL named it
-- audit_logs_user_id_fkey. The replacement keeps that name.

ALTER TABLE audit_logs DROP CONSTRAINT audit_logs_user_id_fkey;

ALTER TABLE audit_logs ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

COMMENT ON COLUMN audit_logs.user_id IS
    'User who performed the action; NULL once that user is deleted (user_name still names them)';
