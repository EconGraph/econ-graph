use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::schema::{
    countries, country_correlations, event_country_impacts, global_economic_events,
    global_economic_indicators, global_indicator_data, leading_indicators, trade_relationships,
};

/// Country model with geographic and economic metadata
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = countries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Country {
    pub id: Uuid,
    pub iso_code: String,
    pub iso_code_2: String,
    pub name: String,
    pub region: String,
    pub sub_region: Option<String>,
    pub income_group: Option<String>,
    pub population: Option<i64>,
    pub gdp_usd: Option<BigDecimal>,
    pub gdp_per_capita_usd: Option<BigDecimal>,
    pub latitude: Option<BigDecimal>,
    pub longitude: Option<BigDecimal>,
    pub currency_code: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New country for insertion
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = countries)]
pub struct NewCountry {
    #[validate(length(min = 3, max = 3))]
    pub iso_code: String,
    #[validate(length(min = 2, max = 2))]
    pub iso_code_2: String,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(length(min = 1, max = 100))]
    pub region: String,
    #[validate(length(max = 100))]
    pub sub_region: Option<String>,
    #[validate(length(max = 50))]
    pub income_group: Option<String>,
    pub population: Option<i64>,
    pub gdp_usd: Option<BigDecimal>,
    pub gdp_per_capita_usd: Option<BigDecimal>,
    pub latitude: Option<BigDecimal>,
    pub longitude: Option<BigDecimal>,
    #[validate(length(max = 3))]
    pub currency_code: Option<String>,
    pub is_active: Option<bool>,
}

/// Global economic indicator definition
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = global_economic_indicators)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GlobalEconomicIndicator {
    pub id: Uuid,
    pub country_id: Uuid,
    pub indicator_code: String,
    pub indicator_name: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub unit: Option<String>,
    pub frequency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New global economic indicator
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = global_economic_indicators)]
pub struct NewGlobalEconomicIndicator {
    pub country_id: Uuid,
    #[validate(length(min = 1, max = 50))]
    pub indicator_code: String,
    #[validate(length(min = 1, max = 500))]
    pub indicator_name: String,
    #[validate(length(min = 1, max = 100))]
    pub category: String,
    #[validate(length(max = 100))]
    pub subcategory: Option<String>,
    #[validate(length(max = 50))]
    pub unit: Option<String>,
    #[validate(length(min = 1, max = 20))]
    pub frequency: String,
}

/// Time series data point for global indicators
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = global_indicator_data)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GlobalIndicatorData {
    pub id: Uuid,
    pub indicator_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub is_preliminary: bool,
    pub data_source: String,
    pub created_at: DateTime<Utc>,
}

/// New global indicator data point
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = global_indicator_data)]
pub struct NewGlobalIndicatorData {
    pub indicator_id: Uuid,
    pub date: NaiveDate,
    pub value: Option<BigDecimal>,
    pub is_preliminary: Option<bool>,
    #[validate(length(min = 1, max = 50))]
    pub data_source: String,
}

/// Economic correlation between two countries
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = country_correlations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct CountryCorrelation {
    pub id: Uuid,
    pub country_a_id: Uuid,
    pub country_b_id: Uuid,
    pub indicator_category: String,
    pub correlation_coefficient: BigDecimal,
    pub time_period_start: NaiveDate,
    pub time_period_end: NaiveDate,
    pub sample_size: i32,
    pub p_value: Option<BigDecimal>,
    pub is_significant: bool,
    pub calculated_at: DateTime<Utc>,
}

/// New country correlation
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = country_correlations)]
pub struct NewCountryCorrelation {
    pub country_a_id: Uuid,
    pub country_b_id: Uuid,
    #[validate(length(min = 1, max = 100))]
    pub indicator_category: String,
    pub correlation_coefficient: BigDecimal,
    pub time_period_start: NaiveDate,
    pub time_period_end: NaiveDate,
    #[validate(range(min = 2))]
    pub sample_size: i32,
    pub p_value: Option<BigDecimal>,
    pub is_significant: Option<bool>,
}

/// Trade relationship between two countries
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = trade_relationships)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TradeRelationship {
    pub id: Uuid,
    pub exporter_country_id: Uuid,
    pub importer_country_id: Uuid,
    pub trade_flow_type: String,
    pub year: i32,
    pub export_value_usd: Option<BigDecimal>,
    pub import_value_usd: Option<BigDecimal>,
    pub trade_balance_usd: Option<BigDecimal>,
    pub trade_intensity: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
}

/// New trade relationship
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = trade_relationships)]
pub struct NewTradeRelationship {
    pub exporter_country_id: Uuid,
    pub importer_country_id: Uuid,
    #[validate(length(min = 1, max = 20))]
    pub trade_flow_type: String,
    #[validate(range(min = 1990, max = 2030))]
    pub year: i32,
    pub export_value_usd: Option<BigDecimal>,
    pub import_value_usd: Option<BigDecimal>,
    pub trade_balance_usd: Option<BigDecimal>,
    pub trade_intensity: Option<BigDecimal>,
}

/// Global economic event
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = global_economic_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GlobalEconomicEvent {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub severity: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub primary_country_id: Option<Uuid>,
    pub affected_regions: Option<Vec<Option<String>>>,
    pub economic_impact_score: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// New global economic event
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = global_economic_events)]
pub struct NewGlobalEconomicEvent {
    #[validate(length(min = 1, max = 500))]
    pub name: String,
    pub description: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub event_type: String,
    #[validate(length(min = 1, max = 20))]
    pub severity: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub primary_country_id: Option<Uuid>,
    pub affected_regions: Option<Vec<Option<String>>>,
    pub economic_impact_score: Option<BigDecimal>,
}

/// Country impact from a global event
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = event_country_impacts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct EventCountryImpact {
    pub id: Uuid,
    pub event_id: Uuid,
    pub country_id: Uuid,
    pub impact_type: String,
    pub impact_magnitude: Option<BigDecimal>,
    pub impact_duration_days: Option<i32>,
    pub recovery_time_days: Option<i32>,
    pub confidence_score: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
}

/// New event country impact
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = event_country_impacts)]
pub struct NewEventCountryImpact {
    pub event_id: Uuid,
    pub country_id: Uuid,
    #[validate(length(min = 1, max = 50))]
    pub impact_type: String,
    pub impact_magnitude: Option<BigDecimal>,
    #[validate(range(min = 1))]
    pub impact_duration_days: Option<i32>,
    #[validate(range(min = 1))]
    pub recovery_time_days: Option<i32>,
    pub confidence_score: Option<BigDecimal>,
}

/// Leading indicator relationship between countries
#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = leading_indicators)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct LeadingIndicator {
    pub id: Uuid,
    pub leading_country_id: Uuid,
    pub following_country_id: Uuid,
    pub indicator_category: String,
    pub lead_time_months: i32,
    pub correlation_strength: BigDecimal,
    pub predictive_accuracy: Option<BigDecimal>,
    pub time_period_start: NaiveDate,
    pub time_period_end: NaiveDate,
    pub calculated_at: DateTime<Utc>,
}

/// New leading indicator relationship
#[derive(Debug, Clone, Insertable, Validate, Deserialize)]
#[diesel(table_name = leading_indicators)]
pub struct NewLeadingIndicator {
    pub leading_country_id: Uuid,
    pub following_country_id: Uuid,
    #[validate(length(min = 1, max = 100))]
    pub indicator_category: String,
    #[validate(range(min = 1, max = 24))]
    pub lead_time_months: i32,
    pub correlation_strength: BigDecimal,
    pub predictive_accuracy: Option<BigDecimal>,
    pub time_period_start: NaiveDate,
    pub time_period_end: NaiveDate,
}

/// Combined country with economic data for network analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryWithEconomicData {
    pub country: Country,
    pub latest_gdp: Option<BigDecimal>,
    pub latest_gdp_growth: Option<BigDecimal>,
    pub latest_inflation: Option<BigDecimal>,
    pub latest_unemployment: Option<BigDecimal>,
    pub trade_partners: Vec<TradePartner>,
    pub economic_health_score: Option<f64>,
}

/// Trade partner information for network visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradePartner {
    pub country: Country,
    pub trade_value_usd: BigDecimal,
    pub trade_intensity: f64,
    pub relationship_type: String, // "Export", "Import", "Bilateral"
}

/// Economic correlation network node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationNetworkNode {
    pub country: Country,
    pub connections: Vec<CorrelationConnection>,
    pub centrality_score: f64,
    pub cluster_id: Option<i32>,
}

/// Connection between countries in correlation network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationConnection {
    pub target_country: Country,
    pub correlation_coefficient: f64,
    pub indicator_category: String,
    pub significance_level: f64,
    pub connection_strength: f64, // Normalized 0-1
}

/// Global economic event with country impacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalEventWithImpacts {
    pub event: GlobalEconomicEvent,
    pub country_impacts: Vec<CountryImpactDetail>,
    pub affected_country_count: i64,
    pub total_economic_impact: Option<BigDecimal>,
}

/// Detailed country impact information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryImpactDetail {
    pub country: Country,
    pub impact: EventCountryImpact,
    pub impact_severity: String, // "Mild", "Moderate", "Severe", "Critical"
    pub recovery_status: String, // "Recovered", "Recovering", "Ongoing"
}

/// Economic indicator categories for global analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndicatorCategory {
    GDP,
    Trade,
    Employment,
    Inflation,
    MonetaryPolicy,
    FiscalPolicy,
    Financial,
    Demographics,
}

impl std::fmt::Display for IndicatorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndicatorCategory::GDP => write!(f, "GDP"),
            IndicatorCategory::Trade => write!(f, "Trade"),
            IndicatorCategory::Employment => write!(f, "Employment"),
            IndicatorCategory::Inflation => write!(f, "Inflation"),
            IndicatorCategory::MonetaryPolicy => write!(f, "MonetaryPolicy"),
            IndicatorCategory::FiscalPolicy => write!(f, "FiscalPolicy"),
            IndicatorCategory::Financial => write!(f, "Financial"),
            IndicatorCategory::Demographics => write!(f, "Demographics"),
        }
    }
}

/// Event severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for EventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSeverity::Low => write!(f, "Low"),
            EventSeverity::Medium => write!(f, "Medium"),
            EventSeverity::High => write!(f, "High"),
            EventSeverity::Critical => write!(f, "Critical"),
        }
    }
}
