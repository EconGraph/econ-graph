//! API key management for all data sources
//!
//! This module provides centralized API key management using environment variables.
//! All data sources should use this module to access their API keys securely.

use std::collections::HashMap;

/// API key configuration for all data sources
#[derive(Debug, Clone)]
pub struct ApiKeyConfig {
    /// API keys for various data sources
    pub keys: HashMap<String, String>,
}

impl ApiKeyConfig {
    /// Create a new API key configuration from environment variables
    pub fn from_env() -> Self {
        let mut keys = HashMap::new();

        // US Government Sources
        if let Ok(key) = std::env::var("FRED_API_KEY") {
            keys.insert("FRED".to_string(), key);
        }
        if let Ok(key) = std::env::var("BLS_API_KEY") {
            keys.insert("BLS".to_string(), key);
        }
        if let Ok(key) = std::env::var("CENSUS_API_KEY") {
            keys.insert("CENSUS".to_string(), key);
        }
        if let Ok(key) = std::env::var("BEA_API_KEY") {
            keys.insert("BEA".to_string(), key);
        }
        if let Ok(key) = std::env::var("FHFA_API_KEY") {
            keys.insert("FHFA".to_string(), key);
        }

        // International Sources
        if let Ok(key) = std::env::var("WORLD_BANK_API_KEY") {
            keys.insert("WORLD_BANK".to_string(), key);
        }
        if let Ok(key) = std::env::var("IMF_API_KEY") {
            keys.insert("IMF".to_string(), key);
        }
        if let Ok(key) = std::env::var("ECB_API_KEY") {
            keys.insert("ECB".to_string(), key);
        }
        if let Ok(key) = std::env::var("OECD_API_KEY") {
            keys.insert("OECD".to_string(), key);
        }
        if let Ok(key) = std::env::var("BOE_API_KEY") {
            keys.insert("BOE".to_string(), key);
        }
        if let Ok(key) = std::env::var("WTO_API_KEY") {
            keys.insert("WTO".to_string(), key);
        }
        if let Ok(key) = std::env::var("BOJ_API_KEY") {
            keys.insert("BOJ".to_string(), key);
        }
        if let Ok(key) = std::env::var("RBA_API_KEY") {
            keys.insert("RBA".to_string(), key);
        }
        if let Ok(key) = std::env::var("BOC_API_KEY") {
            keys.insert("BOC".to_string(), key);
        }
        if let Ok(key) = std::env::var("SNB_API_KEY") {
            keys.insert("SNB".to_string(), key);
        }
        if let Ok(key) = std::env::var("UNSTATS_API_KEY") {
            keys.insert("UNSTATS".to_string(), key);
        }
        if let Ok(key) = std::env::var("ILO_API_KEY") {
            keys.insert("ILO".to_string(), key);
        }

        Self { keys }
    }

    /// Get API key for a specific data source
    pub fn get_key(&self, data_source: &str) -> Option<&String> {
        self.keys.get(data_source)
    }

    /// Check if a data source requires an API key and if we have it
    pub fn has_required_key(&self, data_source: &str, requires_key: bool) -> bool {
        if !requires_key {
            true
        } else {
            self.keys.contains_key(data_source)
        }
    }

    /// Get all configured API keys (for logging/debugging purposes)
    pub fn get_all_keys(&self) -> &HashMap<String, String> {
        &self.keys
    }

    /// Get list of data sources that have API keys configured
    pub fn get_configured_sources(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }

    /// Check if any API keys are configured
    pub fn has_any_keys(&self) -> bool {
        !self.keys.is_empty()
    }
}

/// Environment variable names for all supported data sources
pub const API_KEY_ENV_VARS: &[&str] = &[
    "FRED_API_KEY",
    "BLS_API_KEY",
    "CENSUS_API_KEY",
    "BEA_API_KEY",
    "FHFA_API_KEY",
    "WORLD_BANK_API_KEY",
    "IMF_API_KEY",
    "ECB_API_KEY",
    "OECD_API_KEY",
    "BOE_API_KEY",
    "WTO_API_KEY",
    "BOJ_API_KEY",
    "RBA_API_KEY",
    "BOC_API_KEY",
    "SNB_API_KEY",
    "UNSTATS_API_KEY",
    "ILO_API_KEY",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_config_from_env() {
        // Set test environment variables
        std::env::set_var("FRED_API_KEY", "test_fred_key");
        std::env::set_var("BLS_API_KEY", "test_bls_key");

        let config = ApiKeyConfig::from_env();

        assert_eq!(config.get_key("FRED"), Some(&"test_fred_key".to_string()));
        assert_eq!(config.get_key("BLS"), Some(&"test_bls_key".to_string()));
        assert_eq!(config.get_key("CENSUS"), None);

        // Clean up
        std::env::remove_var("FRED_API_KEY");
        std::env::remove_var("BLS_API_KEY");
    }

    #[test]
    fn test_has_required_key() {
        std::env::set_var("FRED_API_KEY", "test_key");
        let config = ApiKeyConfig::from_env();

        assert!(config.has_required_key("FRED", true));
        assert!(!config.has_required_key("BLS", true));
        assert!(config.has_required_key("WORLD_BANK", false)); // Doesn't require key

        std::env::remove_var("FRED_API_KEY");
    }
}
