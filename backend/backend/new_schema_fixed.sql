--
-- PostgreSQL database dump
--

\restrict kJUmXtBgdI2lPY1oJfmhhVm4U4MCmXSeXN0iNcpgPAs8fAYKdtcEmqCiuP6czxc

-- Dumped from database version 17.6 (Debian 17.6-1.pgdg13+1)
-- Dumped by pg_dump version 17.6 (Debian 17.6-1.pgdg13+1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;


--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';


--
-- Name: annotation_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.annotation_status AS ENUM (
    'active',
    'resolved',
    'archived'
);


--
-- Name: annotation_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.annotation_type AS ENUM (
    'comment',
    'question',
    'concern',
    'insight',
    'risk',
    'opportunity',
    'highlight'
);


--
-- Name: assignment_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.assignment_status AS ENUM (
    'pending',
    'in_progress',
    'completed',
    'overdue',
    'cancelled'
);


--
-- Name: assignment_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.assignment_type AS ENUM (
    'review',
    'analyze',
    'verify',
    'approve',
    'investigate'
);


--
-- Name: balance_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.balance_type AS ENUM (
    'debit',
    'credit'
);


--
-- Name: calculation_method; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.calculation_method AS ENUM (
    'simple',
    'weighted_average',
    'geometric_mean',
    'median'
);


--
-- Name: comparison_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.comparison_type AS ENUM (
    'industry',
    'sector',
    'size',
    'geographic',
    'custom'
);


--
-- Name: compression_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.compression_type AS ENUM (
    'zstd',
    'lz4',
    'gzip',
    'none'
);


--
-- Name: period_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.period_type AS ENUM (
    'duration',
    'instant'
);


--
-- Name: processing_status; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.processing_status AS ENUM (
    'pending',
    'downloaded',
    'processing',
    'completed',
    'failed'
);


--
-- Name: processing_step; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.processing_step AS ENUM (
    'download',
    'parse',
    'validate',
    'store',
    'extract',
    'calculate'
);


--
-- Name: ratio_category; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.ratio_category AS ENUM (
    'profitability',
    'liquidity',
    'leverage',
    'efficiency',
    'market',
    'growth'
);


--
-- Name: statement_section; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.statement_section AS ENUM (
    'revenue',
    'expenses',
    'assets',
    'liabilities',
    'equity',
    'operating',
    'investing',
    'financing'
);


--
-- Name: statement_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.statement_type AS ENUM (
    'income_statement',
    'balance_sheet',
    'cash_flow',
    'equity'
);


--
-- Name: substitution_group; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.substitution_group AS ENUM (
    'item',
    'tuple'
);


--
-- Name: taxonomy_file_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.taxonomy_file_type AS ENUM (
    'schema',
    'label_linkbase',
    'presentation_linkbase',
    'calculation_linkbase',
    'definition_linkbase',
    'reference_linkbase',
    'formula_linkbase'
);


--
-- Name: taxonomy_source_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.taxonomy_source_type AS ENUM (
    'company_specific',
    'us_gaap',
    'sec_dei',
    'fasb_srt',
    'ifrs',
    'other_standard',
    'custom'
);


--
-- Name: xbrl_data_type; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.xbrl_data_type AS ENUM (
    'monetaryItemType',
    'sharesItemType',
    'stringItemType',
    'decimalItemType',
    'integerItemType',
    'booleanItemType',
    'dateItemType',
    'timeItemType'
);


--
-- Name: diesel_manage_updated_at(regclass); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.diesel_manage_updated_at(_tbl regclass) RETURNS void
    LANGUAGE plpgsql
    AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_updated_at BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE diesel_set_updated_at()', _tbl);
END;
$$;


--
-- Name: diesel_set_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.diesel_set_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := current_timestamp;
    END IF;
    RETURN NEW;
END;
$$;


--
-- Name: update_series_metadata_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_series_metadata_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: update_updated_at_column(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_updated_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: __diesel_schema_migrations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.__diesel_schema_migrations (
    version character varying(50) NOT NULL,
    run_on timestamp without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


--
-- Name: annotation_assignments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.annotation_assignments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    annotation_id uuid NOT NULL,
    assigned_to uuid NOT NULL,
    assigned_by uuid NOT NULL,
    assignment_type public.assignment_type NOT NULL,
    status public.assignment_status DEFAULT 'pending'::public.assignment_status NOT NULL,
    due_date timestamp with time zone,
    instructions text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone
);


--
-- Name: annotation_comments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.annotation_comments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    annotation_id uuid NOT NULL,
    user_id uuid NOT NULL,
    content text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: annotation_replies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.annotation_replies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    annotation_id uuid NOT NULL,
    parent_reply_id uuid,
    content text NOT NULL,
    status public.annotation_status DEFAULT 'active'::public.annotation_status NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: annotation_templates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.annotation_templates (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    annotation_type public.annotation_type NOT NULL,
    template_content text NOT NULL,
    usage_count integer DEFAULT 0,
    is_active boolean DEFAULT true NOT NULL,
    created_by uuid NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: audit_logs; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.audit_logs (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    action character varying(100) NOT NULL,
    resource_type character varying(100) NOT NULL,
    resource_id uuid,
    details jsonb,
    ip_address inet,
    user_agent text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: chart_annotation_replies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chart_annotation_replies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    comment_id uuid NOT NULL,
    user_id uuid NOT NULL,
    content text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: chart_annotation_templates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chart_annotation_templates (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    template_content text NOT NULL,
    created_by uuid NOT NULL,
    is_public boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: chart_annotations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chart_annotations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    chart_id character varying(255) NOT NULL,
    user_id uuid NOT NULL,
    annotation_type character varying(50) NOT NULL,
    content text NOT NULL,
    position_x numeric(10,2),
    position_y numeric(10,2),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: chart_collaborators; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chart_collaborators (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    chart_id character varying(255) NOT NULL,
    permission_level character varying(50) DEFAULT 'view'::character varying NOT NULL,
    invited_by uuid,
    invited_at timestamp with time zone DEFAULT now() NOT NULL,
    accepted_at timestamp with time zone
);


--
-- Name: chart_financial_annotations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chart_financial_annotations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    annotation_id uuid NOT NULL,
    financial_metric character varying(100) NOT NULL,
    metric_value numeric(20,6),
    confidence_level numeric(3,2),
    calculation_method text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: companies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.companies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    cik character varying(10) NOT NULL,
    ticker character varying(10),
    name character varying(255) NOT NULL,
    legal_name character varying(500),
    sic_code character varying(4),
    sic_description character varying(255),
    industry character varying(100),
    sector character varying(100),
    business_address jsonb,
    mailing_address jsonb,
    phone character varying(50),
    website character varying(255),
    state_of_incorporation character varying(2),
    state_of_incorporation_description character varying(100),
    fiscal_year_end character varying(4),
    entity_type character varying(50),
    entity_size character varying(20),
    is_active boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: financial_statements; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.financial_statements (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    company_id uuid NOT NULL,
    filing_type character varying(10) NOT NULL,
    form_type character varying(10) NOT NULL,
    accession_number character varying(20) NOT NULL,
    filing_date date NOT NULL,
    period_end_date date NOT NULL,
    fiscal_year integer NOT NULL,
    fiscal_quarter integer,
    document_type character varying(20) DEFAULT 'XBRL'::character varying NOT NULL,
    document_url text NOT NULL,
    xbrl_processing_status public.processing_status DEFAULT 'pending'::public.processing_status NOT NULL,
    is_amended boolean DEFAULT false NOT NULL,
    is_restated boolean DEFAULT false NOT NULL,
    amendment_type character varying(50),
    original_filing_date date,
    restatement_reason text,
    xbrl_file_oid oid,
    xbrl_file_content bytea,
    xbrl_file_size_bytes bigint,
    xbrl_file_compressed boolean DEFAULT true NOT NULL,
    xbrl_file_compression_type public.compression_type DEFAULT 'zstd'::public.compression_type NOT NULL,
    xbrl_file_hash character varying(64),
    xbrl_processing_error text,
    xbrl_processing_started_at timestamp with time zone,
    xbrl_processing_completed_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT valid_file_storage CHECK ((((xbrl_file_content IS NOT NULL) AND (xbrl_file_oid IS NULL)) OR ((xbrl_file_content IS NULL) AND (xbrl_file_oid IS NOT NULL)) OR ((xbrl_file_content IS NULL) AND (xbrl_file_oid IS NULL))))
);


--
-- Name: company_financial_statements; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.company_financial_statements AS
 SELECT fs.id,
    fs.company_id,
    c.name AS company_name,
    c.ticker,
    fs.filing_type,
    fs.period_end_date,
    fs.fiscal_year,
    fs.fiscal_quarter,
    fs.xbrl_processing_status,
    fs.is_amended,
    fs.is_restated,
    fs.created_at
   FROM (public.financial_statements fs
     JOIN public.companies c ON ((fs.company_id = c.id)))
  WHERE (c.is_active = true);


--
-- Name: countries; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.countries (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    iso_code_2 character(2) NOT NULL,
    iso_code_3 character(3) NOT NULL,
    name character varying(255) NOT NULL,
    region character varying(100),
    subregion character varying(100),
    population bigint,
    gdp_usd bigint,
    currency_code character(3),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: country_correlations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.country_correlations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    country_1_id uuid NOT NULL,
    country_2_id uuid NOT NULL,
    correlation_coefficient numeric(5,4) NOT NULL,
    calculation_period_start date NOT NULL,
    calculation_period_end date NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: crawl_attempts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crawl_attempts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    series_id uuid NOT NULL,
    attempt_number integer NOT NULL,
    status character varying(50) NOT NULL,
    started_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone,
    error_message text,
    data_points_retrieved integer DEFAULT 0,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: crawl_queue; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crawl_queue (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    source character varying(50) NOT NULL,
    series_id character varying(255) NOT NULL,
    status character varying(50) DEFAULT 'pending'::character varying NOT NULL,
    priority integer DEFAULT 5 NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    max_attempts integer DEFAULT 3 NOT NULL,
    last_attempt_at timestamp with time zone,
    next_attempt_at timestamp with time zone,
    locked_by character varying(100),
    locked_at timestamp with time zone,
    error_message text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    scheduled_for timestamp with time zone,
    CONSTRAINT check_crawl_queue_lock_consistency CHECK ((((locked_by IS NULL) AND (locked_at IS NULL)) OR ((locked_by IS NOT NULL) AND (locked_at IS NOT NULL)))),
    CONSTRAINT check_crawl_queue_priority CHECK (((priority >= 1) AND (priority <= 10))),
    CONSTRAINT check_crawl_queue_status CHECK (((status)::text = ANY ((ARRAY['pending'::character varying, 'processing'::character varying, 'completed'::character varying, 'failed'::character varying, 'retrying'::character varying, 'cancelled'::character varying])::text[])))
);


--
-- Name: data_points; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.data_points (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    series_id uuid NOT NULL,
    date date NOT NULL,
    value numeric(20,6),
    revision_date date NOT NULL,
    is_original_release boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: data_sources; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.data_sources (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    base_url character varying(500) NOT NULL,
    api_key_required boolean DEFAULT false NOT NULL,
    rate_limit_per_minute integer DEFAULT 60 NOT NULL,
    is_visible boolean DEFAULT true NOT NULL,
    is_enabled boolean DEFAULT true NOT NULL,
    requires_admin_approval boolean DEFAULT false NOT NULL,
    crawl_frequency_hours integer DEFAULT 24 NOT NULL,
    last_crawl_at timestamp with time zone,
    crawl_status character varying(50) DEFAULT 'pending'::character varying,
    crawl_error_message text,
    api_documentation_url character varying(500),
    api_key_name character varying(255),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: economic_series; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.economic_series (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    source_id uuid NOT NULL,
    external_id character varying(255) NOT NULL,
    title character varying(500) NOT NULL,
    description text,
    units character varying(100),
    frequency character varying(50) NOT NULL,
    seasonal_adjustment character varying(100),
    last_updated timestamp with time zone,
    start_date date,
    end_date date,
    is_active boolean DEFAULT true NOT NULL,
    first_discovered_at timestamp with time zone,
    last_crawled_at timestamp with time zone,
    first_missing_date date,
    crawl_status character varying(50),
    crawl_error_message text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: event_country_impacts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.event_country_impacts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    event_id uuid NOT NULL,
    country_id uuid NOT NULL,
    impact_score numeric(3,2) NOT NULL,
    impact_description text,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: financial_annotations; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.financial_annotations (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    statement_id uuid NOT NULL,
    line_item_id uuid,
    annotation_type public.annotation_type NOT NULL,
    title character varying(255) NOT NULL,
    content text NOT NULL,
    status public.annotation_status DEFAULT 'active'::public.annotation_status NOT NULL,
    created_by uuid NOT NULL,
    assigned_to uuid,
    priority integer DEFAULT 1,
    tags text[],
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    resolved_at timestamp with time zone
);


--
-- Name: financial_line_items; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.financial_line_items (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    statement_id uuid NOT NULL,
    taxonomy_concept character varying(255) NOT NULL,
    standard_label character varying(255),
    custom_label character varying(255),
    value numeric(20,6),
    unit character varying(20) DEFAULT 'USD'::character varying NOT NULL,
    context_ref character varying(100) NOT NULL,
    segment_ref character varying(100),
    scenario_ref character varying(100),
    "precision" integer,
    decimals integer,
    statement_type public.statement_type NOT NULL,
    statement_section public.statement_section NOT NULL,
    parent_concept character varying(255),
    level integer DEFAULT 1 NOT NULL,
    order_index integer,
    is_calculated boolean DEFAULT false NOT NULL,
    calculation_formula text,
    is_credit boolean,
    is_debit boolean,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: financial_line_items_with_context; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.financial_line_items_with_context AS
 SELECT fli.id,
    fli.statement_id,
    fli.taxonomy_concept,
    fli.standard_label,
    fli.custom_label,
    fli.value,
    fli.unit,
    fli.statement_type,
    fli.statement_section,
    fli.parent_concept,
    fli.level,
    fli.order_index,
    fli.is_calculated,
    fs.company_id,
    c.name AS company_name,
    fs.period_end_date,
    fs.fiscal_year
   FROM ((public.financial_line_items fli
     JOIN public.financial_statements fs ON ((fli.statement_id = fs.id)))
     JOIN public.companies c ON ((fs.company_id = c.id)))
  WHERE (c.is_active = true);


--
-- Name: financial_ratios; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.financial_ratios (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    statement_id uuid NOT NULL,
    ratio_category public.ratio_category NOT NULL,
    ratio_name character varying(100) NOT NULL,
    ratio_value numeric(10,6),
    ratio_formula text,
    numerator_value numeric(20,6),
    denominator_value numeric(20,6),
    industry_average numeric(10,6),
    sector_average numeric(10,6),
    peer_median numeric(10,6),
    calculation_method public.calculation_method DEFAULT 'simple'::public.calculation_method NOT NULL,
    confidence_score numeric(3,2),
    data_quality_score numeric(3,2),
    calculated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: financial_ratios_with_context; Type: VIEW; Schema: public; Owner: -
--

CREATE VIEW public.financial_ratios_with_context AS
 SELECT fr.id,
    fr.statement_id,
    fr.ratio_category,
    fr.ratio_name,
    fr.ratio_value,
    fr.industry_average,
    fr.sector_average,
    fr.peer_median,
    fr.confidence_score,
    fr.data_quality_score,
    fr.calculated_at,
    fs.company_id,
    c.name AS company_name,
    fs.period_end_date,
    fs.fiscal_year
   FROM ((public.financial_ratios fr
     JOIN public.financial_statements fs ON ((fr.statement_id = fs.id)))
     JOIN public.companies c ON ((fs.company_id = c.id)))
  WHERE (c.is_active = true);


--
-- Name: global_economic_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.global_economic_events (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(500) NOT NULL,
    description text,
    event_type character varying(50) NOT NULL,
    severity character varying(20) NOT NULL,
    start_date date NOT NULL,
    end_date date,
    primary_country_id uuid,
    affected_regions text[],
    economic_impact_score numeric(5,2),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: global_economic_indicators; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.global_economic_indicators (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    category character varying(100) NOT NULL,
    units character varying(100),
    source character varying(255),
    frequency character varying(50),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: global_indicator_data; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.global_indicator_data (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    indicator_id uuid NOT NULL,
    country_id uuid NOT NULL,
    date date NOT NULL,
    value numeric(20,6),
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: leading_indicators; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.leading_indicators (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL,
    description text,
    calculation_method text,
    data_sources text[],
    update_frequency character varying(50),
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: security_events; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.security_events (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid,
    event_type character varying(100) NOT NULL,
    severity character varying(50) DEFAULT 'medium'::character varying NOT NULL,
    description text,
    ip_address inet,
    user_agent text,
    resolved_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: series_metadata; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.series_metadata (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    series_id uuid NOT NULL,
    source_name character varying(255) NOT NULL,
    source_url character varying(500),
    data_type character varying(100),
    geographic_area character varying(255),
    geographic_type character varying(100),
    seasonal_adjustment_type character varying(100),
    base_period character varying(100),
    units_short character varying(50),
    units_label character varying(100),
    observation_start date,
    observation_end date,
    frequency_short character varying(10),
    frequency_long character varying(50),
    notes text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: template_annotation_assignments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.template_annotation_assignments (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    template_id uuid NOT NULL,
    chart_id character varying(255) NOT NULL,
    assigned_by uuid NOT NULL,
    assigned_to uuid NOT NULL,
    assigned_at timestamp with time zone DEFAULT now() NOT NULL,
    completed_at timestamp with time zone
);


--
-- Name: trade_relationships; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.trade_relationships (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    exporter_country_id uuid NOT NULL,
    importer_country_id uuid NOT NULL,
    trade_value_usd bigint NOT NULL,
    trade_volume_units character varying(100),
    trade_year integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: user_data_source_preferences; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_data_source_preferences (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    data_source_id uuid NOT NULL,
    is_favorite boolean DEFAULT false NOT NULL,
    notification_enabled boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: user_sessions; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    session_token character varying(255) NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    username character varying(50) NOT NULL,
    email character varying(255) NOT NULL,
    password_hash character varying(255) NOT NULL,
    first_name character varying(100),
    last_name character varying(100),
    theme character varying(20) DEFAULT 'light'::character varying NOT NULL,
    default_chart_type character varying(50) DEFAULT 'line'::character varying NOT NULL,
    notifications_enabled boolean DEFAULT true NOT NULL,
    collaboration_enabled boolean DEFAULT true NOT NULL,
    is_active boolean DEFAULT true NOT NULL,
    email_verified boolean DEFAULT false NOT NULL,
    last_login_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: xbrl_dts_dependencies; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.xbrl_dts_dependencies (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    parent_schema_id uuid NOT NULL,
    child_schema_id uuid,
    child_namespace character varying(255) NOT NULL,
    dependency_type character varying(50) NOT NULL,
    dependency_location text,
    is_resolved boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT valid_dependency CHECK ((((is_resolved = true) AND (child_schema_id IS NOT NULL)) OR ((is_resolved = false) AND (child_schema_id IS NULL))))
);


--
-- Name: xbrl_instance_dts_references; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.xbrl_instance_dts_references (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    statement_id uuid NOT NULL,
    reference_type character varying(20) NOT NULL,
    reference_role character varying(255),
    reference_href text NOT NULL,
    reference_arcrole character varying(255),
    resolved_schema_id uuid,
    resolved_linkbase_id uuid,
    is_resolved boolean DEFAULT false NOT NULL,
    resolution_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    resolved_at timestamp with time zone,
    CONSTRAINT valid_resolution CHECK ((((is_resolved = true) AND ((resolved_schema_id IS NOT NULL) OR (resolved_linkbase_id IS NOT NULL))) OR ((is_resolved = false) AND (resolved_schema_id IS NULL) AND (resolved_linkbase_id IS NULL))))
);


--
-- Name: xbrl_taxonomy_concepts; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.xbrl_taxonomy_concepts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    concept_name character varying(255) NOT NULL,
    concept_qname character varying(255),
    concept_namespace character varying(255),
    concept_local_name character varying(255),
    schema_id uuid,
    is_abstract boolean DEFAULT false,
    is_nillable boolean DEFAULT true,
    min_occurs integer DEFAULT 1,
    max_occurs integer DEFAULT 1,
    base_type character varying(100),
    facet_constraints jsonb,
    documentation_url text,
    label_roles jsonb,
    calculation_relationships jsonb,
    presentation_relationships jsonb,
    definition_relationships jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: xbrl_taxonomy_linkbases; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.xbrl_taxonomy_linkbases (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    linkbase_filename character varying(255) NOT NULL,
    linkbase_type public.taxonomy_file_type NOT NULL,
    target_namespace character varying(255),
    schema_id uuid,
    file_content bytea,
    file_oid oid,
    file_size_bytes bigint NOT NULL,
    file_hash character varying(64) NOT NULL,
    is_compressed boolean DEFAULT true NOT NULL,
    compression_type public.compression_type DEFAULT 'zstd'::public.compression_type NOT NULL,
    source_url text,
    download_url text,
    original_filename character varying(255),
    processing_status public.processing_status DEFAULT 'downloaded'::public.processing_status NOT NULL,
    processing_error text,
    processing_started_at timestamp with time zone,
    processing_completed_at timestamp with time zone,
    relationships_extracted integer DEFAULT 0,
    labels_extracted integer DEFAULT 0,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT valid_linkbase_file_storage CHECK ((((file_content IS NOT NULL) AND (file_oid IS NULL)) OR ((file_content IS NULL) AND (file_oid IS NOT NULL))))
);


--
-- Name: xbrl_taxonomy_schemas; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.xbrl_taxonomy_schemas (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    schema_namespace character varying(255) NOT NULL,
    schema_filename character varying(255) NOT NULL,
    schema_version character varying(50),
    schema_date date,
    file_type public.taxonomy_file_type DEFAULT 'schema'::public.taxonomy_file_type NOT NULL,
    source_type public.taxonomy_source_type NOT NULL,
    file_content bytea,
    file_oid oid,
    file_size_bytes bigint NOT NULL,
    file_hash character varying(64) NOT NULL,
    is_compressed boolean DEFAULT true NOT NULL,
    compression_type public.compression_type DEFAULT 'zstd'::public.compression_type NOT NULL,
    source_url text,
    download_url text,
    original_filename character varying(255),
    processing_status public.processing_status DEFAULT 'downloaded'::public.processing_status NOT NULL,
    processing_error text,
    processing_started_at timestamp with time zone,
    processing_completed_at timestamp with time zone,
    concepts_extracted integer DEFAULT 0,
    relationships_extracted integer DEFAULT 0,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT valid_file_storage CHECK ((((file_content IS NOT NULL) AND (file_oid IS NULL)) OR ((file_content IS NULL) AND (file_oid IS NOT NULL))))
);


--
-- Name: __diesel_schema_migrations __diesel_schema_migrations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.__diesel_schema_migrations
    ADD CONSTRAINT __diesel_schema_migrations_pkey PRIMARY KEY (version);


--
-- Name: annotation_assignments annotation_assignments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_assignments
    ADD CONSTRAINT annotation_assignments_pkey PRIMARY KEY (id);


--
-- Name: annotation_comments annotation_comments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_comments
    ADD CONSTRAINT annotation_comments_pkey PRIMARY KEY (id);


--
-- Name: annotation_replies annotation_replies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_replies
    ADD CONSTRAINT annotation_replies_pkey PRIMARY KEY (id);


--
-- Name: annotation_templates annotation_templates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_templates
    ADD CONSTRAINT annotation_templates_pkey PRIMARY KEY (id);


--
-- Name: audit_logs audit_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_pkey PRIMARY KEY (id);


--
-- Name: chart_annotation_replies chart_annotation_replies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotation_replies
    ADD CONSTRAINT chart_annotation_replies_pkey PRIMARY KEY (id);


--
-- Name: chart_annotation_templates chart_annotation_templates_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotation_templates
    ADD CONSTRAINT chart_annotation_templates_pkey PRIMARY KEY (id);


--
-- Name: chart_annotations chart_annotations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotations
    ADD CONSTRAINT chart_annotations_pkey PRIMARY KEY (id);


--
-- Name: chart_collaborators chart_collaborators_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_collaborators
    ADD CONSTRAINT chart_collaborators_pkey PRIMARY KEY (id);


--
-- Name: chart_collaborators chart_collaborators_user_id_chart_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_collaborators
    ADD CONSTRAINT chart_collaborators_user_id_chart_id_key UNIQUE (user_id, chart_id);


--
-- Name: chart_financial_annotations chart_financial_annotations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_financial_annotations
    ADD CONSTRAINT chart_financial_annotations_pkey PRIMARY KEY (id);


--
-- Name: companies companies_cik_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.companies
    ADD CONSTRAINT companies_cik_key UNIQUE (cik);


--
-- Name: companies companies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.companies
    ADD CONSTRAINT companies_pkey PRIMARY KEY (id);


--
-- Name: countries countries_iso_code_2_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.countries
    ADD CONSTRAINT countries_iso_code_2_key UNIQUE (iso_code_2);


--
-- Name: countries countries_iso_code_3_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.countries
    ADD CONSTRAINT countries_iso_code_3_key UNIQUE (iso_code_3);


--
-- Name: countries countries_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.countries
    ADD CONSTRAINT countries_name_key UNIQUE (name);


--
-- Name: countries countries_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.countries
    ADD CONSTRAINT countries_pkey PRIMARY KEY (id);


--
-- Name: country_correlations country_correlations_country_1_id_country_2_id_calculation__key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.country_correlations
    ADD CONSTRAINT country_correlations_country_1_id_country_2_id_calculation__key UNIQUE (country_1_id, country_2_id, calculation_period_start, calculation_period_end);


--
-- Name: country_correlations country_correlations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.country_correlations
    ADD CONSTRAINT country_correlations_pkey PRIMARY KEY (id);


--
-- Name: crawl_attempts crawl_attempts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crawl_attempts
    ADD CONSTRAINT crawl_attempts_pkey PRIMARY KEY (id);


--
-- Name: crawl_queue crawl_queue_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crawl_queue
    ADD CONSTRAINT crawl_queue_pkey PRIMARY KEY (id);


--
-- Name: data_points data_points_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_points
    ADD CONSTRAINT data_points_pkey PRIMARY KEY (id);


--
-- Name: data_points data_points_series_id_date_revision_date_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_points
    ADD CONSTRAINT data_points_series_id_date_revision_date_key UNIQUE (series_id, date, revision_date);


--
-- Name: data_sources data_sources_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_sources
    ADD CONSTRAINT data_sources_name_key UNIQUE (name);


--
-- Name: data_sources data_sources_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_sources
    ADD CONSTRAINT data_sources_pkey PRIMARY KEY (id);


--
-- Name: economic_series economic_series_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.economic_series
    ADD CONSTRAINT economic_series_pkey PRIMARY KEY (id);


--
-- Name: economic_series economic_series_source_id_external_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.economic_series
    ADD CONSTRAINT economic_series_source_id_external_id_key UNIQUE (source_id, external_id);


--
-- Name: event_country_impacts event_country_impacts_event_id_country_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_country_impacts
    ADD CONSTRAINT event_country_impacts_event_id_country_id_key UNIQUE (event_id, country_id);


--
-- Name: event_country_impacts event_country_impacts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_country_impacts
    ADD CONSTRAINT event_country_impacts_pkey PRIMARY KEY (id);


--
-- Name: financial_annotations financial_annotations_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_annotations
    ADD CONSTRAINT financial_annotations_pkey PRIMARY KEY (id);


--
-- Name: financial_line_items financial_line_items_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_line_items
    ADD CONSTRAINT financial_line_items_pkey PRIMARY KEY (id);


--
-- Name: financial_ratios financial_ratios_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_ratios
    ADD CONSTRAINT financial_ratios_pkey PRIMARY KEY (id);


--
-- Name: financial_statements financial_statements_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_statements
    ADD CONSTRAINT financial_statements_pkey PRIMARY KEY (id);


--
-- Name: global_economic_events global_economic_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_economic_events
    ADD CONSTRAINT global_economic_events_pkey PRIMARY KEY (id);


--
-- Name: global_economic_indicators global_economic_indicators_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_economic_indicators
    ADD CONSTRAINT global_economic_indicators_name_key UNIQUE (name);


--
-- Name: global_economic_indicators global_economic_indicators_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_economic_indicators
    ADD CONSTRAINT global_economic_indicators_pkey PRIMARY KEY (id);


--
-- Name: global_indicator_data global_indicator_data_indicator_id_country_id_date_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_indicator_data
    ADD CONSTRAINT global_indicator_data_indicator_id_country_id_date_key UNIQUE (indicator_id, country_id, date);


--
-- Name: global_indicator_data global_indicator_data_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_indicator_data
    ADD CONSTRAINT global_indicator_data_pkey PRIMARY KEY (id);


--
-- Name: leading_indicators leading_indicators_name_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leading_indicators
    ADD CONSTRAINT leading_indicators_name_key UNIQUE (name);


--
-- Name: leading_indicators leading_indicators_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.leading_indicators
    ADD CONSTRAINT leading_indicators_pkey PRIMARY KEY (id);


--
-- Name: security_events security_events_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.security_events
    ADD CONSTRAINT security_events_pkey PRIMARY KEY (id);


--
-- Name: series_metadata series_metadata_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.series_metadata
    ADD CONSTRAINT series_metadata_pkey PRIMARY KEY (id);


--
-- Name: series_metadata series_metadata_series_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.series_metadata
    ADD CONSTRAINT series_metadata_series_id_key UNIQUE (series_id);


--
-- Name: template_annotation_assignments template_annotation_assignmen_template_id_chart_id_assigned_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.template_annotation_assignments
    ADD CONSTRAINT template_annotation_assignmen_template_id_chart_id_assigned_key UNIQUE (template_id, chart_id, assigned_to);


--
-- Name: template_annotation_assignments template_annotation_assignments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.template_annotation_assignments
    ADD CONSTRAINT template_annotation_assignments_pkey PRIMARY KEY (id);


--
-- Name: trade_relationships trade_relationships_exporter_country_id_importer_country_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_relationships
    ADD CONSTRAINT trade_relationships_exporter_country_id_importer_country_id_key UNIQUE (exporter_country_id, importer_country_id, trade_year);


--
-- Name: trade_relationships trade_relationships_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_relationships
    ADD CONSTRAINT trade_relationships_pkey PRIMARY KEY (id);


--
-- Name: crawl_queue unique_active_queue_item; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crawl_queue
    ADD CONSTRAINT unique_active_queue_item UNIQUE (source, series_id) DEFERRABLE INITIALLY DEFERRED;


--
-- Name: financial_statements unique_company_filing; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_statements
    ADD CONSTRAINT unique_company_filing UNIQUE (company_id, accession_number);


--
-- Name: xbrl_dts_dependencies unique_dependency; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_dts_dependencies
    ADD CONSTRAINT unique_dependency UNIQUE (parent_schema_id, child_namespace);


--
-- Name: xbrl_taxonomy_schemas unique_schema_namespace_version; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_schemas
    ADD CONSTRAINT unique_schema_namespace_version UNIQUE (schema_namespace, schema_version);


--
-- Name: user_data_source_preferences user_data_source_preferences_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_data_source_preferences
    ADD CONSTRAINT user_data_source_preferences_pkey PRIMARY KEY (id);


--
-- Name: user_data_source_preferences user_data_source_preferences_user_id_data_source_id_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_data_source_preferences
    ADD CONSTRAINT user_data_source_preferences_user_id_data_source_id_key UNIQUE (user_id, data_source_id);


--
-- Name: user_sessions user_sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_pkey PRIMARY KEY (id);


--
-- Name: user_sessions user_sessions_session_token_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_session_token_key UNIQUE (session_token);


--
-- Name: users users_email_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_email_key UNIQUE (email);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: users users_username_key; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_username_key UNIQUE (username);


--
-- Name: xbrl_dts_dependencies xbrl_dts_dependencies_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_dts_dependencies
    ADD CONSTRAINT xbrl_dts_dependencies_pkey PRIMARY KEY (id);


--
-- Name: xbrl_instance_dts_references xbrl_instance_dts_references_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_instance_dts_references
    ADD CONSTRAINT xbrl_instance_dts_references_pkey PRIMARY KEY (id);


--
-- Name: xbrl_taxonomy_concepts xbrl_taxonomy_concepts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_concepts
    ADD CONSTRAINT xbrl_taxonomy_concepts_pkey PRIMARY KEY (id);


--
-- Name: xbrl_taxonomy_linkbases xbrl_taxonomy_linkbases_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_linkbases
    ADD CONSTRAINT xbrl_taxonomy_linkbases_pkey PRIMARY KEY (id);


--
-- Name: xbrl_taxonomy_schemas xbrl_taxonomy_schemas_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_schemas
    ADD CONSTRAINT xbrl_taxonomy_schemas_pkey PRIMARY KEY (id);


--
-- Name: idx_annotation_assignments_annotation_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_assignments_annotation_id ON public.annotation_assignments USING btree (annotation_id);


--
-- Name: idx_annotation_assignments_assigned_to; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_assignments_assigned_to ON public.annotation_assignments USING btree (assigned_to);


--
-- Name: idx_annotation_assignments_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_assignments_status ON public.annotation_assignments USING btree (status);


--
-- Name: idx_annotation_comments_annotation_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_comments_annotation_id ON public.annotation_comments USING btree (annotation_id);


--
-- Name: idx_annotation_comments_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_comments_created_at ON public.annotation_comments USING btree (created_at);


--
-- Name: idx_annotation_comments_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_comments_user_id ON public.annotation_comments USING btree (user_id);


--
-- Name: idx_annotation_replies_annotation_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_replies_annotation_id ON public.annotation_replies USING btree (annotation_id);


--
-- Name: idx_annotation_replies_created_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_replies_created_by ON public.annotation_replies USING btree (created_by);


--
-- Name: idx_annotation_replies_parent_reply_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_replies_parent_reply_id ON public.annotation_replies USING btree (parent_reply_id);


--
-- Name: idx_annotation_templates_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_templates_active ON public.annotation_templates USING btree (is_active);


--
-- Name: idx_annotation_templates_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_annotation_templates_type ON public.annotation_templates USING btree (annotation_type);


--
-- Name: idx_audit_logs_action; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_action ON public.audit_logs USING btree (action);


--
-- Name: idx_audit_logs_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_created_at ON public.audit_logs USING btree (created_at);


--
-- Name: idx_audit_logs_details; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_details ON public.audit_logs USING gin (details);


--
-- Name: idx_audit_logs_resource_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_resource_id ON public.audit_logs USING btree (resource_id);


--
-- Name: idx_audit_logs_resource_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_resource_type ON public.audit_logs USING btree (resource_type);


--
-- Name: idx_audit_logs_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_audit_logs_user_id ON public.audit_logs USING btree (user_id);


--
-- Name: idx_chart_annotation_replies_comment_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_replies_comment_id ON public.chart_annotation_replies USING btree (comment_id);


--
-- Name: idx_chart_annotation_replies_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_replies_created_at ON public.chart_annotation_replies USING btree (created_at);


--
-- Name: idx_chart_annotation_replies_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_replies_user_id ON public.chart_annotation_replies USING btree (user_id);


--
-- Name: idx_chart_annotation_templates_created_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_templates_created_by ON public.chart_annotation_templates USING btree (created_by);


--
-- Name: idx_chart_annotation_templates_is_public; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_templates_is_public ON public.chart_annotation_templates USING btree (is_public);


--
-- Name: idx_chart_annotation_templates_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotation_templates_name ON public.chart_annotation_templates USING btree (name);


--
-- Name: idx_chart_annotations_annotation_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotations_annotation_type ON public.chart_annotations USING btree (annotation_type);


--
-- Name: idx_chart_annotations_chart_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotations_chart_id ON public.chart_annotations USING btree (chart_id);


--
-- Name: idx_chart_annotations_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotations_created_at ON public.chart_annotations USING btree (created_at);


--
-- Name: idx_chart_annotations_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_annotations_user_id ON public.chart_annotations USING btree (user_id);


--
-- Name: idx_chart_collaborators_chart_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_collaborators_chart_id ON public.chart_collaborators USING btree (chart_id);


--
-- Name: idx_chart_collaborators_invited_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_collaborators_invited_by ON public.chart_collaborators USING btree (invited_by);


--
-- Name: idx_chart_collaborators_permission_level; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_collaborators_permission_level ON public.chart_collaborators USING btree (permission_level);


--
-- Name: idx_chart_collaborators_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_collaborators_user_id ON public.chart_collaborators USING btree (user_id);


--
-- Name: idx_chart_financial_annotations_annotation_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_financial_annotations_annotation_id ON public.chart_financial_annotations USING btree (annotation_id);


--
-- Name: idx_chart_financial_annotations_confidence_level; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_financial_annotations_confidence_level ON public.chart_financial_annotations USING btree (confidence_level);


--
-- Name: idx_chart_financial_annotations_financial_metric; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_financial_annotations_financial_metric ON public.chart_financial_annotations USING btree (financial_metric);


--
-- Name: idx_chart_financial_annotations_metric_value; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_chart_financial_annotations_metric_value ON public.chart_financial_annotations USING btree (metric_value);


--
-- Name: idx_companies_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_active ON public.companies USING btree (is_active);


--
-- Name: idx_companies_cik; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_cik ON public.companies USING btree (cik);


--
-- Name: idx_companies_industry; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_industry ON public.companies USING btree (industry);


--
-- Name: idx_companies_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_name ON public.companies USING btree (name);


--
-- Name: idx_companies_sector; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_sector ON public.companies USING btree (sector);


--
-- Name: idx_companies_ticker; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_companies_ticker ON public.companies USING btree (ticker);


--
-- Name: idx_countries_iso_code_2; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_countries_iso_code_2 ON public.countries USING btree (iso_code_2);


--
-- Name: idx_countries_iso_code_3; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_countries_iso_code_3 ON public.countries USING btree (iso_code_3);


--
-- Name: idx_countries_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_countries_name ON public.countries USING btree (name);


--
-- Name: idx_countries_region; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_countries_region ON public.countries USING btree (region);


--
-- Name: idx_countries_subregion; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_countries_subregion ON public.countries USING btree (subregion);


--
-- Name: idx_country_correlations_calculation_period; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_country_correlations_calculation_period ON public.country_correlations USING btree (calculation_period_start, calculation_period_end);


--
-- Name: idx_country_correlations_correlation_coefficient; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_country_correlations_correlation_coefficient ON public.country_correlations USING btree (correlation_coefficient);


--
-- Name: idx_country_correlations_country_1_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_country_correlations_country_1_id ON public.country_correlations USING btree (country_1_id);


--
-- Name: idx_country_correlations_country_2_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_country_correlations_country_2_id ON public.country_correlations USING btree (country_2_id);


--
-- Name: idx_crawl_attempts_attempt_number; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_attempts_attempt_number ON public.crawl_attempts USING btree (attempt_number);


--
-- Name: idx_crawl_attempts_completed_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_attempts_completed_at ON public.crawl_attempts USING btree (completed_at);


--
-- Name: idx_crawl_attempts_series_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_attempts_series_id ON public.crawl_attempts USING btree (series_id);


--
-- Name: idx_crawl_attempts_started_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_attempts_started_at ON public.crawl_attempts USING btree (started_at);


--
-- Name: idx_crawl_attempts_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_attempts_status ON public.crawl_attempts USING btree (status);


--
-- Name: idx_crawl_queue_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_created_at ON public.crawl_queue USING btree (created_at);


--
-- Name: idx_crawl_queue_locked_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_locked_by ON public.crawl_queue USING btree (locked_by);


--
-- Name: idx_crawl_queue_next_attempt_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_next_attempt_at ON public.crawl_queue USING btree (next_attempt_at);


--
-- Name: idx_crawl_queue_priority; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_priority ON public.crawl_queue USING btree (priority DESC);


--
-- Name: idx_crawl_queue_processing; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_processing ON public.crawl_queue USING btree (status, priority DESC, scheduled_for, locked_by) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'retrying'::character varying])::text[]));


--
-- Name: idx_crawl_queue_scheduled_for; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_scheduled_for ON public.crawl_queue USING btree (scheduled_for);


--
-- Name: idx_crawl_queue_series_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_series_id ON public.crawl_queue USING btree (series_id);


--
-- Name: idx_crawl_queue_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_source ON public.crawl_queue USING btree (source);


--
-- Name: idx_crawl_queue_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_crawl_queue_status ON public.crawl_queue USING btree (status);


--
-- Name: idx_data_points_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_points_date ON public.data_points USING btree (date);


--
-- Name: idx_data_points_is_original_release; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_points_is_original_release ON public.data_points USING btree (is_original_release);


--
-- Name: idx_data_points_revision_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_points_revision_date ON public.data_points USING btree (revision_date);


--
-- Name: idx_data_points_series_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_points_series_id ON public.data_points USING btree (series_id);


--
-- Name: idx_data_points_value; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_points_value ON public.data_points USING btree (value);


--
-- Name: idx_data_sources_crawl_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_crawl_status ON public.data_sources USING btree (crawl_status);


--
-- Name: idx_data_sources_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_enabled ON public.data_sources USING btree (is_enabled);


--
-- Name: idx_data_sources_is_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_is_enabled ON public.data_sources USING btree (is_enabled);


--
-- Name: idx_data_sources_is_visible; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_is_visible ON public.data_sources USING btree (is_visible);


--
-- Name: idx_data_sources_last_crawl_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_last_crawl_at ON public.data_sources USING btree (last_crawl_at);


--
-- Name: idx_data_sources_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_name ON public.data_sources USING btree (name);


--
-- Name: idx_data_sources_visible; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_data_sources_visible ON public.data_sources USING btree (is_visible);


--
-- Name: idx_economic_series_crawl_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_crawl_status ON public.economic_series USING btree (crawl_status);


--
-- Name: idx_economic_series_description; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_description ON public.economic_series USING gin (to_tsvector('english'::regconfig, description));


--
-- Name: idx_economic_series_external_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_external_id ON public.economic_series USING btree (external_id);


--
-- Name: idx_economic_series_first_discovered_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_first_discovered_at ON public.economic_series USING btree (first_discovered_at);


--
-- Name: idx_economic_series_frequency; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_frequency ON public.economic_series USING btree (frequency);


--
-- Name: idx_economic_series_is_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_is_active ON public.economic_series USING btree (is_active);


--
-- Name: idx_economic_series_last_crawled_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_last_crawled_at ON public.economic_series USING btree (last_crawled_at);


--
-- Name: idx_economic_series_last_updated; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_last_updated ON public.economic_series USING btree (last_updated);


--
-- Name: idx_economic_series_source_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_source_id ON public.economic_series USING btree (source_id);


--
-- Name: idx_economic_series_title; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_economic_series_title ON public.economic_series USING gin (to_tsvector('english'::regconfig, (title)::text));


--
-- Name: idx_event_country_impacts_country_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_event_country_impacts_country_id ON public.event_country_impacts USING btree (country_id);


--
-- Name: idx_event_country_impacts_event_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_event_country_impacts_event_id ON public.event_country_impacts USING btree (event_id);


--
-- Name: idx_event_country_impacts_impact_score; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_event_country_impacts_impact_score ON public.event_country_impacts USING btree (impact_score);


--
-- Name: idx_financial_annotations_created_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_annotations_created_by ON public.financial_annotations USING btree (created_by);


--
-- Name: idx_financial_annotations_line_item_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_annotations_line_item_id ON public.financial_annotations USING btree (line_item_id);


--
-- Name: idx_financial_annotations_statement_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_annotations_statement_id ON public.financial_annotations USING btree (statement_id);


--
-- Name: idx_financial_annotations_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_annotations_status ON public.financial_annotations USING btree (status);


--
-- Name: idx_financial_annotations_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_annotations_type ON public.financial_annotations USING btree (annotation_type);


--
-- Name: idx_financial_line_items_level; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_level ON public.financial_line_items USING btree (level);


--
-- Name: idx_financial_line_items_order_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_order_index ON public.financial_line_items USING btree (order_index);


--
-- Name: idx_financial_line_items_parent_concept; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_parent_concept ON public.financial_line_items USING btree (parent_concept);


--
-- Name: idx_financial_line_items_statement_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_statement_id ON public.financial_line_items USING btree (statement_id);


--
-- Name: idx_financial_line_items_statement_section; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_statement_section ON public.financial_line_items USING btree (statement_section);


--
-- Name: idx_financial_line_items_statement_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_statement_type ON public.financial_line_items USING btree (statement_type);


--
-- Name: idx_financial_line_items_taxonomy_concept; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_line_items_taxonomy_concept ON public.financial_line_items USING btree (taxonomy_concept);


--
-- Name: idx_financial_ratios_calculated_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_ratios_calculated_at ON public.financial_ratios USING btree (calculated_at);


--
-- Name: idx_financial_ratios_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_ratios_category ON public.financial_ratios USING btree (ratio_category);


--
-- Name: idx_financial_ratios_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_ratios_name ON public.financial_ratios USING btree (ratio_name);


--
-- Name: idx_financial_ratios_statement_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_ratios_statement_id ON public.financial_ratios USING btree (statement_id);


--
-- Name: idx_financial_statements_company_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_company_id ON public.financial_statements USING btree (company_id);


--
-- Name: idx_financial_statements_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_created_at ON public.financial_statements USING btree (created_at);


--
-- Name: idx_financial_statements_filing_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_filing_type ON public.financial_statements USING btree (filing_type);


--
-- Name: idx_financial_statements_fiscal_year; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_fiscal_year ON public.financial_statements USING btree (fiscal_year);


--
-- Name: idx_financial_statements_period_end_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_period_end_date ON public.financial_statements USING btree (period_end_date);


--
-- Name: idx_financial_statements_processing_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_financial_statements_processing_status ON public.financial_statements USING btree (xbrl_processing_status);


--
-- Name: idx_global_economic_events_event_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_events_event_type ON public.global_economic_events USING btree (event_type);


--
-- Name: idx_global_economic_events_primary_country_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_events_primary_country_id ON public.global_economic_events USING btree (primary_country_id);


--
-- Name: idx_global_economic_events_severity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_events_severity ON public.global_economic_events USING btree (severity);


--
-- Name: idx_global_economic_events_start_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_events_start_date ON public.global_economic_events USING btree (start_date);


--
-- Name: idx_global_economic_indicators_category; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_indicators_category ON public.global_economic_indicators USING btree (category);


--
-- Name: idx_global_economic_indicators_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_indicators_name ON public.global_economic_indicators USING btree (name);


--
-- Name: idx_global_economic_indicators_source; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_economic_indicators_source ON public.global_economic_indicators USING btree (source);


--
-- Name: idx_global_indicator_data_country_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_indicator_data_country_id ON public.global_indicator_data USING btree (country_id);


--
-- Name: idx_global_indicator_data_date; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_indicator_data_date ON public.global_indicator_data USING btree (date);


--
-- Name: idx_global_indicator_data_indicator_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_indicator_data_indicator_id ON public.global_indicator_data USING btree (indicator_id);


--
-- Name: idx_global_indicator_data_value; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_global_indicator_data_value ON public.global_indicator_data USING btree (value);


--
-- Name: idx_leading_indicators_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_leading_indicators_name ON public.leading_indicators USING btree (name);


--
-- Name: idx_leading_indicators_update_frequency; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_leading_indicators_update_frequency ON public.leading_indicators USING btree (update_frequency);


--
-- Name: idx_security_events_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_events_created_at ON public.security_events USING btree (created_at);


--
-- Name: idx_security_events_event_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_events_event_type ON public.security_events USING btree (event_type);


--
-- Name: idx_security_events_resolved_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_events_resolved_at ON public.security_events USING btree (resolved_at);


--
-- Name: idx_security_events_severity; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_events_severity ON public.security_events USING btree (severity);


--
-- Name: idx_security_events_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_security_events_user_id ON public.security_events USING btree (user_id);


--
-- Name: idx_series_metadata_data_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_series_metadata_data_type ON public.series_metadata USING btree (data_type);


--
-- Name: idx_series_metadata_frequency_short; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_series_metadata_frequency_short ON public.series_metadata USING btree (frequency_short);


--
-- Name: idx_series_metadata_geographic_area; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_series_metadata_geographic_area ON public.series_metadata USING btree (geographic_area);


--
-- Name: idx_series_metadata_series_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_series_metadata_series_id ON public.series_metadata USING btree (series_id);


--
-- Name: idx_series_metadata_source_name; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_series_metadata_source_name ON public.series_metadata USING btree (source_name);


--
-- Name: idx_template_annotation_assignments_assigned_by; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_template_annotation_assignments_assigned_by ON public.template_annotation_assignments USING btree (assigned_by);


--
-- Name: idx_template_annotation_assignments_assigned_to; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_template_annotation_assignments_assigned_to ON public.template_annotation_assignments USING btree (assigned_to);


--
-- Name: idx_template_annotation_assignments_chart_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_template_annotation_assignments_chart_id ON public.template_annotation_assignments USING btree (chart_id);


--
-- Name: idx_template_annotation_assignments_completed_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_template_annotation_assignments_completed_at ON public.template_annotation_assignments USING btree (completed_at);


--
-- Name: idx_template_annotation_assignments_template_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_template_annotation_assignments_template_id ON public.template_annotation_assignments USING btree (template_id);


--
-- Name: idx_trade_relationships_exporter_country_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_relationships_exporter_country_id ON public.trade_relationships USING btree (exporter_country_id);


--
-- Name: idx_trade_relationships_importer_country_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_relationships_importer_country_id ON public.trade_relationships USING btree (importer_country_id);


--
-- Name: idx_trade_relationships_trade_value_usd; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_relationships_trade_value_usd ON public.trade_relationships USING btree (trade_value_usd);


--
-- Name: idx_trade_relationships_trade_year; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_trade_relationships_trade_year ON public.trade_relationships USING btree (trade_year);


--
-- Name: idx_user_data_source_preferences_data_source_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_data_source_preferences_data_source_id ON public.user_data_source_preferences USING btree (data_source_id);


--
-- Name: idx_user_data_source_preferences_is_favorite; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_data_source_preferences_is_favorite ON public.user_data_source_preferences USING btree (is_favorite);


--
-- Name: idx_user_data_source_preferences_notification_enabled; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_data_source_preferences_notification_enabled ON public.user_data_source_preferences USING btree (notification_enabled);


--
-- Name: idx_user_data_source_preferences_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_data_source_preferences_user_id ON public.user_data_source_preferences USING btree (user_id);


--
-- Name: idx_user_sessions_expires_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_sessions_expires_at ON public.user_sessions USING btree (expires_at);


--
-- Name: idx_user_sessions_last_used_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_sessions_last_used_at ON public.user_sessions USING btree (last_used_at);


--
-- Name: idx_user_sessions_session_token; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_sessions_session_token ON public.user_sessions USING btree (session_token);


--
-- Name: idx_user_sessions_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_user_sessions_user_id ON public.user_sessions USING btree (user_id);


--
-- Name: idx_users_email; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_email ON public.users USING btree (email);


--
-- Name: idx_users_email_verified; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_email_verified ON public.users USING btree (email_verified);


--
-- Name: idx_users_is_active; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_is_active ON public.users USING btree (is_active);


--
-- Name: idx_users_last_login_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_last_login_at ON public.users USING btree (last_login_at);


--
-- Name: idx_users_username; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_users_username ON public.users USING btree (username);


--
-- Name: idx_xbrl_dts_dependencies_child; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_dts_dependencies_child ON public.xbrl_dts_dependencies USING btree (child_schema_id);


--
-- Name: idx_xbrl_dts_dependencies_parent; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_dts_dependencies_parent ON public.xbrl_dts_dependencies USING btree (parent_schema_id);


--
-- Name: idx_xbrl_dts_dependencies_resolved; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_dts_dependencies_resolved ON public.xbrl_dts_dependencies USING btree (is_resolved);


--
-- Name: idx_xbrl_instance_dts_references_resolved; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_instance_dts_references_resolved ON public.xbrl_instance_dts_references USING btree (is_resolved);


--
-- Name: idx_xbrl_instance_dts_references_statement; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_instance_dts_references_statement ON public.xbrl_instance_dts_references USING btree (statement_id);


--
-- Name: idx_xbrl_instance_dts_references_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_instance_dts_references_type ON public.xbrl_instance_dts_references USING btree (reference_type);


--
-- Name: idx_xbrl_taxonomy_concepts_namespace; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_concepts_namespace ON public.xbrl_taxonomy_concepts USING btree (concept_namespace);


--
-- Name: idx_xbrl_taxonomy_concepts_qname; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_concepts_qname ON public.xbrl_taxonomy_concepts USING btree (concept_qname);


--
-- Name: idx_xbrl_taxonomy_concepts_schema_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_concepts_schema_id ON public.xbrl_taxonomy_concepts USING btree (schema_id);


--
-- Name: idx_xbrl_taxonomy_linkbases_schema_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_linkbases_schema_id ON public.xbrl_taxonomy_linkbases USING btree (schema_id);


--
-- Name: idx_xbrl_taxonomy_linkbases_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_linkbases_status ON public.xbrl_taxonomy_linkbases USING btree (processing_status);


--
-- Name: idx_xbrl_taxonomy_linkbases_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_linkbases_type ON public.xbrl_taxonomy_linkbases USING btree (linkbase_type);


--
-- Name: idx_xbrl_taxonomy_schemas_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_schemas_created_at ON public.xbrl_taxonomy_schemas USING btree (created_at);


--
-- Name: idx_xbrl_taxonomy_schemas_namespace; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_schemas_namespace ON public.xbrl_taxonomy_schemas USING btree (schema_namespace);


--
-- Name: idx_xbrl_taxonomy_schemas_source_type; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_schemas_source_type ON public.xbrl_taxonomy_schemas USING btree (source_type);


--
-- Name: idx_xbrl_taxonomy_schemas_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX idx_xbrl_taxonomy_schemas_status ON public.xbrl_taxonomy_schemas USING btree (processing_status);


--
-- Name: annotation_assignments update_annotation_assignments_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_annotation_assignments_updated_at BEFORE UPDATE ON public.annotation_assignments FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: annotation_comments update_annotation_comments_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON public.annotation_comments FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: annotation_replies update_annotation_replies_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_annotation_replies_updated_at BEFORE UPDATE ON public.annotation_replies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: annotation_templates update_annotation_templates_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_annotation_templates_updated_at BEFORE UPDATE ON public.annotation_templates FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: chart_annotation_replies update_chart_annotation_replies_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_chart_annotation_replies_updated_at BEFORE UPDATE ON public.chart_annotation_replies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: chart_annotation_templates update_chart_annotation_templates_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_chart_annotation_templates_updated_at BEFORE UPDATE ON public.chart_annotation_templates FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: chart_annotations update_chart_annotations_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON public.chart_annotations FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: chart_financial_annotations update_chart_financial_annotations_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_chart_financial_annotations_updated_at BEFORE UPDATE ON public.chart_financial_annotations FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: companies update_companies_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_companies_updated_at BEFORE UPDATE ON public.companies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: countries update_countries_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_countries_updated_at BEFORE UPDATE ON public.countries FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: crawl_queue update_crawl_queue_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_crawl_queue_updated_at BEFORE UPDATE ON public.crawl_queue FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: data_points update_data_points_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_data_points_updated_at BEFORE UPDATE ON public.data_points FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: data_sources update_data_sources_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_data_sources_updated_at BEFORE UPDATE ON public.data_sources FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: economic_series update_economic_series_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_economic_series_updated_at BEFORE UPDATE ON public.economic_series FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: financial_annotations update_financial_annotations_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_financial_annotations_updated_at BEFORE UPDATE ON public.financial_annotations FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: financial_line_items update_financial_line_items_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_financial_line_items_updated_at BEFORE UPDATE ON public.financial_line_items FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: financial_ratios update_financial_ratios_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_financial_ratios_updated_at BEFORE UPDATE ON public.financial_ratios FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: financial_statements update_financial_statements_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_financial_statements_updated_at BEFORE UPDATE ON public.financial_statements FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: global_economic_events update_global_economic_events_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_global_economic_events_updated_at BEFORE UPDATE ON public.global_economic_events FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: global_economic_indicators update_global_economic_indicators_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_global_economic_indicators_updated_at BEFORE UPDATE ON public.global_economic_indicators FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: leading_indicators update_leading_indicators_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_leading_indicators_updated_at BEFORE UPDATE ON public.leading_indicators FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: series_metadata update_series_metadata_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_series_metadata_updated_at BEFORE UPDATE ON public.series_metadata FOR EACH ROW EXECUTE FUNCTION public.update_series_metadata_updated_at();


--
-- Name: user_data_source_preferences update_user_data_source_preferences_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_user_data_source_preferences_updated_at BEFORE UPDATE ON public.user_data_source_preferences FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: users update_users_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: xbrl_taxonomy_concepts update_xbrl_taxonomy_concepts_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_xbrl_taxonomy_concepts_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_concepts FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: xbrl_taxonomy_linkbases update_xbrl_taxonomy_linkbases_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_xbrl_taxonomy_linkbases_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_linkbases FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: xbrl_taxonomy_schemas update_xbrl_taxonomy_schemas_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER update_xbrl_taxonomy_schemas_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_schemas FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();


--
-- Name: annotation_assignments annotation_assignments_annotation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_assignments
    ADD CONSTRAINT annotation_assignments_annotation_id_fkey FOREIGN KEY (annotation_id) REFERENCES public.financial_annotations(id) ON DELETE CASCADE;


--
-- Name: annotation_comments annotation_comments_annotation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_comments
    ADD CONSTRAINT annotation_comments_annotation_id_fkey FOREIGN KEY (annotation_id) REFERENCES public.chart_annotations(id) ON DELETE CASCADE;


--
-- Name: annotation_comments annotation_comments_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_comments
    ADD CONSTRAINT annotation_comments_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: annotation_replies annotation_replies_annotation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_replies
    ADD CONSTRAINT annotation_replies_annotation_id_fkey FOREIGN KEY (annotation_id) REFERENCES public.financial_annotations(id) ON DELETE CASCADE;


--
-- Name: annotation_replies annotation_replies_parent_reply_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.annotation_replies
    ADD CONSTRAINT annotation_replies_parent_reply_id_fkey FOREIGN KEY (parent_reply_id) REFERENCES public.annotation_replies(id) ON DELETE CASCADE;


--
-- Name: audit_logs audit_logs_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.audit_logs
    ADD CONSTRAINT audit_logs_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: chart_annotation_replies chart_annotation_replies_comment_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotation_replies
    ADD CONSTRAINT chart_annotation_replies_comment_id_fkey FOREIGN KEY (comment_id) REFERENCES public.annotation_comments(id) ON DELETE CASCADE;


--
-- Name: chart_annotation_replies chart_annotation_replies_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotation_replies
    ADD CONSTRAINT chart_annotation_replies_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: chart_annotation_templates chart_annotation_templates_created_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotation_templates
    ADD CONSTRAINT chart_annotation_templates_created_by_fkey FOREIGN KEY (created_by) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: chart_annotations chart_annotations_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_annotations
    ADD CONSTRAINT chart_annotations_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: chart_collaborators chart_collaborators_invited_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_collaborators
    ADD CONSTRAINT chart_collaborators_invited_by_fkey FOREIGN KEY (invited_by) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: chart_collaborators chart_collaborators_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_collaborators
    ADD CONSTRAINT chart_collaborators_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: chart_financial_annotations chart_financial_annotations_annotation_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chart_financial_annotations
    ADD CONSTRAINT chart_financial_annotations_annotation_id_fkey FOREIGN KEY (annotation_id) REFERENCES public.chart_annotations(id) ON DELETE CASCADE;


--
-- Name: country_correlations country_correlations_country_1_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.country_correlations
    ADD CONSTRAINT country_correlations_country_1_id_fkey FOREIGN KEY (country_1_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: country_correlations country_correlations_country_2_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.country_correlations
    ADD CONSTRAINT country_correlations_country_2_id_fkey FOREIGN KEY (country_2_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: crawl_attempts crawl_attempts_series_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crawl_attempts
    ADD CONSTRAINT crawl_attempts_series_id_fkey FOREIGN KEY (series_id) REFERENCES public.economic_series(id) ON DELETE CASCADE;


--
-- Name: data_points data_points_series_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.data_points
    ADD CONSTRAINT data_points_series_id_fkey FOREIGN KEY (series_id) REFERENCES public.economic_series(id) ON DELETE CASCADE;


--
-- Name: economic_series economic_series_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.economic_series
    ADD CONSTRAINT economic_series_source_id_fkey FOREIGN KEY (source_id) REFERENCES public.data_sources(id) ON DELETE CASCADE;


--
-- Name: event_country_impacts event_country_impacts_country_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_country_impacts
    ADD CONSTRAINT event_country_impacts_country_id_fkey FOREIGN KEY (country_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: event_country_impacts event_country_impacts_event_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.event_country_impacts
    ADD CONSTRAINT event_country_impacts_event_id_fkey FOREIGN KEY (event_id) REFERENCES public.global_economic_events(id) ON DELETE CASCADE;


--
-- Name: financial_annotations financial_annotations_line_item_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_annotations
    ADD CONSTRAINT financial_annotations_line_item_id_fkey FOREIGN KEY (line_item_id) REFERENCES public.financial_line_items(id) ON DELETE CASCADE;


--
-- Name: financial_annotations financial_annotations_statement_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_annotations
    ADD CONSTRAINT financial_annotations_statement_id_fkey FOREIGN KEY (statement_id) REFERENCES public.financial_statements(id) ON DELETE CASCADE;


--
-- Name: financial_line_items financial_line_items_statement_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_line_items
    ADD CONSTRAINT financial_line_items_statement_id_fkey FOREIGN KEY (statement_id) REFERENCES public.financial_statements(id) ON DELETE CASCADE;


--
-- Name: financial_ratios financial_ratios_statement_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_ratios
    ADD CONSTRAINT financial_ratios_statement_id_fkey FOREIGN KEY (statement_id) REFERENCES public.financial_statements(id) ON DELETE CASCADE;


--
-- Name: financial_statements financial_statements_company_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.financial_statements
    ADD CONSTRAINT financial_statements_company_id_fkey FOREIGN KEY (company_id) REFERENCES public.companies(id) ON DELETE CASCADE;


--
-- Name: global_indicator_data global_indicator_data_country_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_indicator_data
    ADD CONSTRAINT global_indicator_data_country_id_fkey FOREIGN KEY (country_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: global_indicator_data global_indicator_data_indicator_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.global_indicator_data
    ADD CONSTRAINT global_indicator_data_indicator_id_fkey FOREIGN KEY (indicator_id) REFERENCES public.global_economic_indicators(id) ON DELETE CASCADE;


--
-- Name: security_events security_events_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.security_events
    ADD CONSTRAINT security_events_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE SET NULL;


--
-- Name: series_metadata series_metadata_series_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.series_metadata
    ADD CONSTRAINT series_metadata_series_id_fkey FOREIGN KEY (series_id) REFERENCES public.economic_series(id) ON DELETE CASCADE;


--
-- Name: template_annotation_assignments template_annotation_assignments_assigned_by_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.template_annotation_assignments
    ADD CONSTRAINT template_annotation_assignments_assigned_by_fkey FOREIGN KEY (assigned_by) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: template_annotation_assignments template_annotation_assignments_assigned_to_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.template_annotation_assignments
    ADD CONSTRAINT template_annotation_assignments_assigned_to_fkey FOREIGN KEY (assigned_to) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: template_annotation_assignments template_annotation_assignments_template_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.template_annotation_assignments
    ADD CONSTRAINT template_annotation_assignments_template_id_fkey FOREIGN KEY (template_id) REFERENCES public.chart_annotation_templates(id) ON DELETE CASCADE;


--
-- Name: trade_relationships trade_relationships_exporter_country_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_relationships
    ADD CONSTRAINT trade_relationships_exporter_country_id_fkey FOREIGN KEY (exporter_country_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: trade_relationships trade_relationships_importer_country_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.trade_relationships
    ADD CONSTRAINT trade_relationships_importer_country_id_fkey FOREIGN KEY (importer_country_id) REFERENCES public.countries(id) ON DELETE CASCADE;


--
-- Name: user_data_source_preferences user_data_source_preferences_data_source_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_data_source_preferences
    ADD CONSTRAINT user_data_source_preferences_data_source_id_fkey FOREIGN KEY (data_source_id) REFERENCES public.data_sources(id) ON DELETE CASCADE;


--
-- Name: user_data_source_preferences user_data_source_preferences_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_data_source_preferences
    ADD CONSTRAINT user_data_source_preferences_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: user_sessions user_sessions_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_sessions
    ADD CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: xbrl_dts_dependencies xbrl_dts_dependencies_child_schema_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_dts_dependencies
    ADD CONSTRAINT xbrl_dts_dependencies_child_schema_id_fkey FOREIGN KEY (child_schema_id) REFERENCES public.xbrl_taxonomy_schemas(id) ON DELETE CASCADE;


--
-- Name: xbrl_dts_dependencies xbrl_dts_dependencies_parent_schema_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_dts_dependencies
    ADD CONSTRAINT xbrl_dts_dependencies_parent_schema_id_fkey FOREIGN KEY (parent_schema_id) REFERENCES public.xbrl_taxonomy_schemas(id) ON DELETE CASCADE;


--
-- Name: xbrl_instance_dts_references xbrl_instance_dts_references_resolved_linkbase_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_instance_dts_references
    ADD CONSTRAINT xbrl_instance_dts_references_resolved_linkbase_id_fkey FOREIGN KEY (resolved_linkbase_id) REFERENCES public.xbrl_taxonomy_linkbases(id);


--
-- Name: xbrl_instance_dts_references xbrl_instance_dts_references_resolved_schema_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_instance_dts_references
    ADD CONSTRAINT xbrl_instance_dts_references_resolved_schema_id_fkey FOREIGN KEY (resolved_schema_id) REFERENCES public.xbrl_taxonomy_schemas(id);


--
-- Name: xbrl_instance_dts_references xbrl_instance_dts_references_statement_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_instance_dts_references
    ADD CONSTRAINT xbrl_instance_dts_references_statement_id_fkey FOREIGN KEY (statement_id) REFERENCES public.financial_statements(id) ON DELETE CASCADE;


--
-- Name: xbrl_taxonomy_concepts xbrl_taxonomy_concepts_schema_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_concepts
    ADD CONSTRAINT xbrl_taxonomy_concepts_schema_id_fkey FOREIGN KEY (schema_id) REFERENCES public.xbrl_taxonomy_schemas(id);


--
-- Name: xbrl_taxonomy_linkbases xbrl_taxonomy_linkbases_schema_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.xbrl_taxonomy_linkbases
    ADD CONSTRAINT xbrl_taxonomy_linkbases_schema_id_fkey FOREIGN KEY (schema_id) REFERENCES public.xbrl_taxonomy_schemas(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

\unrestrict kJUmXtBgdI2lPY1oJfmhhVm4U4MCmXSeXN0iNcpgPAs8fAYKdtcEmqCiuP6czxc
