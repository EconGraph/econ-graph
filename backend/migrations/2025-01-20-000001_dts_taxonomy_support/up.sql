-- Migration: Add DTS (Discoverable Taxonomy Set) support tables
-- Created: 2025-01-20
-- Purpose: Support storage of XBRL taxonomy schemas, linkbases, and DTS metadata

-- Create enum types for taxonomy-related fields
CREATE TYPE taxonomy_file_type AS ENUM (
    'schema',
    'label_linkbase',
    'presentation_linkbase',
    'calculation_linkbase',
    'definition_linkbase',
    'reference_linkbase',
    'formula_linkbase'
);

CREATE TYPE taxonomy_source_type AS ENUM (
    'company_specific',
    'us_gaap',
    'sec_dei',
    'fasb_srt',
    'ifrs',
    'other_standard',
    'custom'
);

CREATE TYPE taxonomy_status AS ENUM (
    'downloaded',
    'processing',
    'processed',
    'error',
    'outdated'
);

-- Table for storing XBRL taxonomy schema files (.xsd)
CREATE TABLE xbrl_taxonomy_schemas (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Schema identification
    schema_namespace VARCHAR(255) NOT NULL,
    schema_filename VARCHAR(255) NOT NULL,
    schema_version VARCHAR(50),
    schema_date DATE,

    -- File metadata
    file_type taxonomy_file_type NOT NULL DEFAULT 'schema',
    source_type taxonomy_source_type NOT NULL,

    -- Storage information
    file_content BYTEA,
    file_oid OID,
    file_size_bytes BIGINT NOT NULL,
    file_hash VARCHAR(64) NOT NULL, -- SHA-256 hash

    -- Compression
    is_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    compression_type VARCHAR(10) NOT NULL DEFAULT 'zstd',

    -- Source information
    source_url TEXT,
    download_url TEXT,
    original_filename VARCHAR(255),

    -- Processing status
    processing_status taxonomy_status NOT NULL DEFAULT 'downloaded',
    processing_error TEXT,
    processing_started_at TIMESTAMPTZ,
    processing_completed_at TIMESTAMPTZ,

    -- Metadata
    concepts_extracted INTEGER DEFAULT 0,
    relationships_extracted INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_schema_namespace_version UNIQUE (schema_namespace, schema_version),
    CONSTRAINT valid_file_storage CHECK (
        (file_content IS NOT NULL AND file_oid IS NULL) OR
        (file_content IS NULL AND file_oid IS NOT NULL)
    )
);

-- Table for storing XBRL taxonomy linkbase files (.xml)
CREATE TABLE xbrl_taxonomy_linkbases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Linkbase identification
    linkbase_filename VARCHAR(255) NOT NULL,
    linkbase_type taxonomy_file_type NOT NULL,
    target_namespace VARCHAR(255),

    -- Related schema
    schema_id UUID REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,

    -- File metadata
    file_content BYTEA,
    file_oid OID,
    file_size_bytes BIGINT NOT NULL,
    file_hash VARCHAR(64) NOT NULL, -- SHA-256 hash

    -- Compression
    is_compressed BOOLEAN NOT NULL DEFAULT TRUE,
    compression_type VARCHAR(10) NOT NULL DEFAULT 'zstd',

    -- Source information
    source_url TEXT,
    download_url TEXT,
    original_filename VARCHAR(255),

    -- Processing status
    processing_status taxonomy_status NOT NULL DEFAULT 'downloaded',
    processing_error TEXT,
    processing_started_at TIMESTAMPTZ,
    processing_completed_at TIMESTAMPTZ,

    -- Metadata
    relationships_extracted INTEGER DEFAULT 0,
    labels_extracted INTEGER DEFAULT 0,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT valid_linkbase_file_storage CHECK (
        (file_content IS NOT NULL AND file_oid IS NULL) OR
        (file_content IS NULL AND file_oid IS NOT NULL)
    )
);

-- Table for tracking DTS dependencies (taxonomies referencing other taxonomies)
CREATE TABLE xbrl_dts_dependencies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Dependency relationship
    parent_schema_id UUID NOT NULL REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,
    child_schema_id UUID REFERENCES xbrl_taxonomy_schemas(id) ON DELETE CASCADE,
    child_namespace VARCHAR(255) NOT NULL, -- For external dependencies not yet downloaded

    -- Dependency metadata
    dependency_type VARCHAR(50) NOT NULL, -- 'import', 'include', 'reference'
    dependency_location TEXT, -- URL or path where dependency was found
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE, -- Whether child_schema_id is populated

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Constraints
    CONSTRAINT unique_dependency UNIQUE (parent_schema_id, child_namespace),
    CONSTRAINT valid_dependency CHECK (
        (is_resolved = TRUE AND child_schema_id IS NOT NULL) OR
        (is_resolved = FALSE AND child_schema_id IS NULL)
    )
);

-- Table for storing XBRL instance file DTS references
CREATE TABLE xbrl_instance_dts_references (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Related financial statement
    statement_id UUID NOT NULL REFERENCES financial_statements(id) ON DELETE CASCADE,

    -- DTS reference information
    reference_type VARCHAR(20) NOT NULL, -- 'schemaRef', 'linkbaseRef'
    reference_role VARCHAR(255), -- xlink:role attribute
    reference_href TEXT NOT NULL, -- xlink:href attribute
    reference_arcrole VARCHAR(255), -- xlink:arcrole attribute (for linkbaseRef)

    -- Resolved taxonomy
    resolved_schema_id UUID REFERENCES xbrl_taxonomy_schemas(id),
    resolved_linkbase_id UUID REFERENCES xbrl_taxonomy_linkbases(id),

    -- Status
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE,
    resolution_error TEXT,

    -- Timestamps
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT valid_resolution CHECK (
        (is_resolved = TRUE AND (resolved_schema_id IS NOT NULL OR resolved_linkbase_id IS NOT NULL)) OR
        (is_resolved = FALSE AND resolved_schema_id IS NULL AND resolved_linkbase_id IS NULL)
    )
);

-- Extend existing xbrl_taxonomy_concepts table with additional metadata
ALTER TABLE xbrl_taxonomy_concepts
ADD COLUMN IF NOT EXISTS schema_id UUID REFERENCES xbrl_taxonomy_schemas(id),
ADD COLUMN IF NOT EXISTS concept_qname VARCHAR(255),
ADD COLUMN IF NOT EXISTS concept_namespace VARCHAR(255),
ADD COLUMN IF NOT EXISTS concept_local_name VARCHAR(255),
ADD COLUMN IF NOT EXISTS is_abstract BOOLEAN DEFAULT FALSE,
ADD COLUMN IF NOT EXISTS is_nillable BOOLEAN DEFAULT TRUE,
ADD COLUMN IF NOT EXISTS min_occurs INTEGER DEFAULT 1,
ADD COLUMN IF NOT EXISTS max_occurs INTEGER DEFAULT 1,
ADD COLUMN IF NOT EXISTS base_type VARCHAR(100),
ADD COLUMN IF NOT EXISTS facet_constraints JSONB,
ADD COLUMN IF NOT EXISTS documentation_url TEXT,
ADD COLUMN IF NOT EXISTS label_roles JSONB, -- Multiple label roles and their values
ADD COLUMN IF NOT EXISTS calculation_relationships JSONB, -- Calculation linkbase relationships
ADD COLUMN IF NOT EXISTS presentation_relationships JSONB, -- Presentation linkbase relationships
ADD COLUMN IF NOT EXISTS definition_relationships JSONB; -- Definition linkbase relationships

-- Create indexes for performance
CREATE INDEX idx_xbrl_taxonomy_schemas_namespace ON xbrl_taxonomy_schemas(schema_namespace);
CREATE INDEX idx_xbrl_taxonomy_schemas_source_type ON xbrl_taxonomy_schemas(source_type);
CREATE INDEX idx_xbrl_taxonomy_schemas_status ON xbrl_taxonomy_schemas(processing_status);
CREATE INDEX idx_xbrl_taxonomy_schemas_created_at ON xbrl_taxonomy_schemas(created_at);

CREATE INDEX idx_xbrl_taxonomy_linkbases_schema_id ON xbrl_taxonomy_linkbases(schema_id);
CREATE INDEX idx_xbrl_taxonomy_linkbases_type ON xbrl_taxonomy_linkbases(linkbase_type);
CREATE INDEX idx_xbrl_taxonomy_linkbases_status ON xbrl_taxonomy_linkbases(processing_status);

CREATE INDEX idx_xbrl_dts_dependencies_parent ON xbrl_dts_dependencies(parent_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_child ON xbrl_dts_dependencies(child_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_resolved ON xbrl_dts_dependencies(is_resolved);

CREATE INDEX idx_xbrl_instance_dts_references_statement ON xbrl_instance_dts_references(statement_id);
CREATE INDEX idx_xbrl_instance_dts_references_type ON xbrl_instance_dts_references(reference_type);
CREATE INDEX idx_xbrl_instance_dts_references_resolved ON xbrl_instance_dts_references(is_resolved);

CREATE INDEX idx_xbrl_taxonomy_concepts_schema_id ON xbrl_taxonomy_concepts(schema_id);
CREATE INDEX idx_xbrl_taxonomy_concepts_qname ON xbrl_taxonomy_concepts(concept_qname);
CREATE INDEX idx_xbrl_taxonomy_concepts_namespace ON xbrl_taxonomy_concepts(concept_namespace);

-- Create updated_at triggers
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_xbrl_taxonomy_schemas_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_schemas
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_xbrl_taxonomy_linkbases_updated_at
    BEFORE UPDATE ON xbrl_taxonomy_linkbases
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
