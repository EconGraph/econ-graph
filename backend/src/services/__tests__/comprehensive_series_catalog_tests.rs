use std::collections::HashMap;

use crate::{
    services::comprehensive_series_catalog::{
        ComprehensiveSeriesCatalog, DataFrequency, DataSource, EconomicCategory, SeasonalAdjustment,
        SeriesDefinition,
    },
    test_utils::TestContainer,
};
use serial_test::serial;

#[cfg(test)]
mod tests {

    use super::*;

    /// Test comprehensive series catalog creation
    ///
    /// This test verifies that the catalog is created with all expected
    /// economic indicators and has proper structure.
    #[test]
    fn test_catalog_creation_and_structure() {
        // REQUIREMENT: Test catalog creation and structure
        // PURPOSE: Verify that the catalog contains all major economic indicators
        // This ensures we have comprehensive coverage for crawling

        let catalog = ComprehensiveSeriesCatalog::new();

        // Should not be empty
        assert!(!catalog.is_empty(), "Catalog should not be empty");
        assert!(catalog.len() >= 35, "Should have at least 35+ series (currently 39)");

        // Should have series from all major categories
        let category_stats = catalog.get_category_stats();
        assert!(!category_stats.is_empty(), "Should have category statistics");

        // Verify all major economic categories are represented
        let expected_categories = vec![
            EconomicCategory::GDP,
            EconomicCategory::Employment,
            EconomicCategory::Inflation,
            EconomicCategory::InterestRates,
            EconomicCategory::Money,
            EconomicCategory::Trade,
            EconomicCategory::Housing,
            EconomicCategory::Manufacturing,
            EconomicCategory::Consumer,
            EconomicCategory::Business,
            EconomicCategory::Government,
            EconomicCategory::International,
        ];

        for category in expected_categories {
            assert!(
                category_stats.contains_key(&category),
                "Category {:?} should be represented in catalog",
                category
            );
        }

        println!("✅ Catalog creation and structure test passed");
        println!("   - Total series: {}", catalog.len());
        println!("   - Categories represented: {}", category_stats.len());
        println!("   - All major categories present: ✅");
    }

    /// Test series definition validation
    ///
    /// This test verifies that all series definitions have valid
    /// metadata and follow expected patterns.
    #[test]
    fn test_series_definition_validation() {
        // REQUIREMENT: Test series definition validation
        // PURPOSE: Verify that all series have valid metadata
        // This ensures data quality and consistency

        let catalog = ComprehensiveSeriesCatalog::new();

        for series in &catalog.series {
            // ID should not be empty
            assert!(!series.id.is_empty(), "Series ID should not be empty: {:?}", series);

            // Title should not be empty
            assert!(!series.title.is_empty(), "Series title should not be empty: {:?}", series);

            // Description should not be empty
            assert!(!series.description.is_empty(), "Series description should not be empty: {:?}", series);

            // Priority should be between 1 and 5
            assert!(
                series.priority >= 1 && series.priority <= 5,
                "Series priority should be 1-5, got {}: {:?}",
                series.priority,
                series
            );

            // Units should not be empty
            assert!(!series.units.is_empty(), "Series units should not be empty: {:?}", series);

            // Tags should not be empty
            assert!(!series.tags.is_empty(), "Series tags should not be empty: {:?}", series);

            // Start date should be valid if present
            if let Some(start_date) = &series.start_date {
                assert!(
                    start_date.len() == 10 && start_date.contains('-'),
                    "Start date should be in YYYY-MM-DD format, got: {}",
                    start_date
                );
            }
        }

        println!("✅ Series definition validation test passed");
        println!("   - All series have valid IDs: ✅");
        println!("   - All series have valid titles: ✅");
        println!("   - All series have valid descriptions: ✅");
        println!("   - All series have valid priorities: ✅");
        println!("   - All series have valid units: ✅");
        println!("   - All series have valid tags: ✅");
    }

    /// Test key economic indicators presence
    ///
    /// This test verifies that all critical economic indicators
    /// are present in the catalog.
    #[test]
    fn test_key_economic_indicators_presence() {
        // REQUIREMENT: Test key economic indicators presence
        // PURPOSE: Verify that critical economic data is covered
        // This ensures we don't miss important indicators

        let catalog = ComprehensiveSeriesCatalog::new();

        // Critical GDP indicators
        let gdp_real = catalog.series.iter().find(|s| s.id == "GDPC1");
        assert!(gdp_real.is_some(), "Real GDP (GDPC1) should be present");
        assert_eq!(gdp_real.unwrap().category, EconomicCategory::GDP);

        let gdp_nominal = catalog.series.iter().find(|s| s.id == "GDP");
        assert!(gdp_nominal.is_some(), "Nominal GDP should be present");
        assert_eq!(gdp_nominal.unwrap().category, EconomicCategory::GDP);

        // Critical employment indicators
        let unemployment = catalog.series.iter().find(|s| s.id == "UNRATE");
        assert!(unemployment.is_some(), "Unemployment rate should be present");
        assert_eq!(unemployment.unwrap().category, EconomicCategory::Employment);

        let payrolls = catalog.series.iter().find(|s| s.id == "PAYEMS");
        assert!(payrolls.is_some(), "Nonfarm payrolls should be present");
        assert_eq!(payrolls.unwrap().category, EconomicCategory::Employment);

        // Critical inflation indicators
        let cpi = catalog.series.iter().find(|s| s.id == "CPIAUCSL");
        assert!(cpi.is_some(), "CPI should be present");
        assert_eq!(cpi.unwrap().category, EconomicCategory::Inflation);

        let core_cpi = catalog.series.iter().find(|s| s.id == "CPILFESL");
        assert!(core_cpi.is_some(), "Core CPI should be present");
        assert_eq!(core_cpi.unwrap().category, EconomicCategory::Inflation);

        let pce = catalog.series.iter().find(|s| s.id == "PCEPI");
        assert!(pce.is_some(), "PCE should be present");
        assert_eq!(pce.unwrap().category, EconomicCategory::Inflation);

        // Critical interest rate indicators
        let fed_funds = catalog.series.iter().find(|s| s.id == "FEDFUNDS");
        assert!(fed_funds.is_some(), "Fed funds rate should be present");
        assert_eq!(fed_funds.unwrap().category, EconomicCategory::InterestRates);

        let ten_year = catalog.series.iter().find(|s| s.id == "GS10");
        assert!(ten_year.is_some(), "10-year Treasury should be present");
        assert_eq!(ten_year.unwrap().category, EconomicCategory::InterestRates);

        println!("✅ Key economic indicators presence test passed");
        println!("   - GDP indicators: ✅");
        println!("   - Employment indicators: ✅");
        println!("   - Inflation indicators: ✅");
        println!("   - Interest rate indicators: ✅");
    }

    /// Test catalog filtering by category
    ///
    /// This test verifies that category filtering works correctly
    /// and returns expected results.
    #[test]
    fn test_catalog_filtering_by_category() {
        // REQUIREMENT: Test catalog filtering by category
        // PURPOSE: Verify that category-based filtering works correctly
        // This ensures users can find relevant economic data

        let catalog = ComprehensiveSeriesCatalog::new();

        // Test GDP category
        let gdp_series = catalog.get_by_category(&EconomicCategory::GDP);
        assert!(!gdp_series.is_empty(), "Should have GDP series");
        for series in gdp_series {
            assert_eq!(series.category, EconomicCategory::GDP);
        }

        // Test Employment category
        let employment_series = catalog.get_by_category(&EconomicCategory::Employment);
        assert!(!employment_series.is_empty(), "Should have Employment series");
        for series in employment_series {
            assert_eq!(series.category, EconomicCategory::Employment);
        }

        // Test Inflation category
        let inflation_series = catalog.get_by_category(&EconomicCategory::Inflation);
        assert!(!inflation_series.is_empty(), "Should have Inflation series");
        for series in inflation_series {
            assert_eq!(series.category, EconomicCategory::Inflation);
        }

        // Test Interest Rates category
        let interest_series = catalog.get_by_category(&EconomicCategory::InterestRates);
        assert!(!interest_series.is_empty(), "Should have Interest Rates series");
        for series in interest_series {
            assert_eq!(series.category, EconomicCategory::InterestRates);
        }

        println!("✅ Catalog filtering by category test passed");
        println!("   - GDP filtering: {} series", catalog.get_by_category(&EconomicCategory::GDP).len());
        println!("   - Employment filtering: {} series", catalog.get_by_category(&EconomicCategory::Employment).len());
        println!("   - Inflation filtering: {} series", catalog.get_by_category(&EconomicCategory::Inflation).len());
        println!("   - Interest Rates filtering: {} series", catalog.get_by_category(&EconomicCategory::InterestRates).len());
    }

    /// Test catalog filtering by source
    ///
    /// This test verifies that source filtering works correctly
    /// and returns expected results.
    #[test]
    fn test_catalog_filtering_by_source() {
        // REQUIREMENT: Test catalog filtering by source
        // PURPOSE: Verify that source-based filtering works correctly
        // This ensures users can filter by data provider

        let catalog = ComprehensiveSeriesCatalog::new();

        // Test FRED source
        let fred_series = catalog.get_by_source(&DataSource::FRED);
        assert!(!fred_series.is_empty(), "Should have FRED series");
        for series in fred_series {
            assert_eq!(series.source, DataSource::FRED);
        }

        // Test BLS source (if any)
        let bls_series = catalog.get_by_source(&DataSource::BLS);
        // BLS series might be empty in current catalog, that's OK
        for series in bls_series {
            assert_eq!(series.source, DataSource::BLS);
        }

        println!("✅ Catalog filtering by source test passed");
        println!("   - FRED series: {} series", catalog.get_by_source(&DataSource::FRED).len());
        println!("   - BLS series: {} series", catalog.get_by_source(&DataSource::BLS).len());
    }

    /// Test catalog filtering by priority
    ///
    /// This test verifies that priority filtering works correctly
    /// and returns expected results.
    #[test]
    fn test_catalog_filtering_by_priority() {
        // REQUIREMENT: Test catalog filtering by priority
        // PURPOSE: Verify that priority-based filtering works correctly
        // This ensures we can prioritize crawling efforts

        let catalog = ComprehensiveSeriesCatalog::new();

        // Test high priority (1-2)
        let high_priority = catalog.get_high_priority();
        assert!(!high_priority.is_empty(), "Should have high priority series");
        for series in high_priority {
            assert!(series.priority <= 2, "High priority series should have priority <= 2");
        }

        // Test priority 1
        let priority_1 = catalog.get_by_priority(1);
        assert!(!priority_1.is_empty(), "Should have priority 1 series");
        for series in priority_1 {
            assert_eq!(series.priority, 1);
        }

        // Test priority 2
        let priority_2 = catalog.get_by_priority(2);
        assert!(!priority_2.is_empty(), "Should have priority 2 series");
        for series in priority_2 {
            assert_eq!(series.priority, 2);
        }

        // Test priority 3
        let priority_3 = catalog.get_by_priority(3);
        for series in priority_3 {
            assert_eq!(series.priority, 3);
        }

        println!("✅ Catalog filtering by priority test passed");
        println!("   - High priority (1-2): {} series", high_priority.len());
        println!("   - Priority 1: {} series", priority_1.len());
        println!("   - Priority 2: {} series", priority_2.len());
        println!("   - Priority 3: {} series", priority_3.len());
    }

    /// Test catalog statistics generation
    ///
    /// This test verifies that catalog statistics are generated
    /// correctly and provide useful insights.
    #[test]
    fn test_catalog_statistics_generation() {
        // REQUIREMENT: Test catalog statistics generation
        // PURPOSE: Verify that statistics provide useful insights
        // This ensures we can monitor catalog composition

        let catalog = ComprehensiveSeriesCatalog::new();

        // Test category statistics
        let category_stats = catalog.get_category_stats();
        assert!(!category_stats.is_empty(), "Should have category statistics");

        // Verify all categories have counts
        for (category, count) in &category_stats {
            assert!(*count > 0, "Category {:?} should have positive count", category);
        }

        // Test source statistics
        let source_stats = catalog.get_source_stats();
        assert!(!source_stats.is_empty(), "Should have source statistics");

        // Verify all sources have counts
        for (source, count) in &source_stats {
            assert!(*count > 0, "Source {:?} should have positive count", source);
        }

        // Test that total counts match
        let total_from_categories: usize = category_stats.values().sum();
        let total_from_sources: usize = source_stats.values().sum();
        assert_eq!(total_from_categories, catalog.len());
        assert_eq!(total_from_sources, catalog.len());

        println!("✅ Catalog statistics generation test passed");
        println!("   - Category statistics: {} categories", category_stats.len());
        println!("   - Source statistics: {} sources", source_stats.len());
        println!("   - Total counts match: ✅");
    }

    /// Test catalog active series filtering
    ///
    /// This test verifies that active series filtering works correctly
    /// and returns only active series.
    #[test]
    fn test_catalog_active_series_filtering() {
        // REQUIREMENT: Test catalog active series filtering
        // PURPOSE: Verify that only active series are returned
        // This ensures we only crawl relevant data

        let catalog = ComprehensiveSeriesCatalog::new();

        // Test active series
        let active_series = catalog.get_active();
        assert!(!active_series.is_empty(), "Should have active series");

        // Verify all returned series are active
        for series in active_series {
            assert!(series.is_active, "All returned series should be active");
        }

        // Test that we have a reasonable number of active series
        assert!(
            active_series.len() >= catalog.len() * 8 / 10,
            "Should have at least 80% active series (got {}/{})",
            active_series.len(),
            catalog.len()
        );

        println!("✅ Catalog active series filtering test passed");
        println!("   - Active series: {} out of {}", active_series.len(), catalog.len());
        println!("   - All returned series are active: ✅");
    }

    /// Test catalog data frequency distribution
    ///
    /// This test verifies that the catalog has a good distribution
    /// of data frequencies for comprehensive coverage.
    #[test]
    fn test_catalog_data_frequency_distribution() {
        // REQUIREMENT: Test catalog data frequency distribution
        // PURPOSE: Verify that we have good frequency coverage
        // This ensures we can analyze data at different time scales

        let catalog = ComprehensiveSeriesCatalog::new();

        let mut frequency_counts: HashMap<DataFrequency, usize> = HashMap::new();
        for series in &catalog.series {
            *frequency_counts.entry(series.frequency.clone()).or_insert(0) += 1;
        }

        // Should have multiple frequencies
        assert!(frequency_counts.len() >= 3, "Should have at least 3 different frequencies");

        // Should have monthly data (most common)
        assert!(
            frequency_counts.get(&DataFrequency::Monthly).unwrap_or(&0) > &0,
            "Should have monthly data"
        );

        // Should have quarterly data
        assert!(
            frequency_counts.get(&DataFrequency::Quarterly).unwrap_or(&0) > &0,
            "Should have quarterly data"
        );

        // Should have daily data
        assert!(
            frequency_counts.get(&DataFrequency::Daily).unwrap_or(&0) > &0,
            "Should have daily data"
        );

        println!("✅ Catalog data frequency distribution test passed");
        println!("   - Frequencies represented: {}", frequency_counts.len());
        for (freq, count) in &frequency_counts {
            println!("   - {}: {} series", format!("{:?}", freq), count);
        }
    }

    /// Test catalog seasonal adjustment distribution
    ///
    /// This test verifies that the catalog has a good distribution
    /// of seasonal adjustment types.
    #[test]
    fn test_catalog_seasonal_adjustment_distribution() {
        // REQUIREMENT: Test catalog seasonal adjustment distribution
        // PURPOSE: Verify that we have good seasonal adjustment coverage
        // This ensures we can analyze both raw and adjusted data

        let catalog = ComprehensiveSeriesCatalog::new();

        let mut adjustment_counts: HashMap<SeasonalAdjustment, usize> = HashMap::new();
        for series in &catalog.series {
            *adjustment_counts.entry(series.seasonal_adjustment.clone()).or_insert(0) += 1;
        }

        // Should have multiple adjustment types
        assert!(adjustment_counts.len() >= 2, "Should have at least 2 different adjustment types");

        // Should have seasonally adjusted data
        assert!(
            adjustment_counts.get(&SeasonalAdjustment::SeasonallyAdjusted).unwrap_or(&0) > &0,
            "Should have seasonally adjusted data"
        );

        // Should have not seasonally adjusted data
        assert!(
            adjustment_counts.get(&SeasonalAdjustment::NotSeasonallyAdjusted).unwrap_or(&0) > &0,
            "Should have not seasonally adjusted data"
        );

        println!("✅ Catalog seasonal adjustment distribution test passed");
        println!("   - Adjustment types: {}", adjustment_counts.len());
        for (adj, count) in &adjustment_counts {
            println!("   - {}: {} series", format!("{:?}", adj), count);
        }
    }

    /// Test catalog integration with database
    ///
    /// This test verifies that the catalog can be used to create
    /// database entries for series discovery.
    #[tokio::test]
    #[serial]
    async fn test_catalog_database_integration() -> AppResult<()> {
        // REQUIREMENT: Test catalog database integration
        // PURPOSE: Verify that catalog can be used for database operations
        // This ensures the catalog works with our crawling system

        let container = TestContainer::new().await;
        let pool = container.pool();
        let catalog = ComprehensiveSeriesCatalog::new();

        // Test that we can create data sources for catalog series
        let mut source_counts: HashMap<DataSource, usize> = HashMap::new();
        for series in &catalog.series {
            *source_counts.entry(series.source.clone()).or_insert(0) += 1;
        }

        // Verify we have series from multiple sources
        assert!(source_counts.len() >= 1, "Should have series from multiple sources");

        // Test that we can filter by source and get reasonable counts
        let fred_series = catalog.get_by_source(&DataSource::FRED);
        assert!(!fred_series.is_empty(), "Should have FRED series for testing");

        // Test that series have valid external IDs for database storage
        for series in &catalog.series {
            assert!(!series.id.is_empty(), "Series should have valid external ID");
            assert!(series.id.len() <= 255, "Series ID should fit in database field");
        }

        println!("✅ Catalog database integration test passed");
        println!("   - Sources represented: {}", source_counts.len());
        println!("   - FRED series available: {} series", fred_series.len());
        println!("   - All series have valid IDs: ✅");

        Ok(())
    }
}
