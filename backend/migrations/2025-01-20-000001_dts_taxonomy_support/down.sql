-- Migration: Remove DTS (Discoverable Taxonomy Set) support tables
-- Created: 2025-01-20
-- Purpose: Rollback DTS taxonomy support

-- Drop triggers first
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_linkbases_updated_at ON xbrl_taxonomy_linkbases;
DROP TRIGGER IF EXISTS update_xbrl_taxonomy_schemas_updated_at ON xbrl_taxonomy_schemas;

-- Drop tables in reverse dependency order
DROP TABLE IF EXISTS xbrl_instance_dts_references;
DROP TABLE IF EXISTS xbrl_dts_dependencies;
DROP TABLE IF EXISTS xbrl_taxonomy_linkbases;
DROP TABLE IF EXISTS xbrl_taxonomy_schemas;

-- Remove added columns from existing table
ALTER TABLE xbrl_taxonomy_concepts
DROP COLUMN IF EXISTS schema_id,
DROP COLUMN IF EXISTS concept_qname,
DROP COLUMN IF EXISTS concept_namespace,
DROP COLUMN IF EXISTS concept_local_name,
DROP COLUMN IF EXISTS is_abstract,
DROP COLUMN IF EXISTS is_nillable,
DROP COLUMN IF EXISTS min_occurs,
DROP COLUMN IF EXISTS max_occurs,
DROP COLUMN IF EXISTS base_type,
DROP COLUMN IF EXISTS facet_constraints,
DROP COLUMN IF EXISTS documentation_url,
DROP COLUMN IF EXISTS label_roles,
DROP COLUMN IF EXISTS calculation_relationships,
DROP COLUMN IF EXISTS presentation_relationships,
DROP COLUMN IF EXISTS definition_relationships;

-- Drop enum types
DROP TYPE IF EXISTS taxonomy_status;
DROP TYPE IF EXISTS taxonomy_source_type;
DROP TYPE IF EXISTS taxonomy_file_type;
