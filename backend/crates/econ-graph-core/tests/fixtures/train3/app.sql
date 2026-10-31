-- User content refers to data by identity; there are no cross-plane foreign keys.
INSERT INTO users (id, email, name, created_at, updated_at) VALUES
('31000000-0000-4000-8000-000000000001', 'train3-owner@example.invalid', 'Fixture owner', '2026-01-01Z', '2026-01-01Z'),
('31000000-0000-4000-8000-000000000002', 'train3-viewer@example.invalid', 'Fixture viewer', '2026-01-01Z', '2026-01-01Z');
INSERT INTO chart_annotations (id, user_id, series_id, chart_id, annotation_date, title, description, visibility, created_at, updated_at)
VALUES ('31000000-0000-4000-8000-000000000003', '31000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000003', '31000000-0000-4000-8000-000000000004', '2026-01-01', 'Keep my original target', 'Retained when a data target disappears', 'private', '2026-01-01Z', '2026-01-01Z');
INSERT INTO annotation_comments (id, annotation_id, user_id, content, created_at, updated_at)
VALUES ('31000000-0000-4000-8000-000000000005', '31000000-0000-4000-8000-000000000003', '31000000-0000-4000-8000-000000000002', 'Preserve this comment', '2026-01-01Z', '2026-01-01Z');
INSERT INTO chart_collaborators (id, chart_id, user_id, invited_by, role, permissions, created_at)
VALUES ('31000000-0000-4000-8000-000000000006', '31000000-0000-4000-8000-000000000004', '31000000-0000-4000-8000-000000000002', '31000000-0000-4000-8000-000000000001', 'viewer', '{"view":true,"annotate":false,"edit":false}', '2026-01-01Z');
INSERT INTO financial_annotations (id, statement_id, line_item_id, annotation_type, title, content, created_by, created_at, updated_at)
VALUES ('31000000-0000-4000-8000-000000000007', '32000000-0000-4000-8000-000000000001', '32000000-0000-4000-8000-000000000002', 'comment', 'Legacy filed fact', 'Do not retarget this note to an amendment', '31000000-0000-4000-8000-000000000001', '2026-01-01Z', '2026-01-01Z');
