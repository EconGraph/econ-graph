// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "annotation_status"))]
    pub struct AnnotationStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "annotation_type"))]
    pub struct AnnotationType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "assignment_status"))]
    pub struct AssignmentStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "assignment_type"))]
    pub struct AssignmentType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "calculation_method"))]
    pub struct CalculationMethod;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "compression_type"))]
    pub struct CompressionType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "processing_status"))]
    pub struct ProcessingStatus;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "ratio_category"))]
    pub struct RatioCategory;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "statement_section"))]
    pub struct StatementSection;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "statement_type"))]
    pub struct StatementType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "taxonomy_file_type"))]
    pub struct TaxonomyFileType;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "taxonomy_source_type"))]
    pub struct TaxonomySourceType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::AssignmentType;
    use super::sql_types::AssignmentStatus;

    annotation_assignments (id) {
        id -> Uuid,
        annotation_id -> Uuid,
        assigned_to -> Uuid,
        assigned_by -> Uuid,
        assignment_type -> AssignmentType,
        status -> AssignmentStatus,
        due_date -> Nullable<Timestamptz>,
        instructions -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    annotation_comments (id) {
        id -> Uuid,
        annotation_id -> Uuid,
        user_id -> Uuid,
        content -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::AnnotationStatus;

    annotation_replies (id) {
        id -> Uuid,
        annotation_id -> Uuid,
        parent_reply_id -> Nullable<Uuid>,
        content -> Text,
        status -> AnnotationStatus,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::AnnotationType;

    annotation_templates (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        annotation_type -> AnnotationType,
        template_content -> Text,
        usage_count -> Nullable<Int4>,
        is_active -> Bool,
        created_by -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    audit_logs (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        #[max_length = 100]
        action -> Varchar,
        #[max_length = 100]
        resource_type -> Varchar,
        resource_id -> Nullable<Uuid>,
        details -> Nullable<Jsonb>,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    chart_annotation_replies (id) {
        id -> Uuid,
        comment_id -> Uuid,
        user_id -> Uuid,
        content -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    chart_annotation_templates (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        template_content -> Text,
        created_by -> Uuid,
        is_public -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    chart_annotations (id) {
        id -> Uuid,
        #[max_length = 255]
        chart_id -> Varchar,
        user_id -> Uuid,
        #[max_length = 50]
        annotation_type -> Varchar,
        content -> Text,
        position_x -> Nullable<Numeric>,
        position_y -> Nullable<Numeric>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    chart_collaborators (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        chart_id -> Varchar,
        #[max_length = 50]
        permission_level -> Varchar,
        invited_by -> Nullable<Uuid>,
        invited_at -> Timestamptz,
        accepted_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    chart_financial_annotations (id) {
        id -> Uuid,
        annotation_id -> Uuid,
        #[max_length = 100]
        financial_metric -> Varchar,
        metric_value -> Nullable<Numeric>,
        confidence_level -> Nullable<Numeric>,
        calculation_method -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    companies (id) {
        id -> Uuid,
        #[max_length = 10]
        cik -> Varchar,
        #[max_length = 10]
        ticker -> Nullable<Varchar>,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 500]
        legal_name -> Nullable<Varchar>,
        #[max_length = 4]
        sic_code -> Nullable<Varchar>,
        #[max_length = 255]
        sic_description -> Nullable<Varchar>,
        #[max_length = 100]
        industry -> Nullable<Varchar>,
        #[max_length = 100]
        sector -> Nullable<Varchar>,
        business_address -> Nullable<Jsonb>,
        mailing_address -> Nullable<Jsonb>,
        #[max_length = 50]
        phone -> Nullable<Varchar>,
        #[max_length = 255]
        website -> Nullable<Varchar>,
        #[max_length = 2]
        state_of_incorporation -> Nullable<Varchar>,
        #[max_length = 100]
        state_of_incorporation_description -> Nullable<Varchar>,
        #[max_length = 4]
        fiscal_year_end -> Nullable<Varchar>,
        #[max_length = 50]
        entity_type -> Nullable<Varchar>,
        #[max_length = 20]
        entity_size -> Nullable<Varchar>,
        is_active -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    countries (id) {
        id -> Uuid,
        #[max_length = 2]
        iso_code_2 -> Bpchar,
        #[max_length = 3]
        iso_code_3 -> Bpchar,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 100]
        region -> Nullable<Varchar>,
        #[max_length = 100]
        subregion -> Nullable<Varchar>,
        population -> Nullable<Int8>,
        gdp_usd -> Nullable<Int8>,
        #[max_length = 3]
        currency_code -> Nullable<Bpchar>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    country_correlations (id) {
        id -> Uuid,
        country_1_id -> Uuid,
        country_2_id -> Uuid,
        correlation_coefficient -> Numeric,
        calculation_period_start -> Date,
        calculation_period_end -> Date,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    crawl_attempts (id) {
        id -> Uuid,
        series_id -> Uuid,
        attempt_number -> Int4,
        #[max_length = 50]
        status -> Varchar,
        started_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        error_message -> Nullable<Text>,
        data_points_retrieved -> Nullable<Int4>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    crawl_queue (id) {
        id -> Uuid,
        #[max_length = 50]
        source -> Varchar,
        #[max_length = 255]
        series_id -> Varchar,
        #[max_length = 50]
        status -> Varchar,
        priority -> Int4,
        attempts -> Int4,
        max_attempts -> Int4,
        last_attempt_at -> Nullable<Timestamptz>,
        next_attempt_at -> Nullable<Timestamptz>,
        #[max_length = 100]
        locked_by -> Nullable<Varchar>,
        locked_at -> Nullable<Timestamptz>,
        error_message -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        scheduled_for -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    data_points (id) {
        id -> Uuid,
        series_id -> Uuid,
        date -> Date,
        value -> Nullable<Numeric>,
        revision_date -> Date,
        is_original_release -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    data_sources (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 500]
        base_url -> Varchar,
        api_key_required -> Bool,
        rate_limit_per_minute -> Int4,
        is_visible -> Bool,
        is_enabled -> Bool,
        requires_admin_approval -> Bool,
        crawl_frequency_hours -> Int4,
        last_crawl_at -> Nullable<Timestamptz>,
        #[max_length = 50]
        crawl_status -> Nullable<Varchar>,
        crawl_error_message -> Nullable<Text>,
        #[max_length = 500]
        api_documentation_url -> Nullable<Varchar>,
        #[max_length = 255]
        api_key_name -> Nullable<Varchar>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    economic_series (id) {
        id -> Uuid,
        source_id -> Uuid,
        #[max_length = 255]
        external_id -> Varchar,
        #[max_length = 500]
        title -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 100]
        units -> Nullable<Varchar>,
        #[max_length = 50]
        frequency -> Varchar,
        #[max_length = 100]
        seasonal_adjustment -> Nullable<Varchar>,
        last_updated -> Nullable<Timestamptz>,
        start_date -> Nullable<Date>,
        end_date -> Nullable<Date>,
        is_active -> Bool,
        first_discovered_at -> Nullable<Timestamptz>,
        last_crawled_at -> Nullable<Timestamptz>,
        first_missing_date -> Nullable<Date>,
        #[max_length = 50]
        crawl_status -> Nullable<Varchar>,
        crawl_error_message -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    event_country_impacts (id) {
        id -> Uuid,
        event_id -> Uuid,
        country_id -> Uuid,
        impact_score -> Numeric,
        impact_description -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::AnnotationType;
    use super::sql_types::AnnotationStatus;

    financial_annotations (id) {
        id -> Uuid,
        statement_id -> Uuid,
        line_item_id -> Nullable<Uuid>,
        annotation_type -> AnnotationType,
        #[max_length = 255]
        title -> Varchar,
        content -> Text,
        status -> AnnotationStatus,
        created_by -> Uuid,
        assigned_to -> Nullable<Uuid>,
        priority -> Nullable<Int4>,
        tags -> Nullable<Array<Nullable<Text>>>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::StatementType;
    use super::sql_types::StatementSection;

    financial_line_items (id) {
        id -> Uuid,
        statement_id -> Uuid,
        #[max_length = 255]
        taxonomy_concept -> Varchar,
        #[max_length = 255]
        standard_label -> Nullable<Varchar>,
        #[max_length = 255]
        custom_label -> Nullable<Varchar>,
        value -> Nullable<Numeric>,
        #[max_length = 20]
        unit -> Varchar,
        #[max_length = 100]
        context_ref -> Varchar,
        #[max_length = 100]
        segment_ref -> Nullable<Varchar>,
        #[max_length = 100]
        scenario_ref -> Nullable<Varchar>,
        precision -> Nullable<Int4>,
        decimals -> Nullable<Int4>,
        statement_type -> StatementType,
        statement_section -> StatementSection,
        #[max_length = 255]
        parent_concept -> Nullable<Varchar>,
        level -> Int4,
        order_index -> Nullable<Int4>,
        is_calculated -> Bool,
        calculation_formula -> Nullable<Text>,
        is_credit -> Nullable<Bool>,
        is_debit -> Nullable<Bool>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::RatioCategory;
    use super::sql_types::CalculationMethod;

    financial_ratios (id) {
        id -> Uuid,
        statement_id -> Uuid,
        ratio_category -> RatioCategory,
        #[max_length = 100]
        ratio_name -> Varchar,
        ratio_value -> Nullable<Numeric>,
        ratio_formula -> Nullable<Text>,
        numerator_value -> Nullable<Numeric>,
        denominator_value -> Nullable<Numeric>,
        industry_average -> Nullable<Numeric>,
        sector_average -> Nullable<Numeric>,
        peer_median -> Nullable<Numeric>,
        calculation_method -> CalculationMethod,
        confidence_score -> Nullable<Numeric>,
        data_quality_score -> Nullable<Numeric>,
        calculated_at -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ProcessingStatus;
    use super::sql_types::CompressionType;

    financial_statements (id) {
        id -> Uuid,
        company_id -> Uuid,
        #[max_length = 10]
        filing_type -> Varchar,
        #[max_length = 10]
        form_type -> Varchar,
        #[max_length = 20]
        accession_number -> Varchar,
        filing_date -> Date,
        period_end_date -> Date,
        fiscal_year -> Int4,
        fiscal_quarter -> Nullable<Int4>,
        #[max_length = 20]
        document_type -> Varchar,
        document_url -> Text,
        xbrl_processing_status -> ProcessingStatus,
        is_amended -> Bool,
        is_restated -> Bool,
        #[max_length = 50]
        amendment_type -> Nullable<Varchar>,
        original_filing_date -> Nullable<Date>,
        restatement_reason -> Nullable<Text>,
        xbrl_file_oid -> Nullable<Oid>,
        xbrl_file_content -> Nullable<Bytea>,
        xbrl_file_size_bytes -> Nullable<Int8>,
        xbrl_file_compressed -> Bool,
        xbrl_file_compression_type -> CompressionType,
        #[max_length = 64]
        xbrl_file_hash -> Nullable<Varchar>,
        xbrl_processing_error -> Nullable<Text>,
        xbrl_processing_started_at -> Nullable<Timestamptz>,
        xbrl_processing_completed_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    global_economic_events (id) {
        id -> Uuid,
        #[max_length = 500]
        name -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 50]
        event_type -> Varchar,
        #[max_length = 20]
        severity -> Varchar,
        start_date -> Date,
        end_date -> Nullable<Date>,
        primary_country_id -> Nullable<Uuid>,
        affected_regions -> Nullable<Array<Nullable<Text>>>,
        economic_impact_score -> Nullable<Numeric>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    global_economic_indicators (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 100]
        category -> Varchar,
        #[max_length = 100]
        units -> Nullable<Varchar>,
        #[max_length = 255]
        source -> Nullable<Varchar>,
        #[max_length = 50]
        frequency -> Nullable<Varchar>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    global_indicator_data (id) {
        id -> Uuid,
        indicator_id -> Uuid,
        country_id -> Uuid,
        date -> Date,
        value -> Nullable<Numeric>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    leading_indicators (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        calculation_method -> Nullable<Text>,
        data_sources -> Nullable<Array<Nullable<Text>>>,
        #[max_length = 50]
        update_frequency -> Nullable<Varchar>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    security_events (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        #[max_length = 100]
        event_type -> Varchar,
        #[max_length = 50]
        severity -> Varchar,
        description -> Nullable<Text>,
        ip_address -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        resolved_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    series_metadata (id) {
        id -> Uuid,
        series_id -> Uuid,
        #[max_length = 255]
        source_name -> Varchar,
        #[max_length = 500]
        source_url -> Nullable<Varchar>,
        #[max_length = 100]
        data_type -> Nullable<Varchar>,
        #[max_length = 255]
        geographic_area -> Nullable<Varchar>,
        #[max_length = 100]
        geographic_type -> Nullable<Varchar>,
        #[max_length = 100]
        seasonal_adjustment_type -> Nullable<Varchar>,
        #[max_length = 100]
        base_period -> Nullable<Varchar>,
        #[max_length = 50]
        units_short -> Nullable<Varchar>,
        #[max_length = 100]
        units_label -> Nullable<Varchar>,
        observation_start -> Nullable<Date>,
        observation_end -> Nullable<Date>,
        #[max_length = 10]
        frequency_short -> Nullable<Varchar>,
        #[max_length = 50]
        frequency_long -> Nullable<Varchar>,
        notes -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    template_annotation_assignments (id) {
        id -> Uuid,
        template_id -> Uuid,
        #[max_length = 255]
        chart_id -> Varchar,
        assigned_by -> Uuid,
        assigned_to -> Uuid,
        assigned_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    trade_relationships (id) {
        id -> Uuid,
        exporter_country_id -> Uuid,
        importer_country_id -> Uuid,
        trade_value_usd -> Int8,
        #[max_length = 100]
        trade_volume_units -> Nullable<Varchar>,
        trade_year -> Int4,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    user_data_source_preferences (id) {
        id -> Uuid,
        user_id -> Uuid,
        data_source_id -> Uuid,
        is_favorite -> Bool,
        notification_enabled -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    user_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        session_token -> Varchar,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
        last_used_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        #[max_length = 50]
        username -> Varchar,
        #[max_length = 255]
        email -> Varchar,
        #[max_length = 255]
        password_hash -> Varchar,
        #[max_length = 100]
        first_name -> Nullable<Varchar>,
        #[max_length = 100]
        last_name -> Nullable<Varchar>,
        #[max_length = 20]
        theme -> Varchar,
        #[max_length = 50]
        default_chart_type -> Varchar,
        notifications_enabled -> Bool,
        collaboration_enabled -> Bool,
        is_active -> Bool,
        email_verified -> Bool,
        last_login_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    xbrl_dts_dependencies (id) {
        id -> Uuid,
        parent_schema_id -> Uuid,
        child_schema_id -> Nullable<Uuid>,
        #[max_length = 255]
        child_namespace -> Varchar,
        #[max_length = 50]
        dependency_type -> Varchar,
        dependency_location -> Nullable<Text>,
        is_resolved -> Bool,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    xbrl_instance_dts_references (id) {
        id -> Uuid,
        statement_id -> Uuid,
        #[max_length = 20]
        reference_type -> Varchar,
        #[max_length = 255]
        reference_role -> Nullable<Varchar>,
        reference_href -> Text,
        #[max_length = 255]
        reference_arcrole -> Nullable<Varchar>,
        resolved_schema_id -> Nullable<Uuid>,
        resolved_linkbase_id -> Nullable<Uuid>,
        is_resolved -> Bool,
        resolution_error -> Nullable<Text>,
        created_at -> Timestamptz,
        resolved_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    xbrl_taxonomy_concepts (id) {
        id -> Uuid,
        #[max_length = 255]
        concept_name -> Varchar,
        #[max_length = 255]
        concept_qname -> Nullable<Varchar>,
        #[max_length = 255]
        concept_namespace -> Nullable<Varchar>,
        #[max_length = 255]
        concept_local_name -> Nullable<Varchar>,
        schema_id -> Nullable<Uuid>,
        is_abstract -> Nullable<Bool>,
        is_nillable -> Nullable<Bool>,
        min_occurs -> Nullable<Int4>,
        max_occurs -> Nullable<Int4>,
        #[max_length = 100]
        base_type -> Nullable<Varchar>,
        facet_constraints -> Nullable<Jsonb>,
        documentation_url -> Nullable<Text>,
        label_roles -> Nullable<Jsonb>,
        calculation_relationships -> Nullable<Jsonb>,
        presentation_relationships -> Nullable<Jsonb>,
        definition_relationships -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::TaxonomyFileType;
    use super::sql_types::CompressionType;
    use super::sql_types::ProcessingStatus;

    xbrl_taxonomy_linkbases (id) {
        id -> Uuid,
        #[max_length = 255]
        linkbase_filename -> Varchar,
        linkbase_type -> TaxonomyFileType,
        #[max_length = 255]
        target_namespace -> Nullable<Varchar>,
        schema_id -> Nullable<Uuid>,
        file_content -> Nullable<Bytea>,
        file_oid -> Nullable<Oid>,
        file_size_bytes -> Int8,
        #[max_length = 64]
        file_hash -> Varchar,
        is_compressed -> Bool,
        compression_type -> CompressionType,
        source_url -> Nullable<Text>,
        download_url -> Nullable<Text>,
        #[max_length = 255]
        original_filename -> Nullable<Varchar>,
        processing_status -> ProcessingStatus,
        processing_error -> Nullable<Text>,
        processing_started_at -> Nullable<Timestamptz>,
        processing_completed_at -> Nullable<Timestamptz>,
        relationships_extracted -> Nullable<Int4>,
        labels_extracted -> Nullable<Int4>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::TaxonomyFileType;
    use super::sql_types::TaxonomySourceType;
    use super::sql_types::CompressionType;
    use super::sql_types::ProcessingStatus;

    xbrl_taxonomy_schemas (id) {
        id -> Uuid,
        #[max_length = 255]
        schema_namespace -> Varchar,
        #[max_length = 255]
        schema_filename -> Varchar,
        #[max_length = 50]
        schema_version -> Nullable<Varchar>,
        schema_date -> Nullable<Date>,
        file_type -> TaxonomyFileType,
        source_type -> TaxonomySourceType,
        file_content -> Nullable<Bytea>,
        file_oid -> Nullable<Oid>,
        file_size_bytes -> Int8,
        #[max_length = 64]
        file_hash -> Varchar,
        is_compressed -> Bool,
        compression_type -> CompressionType,
        source_url -> Nullable<Text>,
        download_url -> Nullable<Text>,
        #[max_length = 255]
        original_filename -> Nullable<Varchar>,
        processing_status -> ProcessingStatus,
        processing_error -> Nullable<Text>,
        processing_started_at -> Nullable<Timestamptz>,
        processing_completed_at -> Nullable<Timestamptz>,
        concepts_extracted -> Nullable<Int4>,
        relationships_extracted -> Nullable<Int4>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(annotation_assignments -> financial_annotations (annotation_id));
diesel::joinable!(annotation_comments -> chart_annotations (annotation_id));
diesel::joinable!(annotation_comments -> users (user_id));
diesel::joinable!(annotation_replies -> financial_annotations (annotation_id));
diesel::joinable!(audit_logs -> users (user_id));
diesel::joinable!(chart_annotation_replies -> annotation_comments (comment_id));
diesel::joinable!(chart_annotation_replies -> users (user_id));
diesel::joinable!(chart_annotation_templates -> users (created_by));
diesel::joinable!(chart_annotations -> users (user_id));
diesel::joinable!(chart_financial_annotations -> chart_annotations (annotation_id));
diesel::joinable!(crawl_attempts -> economic_series (series_id));
diesel::joinable!(data_points -> economic_series (series_id));
diesel::joinable!(economic_series -> data_sources (source_id));
diesel::joinable!(event_country_impacts -> countries (country_id));
diesel::joinable!(event_country_impacts -> global_economic_events (event_id));
diesel::joinable!(financial_annotations -> financial_line_items (line_item_id));
diesel::joinable!(financial_annotations -> financial_statements (statement_id));
diesel::joinable!(financial_line_items -> financial_statements (statement_id));
diesel::joinable!(financial_ratios -> financial_statements (statement_id));
diesel::joinable!(financial_statements -> companies (company_id));
diesel::joinable!(global_indicator_data -> countries (country_id));
diesel::joinable!(global_indicator_data -> global_economic_indicators (indicator_id));
diesel::joinable!(security_events -> users (user_id));
diesel::joinable!(series_metadata -> economic_series (series_id));
diesel::joinable!(template_annotation_assignments -> chart_annotation_templates (template_id));
diesel::joinable!(user_data_source_preferences -> data_sources (data_source_id));
diesel::joinable!(user_data_source_preferences -> users (user_id));
diesel::joinable!(user_sessions -> users (user_id));
diesel::joinable!(xbrl_instance_dts_references -> financial_statements (statement_id));
diesel::joinable!(xbrl_instance_dts_references -> xbrl_taxonomy_linkbases (resolved_linkbase_id));
diesel::joinable!(xbrl_instance_dts_references -> xbrl_taxonomy_schemas (resolved_schema_id));
diesel::joinable!(xbrl_taxonomy_concepts -> xbrl_taxonomy_schemas (schema_id));
diesel::joinable!(xbrl_taxonomy_linkbases -> xbrl_taxonomy_schemas (schema_id));

diesel::allow_tables_to_appear_in_same_query!(
    annotation_assignments,
    annotation_comments,
    annotation_replies,
    annotation_templates,
    audit_logs,
    chart_annotation_replies,
    chart_annotation_templates,
    chart_annotations,
    chart_collaborators,
    chart_financial_annotations,
    companies,
    countries,
    country_correlations,
    crawl_attempts,
    crawl_queue,
    data_points,
    data_sources,
    economic_series,
    event_country_impacts,
    financial_annotations,
    financial_line_items,
    financial_ratios,
    financial_statements,
    global_economic_events,
    global_economic_indicators,
    global_indicator_data,
    leading_indicators,
    security_events,
    series_metadata,
    template_annotation_assignments,
    trade_relationships,
    user_data_source_preferences,
    user_sessions,
    users,
    xbrl_dts_dependencies,
    xbrl_instance_dts_references,
    xbrl_taxonomy_concepts,
    xbrl_taxonomy_linkbases,
    xbrl_taxonomy_schemas,
);
