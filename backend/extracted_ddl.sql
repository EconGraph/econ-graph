CREATE TYPE public.annotation_status AS ENUM (
CREATE TYPE public.annotation_type AS ENUM (
CREATE TYPE public.assignment_status AS ENUM (
CREATE TYPE public.assignment_type AS ENUM (
CREATE TYPE public.balance_type AS ENUM (
CREATE TYPE public.calculation_method AS ENUM (
CREATE TYPE public.comparison_type AS ENUM (
CREATE TYPE public.compression_type AS ENUM (
CREATE TYPE public.period_type AS ENUM (
CREATE TYPE public.processing_status AS ENUM (
CREATE TYPE public.processing_step AS ENUM (
CREATE TYPE public.ratio_category AS ENUM (
CREATE TYPE public.statement_section AS ENUM (
CREATE TYPE public.statement_type AS ENUM (
CREATE TYPE public.substitution_group AS ENUM (
CREATE TYPE public.taxonomy_file_type AS ENUM (
CREATE TYPE public.taxonomy_source_type AS ENUM (
CREATE TYPE public.xbrl_data_type AS ENUM (
CREATE TABLE public.__diesel_schema_migrations (
CREATE TABLE public.annotation_assignments (
CREATE TABLE public.annotation_comments (
CREATE TABLE public.annotation_replies (
CREATE TABLE public.annotation_templates (
CREATE TABLE public.audit_logs (
CREATE TABLE public.chart_annotations (
CREATE TABLE public.chart_collaborators (
CREATE TABLE public.companies (
CREATE TABLE public.financial_statements (
CREATE TABLE public.countries (
CREATE TABLE public.country_correlations (
CREATE TABLE public.crawl_attempts (
CREATE TABLE public.crawl_queue (
CREATE TABLE public.data_points (
CREATE TABLE public.data_sources (
CREATE TABLE public.economic_series (
CREATE TABLE public.event_country_impacts (
CREATE TABLE public.financial_annotations (
CREATE TABLE public.financial_line_items (
CREATE TABLE public.financial_ratios (
CREATE TABLE public.global_economic_events (
CREATE TABLE public.global_economic_indicators (
CREATE TABLE public.global_indicator_data (
CREATE TABLE public.leading_indicators (
CREATE TABLE public.security_events (
CREATE TABLE public.series_metadata (
CREATE TABLE public.trade_relationships (
CREATE TABLE public.user_data_source_preferences (
CREATE TABLE public.user_sessions (
CREATE TABLE public.users (
CREATE TABLE public.xbrl_dts_dependencies (
CREATE TABLE public.xbrl_instance_dts_references (
CREATE TABLE public.xbrl_taxonomy_concepts (
CREATE TABLE public.xbrl_taxonomy_linkbases (
CREATE TABLE public.xbrl_taxonomy_schemas (
CREATE INDEX idx_annotation_assignments_annotation_id ON public.annotation_assignments USING btree (annotation_id);
CREATE INDEX idx_annotation_assignments_assigned_to ON public.annotation_assignments USING btree (assigned_to);
CREATE INDEX idx_annotation_assignments_status ON public.annotation_assignments USING btree (status);
CREATE INDEX idx_annotation_comments_annotation_id ON public.annotation_comments USING btree (annotation_id);
CREATE INDEX idx_annotation_comments_user_id ON public.annotation_comments USING btree (user_id);
CREATE INDEX idx_annotation_replies_annotation_id ON public.annotation_replies USING btree (annotation_id);
CREATE INDEX idx_annotation_replies_created_by ON public.annotation_replies USING btree (created_by);
CREATE INDEX idx_annotation_replies_parent_reply_id ON public.annotation_replies USING btree (parent_reply_id);
CREATE INDEX idx_annotation_templates_active ON public.annotation_templates USING btree (is_active);
CREATE INDEX idx_annotation_templates_type ON public.annotation_templates USING btree (annotation_type);
CREATE INDEX idx_audit_logs_action ON public.audit_logs USING btree (action);
CREATE INDEX idx_audit_logs_created_at ON public.audit_logs USING btree (created_at);
CREATE INDEX idx_audit_logs_resource_type ON public.audit_logs USING btree (resource_type);
CREATE INDEX idx_audit_logs_user_id ON public.audit_logs USING btree (user_id);
CREATE INDEX idx_chart_annotations_date ON public.chart_annotations USING btree (annotation_date);
CREATE INDEX idx_chart_annotations_series_id ON public.chart_annotations USING btree (series_id);
CREATE INDEX idx_chart_annotations_user_id ON public.chart_annotations USING btree (user_id);
CREATE INDEX idx_chart_collaborators_chart_id ON public.chart_collaborators USING btree (chart_id);
CREATE INDEX idx_chart_collaborators_user_id ON public.chart_collaborators USING btree (user_id);
CREATE INDEX idx_companies_active ON public.companies USING btree (is_active);
CREATE INDEX idx_companies_cik ON public.companies USING btree (cik);
CREATE INDEX idx_companies_industry ON public.companies USING btree (industry);
CREATE INDEX idx_companies_name ON public.companies USING btree (name);
CREATE INDEX idx_companies_sector ON public.companies USING btree (sector);
CREATE INDEX idx_companies_ticker ON public.companies USING btree (ticker);
CREATE INDEX idx_correlations_category ON public.country_correlations USING btree (indicator_category);
CREATE INDEX idx_correlations_countries ON public.country_correlations USING btree (country_a_id, country_b_id);
CREATE INDEX idx_correlations_strength ON public.country_correlations USING btree (correlation_coefficient DESC) WHERE (is_significant = true);
CREATE INDEX idx_countries_income_group ON public.countries USING btree (income_group);
CREATE INDEX idx_countries_iso_codes ON public.countries USING btree (iso_code, iso_code_2);
CREATE INDEX idx_countries_region ON public.countries USING btree (region);
CREATE INDEX idx_crawl_attempts_attempted_at ON public.crawl_attempts USING btree (attempted_at);
CREATE INDEX idx_crawl_attempts_data_found ON public.crawl_attempts USING btree (data_found);
CREATE INDEX idx_crawl_attempts_error_type ON public.crawl_attempts USING btree (error_type);
CREATE INDEX idx_crawl_attempts_latest_data_date ON public.crawl_attempts USING btree (latest_data_date);
CREATE INDEX idx_crawl_attempts_series_attempted ON public.crawl_attempts USING btree (series_id, attempted_at);
CREATE INDEX idx_crawl_attempts_series_id ON public.crawl_attempts USING btree (series_id);
CREATE INDEX idx_crawl_attempts_series_success ON public.crawl_attempts USING btree (series_id, success);
CREATE INDEX idx_crawl_attempts_success ON public.crawl_attempts USING btree (success);
CREATE INDEX idx_crawl_attempts_success_attempted ON public.crawl_attempts USING btree (success, attempted_at);
CREATE INDEX idx_crawl_queue_created_at ON public.crawl_queue USING btree (created_at);
CREATE INDEX idx_crawl_queue_locked_by ON public.crawl_queue USING btree (locked_by);
CREATE INDEX idx_crawl_queue_priority ON public.crawl_queue USING btree (priority DESC);
CREATE INDEX idx_crawl_queue_processing ON public.crawl_queue USING btree (status, priority DESC, scheduled_for, locked_by) WHERE ((status)::text = ANY ((ARRAY['pending'::character varying, 'retrying'::character varying])::text[]));
CREATE INDEX idx_crawl_queue_scheduled_for ON public.crawl_queue USING btree (scheduled_for);
CREATE INDEX idx_crawl_queue_source ON public.crawl_queue USING btree (source);
CREATE INDEX idx_crawl_queue_status ON public.crawl_queue USING btree (status);
CREATE INDEX idx_data_points_date ON public.data_points USING btree (date);
CREATE INDEX idx_data_points_is_original_release ON public.data_points USING btree (is_original_release);
CREATE INDEX idx_data_points_latest_revision ON public.data_points USING btree (series_id, date, revision_date DESC, value);
CREATE INDEX idx_data_points_revision_date ON public.data_points USING btree (revision_date);
CREATE INDEX idx_data_points_series_date ON public.data_points USING btree (series_id, date);
CREATE INDEX idx_data_points_series_date_revision ON public.data_points USING btree (series_id, date, revision_date);
CREATE INDEX idx_data_points_series_id ON public.data_points USING btree (series_id);
CREATE INDEX idx_data_sources_crawl_status ON public.data_sources USING btree (crawl_status);
CREATE INDEX idx_data_sources_enabled ON public.data_sources USING btree (is_enabled);
CREATE INDEX idx_data_sources_name ON public.data_sources USING btree (name);
CREATE INDEX idx_data_sources_visible ON public.data_sources USING btree (is_visible);
CREATE INDEX idx_economic_series_description ON public.economic_series USING gin (to_tsvector('english'::regconfig, description));
CREATE INDEX idx_economic_series_external_id ON public.economic_series USING btree (external_id);
CREATE INDEX idx_economic_series_frequency ON public.economic_series USING btree (frequency);
CREATE INDEX idx_economic_series_is_active ON public.economic_series USING btree (is_active);
CREATE INDEX idx_economic_series_last_updated ON public.economic_series USING btree (last_updated);
CREATE INDEX idx_economic_series_source_id ON public.economic_series USING btree (source_id);
CREATE INDEX idx_economic_series_title ON public.economic_series USING gin (to_tsvector('english'::regconfig, (title)::text));
CREATE INDEX idx_event_impacts_country ON public.event_country_impacts USING btree (country_id, impact_magnitude DESC);
CREATE INDEX idx_events_date ON public.global_economic_events USING btree (start_date DESC);
CREATE INDEX idx_events_severity ON public.global_economic_events USING btree (severity, economic_impact_score DESC);
CREATE INDEX idx_financial_annotations_created_by ON public.financial_annotations USING btree (created_by);
CREATE INDEX idx_financial_annotations_line_item_id ON public.financial_annotations USING btree (line_item_id);
CREATE INDEX idx_financial_annotations_statement_id ON public.financial_annotations USING btree (statement_id);
CREATE INDEX idx_financial_annotations_status ON public.financial_annotations USING btree (status);
CREATE INDEX idx_financial_annotations_type ON public.financial_annotations USING btree (annotation_type);
CREATE INDEX idx_financial_line_items_level ON public.financial_line_items USING btree (level);
CREATE INDEX idx_financial_line_items_order_index ON public.financial_line_items USING btree (order_index);
CREATE INDEX idx_financial_line_items_parent_concept ON public.financial_line_items USING btree (parent_concept);
CREATE INDEX idx_financial_line_items_statement_id ON public.financial_line_items USING btree (statement_id);
CREATE INDEX idx_financial_line_items_statement_section ON public.financial_line_items USING btree (statement_section);
CREATE INDEX idx_financial_line_items_statement_type ON public.financial_line_items USING btree (statement_type);
CREATE INDEX idx_financial_line_items_taxonomy_concept ON public.financial_line_items USING btree (taxonomy_concept);
CREATE INDEX idx_financial_ratios_calculated_at ON public.financial_ratios USING btree (calculated_at);
CREATE INDEX idx_financial_ratios_category ON public.financial_ratios USING btree (ratio_category);
CREATE INDEX idx_financial_ratios_name ON public.financial_ratios USING btree (ratio_name);
CREATE INDEX idx_financial_ratios_statement_id ON public.financial_ratios USING btree (statement_id);
CREATE INDEX idx_financial_statements_company_id ON public.financial_statements USING btree (company_id);
CREATE INDEX idx_financial_statements_created_at ON public.financial_statements USING btree (created_at);
CREATE INDEX idx_financial_statements_filing_type ON public.financial_statements USING btree (filing_type);
CREATE INDEX idx_financial_statements_fiscal_year ON public.financial_statements USING btree (fiscal_year);
CREATE INDEX idx_financial_statements_period_end_date ON public.financial_statements USING btree (period_end_date);
CREATE INDEX idx_financial_statements_processing_status ON public.financial_statements USING btree (xbrl_processing_status);
CREATE INDEX idx_global_data_date_value ON public.global_indicator_data USING btree (date, value) WHERE (value IS NOT NULL);
CREATE INDEX idx_global_data_indicator_date ON public.global_indicator_data USING btree (indicator_id, date DESC);
CREATE INDEX idx_global_indicators_code ON public.global_economic_indicators USING btree (indicator_code);
CREATE INDEX idx_global_indicators_country_category ON public.global_economic_indicators USING btree (country_id, category);
CREATE INDEX idx_leading_indicators_strength ON public.leading_indicators USING btree (correlation_strength DESC, predictive_accuracy DESC);
CREATE INDEX idx_security_events_created_at ON public.security_events USING btree (created_at);
CREATE INDEX idx_security_events_event_type ON public.security_events USING btree (event_type);
CREATE INDEX idx_security_events_resolved ON public.security_events USING btree (resolved);
CREATE INDEX idx_security_events_severity ON public.security_events USING btree (severity);
CREATE INDEX idx_security_events_user_id ON public.security_events USING btree (user_id);
CREATE INDEX idx_series_metadata_active ON public.series_metadata USING btree (is_active);
CREATE INDEX idx_series_metadata_external_id ON public.series_metadata USING btree (external_id);
CREATE INDEX idx_series_metadata_last_discovered ON public.series_metadata USING btree (last_discovered_at);
CREATE INDEX idx_series_metadata_source_id ON public.series_metadata USING btree (source_id);
CREATE INDEX idx_trade_exporter_year ON public.trade_relationships USING btree (exporter_country_id, year DESC);
CREATE INDEX idx_trade_importer_year ON public.trade_relationships USING btree (importer_country_id, year DESC);
CREATE INDEX idx_trade_value ON public.trade_relationships USING btree (export_value_usd DESC) WHERE (export_value_usd IS NOT NULL);
CREATE INDEX idx_user_data_source_preferences_data_source_id ON public.user_data_source_preferences USING btree (data_source_id);
CREATE INDEX idx_user_data_source_preferences_user_id ON public.user_data_source_preferences USING btree (user_id);
CREATE INDEX idx_user_data_source_preferences_visible ON public.user_data_source_preferences USING btree (is_visible);
CREATE INDEX idx_user_sessions_expires_at ON public.user_sessions USING btree (expires_at);
CREATE INDEX idx_user_sessions_token_hash ON public.user_sessions USING btree (token_hash);
CREATE INDEX idx_user_sessions_user_id ON public.user_sessions USING btree (user_id);
CREATE INDEX idx_users_email ON public.users USING btree (email);
CREATE INDEX idx_users_provider ON public.users USING btree (provider, provider_id);
CREATE INDEX idx_xbrl_dts_dependencies_child ON public.xbrl_dts_dependencies USING btree (child_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_parent ON public.xbrl_dts_dependencies USING btree (parent_schema_id);
CREATE INDEX idx_xbrl_dts_dependencies_resolved ON public.xbrl_dts_dependencies USING btree (is_resolved);
CREATE INDEX idx_xbrl_instance_dts_references_resolved ON public.xbrl_instance_dts_references USING btree (is_resolved);
CREATE INDEX idx_xbrl_instance_dts_references_statement ON public.xbrl_instance_dts_references USING btree (statement_id);
CREATE INDEX idx_xbrl_instance_dts_references_type ON public.xbrl_instance_dts_references USING btree (reference_type);
CREATE INDEX idx_xbrl_taxonomy_concepts_namespace ON public.xbrl_taxonomy_concepts USING btree (concept_namespace);
CREATE INDEX idx_xbrl_taxonomy_concepts_qname ON public.xbrl_taxonomy_concepts USING btree (concept_qname);
CREATE INDEX idx_xbrl_taxonomy_concepts_schema_id ON public.xbrl_taxonomy_concepts USING btree (schema_id);
CREATE INDEX idx_xbrl_taxonomy_linkbases_schema_id ON public.xbrl_taxonomy_linkbases USING btree (schema_id);
CREATE INDEX idx_xbrl_taxonomy_linkbases_status ON public.xbrl_taxonomy_linkbases USING btree (processing_status);
CREATE INDEX idx_xbrl_taxonomy_linkbases_type ON public.xbrl_taxonomy_linkbases USING btree (linkbase_type);
CREATE INDEX idx_xbrl_taxonomy_schemas_created_at ON public.xbrl_taxonomy_schemas USING btree (created_at);
CREATE INDEX idx_xbrl_taxonomy_schemas_namespace ON public.xbrl_taxonomy_schemas USING btree (schema_namespace);
CREATE INDEX idx_xbrl_taxonomy_schemas_source_type ON public.xbrl_taxonomy_schemas USING btree (source_type);
CREATE INDEX idx_xbrl_taxonomy_schemas_status ON public.xbrl_taxonomy_schemas USING btree (processing_status);
CREATE TRIGGER set_crawl_attempts_updated_at BEFORE UPDATE ON public.crawl_attempts FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER trigger_update_series_metadata_updated_at BEFORE UPDATE ON public.series_metadata FOR EACH ROW EXECUTE FUNCTION public.update_series_metadata_updated_at();
CREATE TRIGGER update_annotation_assignments_updated_at BEFORE UPDATE ON public.annotation_assignments FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_annotation_comments_updated_at BEFORE UPDATE ON public.annotation_comments FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_annotation_replies_updated_at BEFORE UPDATE ON public.annotation_replies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_annotation_templates_updated_at BEFORE UPDATE ON public.annotation_templates FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_chart_annotations_updated_at BEFORE UPDATE ON public.chart_annotations FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_companies_updated_at BEFORE UPDATE ON public.companies FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_countries_updated_at BEFORE UPDATE ON public.countries FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_crawl_queue_updated_at BEFORE UPDATE ON public.crawl_queue FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_data_points_updated_at BEFORE UPDATE ON public.data_points FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_data_sources_updated_at BEFORE UPDATE ON public.data_sources FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_economic_series_updated_at BEFORE UPDATE ON public.economic_series FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_financial_annotations_updated_at BEFORE UPDATE ON public.financial_annotations FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_financial_line_items_updated_at BEFORE UPDATE ON public.financial_line_items FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_financial_ratios_updated_at BEFORE UPDATE ON public.financial_ratios FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_financial_statements_updated_at BEFORE UPDATE ON public.financial_statements FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_global_events_updated_at BEFORE UPDATE ON public.global_economic_events FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_global_indicators_updated_at BEFORE UPDATE ON public.global_economic_indicators FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_user_data_source_preferences_updated_at BEFORE UPDATE ON public.user_data_source_preferences FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON public.users FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_xbrl_taxonomy_concepts_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_concepts FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_xbrl_taxonomy_linkbases_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_linkbases FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
CREATE TRIGGER update_xbrl_taxonomy_schemas_updated_at BEFORE UPDATE ON public.xbrl_taxonomy_schemas FOR EACH ROW EXECUTE FUNCTION public.update_updated_at_column();
