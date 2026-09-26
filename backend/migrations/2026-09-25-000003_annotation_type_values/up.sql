-- The Rust AnnotationType enum (econ-graph-core/src/enums.rs) writes six values the
-- annotation_type Postgres enum never had. Add them so every variant can be stored.
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'revenue_growth';
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'cost_concern';
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'cash_flow';
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'balance_sheet';
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'one_time_item';
ALTER TYPE annotation_type ADD VALUE IF NOT EXISTS 'industry_context';
