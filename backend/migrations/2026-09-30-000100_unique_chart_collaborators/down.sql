-- Deleted duplicates and discarded permissions cannot be reconstructed.
ALTER TABLE chart_collaborators
DROP CONSTRAINT chart_collaborators_chart_user_unique;
