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
        is_resolved -> Nullable<Bool>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
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
        user_id -> Uuid,
        #[max_length = 255]
        user_name -> Varchar,
        #[max_length = 100]
        action -> Varchar,
        #[max_length = 50]
        resource_type -> Varchar,
        #[max_length = 255]
        resource_id -> Nullable<Varchar>,
        ip_address -> Nullable<Text>,
        user_agent -> Nullable<Text>,
        details -> Nullable<Jsonb>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    chart_annotations (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        series_id -> Nullable<Varchar>,
        chart_id -> Nullable<Uuid>,
        annotation_date -> Date,
        annotation_value -> Nullable<Numeric>,
        #[max_length = 255]
        title -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 7]
        color -> Nullable<Varchar>,
        #[max_length = 20]
        annotation_type -> Nullable<Varchar>,
        is_visible -> Nullable<Bool>,
        is_pinned -> Nullable<Bool>,
        tags -> Nullable<Array<Nullable<Text>>>,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    chart_collaborators (id) {
        id -> Uuid,
        chart_id -> Uuid,
        user_id -> Uuid,
        invited_by -> Nullable<Uuid>,
        #[max_length = 20]
        role -> Nullable<Varchar>,
        permissions -> Nullable<Jsonb>,
        created_at -> Nullable<Timestamptz>,
        last_accessed_at -> Nullable<Timestamptz>,
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
        #[max_length = 3]
        iso_code -> Varchar,
        #[max_length = 2]
        iso_code_2 -> Varchar,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 100]
        region -> Varchar,
        #[max_length = 100]
        sub_region -> Nullable<Varchar>,
        #[max_length = 50]
        income_group -> Nullable<Varchar>,
        population -> Nullable<Int8>,
        gdp_usd -> Nullable<Numeric>,
        gdp_per_capita_usd -> Nullable<Numeric>,
        latitude -> Nullable<Numeric>,
        longitude -> Nullable<Numeric>,
        #[max_length = 3]
        currency_code -> Nullable<Varchar>,
        is_active -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    country_correlations (id) {
        id -> Uuid,
        country_a_id -> Uuid,
        country_b_id -> Uuid,
        #[max_length = 100]
        indicator_category -> Varchar,
        correlation_coefficient -> Numeric,
        time_period_start -> Date,
        time_period_end -> Date,
        sample_size -> Int4,
        p_value -> Nullable<Numeric>,
        is_significant -> Bool,
        calculated_at -> Timestamptz,
    }
}

diesel::table! {
    crawl_attempts (id) {
        id -> Uuid,
        series_id -> Uuid,
        attempted_at -> Timestamptz,
        completed_at -> Nullable<Timestamptz>,
        #[max_length = 50]
        crawl_method -> Varchar,
        crawl_url -> Nullable<Text>,
        http_status_code -> Nullable<Int4>,
        data_found -> Bool,
        new_data_points -> Nullable<Int4>,
        latest_data_date -> Nullable<Date>,
        data_freshness_hours -> Nullable<Int4>,
        success -> Bool,
        #[max_length = 50]
        error_type -> Nullable<Varchar>,
        error_message -> Nullable<Text>,
        retry_count -> Nullable<Int4>,
        response_time_ms -> Nullable<Int4>,
        data_size_bytes -> Nullable<Int4>,
        rate_limit_remaining -> Nullable<Int4>,
        user_agent -> Nullable<Text>,
        request_headers -> Nullable<Jsonb>,
        response_headers -> Nullable<Jsonb>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    crawl_queue (id) {
        id -> Uuid,
        #[max_length = 50]
        source -> Varchar,
        #[max_length = 255]
        series_id -> Varchar,
        priority -> Int4,
        #[max_length = 20]
        status -> Varchar,
        retry_count -> Int4,
        max_retries -> Int4,
        error_message -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        scheduled_for -> Nullable<Timestamptz>,
        #[max_length = 100]
        locked_by -> Nullable<Varchar>,
        locked_at -> Nullable<Timestamptz>,
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
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        first_discovered_at -> Nullable<Timestamptz>,
        last_crawled_at -> Nullable<Timestamptz>,
        first_missing_date -> Nullable<Date>,
        #[max_length = 50]
        crawl_status -> Nullable<Varchar>,
        crawl_error_message -> Nullable<Text>,
    }
}

diesel::table! {
    event_country_impacts (id) {
        id -> Uuid,
        event_id -> Uuid,
        country_id -> Uuid,
        #[max_length = 50]
        impact_type -> Varchar,
        impact_magnitude -> Nullable<Numeric>,
        impact_duration_days -> Nullable<Int4>,
        recovery_time_days -> Nullable<Int4>,
        confidence_score -> Nullable<Numeric>,
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
        country_id -> Uuid,
        #[max_length = 50]
        indicator_code -> Varchar,
        #[max_length = 500]
        indicator_name -> Varchar,
        #[max_length = 100]
        category -> Varchar,
        #[max_length = 100]
        subcategory -> Nullable<Varchar>,
        #[max_length = 50]
        unit -> Nullable<Varchar>,
        #[max_length = 20]
        frequency -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    global_indicator_data (id) {
        id -> Uuid,
        indicator_id -> Uuid,
        date -> Date,
        value -> Nullable<Numeric>,
        is_preliminary -> Bool,
        #[max_length = 50]
        data_source -> Varchar,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    leading_indicators (id) {
        id -> Uuid,
        leading_country_id -> Uuid,
        following_country_id -> Uuid,
        #[max_length = 100]
        indicator_category -> Varchar,
        lead_time_months -> Int4,
        correlation_strength -> Numeric,
        predictive_accuracy -> Nullable<Numeric>,
        time_period_start -> Date,
        time_period_end -> Date,
        calculated_at -> Timestamptz,
    }
}

diesel::table! {
    security_events (id) {
        id -> Uuid,
        #[max_length = 50]
        event_type -> Varchar,
        user_id -> Nullable<Uuid>,
        #[max_length = 255]
        user_email -> Nullable<Varchar>,
        #[max_length = 20]
        severity -> Varchar,
        ip_address -> Nullable<Text>,
        user_agent -> Nullable<Text>,
        description -> Text,
        metadata -> Nullable<Jsonb>,
        resolved -> Nullable<Bool>,
        resolved_by -> Nullable<Uuid>,
        resolved_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    series_metadata (id) {
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
        frequency -> Nullable<Varchar>,
        #[max_length = 100]
        geographic_level -> Nullable<Varchar>,
        data_url -> Nullable<Text>,
        api_endpoint -> Nullable<Text>,
        last_discovered_at -> Nullable<Timestamptz>,
        is_active -> Bool,
        created_at -> Nullable<Timestamptz>,
        updated_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    trade_relationships (id) {
        id -> Uuid,
        exporter_country_id -> Uuid,
        importer_country_id -> Uuid,
        #[max_length = 20]
        trade_flow_type -> Varchar,
        year -> Int4,
        export_value_usd -> Nullable<Numeric>,
        import_value_usd -> Nullable<Numeric>,
        trade_balance_usd -> Nullable<Numeric>,
        trade_intensity -> Nullable<Numeric>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    user_data_source_preferences (id) {
        id -> Uuid,
        user_id -> Uuid,
        data_source_id -> Uuid,
        is_visible -> Bool,
        is_favorite -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    user_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 255]
        token_hash -> Varchar,
        expires_at -> Timestamptz,
        created_at -> Timestamptz,
        last_used_at -> Timestamptz,
        user_agent -> Nullable<Text>,
        ip_address -> Nullable<Text>,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        #[max_length = 255]
        email -> Varchar,
        #[max_length = 255]
        name -> Varchar,
        avatar_url -> Nullable<Text>,
        #[max_length = 50]
        provider -> Varchar,
        #[max_length = 255]
        provider_id -> Nullable<Varchar>,
        #[max_length = 255]
        password_hash -> Nullable<Varchar>,
        #[max_length = 50]
        role -> Varchar,
        #[max_length = 255]
        organization -> Nullable<Varchar>,
        #[max_length = 20]
        theme -> Varchar,
        #[max_length = 50]
        default_chart_type -> Varchar,
        notifications_enabled -> Bool,
        collaboration_enabled -> Bool,
        is_active -> Bool,
        email_verified -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        last_login_at -> Nullable<Timestamptz>,
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
diesel::joinable!(chart_annotations -> users (user_id));
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
diesel::joinable!(global_economic_events -> countries (primary_country_id));
diesel::joinable!(global_economic_indicators -> countries (country_id));
diesel::joinable!(global_indicator_data -> global_economic_indicators (indicator_id));
diesel::joinable!(series_metadata -> data_sources (source_id));
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
    chart_annotations,
    chart_collaborators,
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
