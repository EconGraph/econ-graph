-- Ambiguous duplicate grants are reconciled to their least privileged role.
-- Unknown and NULL roles grant only view in the service. Clear legacy permission
-- JSON on reconciled rows so it cannot restore a discarded grant later.
-- Preserve the oldest row's identity and invitation metadata deterministically.
LOCK TABLE chart_collaborators IN ACCESS EXCLUSIVE MODE;

WITH ranked AS (
    SELECT id, chart_id, user_id,
           row_number() OVER (PARTITION BY chart_id, user_id ORDER BY created_at ASC NULLS LAST, id) AS ordinal,
           count(*) OVER (PARTITION BY chart_id, user_id) AS copies,
           min(CASE lower(role)
               WHEN 'admin' THEN 3 WHEN 'edit' THEN 2 WHEN 'comment' THEN 1 ELSE 0 END)
               OVER (PARTITION BY chart_id, user_id) AS least_role
    FROM chart_collaborators
)
UPDATE chart_collaborators AS c
SET role = CASE ranked.least_role
    WHEN 3 THEN 'admin' WHEN 2 THEN 'edit' WHEN 1 THEN 'comment' ELSE 'view' END,
    permissions = NULL
FROM ranked
WHERE c.id = ranked.id AND ranked.ordinal = 1 AND ranked.copies > 1;

WITH ranked AS (
    SELECT id, row_number() OVER (
        PARTITION BY chart_id, user_id ORDER BY created_at ASC NULLS LAST, id
    ) AS ordinal
    FROM chart_collaborators
)
DELETE FROM chart_collaborators AS c USING ranked
WHERE c.id = ranked.id AND ranked.ordinal > 1;

ALTER TABLE chart_collaborators
ADD CONSTRAINT chart_collaborators_chart_user_unique UNIQUE (chart_id, user_id);
