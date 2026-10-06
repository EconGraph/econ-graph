// REQUIREMENT: Comprehensive database integration tests for economic series
// PURPOSE: Test series operations with real PostgreSQL database
// This ensures the economic series data model works correctly with all expected operations

use crate::models::{
    data_source::{DataSource, NewDataSource},
    economic_series::{EconomicSeries, NewEconomicSeries, SeriesFrequency},
};
use crate::test_utils::db_test;
use crate::test_utils::{DatabaseTestExt, TestContainer};
use chrono::NaiveDate;
use std::sync::Arc;
use uuid::Uuid;

// Simple unit tests that don't require complex database integration
#[cfg(test)]
mod simple_tests {
    use super::*;

    #[test]
    fn test_series_frequency_conversion() {
        // REQUIREMENT: Test series frequency enumeration
        // PURPOSE: Verify that frequency types convert correctly to strings
        // This tests the basic enum functionality for data categorization

        let monthly = SeriesFrequency::Monthly.to_string();
        let quarterly = SeriesFrequency::Quarterly.to_string();
        let annual = SeriesFrequency::Annual.to_string();

        assert_eq!(monthly, "Monthly");
        assert_eq!(quarterly, "Quarterly");
        assert_eq!(annual, "Annual");
    }

    #[test]
    fn test_new_economic_series_creation() {
        // REQUIREMENT: Test economic series struct creation
        // PURPOSE: Verify that series can be created with correct field types
        // This tests the basic struct functionality without database interaction

        let series = NewEconomicSeries {
            source_id: Uuid::new_v4(),
            external_id: "TEST_001".to_string(),
            title: "Test Economic Series".to_string(),
            description: Some("A test series for validation".to_string()),
            units: Some("Percent".to_string()),
            frequency: "monthly".to_string(),
            seasonal_adjustment: None,
            start_date: Some(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap()),
            end_date: None,
            is_active: true,
            first_discovered_at: Some(chrono::Utc::now()),
            last_crawled_at: None,
            first_missing_date: None,
            crawl_status: None,
            crawl_error_message: None,
            dataset_id: Uuid::new_v4(),
            dimensions: Default::default(),
            default_measure: None,
        };

        assert_eq!(series.external_id, "TEST_001");
        assert_eq!(series.title, "Test Economic Series");
        assert_eq!(series.frequency, "monthly");
        assert!(series.is_active);
    }

    #[test]
    fn classify_raw_matches_each_source_adapter_spelling() {
        // REQUIREMENT: classify_raw must recognize every frequency string the release-1 source
        // adapters emit, not just the bare label, so a search filter finds series from any source.
        use SeriesFrequency::*;

        let cases = [
            // FRED stores long descriptive text, not a bare label.
            ("Daily, Close", Daily),
            ("Weekly, Ending Friday", Weekly),
            ("Monthly", Monthly),
            ("Quarterly", Quarterly),
            ("Annual", Annual),
            ("Semiannual", SemiAnnual),
            // BEA
            ("Annual", Annual),
            // BLS
            ("Monthly", Monthly),
            ("Quarterly", Quarterly),
            ("Semi-Annual", SemiAnnual),
            // FHFA (title-cased)
            ("Monthly", Monthly),
            // World Bank / Census BDS
            ("Annual", Annual),
            // Unknown text falls back to Irregular.
            ("Biennial", Irregular),
            ("", Irregular),
        ];

        for (raw, expected) in cases {
            assert_eq!(
                SeriesFrequency::classify_raw(raw),
                expected,
                "classify_raw({raw:?})"
            );
        }
    }

    #[test]
    fn sql_like_patterns_match_classify_raw() {
        // REQUIREMENT: the SQL patterns used to filter on frequency must agree with classify_raw,
        // so a WHERE ... ILIKE ANY(patterns) filter finds exactly what classify_raw would accept.
        for variant in [
            SeriesFrequency::Daily,
            SeriesFrequency::Weekly,
            SeriesFrequency::Monthly,
            SeriesFrequency::Quarterly,
            SeriesFrequency::SemiAnnual,
            SeriesFrequency::Annual,
        ] {
            for pattern in variant.sql_like_patterns() {
                let prefix = pattern.strip_suffix('%').expect("pattern ends with %");
                assert_eq!(
                    SeriesFrequency::classify_raw(prefix),
                    variant,
                    "pattern {pattern:?} should classify back to {variant:?}"
                );
            }
        }

        assert!(SeriesFrequency::Irregular.sql_like_patterns().is_empty());
    }
}

// Complex database integration tests disabled - replaced with modern async integration tests

/*
db_test!(test_create_and_retrieve_series, |container: Arc<TestContainer>| async move {
    // Complex integration test - disabled for now
});

db_test!(test_series_by_data_source, |container: Arc<TestContainer>| async move {
    // Complex integration test - disabled for now
});

db_test!(test_active_inactive_series, |container: Arc<TestContainer>| async move {
    // Complex integration test - disabled for now
});

db_test!(test_series_search_by_title, |container: Arc<TestContainer>| async move {
    // Complex integration test - disabled for now
});

db_test!(test_series_unique_constraints, |container: Arc<TestContainer>| async move {
    // Complex integration test - disabled for now
});
*/
