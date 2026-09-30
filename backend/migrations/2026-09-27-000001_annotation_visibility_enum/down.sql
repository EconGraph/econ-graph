ALTER TABLE chart_annotations ADD COLUMN is_visible BOOLEAN DEFAULT true;

UPDATE chart_annotations SET is_visible = (visibility = 'public');

ALTER TABLE chart_annotations DROP COLUMN visibility;

DROP TYPE annotation_visibility;
