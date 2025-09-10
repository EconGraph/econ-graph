use async_graphql::{Context, Object, Result as GqlResult, SimpleObject, Enum};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::database::DatabasePool;
use crate::services::GlobalAnalysisService;
use crate::models::{
    Country, CountryWithEconomicData, CorrelationNetworkNode, GlobalEventWithImpacts,
    CountryCorrelation, CorrelationConnection, TradePartner, CountryImpactDetail,
    GlobalEconomicEvent, EventCountryImpact
};

/// GraphQL representation of a country
#[derive(SimpleObject)]
#[graphql(complex)]
pub struct CountryType {
    pub id: String,
    pub iso_code: String,
    pub iso_code_2: String,
    pub name: String,
    pub region: String,
    pub sub_region: Option<String>,
    pub income_group: Option<String>,
    pub population: Option<i64>,
    pub gdp_usd: Option<String>, // Decimal as string for GraphQL
    pub gdp_per_capita_usd: Option<String>,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub currency_code: Option<String>,
    pub is_active: bool,
}

impl From<Country> for CountryType {
    fn from(country: Country) -> Self {
        Self {
            id: country.id.to_string(),
            iso_code: country.iso_code,
            iso_code_2: country.iso_code_2,
            name: country.name,
            region: country.region,
            sub_region: country.sub_region,
            income_group: country.income_group,
            population: country.population,
            gdp_usd: country.gdp_usd.map(|d| d.to_string()),
            gdp_per_capita_usd: country.gdp_per_capita_usd.map(|d| d.to_string()),
            latitude: country.latitude.map(|d| d.to_string()),
            longitude: country.longitude.map(|d| d.to_string()),
            currency_code: country.currency_code,
            is_active: country.is_active,
        }
    }
}

/// Country with economic data for network analysis
#[derive(SimpleObject)]
pub struct CountryWithEconomicDataType {
    pub country: CountryType,
    pub latest_gdp: Option<String>,
    pub latest_gdp_growth: Option<String>,
    pub latest_inflation: Option<String>,
    pub latest_unemployment: Option<String>,
    pub trade_partners: Vec<TradePartnerType>,
    pub economic_health_score: Option<f64>,
}

impl From<CountryWithEconomicData> for CountryWithEconomicDataType {
    fn from(data: CountryWithEconomicData) -> Self {
        Self {
            country: data.country.into(),
            latest_gdp: data.latest_gdp.map(|d| d.to_string()),
            latest_gdp_growth: data.latest_gdp_growth.map(|d| d.to_string()),
            latest_inflation: data.latest_inflation.map(|d| d.to_string()),
            latest_unemployment: data.latest_unemployment.map(|d| d.to_string()),
            trade_partners: data.trade_partners.into_iter().map(Into::into).collect(),
            economic_health_score: data.economic_health_score,
        }
    }
}

/// Trade partner information
#[derive(SimpleObject)]
pub struct TradePartnerType {
    pub country: CountryType,
    pub trade_value_usd: String,
    pub trade_intensity: f64,
    pub relationship_type: String,
}

impl From<TradePartner> for TradePartnerType {
    fn from(partner: TradePartner) -> Self {
        Self {
            country: partner.country.into(),
            trade_value_usd: partner.trade_value_usd.to_string(),
            trade_intensity: partner.trade_intensity,
            relationship_type: partner.relationship_type,
        }
    }
}

/// Correlation network node for visualization
#[derive(SimpleObject)]
pub struct CorrelationNetworkNodeType {
    pub country: CountryType,
    pub connections: Vec<CorrelationConnectionType>,
    pub centrality_score: f64,
    pub cluster_id: Option<i32>,
}

impl From<CorrelationNetworkNode> for CorrelationNetworkNodeType {
    fn from(node: CorrelationNetworkNode) -> Self {
        Self {
            country: node.country.into(),
            connections: node.connections.into_iter().map(Into::into).collect(),
            centrality_score: node.centrality_score,
            cluster_id: node.cluster_id,
        }
    }
}

/// Connection between countries in correlation network
#[derive(SimpleObject)]
pub struct CorrelationConnectionType {
    pub target_country: CountryType,
    pub correlation_coefficient: f64,
    pub indicator_category: String,
    pub significance_level: f64,
    pub connection_strength: f64,
}

impl From<CorrelationConnection> for CorrelationConnectionType {
    fn from(connection: CorrelationConnection) -> Self {
        Self {
            target_country: connection.target_country.into(),
            correlation_coefficient: connection.correlation_coefficient,
            indicator_category: connection.indicator_category,
            significance_level: connection.significance_level,
            connection_strength: connection.connection_strength,
        }
    }
}

/// Global economic event with impacts
#[derive(SimpleObject)]
pub struct GlobalEventWithImpactsType {
    pub event: GlobalEconomicEventType,
    pub country_impacts: Vec<CountryImpactDetailType>,
    pub affected_country_count: i64,
    pub total_economic_impact: Option<String>,
}

impl From<GlobalEventWithImpacts> for GlobalEventWithImpactsType {
    fn from(event_with_impacts: GlobalEventWithImpacts) -> Self {
        Self {
            event: event_with_impacts.event.into(),
            country_impacts: event_with_impacts.country_impacts.into_iter().map(Into::into).collect(),
            affected_country_count: event_with_impacts.affected_country_count,
            total_economic_impact: event_with_impacts.total_economic_impact.map(|d| d.to_string()),
        }
    }
}

/// Global economic event
#[derive(SimpleObject)]
pub struct GlobalEconomicEventType {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub event_type: String,
    pub severity: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub primary_country_id: Option<String>,
    pub affected_regions: Option<Vec<String>>,
    pub economic_impact_score: Option<String>,
}

impl From<GlobalEconomicEvent> for GlobalEconomicEventType {
    fn from(event: GlobalEconomicEvent) -> Self {
        Self {
            id: event.id.to_string(),
            name: event.name,
            description: event.description,
            event_type: event.event_type,
            severity: event.severity,
            start_date: event.start_date.to_string(),
            end_date: event.end_date.map(|d| d.to_string()),
            primary_country_id: event.primary_country_id.map(|id| id.to_string()),
            affected_regions: event.affected_regions.map(|regions| {
                regions.into_iter().filter_map(|r| r).collect()
            }),
            economic_impact_score: event.economic_impact_score.map(|s| s.to_string()),
        }
    }
}

/// Country impact detail
#[derive(SimpleObject)]
pub struct CountryImpactDetailType {
    pub country: CountryType,
    pub impact: EventCountryImpactType,
    pub impact_severity: String,
    pub recovery_status: String,
}

impl From<CountryImpactDetail> for CountryImpactDetailType {
    fn from(detail: CountryImpactDetail) -> Self {
        Self {
            country: detail.country.into(),
            impact: detail.impact.into(),
            impact_severity: detail.impact_severity,
            recovery_status: detail.recovery_status,
        }
    }
}

/// Event country impact
#[derive(SimpleObject)]
pub struct EventCountryImpactType {
    pub id: String,
    pub event_id: String,
    pub country_id: String,
    pub impact_type: String,
    pub impact_magnitude: Option<String>,
    pub impact_duration_days: Option<i32>,
    pub recovery_time_days: Option<i32>,
    pub confidence_score: Option<String>,
}

impl From<EventCountryImpact> for EventCountryImpactType {
    fn from(impact: EventCountryImpact) -> Self {
        Self {
            id: impact.id.to_string(),
            event_id: impact.event_id.to_string(),
            country_id: impact.country_id.to_string(),
            impact_type: impact.impact_type,
            impact_magnitude: impact.impact_magnitude.map(|m| m.to_string()),
            impact_duration_days: impact.impact_duration_days,
            recovery_time_days: impact.recovery_time_days,
            confidence_score: impact.confidence_score.map(|s| s.to_string()),
        }
    }
}

/// Economic indicator category enum
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum IndicatorCategoryType {
    GDP,
    Trade,
    Employment,
    Inflation,
    MonetaryPolicy,
    FiscalPolicy,
    Financial,
    Demographics,
}

impl std::fmt::Display for IndicatorCategoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndicatorCategoryType::GDP => write!(f, "GDP"),
            IndicatorCategoryType::Trade => write!(f, "Trade"),
            IndicatorCategoryType::Employment => write!(f, "Employment"),
            IndicatorCategoryType::Inflation => write!(f, "Inflation"),
            IndicatorCategoryType::MonetaryPolicy => write!(f, "MonetaryPolicy"),
            IndicatorCategoryType::FiscalPolicy => write!(f, "FiscalPolicy"),
            IndicatorCategoryType::Financial => write!(f, "Financial"),
            IndicatorCategoryType::Demographics => write!(f, "Demographics"),
        }
    }
}

/// Global Analysis Query resolvers
#[derive(Default)]
pub struct GlobalAnalysisQuery;

#[Object]
impl GlobalAnalysisQuery {
    /// Get all countries with their latest economic data
    async fn countries_with_economic_data(
        &self,
        ctx: &Context<'_>,
    ) -> GqlResult<Vec<CountryWithEconomicDataType>> {
        let pool = ctx.data::<DatabasePool>()?;

        let countries = GlobalAnalysisService::get_countries_with_economic_data(pool)
            .await
            .map_err(|e| async_graphql::Error::new(format!("Failed to get countries: {}", e)))?;

        Ok(countries.into_iter().map(Into::into).collect())
    }

    /// Get correlation network for a specific indicator category
    async fn correlation_network(
        &self,
        ctx: &Context<'_>,
        indicator_category: IndicatorCategoryType,
        min_correlation: Option<f64>,
    ) -> GqlResult<Vec<CorrelationNetworkNodeType>> {
        let pool = ctx.data::<DatabasePool>()?;
        let min_corr = min_correlation.unwrap_or(0.3);

        let network = GlobalAnalysisService::get_correlation_network(
            pool,
            &indicator_category.to_string(),
            min_corr,
        )
        .await
        .map_err(|e| async_graphql::Error::new(format!("Failed to get correlation network: {}", e)))?;

        Ok(network.into_iter().map(Into::into).collect())
    }

    /// Get global economic events with their country impacts
    async fn global_events_with_impacts(
        &self,
        ctx: &Context<'_>,
        start_date: Option<String>,
        end_date: Option<String>,
        min_impact_score: Option<f64>,
    ) -> GqlResult<Vec<GlobalEventWithImpactsType>> {
        let pool = ctx.data::<DatabasePool>()?;

        let start = start_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
        let end = end_date
            .as_ref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

        let events = GlobalAnalysisService::get_global_events_with_impacts(
            pool,
            start,
            end,
            min_impact_score,
        )
        .await
        .map_err(|e| async_graphql::Error::new(format!("Failed to get global events: {}", e)))?;

        Ok(events.into_iter().map(Into::into).collect())
    }

    /// Calculate correlations between countries for a specific indicator
    async fn calculate_country_correlations(
        &self,
        ctx: &Context<'_>,
        indicator_category: IndicatorCategoryType,
        start_date: String,
        end_date: String,
        min_correlation: Option<f64>,
    ) -> GqlResult<Vec<CountryCorrelationType>> {
        let pool = ctx.data::<DatabasePool>()?;

        let start = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
            .map_err(|_| async_graphql::Error::new("Invalid start_date format. Use YYYY-MM-DD"))?;
        let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
            .map_err(|_| async_graphql::Error::new("Invalid end_date format. Use YYYY-MM-DD"))?;

        let min_corr = min_correlation.unwrap_or(0.3);

        let correlations = GlobalAnalysisService::calculate_country_correlations(
            pool,
            &indicator_category.to_string(),
            start,
            end,
            min_corr,
        )
        .await
        .map_err(|e| async_graphql::Error::new(format!("Failed to calculate correlations: {}", e)))?;

        Ok(correlations.into_iter().map(Into::into).collect())
    }
}

/// Country correlation GraphQL type
#[derive(SimpleObject)]
pub struct CountryCorrelationType {
    pub id: String,
    pub country_a_id: String,
    pub country_b_id: String,
    pub indicator_category: String,
    pub correlation_coefficient: String,
    pub time_period_start: String,
    pub time_period_end: String,
    pub sample_size: i32,
    pub p_value: Option<String>,
    pub is_significant: bool,
}

impl From<CountryCorrelation> for CountryCorrelationType {
    fn from(correlation: CountryCorrelation) -> Self {
        Self {
            id: correlation.id.to_string(),
            country_a_id: correlation.country_a_id.to_string(),
            country_b_id: correlation.country_b_id.to_string(),
            indicator_category: correlation.indicator_category,
            correlation_coefficient: correlation.correlation_coefficient.to_string(),
            time_period_start: correlation.time_period_start.to_string(),
            time_period_end: correlation.time_period_end.to_string(),
            sample_size: correlation.sample_size,
            p_value: correlation.p_value.map(|p| p.to_string()),
            is_significant: correlation.is_significant,
        }
    }
}
