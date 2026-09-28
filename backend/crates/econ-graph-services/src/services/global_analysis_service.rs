use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{
    Country, CountryWithEconomicData, GlobalEconomicEvent, GlobalEventWithImpacts, TradePartner,
};
use econ_graph_core::schema::countries;
use rust_decimal::prelude::{Decimal, FromStr};
use std::collections::HashMap;
// Removed duplicate Decimal import - already imported from prelude

/// Service for global economic network analysis
pub struct GlobalAnalysisService;

impl GlobalAnalysisService {
    /// Get all active countries with basic economic data
    pub async fn get_countries_with_economic_data(
        pool: &DatabasePool,
    ) -> AppResult<Vec<CountryWithEconomicData>> {
        use econ_graph_core::schema::countries::dsl::*;

        let mut conn = pool.get().await.map_err(|e| {
            tracing::error!("Failed to get database connection: {}", e);
            AppError::database_error("Database connection failed".to_string())
        })?;

        let countries_data = countries
            .filter(is_active.eq(true))
            .order(name.asc())
            .load::<Country>(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load countries: {}", e);
                AppError::database_error(e.to_string())
            })?;

        let mut result = Vec::new();

        for country in countries_data {
            let economic_data = Self::get_latest_economic_indicators(&mut conn, country.id).await?;
            let trade_partners = Self::get_top_trade_partners(&mut conn, country.id, 5).await?;
            let health_score = Self::calculate_economic_health_score(&economic_data);

            result.push(CountryWithEconomicData {
                country,
                latest_gdp: economic_data
                    .get("GDP")
                    .map(|d| BigDecimal::from_str(&d.to_string()).unwrap_or_default()),
                latest_gdp_growth: economic_data
                    .get("GDP_GROWTH")
                    .map(|d| BigDecimal::from_str(&d.to_string()).unwrap_or_default()),
                latest_inflation: economic_data
                    .get("INFLATION")
                    .map(|d| BigDecimal::from_str(&d.to_string()).unwrap_or_default()),
                latest_unemployment: economic_data
                    .get("UNEMPLOYMENT")
                    .map(|d| BigDecimal::from_str(&d.to_string()).unwrap_or_default()),
                trade_partners,
                economic_health_score: health_score,
            });
        }

        Ok(result)
    }

    /// Get global economic events with their country impacts
    pub async fn get_global_events_with_impacts(
        pool: &DatabasePool,
        start_date_filter: Option<NaiveDate>,
        end_date_filter: Option<NaiveDate>,
        min_impact_score: Option<f64>,
    ) -> AppResult<Vec<GlobalEventWithImpacts>> {
        use econ_graph_core::schema::global_economic_events::dsl::*;

        let mut conn = pool.get().await.map_err(|e| {
            tracing::error!("Failed to get database connection: {}", e);
            AppError::database_error("Database connection failed".to_string())
        })?;

        let mut query = global_economic_events.into_boxed();

        if let Some(start) = start_date_filter {
            query = query.filter(start_date.ge(start));
        }

        if let Some(end) = end_date_filter {
            query = query.filter(end_date.le(end));
        }

        if let Some(min_score) = min_impact_score {
            query = query.filter(economic_impact_score.ge(BigDecimal::from(min_score as i64)));
        }

        let events = query
            .order(start_date.desc())
            .load::<GlobalEconomicEvent>(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load global events: {}", e);
                AppError::database_error(e.to_string())
            })?;

        let mut result = Vec::new();

        for event in events {
            let country_impacts = Self::get_event_country_impacts(&mut conn, event.id).await?;
            let affected_country_count = country_impacts.len() as i64;
            let total_economic_impact = event.economic_impact_score.clone();

            result.push(GlobalEventWithImpacts {
                event,
                country_impacts,
                affected_country_count,
                total_economic_impact,
            });
        }

        Ok(result)
    }

    /// Get top trade partners for a country
    async fn get_top_trade_partners(
        conn: &mut AsyncPgConnection,
        country_id: uuid::Uuid,
        limit: i64,
    ) -> AppResult<Vec<TradePartner>> {
        use econ_graph_core::schema::trade_relationships::dsl::*;

        let trade_data = trade_relationships
            .filter(
                exporter_country_id
                    .eq(country_id)
                    .or(importer_country_id.eq(country_id)),
            )
            .order(export_value_usd.desc().nulls_last())
            .limit(limit)
            .load::<econ_graph_core::models::TradeRelationship>(conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load trade relationships: {}", e);
                AppError::database_error(e.to_string())
            })?;

        let mut partners = Vec::new();

        for trade in trade_data {
            let partner_id = if trade.exporter_country_id == country_id {
                trade.importer_country_id
            } else {
                trade.exporter_country_id
            };

            let partner_country = countries::table
                .find(partner_id)
                .first::<Country>(conn)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to load partner country: {}", e);
                    AppError::database_error(e.to_string())
                })?;

            let trade_value = trade.export_value_usd.unwrap_or_default();
            let trade_intensity_value = trade
                .trade_intensity
                .map(|t| t.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0);
            let relationship_type = if trade.exporter_country_id == country_id {
                "Export".to_string()
            } else {
                "Import".to_string()
            };

            partners.push(TradePartner {
                country: partner_country,
                trade_value_usd: trade_value,
                trade_intensity: trade_intensity_value,
                relationship_type,
            });
        }

        Ok(partners)
    }

    /// Get latest economic indicators for a country
    async fn get_latest_economic_indicators(
        conn: &mut AsyncPgConnection,
        country_id: uuid::Uuid,
    ) -> AppResult<HashMap<String, Decimal>> {
        use econ_graph_core::schema::global_economic_indicators::dsl::*;
        use econ_graph_core::schema::global_indicator_data::dsl::*;

        let indicators = global_economic_indicators
            .filter(country_id.eq(country_id))
            .load::<econ_graph_core::models::GlobalEconomicIndicator>(conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load indicators: {}", e);
                AppError::database_error(e.to_string())
            })?;

        let mut result = HashMap::new();

        for indicator in indicators {
            if let Ok(latest_data) = global_indicator_data
                .filter(indicator_id.eq(indicator.id))
                .filter(value.is_not_null())
                .order(date.desc())
                .first::<econ_graph_core::models::GlobalIndicatorData>(conn)
                .await
            {
                // Convert BigDecimal to Decimal for compatibility
                let decimal_value =
                    Decimal::from_str(&latest_data.value.unwrap_or_default().to_string())
                        .unwrap_or_default();
                result.insert(indicator.category.clone(), decimal_value);
            }
        }

        Ok(result)
    }

    /// Calculate economic health score based on indicators
    fn calculate_economic_health_score(indicators: &HashMap<String, Decimal>) -> Option<f64> {
        let mut score: f64 = 50.0; // Base score
        let mut factors = 0;

        // GDP growth factor
        if let Some(gdp_growth) = indicators.get("GDP_GROWTH") {
            let growth_rate = gdp_growth.to_f64().unwrap_or(0.0);
            score += (growth_rate * 10.0).clamp(-20.0, 20.0);
            factors += 1;
        }

        // Unemployment factor (inverse)
        if let Some(unemployment) = indicators.get("UNEMPLOYMENT") {
            let unemployment_rate = unemployment.to_f64().unwrap_or(0.0);
            score -= (unemployment_rate * 2.0).min(20.0);
            factors += 1;
        }

        // Inflation factor (optimal around 2%)
        if let Some(inflation) = indicators.get("INFLATION") {
            let inflation_rate = inflation.to_f64().unwrap_or(0.0);
            let inflation_deviation = (inflation_rate - 2.0).abs();
            score -= (inflation_deviation * 5.0).min(15.0);
            factors += 1;
        }

        if factors > 0 {
            Some(score.clamp(0.0, 100.0))
        } else {
            None
        }
    }

    /// Get country impacts for a global event
    async fn get_event_country_impacts(
        conn: &mut AsyncPgConnection,
        event_id: uuid::Uuid,
    ) -> AppResult<Vec<econ_graph_core::models::CountryImpactDetail>> {
        use econ_graph_core::schema::event_country_impacts::dsl::*;

        let impacts = event_country_impacts
            .filter(event_id.eq(event_id))
            .load::<econ_graph_core::models::EventCountryImpact>(conn)
            .await
            .map_err(|e| {
                tracing::error!("Failed to load event impacts: {}", e);
                AppError::database_error(e.to_string())
            })?;

        let mut result = Vec::new();

        for impact in impacts {
            let country = countries::table
                .find(impact.country_id)
                .first::<Country>(conn)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to load country: {}", e);
                    AppError::database_error(e.to_string())
                })?;

            let impact_severity = Self::classify_impact_severity(&impact);
            let recovery_status = Self::classify_recovery_status(&impact);

            result.push(econ_graph_core::models::CountryImpactDetail {
                country,
                impact,
                impact_severity,
                recovery_status,
            });
        }

        Ok(result)
    }

    /// Classify impact severity based on magnitude
    fn classify_impact_severity(impact: &econ_graph_core::models::EventCountryImpact) -> String {
        match impact
            .impact_magnitude
            .as_ref()
            .map(|m| m.to_f64().unwrap_or(0.0).abs())
        {
            Some(magnitude) if magnitude >= 10.0 => "Critical".to_string(),
            Some(magnitude) if magnitude >= 5.0 => "Severe".to_string(),
            Some(magnitude) if magnitude >= 2.0 => "Moderate".to_string(),
            _ => "Mild".to_string(),
        }
    }

    /// Classify recovery status based on recovery time
    fn classify_recovery_status(impact: &econ_graph_core::models::EventCountryImpact) -> String {
        match impact.recovery_time_days {
            Some(days) if days <= 90 => "Recovered".to_string(),
            Some(days) if days <= 365 => "Recovering".to_string(),
            _ => "Ongoing".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use econ_graph_core::models::{
        NewCountry, NewEventCountryImpact, NewGlobalEconomicEvent, NewGlobalEconomicIndicator,
        NewGlobalIndicatorData, NewTradeRelationship,
    };
    use econ_graph_core::test_utils::TestContainer;
    use serial_test::serial;
    use std::sync::Arc;
    use uuid::Uuid;

    /// Test helper to create test data for global analysis
    async fn setup_test_data(container: &TestContainer) -> AppResult<(Uuid, Uuid, Uuid)> {
        let pool = container.pool();
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::database_error(format!("Failed to get connection: {}", e)))?;

        // Generate unique test identifiers to avoid conflicts with migration data
        let test_id = Uuid::new_v4().to_string()[..8].to_string(); // Use 8 chars for better uniqueness

        // Create test countries using NewCountry structs with TEST prefix to avoid conflicts
        let country_a = NewCountry {
            iso_code: format!("T{}", &test_id[..2]), // T + 2 chars = 3 total
            iso_code_2: format!("T{}", &test_id[..1]), // T + 1 char = 2 total
            name: format!("TEST United States {}", test_id),
            region: "North America".to_string(),
            sub_region: Some("Northern America".to_string()),
            income_group: Some("High income".to_string()),
            population: Some(331_000_000),
            gdp_usd: Some(BigDecimal::from(25_000_000_000_000i64)),
            gdp_per_capita_usd: Some(BigDecimal::from(75_000i64)),
            latitude: Some(BigDecimal::from_str("39.8283").unwrap()),
            longitude: Some(BigDecimal::from_str("-98.5795").unwrap()),
            currency_code: Some("USD".to_string()),
            is_active: Some(true),
        };

        let country_b = NewCountry {
            iso_code: format!("C{}", &test_id[..2]), // C + 2 chars = 3 total
            iso_code_2: format!("C{}", &test_id[..1]), // C + 1 char = 2 total
            name: format!("TEST China {}", test_id),
            region: "Asia".to_string(),
            sub_region: Some("Eastern Asia".to_string()),
            income_group: Some("Upper middle income".to_string()),
            population: Some(1_400_000_000),
            gdp_usd: Some(BigDecimal::from(17_000_000_000_000i64)),
            gdp_per_capita_usd: Some(BigDecimal::from(12_000i64)),
            latitude: Some(BigDecimal::from_str("35.8617").unwrap()),
            longitude: Some(BigDecimal::from_str("104.1954").unwrap()),
            currency_code: Some("CNY".to_string()),
            is_active: Some(true),
        };

        let country_c = NewCountry {
            iso_code: format!("G{}", &test_id[..2]), // G + 2 chars = 3 total
            iso_code_2: format!("G{}", &test_id[..1]), // G + 1 char = 2 total
            name: format!("TEST Germany {}", test_id),
            region: "Europe".to_string(),
            sub_region: Some("Western Europe".to_string()),
            income_group: Some("High income".to_string()),
            population: Some(83_000_000),
            gdp_usd: Some(BigDecimal::from(4_000_000_000_000i64)),
            gdp_per_capita_usd: Some(BigDecimal::from(48_000i64)),
            latitude: Some(BigDecimal::from_str("51.1657").unwrap()),
            longitude: Some(BigDecimal::from_str("10.4515").unwrap()),
            currency_code: Some("EUR".to_string()),
            is_active: Some(true),
        };

        // Insert countries
        let country_a_result = diesel::insert_into(econ_graph_core::schema::countries::table)
            .values(&country_a)
            .returning(Country::as_returning())
            .get_result::<Country>(&mut conn)
            .await
            .map_err(|e| AppError::database_error(format!("Failed to create country A: {}", e)))?;

        let country_b_result = diesel::insert_into(econ_graph_core::schema::countries::table)
            .values(&country_b)
            .returning(Country::as_returning())
            .get_result::<Country>(&mut conn)
            .await
            .map_err(|e| AppError::database_error(format!("Failed to create country B: {}", e)))?;

        let country_c_result = diesel::insert_into(econ_graph_core::schema::countries::table)
            .values(&country_c)
            .returning(Country::as_returning())
            .get_result::<Country>(&mut conn)
            .await
            .map_err(|e| AppError::database_error(format!("Failed to create country C: {}", e)))?;

        // Create economic indicators for country A
        let gdp_indicator = NewGlobalEconomicIndicator {
            country_id: country_a_result.id,
            indicator_code: "GDP".to_string(),
            indicator_name: "Gross Domestic Product".to_string(),
            category: "GDP".to_string(),
            subcategory: Some("Total economic output".to_string()),
            unit: Some("USD".to_string()),
            frequency: "Annual".to_string(),
        };

        let gdp_growth_indicator = NewGlobalEconomicIndicator {
            country_id: country_a_result.id,
            indicator_code: "GDP_GROWTH".to_string(),
            indicator_name: "GDP Growth Rate".to_string(),
            category: "GDP_GROWTH".to_string(),
            subcategory: Some("Annual GDP growth percentage".to_string()),
            unit: Some("Percent".to_string()),
            frequency: "Annual".to_string(),
        };

        // Insert indicators
        let gdp_indicator_result =
            diesel::insert_into(econ_graph_core::schema::global_economic_indicators::table)
                .values(&gdp_indicator)
                .returning(econ_graph_core::models::GlobalEconomicIndicator::as_returning())
                .get_result::<econ_graph_core::models::GlobalEconomicIndicator>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::database_error(format!("Failed to create GDP indicator: {}", e))
                })?;

        let gdp_growth_indicator_result =
            diesel::insert_into(econ_graph_core::schema::global_economic_indicators::table)
                .values(&gdp_growth_indicator)
                .returning(econ_graph_core::models::GlobalEconomicIndicator::as_returning())
                .get_result::<econ_graph_core::models::GlobalEconomicIndicator>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::database_error(format!(
                        "Failed to create GDP growth indicator: {}",
                        e
                    ))
                })?;

        // Create indicator data
        let gdp_data = NewGlobalIndicatorData {
            indicator_id: gdp_indicator_result.id,
            date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            value: Some(BigDecimal::from(25_000_000_000_000i64)),
            is_preliminary: Some(false),
            data_source: "Test Data".to_string(),
        };

        let gdp_growth_data = NewGlobalIndicatorData {
            indicator_id: gdp_growth_indicator_result.id,
            date: NaiveDate::from_ymd_opt(2023, 12, 31).unwrap(),
            value: Some(BigDecimal::from_str("2.5").unwrap()),
            is_preliminary: Some(false),
            data_source: "Test Data".to_string(),
        };

        diesel::insert_into(econ_graph_core::schema::global_indicator_data::table)
            .values(&gdp_data)
            .execute(&mut conn)
            .await
            .map_err(|e| AppError::database_error(format!("Failed to create GDP data: {}", e)))?;

        diesel::insert_into(econ_graph_core::schema::global_indicator_data::table)
            .values(&gdp_growth_data)
            .execute(&mut conn)
            .await
            .map_err(|e| {
                AppError::database_error(format!("Failed to create GDP growth data: {}", e))
            })?;

        // Create trade relationship
        let trade_relationship = NewTradeRelationship {
            exporter_country_id: country_a_result.id,
            importer_country_id: country_b_result.id,
            trade_flow_type: "Export".to_string(),
            year: 2023,
            export_value_usd: Some(BigDecimal::from(500_000_000_000i64)),
            import_value_usd: Some(BigDecimal::from(400_000_000_000i64)),
            trade_balance_usd: Some(BigDecimal::from(100_000_000_000i64)),
            trade_intensity: Some(BigDecimal::from_str("0.8").unwrap()),
        };

        diesel::insert_into(econ_graph_core::schema::trade_relationships::table)
            .values(&trade_relationship)
            .execute(&mut conn)
            .await
            .map_err(|e| {
                AppError::database_error(format!("Failed to create trade relationship: {}", e))
            })?;

        // Create global economic event with unique name to avoid conflicts with migration data
        let event = NewGlobalEconomicEvent {
            name: format!("TEST COVID-19 Pandemic {}", test_id),
            description: Some("Global economic disruption from COVID-19".to_string()),
            event_type: "Pandemic".to_string(),
            severity: "Critical".to_string(),
            start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(2022, 12, 31).unwrap()),
            primary_country_id: Some(country_a_result.id),
            affected_regions: Some(vec![Some("Global".to_string())]),
            economic_impact_score: Some(BigDecimal::from_str("9.5").unwrap()),
        };

        let event_result =
            diesel::insert_into(econ_graph_core::schema::global_economic_events::table)
                .values(&event)
                .returning(GlobalEconomicEvent::as_returning())
                .get_result::<GlobalEconomicEvent>(&mut conn)
                .await
                .map_err(|e| {
                    AppError::database_error(format!("Failed to create global event: {}", e))
                })?;

        // Create event country impact
        let impact = NewEventCountryImpact {
            event_id: event_result.id,
            country_id: country_a_result.id,
            impact_type: "Economic".to_string(),
            impact_magnitude: Some(BigDecimal::from_str("-5.2").unwrap()),
            impact_duration_days: Some(365),
            recovery_time_days: Some(730),
            confidence_score: Some(BigDecimal::from_str("0.9").unwrap()),
        };

        diesel::insert_into(econ_graph_core::schema::event_country_impacts::table)
            .values(&impact)
            .execute(&mut conn)
            .await
            .map_err(|e| {
                AppError::database_error(format!("Failed to create event impact: {}", e))
            })?;

        Ok((
            country_a_result.id,
            country_b_result.id,
            country_c_result.id,
        ))
    }

    #[tokio::test]
    #[serial]
    async fn test_get_countries_with_economic_data() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();

        // Setup test data
        let (country_a_id, _country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Test the service method
        let result = GlobalAnalysisService::get_countries_with_economic_data(pool)
            .await
            .expect("Failed to get countries with economic data");

        // Verify results
        assert!(!result.is_empty(), "Should return at least one country");

        let usa_country = result
            .iter()
            .find(|c| c.country.id == country_a_id)
            .expect("Should find USA country");

        assert!(usa_country.country.name.starts_with("TEST United States"));
        assert!(usa_country.country.iso_code.starts_with("T"));
        assert!(usa_country.latest_gdp.is_some(), "Should have GDP data");
        assert!(
            usa_country.latest_gdp_growth.is_some(),
            "Should have GDP growth data"
        );
        assert!(
            usa_country.economic_health_score.is_some(),
            "Should have health score"
        );
        assert!(
            !usa_country.trade_partners.is_empty(),
            "Should have trade partners"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_get_global_events_with_impacts() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();

        // Setup test data
        let (_country_a_id, _country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Test the service method
        let result = GlobalAnalysisService::get_global_events_with_impacts(
            pool, None, // start_date_filter
            None, // end_date_filter
            None, // min_impact_score
        )
        .await
        .expect("Failed to get global events with impacts");

        // Verify results
        assert!(!result.is_empty(), "Should return at least one event");

        // Find our test event among all events
        let test_event = result
            .iter()
            .find(|e| e.event.name.starts_with("TEST COVID-19 Pandemic"))
            .expect("Should find our test event");

        println!("Event name: {}", test_event.event.name);
        println!("Event type: {}", test_event.event.event_type);
        println!("Event severity: {}", test_event.event.severity);
        assert_eq!(test_event.event.event_type, "Pandemic");
        assert_eq!(test_event.event.severity, "Critical");
        assert!(
            !test_event.country_impacts.is_empty(),
            "Should have country impacts"
        );
        assert!(
            test_event.affected_country_count > 0,
            "Should have affected countries"
        );
        assert!(
            test_event.total_economic_impact.is_some(),
            "Should have economic impact score"
        );
    }

    /// Test just the event insertion to verify data is stored correctly
    #[tokio::test]
    #[serial]
    async fn test_event_insertion_only() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get connection");

        // Create a simple test event
        let test_event = NewGlobalEconomicEvent {
            name: "Test Event".to_string(),
            description: Some("Test description".to_string()),
            event_type: "Pandemic".to_string(),
            severity: "Critical".to_string(),
            start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            end_date: Some(NaiveDate::from_ymd_opt(2022, 12, 31).unwrap()),
            primary_country_id: None,
            affected_regions: Some(vec![Some("Global".to_string())]),
            economic_impact_score: Some(BigDecimal::from_str("9.5").unwrap()),
        };

        // Insert the event
        let inserted_event =
            diesel::insert_into(econ_graph_core::schema::global_economic_events::table)
                .values(&test_event)
                .returning(GlobalEconomicEvent::as_returning())
                .get_result::<GlobalEconomicEvent>(&mut conn)
                .await
                .expect("Failed to insert test event");

        println!("Inserted event type: {}", inserted_event.event_type);
        assert_eq!(inserted_event.event_type, "Pandemic");
        assert_eq!(inserted_event.name, "Test Event");
    }

    /// Test direct database query to see what's actually stored
    #[tokio::test]
    #[serial]
    async fn test_direct_event_query() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();
        let mut conn = pool.get().await.expect("Failed to get connection");

        // Setup test data
        let (_country_a_id, _country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Query events directly from database - look for our test event
        use econ_graph_core::schema::global_economic_events::dsl::*;
        let events = global_economic_events
            .filter(name.like("TEST COVID-19 Pandemic%"))
            .load::<GlobalEconomicEvent>(&mut conn)
            .await
            .expect("Failed to query events");

        assert!(!events.is_empty(), "Should find the test event");
        let event = &events[0];
        println!("Direct query - Event type: {}", event.event_type);
        assert_eq!(event.event_type, "Pandemic");
    }

    #[tokio::test]
    #[serial]
    async fn test_get_global_events_with_filters() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();

        // Setup test data
        let (_country_a_id, _country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Test with date filters that include our test event (2020-2022)
        let start_filter = NaiveDate::from_ymd_opt(2019, 1, 1);
        let end_filter = NaiveDate::from_ymd_opt(2023, 12, 31); // Extended to include our test event
        let min_impact = Some(5.0);

        let result = GlobalAnalysisService::get_global_events_with_impacts(
            pool,
            start_filter,
            end_filter,
            min_impact,
        )
        .await
        .expect("Failed to get filtered global events");

        // Verify results - find our test event
        let test_event = result
            .iter()
            .find(|e| e.event.name.starts_with("TEST COVID-19 Pandemic"))
            .expect("Should find our test event within the date range");

        assert!(test_event.event.start_date >= start_filter.unwrap());
        assert!(
            test_event
                .event
                .end_date
                .unwrap_or(test_event.event.start_date)
                <= end_filter.unwrap()
        );

        if let Some(impact_score) = &test_event.total_economic_impact {
            assert!(impact_score.to_f64().unwrap_or(0.0) >= 5.0);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_calculate_economic_health_score() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();

        // Setup test data
        let (country_a_id, _country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Get countries with economic data
        let result = GlobalAnalysisService::get_countries_with_economic_data(pool)
            .await
            .expect("Failed to get countries with economic data");

        let usa_country = result
            .iter()
            .find(|c| c.country.id == country_a_id)
            .expect("Should find USA country");

        // Verify health score calculation
        if let Some(health_score) = usa_country.economic_health_score {
            assert!(
                health_score >= 0.0 && health_score <= 100.0,
                "Health score should be between 0 and 100, got: {}",
                health_score
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_trade_partners_retrieval() {
        let container = TestContainer::new().await;
        let _ = container.clean_database().await; // Clean database to avoid test pollution
        let pool = container.pool();

        // Setup test data
        let (country_a_id, country_b_id, _country_c_id) = setup_test_data(&container)
            .await
            .expect("Failed to setup test data");

        // Get countries with economic data
        let result = GlobalAnalysisService::get_countries_with_economic_data(pool)
            .await
            .expect("Failed to get countries with economic data");

        let usa_country = result
            .iter()
            .find(|c| c.country.id == country_a_id)
            .expect("Should find USA country");

        // Verify trade partners
        assert!(
            !usa_country.trade_partners.is_empty(),
            "Should have trade partners"
        );

        let china_partner = usa_country
            .trade_partners
            .iter()
            .find(|p| p.country.id == country_b_id)
            .expect("Should find China as trade partner");

        assert!(china_partner.country.name.starts_with("TEST China"));
        assert!(china_partner.trade_value_usd > 0);
        assert!(china_partner.trade_intensity > 0.0);
        assert!(!china_partner.relationship_type.is_empty());
    }
}
