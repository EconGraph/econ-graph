use crate::config::ApiKeyConfig;

#[cfg(test)]
mod tests {

    use super::*;
    use std::env;

    /// Test API key configuration loading from environment
    ///
    /// This test verifies that our API key management system correctly
    /// loads keys from environment variables and handles missing keys.
    #[test]
    fn test_api_key_loading_from_env() {
        // REQUIREMENT: Test API key configuration loading
        // PURPOSE: Verify that API keys are loaded correctly from environment
        // This ensures the environment-based configuration works properly

        // Clear any existing environment variables
        env::remove_var("FRED_API_KEY");
        env::remove_var("BLS_API_KEY");
        env::remove_var("CENSUS_API_KEY");
        env::remove_var("BEA_API_KEY");

        // Test loading with no keys set
        let config = ApiKeyConfig::from_env();
        assert!(!config.has_any_keys(), "Should have no keys when none are set");

        // Test loading with some keys set
        env::set_var("FRED_API_KEY", "test_fred_key");
        env::set_var("BLS_API_KEY", "test_bls_key");

        let config = ApiKeyConfig::from_env();
        assert!(config.has_any_keys(), "Should have keys when some are set");
        assert!(config.has_required_key("FRED_API_KEY"), "Should have FRED key");
        assert!(config.has_required_key("BLS_API_KEY"), "Should have BLS key");
        assert!(!config.has_required_key("CENSUS_API_KEY"), "Should not have CENSUS key");

        // Test key retrieval
        assert_eq!(config.get_key("FRED_API_KEY"), Some("test_fred_key".to_string()));
        assert_eq!(config.get_key("BLS_API_KEY"), Some("test_bls_key".to_string()));
        assert_eq!(config.get_key("CENSUS_API_KEY"), None);

        // Clean up
        env::remove_var("FRED_API_KEY");
        env::remove_var("BLS_API_KEY");
        env::remove_var("CENSUS_API_KEY");
        env::remove_var("BEA_API_KEY");

        println!("✅ API key loading test passed");
        println!("   - Empty config handled: ✅");
        println!("   - Partial config handled: ✅");
        println!("   - Key retrieval working: ✅");
    }

    /// Test configured data sources detection
    ///
    /// This test verifies that we can correctly identify which data sources
    /// have API keys configured.
    #[test]
    fn test_configured_sources_detection() {
        // REQUIREMENT: Test configured data sources detection
        // PURPOSE: Verify that we can identify which sources have API keys
        // This ensures proper data source filtering

        // Clear environment
        env::remove_var("FRED_API_KEY");
        env::remove_var("BLS_API_KEY");
        env::remove_var("CENSUS_API_KEY");
        env::remove_var("BEA_API_KEY");

        // Test with no keys
        let config = ApiKeyConfig::from_env();
        let configured = config.get_configured_sources();
        assert!(configured.is_empty(), "Should have no configured sources");

        // Test with some keys
        env::set_var("FRED_API_KEY", "test_key");
        env::set_var("CENSUS_API_KEY", "test_key");

        let config = ApiKeyConfig::from_env();
        let configured = config.get_configured_sources();

        assert!(configured.contains(&"FRED".to_string()), "Should include FRED");
        assert!(configured.contains(&"CENSUS".to_string()), "Should include CENSUS");
        assert!(!configured.contains(&"BLS".to_string()), "Should not include BLS");
        assert!(!configured.contains(&"BEA".to_string()), "Should not include BEA");

        // Clean up
        env::remove_var("FRED_API_KEY");
        env::remove_var("BLS_API_KEY");
        env::remove_var("CENSUS_API_KEY");
        env::remove_var("BEA_API_KEY");

        println!("✅ Configured sources detection test passed");
        println!("   - Empty detection: ✅");
        println!("   - Partial detection: ✅");
        println!("   - Source filtering: ✅");
    }

    /// Test API key validation for different data sources
    ///
    /// This test verifies that our API key validation correctly identifies
    /// which data sources require API keys and which don't.
    #[test]
    fn test_data_source_api_key_requirements() {
        // REQUIREMENT: Test data source API key requirements
        // PURPOSE: Verify that we correctly identify which sources need keys
        // This ensures proper validation of data source configurations

        use crate::models::data_source::DataSource;

        // Test FRED (requires API key)
        let fred_source = DataSource::fred();
        assert!(fred_source.api_key_required, "FRED should require API key");
        assert_eq!(fred_source.api_key_name, Some("FRED_API_KEY".to_string()));

        // Test BLS (does not require API key)
        let bls_source = DataSource::bls();
        assert!(!bls_source.api_key_required, "BLS should not require API key");
        assert_eq!(bls_source.api_key_name, None);

        // Test Census (does not require API key)
        let census_source = DataSource::census();
        assert!(!census_source.api_key_required, "Census should not require API key");
        assert_eq!(census_source.api_key_name, None);

        // Test BEA (does not require API key)
        let bea_source = DataSource::bea();
        assert!(!bea_source.api_key_required, "BEA should not require API key");
        assert_eq!(bea_source.api_key_name, None);

        println!("✅ Data source API key requirements test passed");
        println!("   - FRED requires key: ✅");
        println!("   - BLS no key needed: ✅");
        println!("   - Census no key needed: ✅");
        println!("   - BEA no key needed: ✅");
    }

    /// Test error handling for missing required API keys
    ///
    /// This test verifies that our system properly handles cases where
    /// required API keys are missing.
    #[test]
    fn test_missing_required_api_keys() {
        // REQUIREMENT: Test missing required API key handling
        // PURPOSE: Verify that missing keys are handled gracefully
        // This ensures robust error handling

        // Clear environment
        env::remove_var("FRED_API_KEY");

        let config = ApiKeyConfig::from_env();

        // FRED requires an API key but we don't have one
        assert!(!config.has_required_key("FRED_API_KEY"), "Should not have FRED key");
        assert_eq!(config.get_key("FRED_API_KEY"), None);

        // BLS doesn't require an API key, so this should be fine
        assert!(!config.has_required_key("BLS_API_KEY"), "Should not have BLS key");
        assert_eq!(config.get_key("BLS_API_KEY"), None);

        // Test configured sources (should be empty)
        let configured = config.get_configured_sources();
        assert!(configured.is_empty(), "Should have no configured sources");

        println!("✅ Missing required API keys test passed");
        println!("   - Missing keys handled: ✅");
        println!("   - No crashes on missing keys: ✅");
        println!("   - Empty configuration handled: ✅");
    }
}
