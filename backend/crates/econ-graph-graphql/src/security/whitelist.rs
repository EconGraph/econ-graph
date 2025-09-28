//! # Query Whitelist/Blacklist
//!
//! This module provides query whitelist and blacklist functionality to control
//! which queries are allowed or blocked based on patterns and rules.
//!
//! # Filtering Strategies
//!
//! - **Whitelist**: Only allow queries that match specific patterns
//! - **Blacklist**: Block queries that match specific patterns
//! - **Pattern Matching**: Support for regex patterns and exact matches
//! - **Rule-based**: Support for complex filtering rules
//!
//! # Security Benefits
//!
//! - Prevents execution of known malicious queries
//! - Allows fine-grained control over query execution
//! - Supports dynamic rule updates
//! - Provides audit trail for filtered queries
//!
//! # Configuration
//!
//! - `enable_whitelist`: Whether to enable whitelist filtering
//! - `enable_blacklist`: Whether to enable blacklist filtering
//! - `whitelist_patterns`: List of allowed query patterns
//! - `blacklist_patterns`: List of blocked query patterns
//! - `case_sensitive`: Whether pattern matching is case sensitive

use regex::Regex;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Query filter configuration
#[derive(Debug, Clone)]
pub struct QueryFilterConfig {
    /// Enable whitelist filtering
    pub enable_whitelist: bool,
    /// Enable blacklist filtering
    pub enable_blacklist: bool,
    /// Whitelist patterns
    pub whitelist_patterns: Vec<String>,
    /// Blacklist patterns
    pub blacklist_patterns: Vec<String>,
    /// Case sensitive matching
    pub case_sensitive: bool,
    /// Use regex patterns
    pub use_regex: bool,
    /// Allow partial matches
    pub allow_partial_matches: bool,
}

impl Default for QueryFilterConfig {
    fn default() -> Self {
        Self {
            enable_whitelist: false,
            enable_blacklist: true,
            whitelist_patterns: Vec::new(),
            blacklist_patterns: vec![
                "__schema".to_string(),
                "__type".to_string(),
                "__typename".to_string(),
                "system".to_string(),
                "admin".to_string(),
                "root".to_string(),
                "config".to_string(),
                "settings".to_string(),
                "debug".to_string(),
                "test".to_string(),
            ],
            case_sensitive: false,
            use_regex: false,
            allow_partial_matches: true,
        }
    }
}

/// Query filter rule
#[derive(Debug, Clone)]
pub struct QueryFilterRule {
    /// Rule identifier
    pub id: String,
    /// Rule pattern
    pub pattern: String,
    /// Rule type
    pub rule_type: FilterRuleType,
    /// Rule action
    pub action: FilterAction,
    /// Rule description
    pub description: String,
    /// Rule enabled
    pub enabled: bool,
    /// Rule priority
    pub priority: u32,
}

/// Filter rule type
#[derive(Debug, Clone, PartialEq)]
pub enum FilterRuleType {
    /// Exact match
    Exact,
    /// Contains match
    Contains,
    /// Regex match
    Regex,
    /// Starts with
    StartsWith,
    /// Ends with
    EndsWith,
}

/// Filter action
#[derive(Debug, Clone, PartialEq)]
pub enum FilterAction {
    /// Allow the query
    Allow,
    /// Block the query
    Block,
    /// Log and allow
    LogAndAllow,
    /// Log and block
    LogAndBlock,
}

/// Query filter
pub struct QueryFilter {
    /// Filter configuration
    config: QueryFilterConfig,
    /// Compiled regex patterns
    regex_patterns: HashMap<String, Regex>,
    /// Filter rules
    rules: Vec<QueryFilterRule>,
}

impl QueryFilter {
    /// Create a new query filter
    pub fn new(config: QueryFilterConfig) -> Self {
        let mut filter = Self {
            config,
            regex_patterns: HashMap::new(),
            rules: Vec::new(),
        };

        // Compile regex patterns
        filter.compile_patterns();

        // Initialize default rules
        filter.initialize_default_rules();

        filter
    }

    /// Validate a query against the filter
    pub fn validate_query(&self, query: &str) -> Result<(), String> {
        // Check blacklist first
        if self.config.enable_blacklist {
            self.check_blacklist(query)?;
        }

        // Check whitelist
        if self.config.enable_whitelist {
            self.check_whitelist(query)?;
        }

        // Check custom rules
        self.check_rules(query)?;

        debug!("Query passed all filter checks");
        Ok(())
    }

    /// Check query against blacklist
    fn check_blacklist(&self, query: &str) -> Result<(), String> {
        for pattern in &self.config.blacklist_patterns {
            if self.matches_pattern(query, pattern) {
                return Err(format!("Query blocked by blacklist pattern: {}", pattern));
            }
        }
        Ok(())
    }

    /// Check query against whitelist
    fn check_whitelist(&self, query: &str) -> Result<(), String> {
        if self.config.whitelist_patterns.is_empty() {
            return Ok(());
        }

        for pattern in &self.config.whitelist_patterns {
            if self.matches_pattern(query, pattern) {
                return Ok(());
            }
        }

        Err("Query not in whitelist".to_string())
    }

    /// Check query against custom rules
    fn check_rules(&self, query: &str) -> Result<(), String> {
        // Sort rules by priority (higher priority first)
        let mut sorted_rules = self.rules.clone();
        sorted_rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        for rule in sorted_rules {
            if !rule.enabled {
                continue;
            }

            if self.matches_rule(query, &rule) {
                match rule.action {
                    FilterAction::Allow => {
                        debug!("Query allowed by rule: {}", rule.id);
                        return Ok(());
                    }
                    FilterAction::Block => {
                        return Err(format!(
                            "Query blocked by rule: {} - {}",
                            rule.id, rule.description
                        ));
                    }
                    FilterAction::LogAndAllow => {
                        warn!(
                            "Query logged and allowed by rule: {} - {}",
                            rule.id, rule.description
                        );
                        return Ok(());
                    }
                    FilterAction::LogAndBlock => {
                        warn!(
                            "Query logged and blocked by rule: {} - {}",
                            rule.id, rule.description
                        );
                        return Err(format!(
                            "Query blocked by rule: {} - {}",
                            rule.id, rule.description
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if query matches a pattern
    fn matches_pattern(&self, query: &str, pattern: &str) -> bool {
        let query_to_check = if self.config.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let pattern_to_check = if self.config.case_sensitive {
            pattern.to_string()
        } else {
            pattern.to_lowercase()
        };

        if self.config.use_regex {
            if let Some(regex) = self.regex_patterns.get(pattern) {
                regex.is_match(&query_to_check)
            } else {
                false
            }
        } else if self.config.allow_partial_matches {
            query_to_check.contains(&pattern_to_check)
        } else {
            query_to_check == pattern_to_check
        }
    }

    /// Check if query matches a rule
    fn matches_rule(&self, query: &str, rule: &QueryFilterRule) -> bool {
        let query_to_check = if self.config.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let pattern_to_check = if self.config.case_sensitive {
            rule.pattern.clone()
        } else {
            rule.pattern.to_lowercase()
        };

        match rule.rule_type {
            FilterRuleType::Exact => query_to_check == pattern_to_check,
            FilterRuleType::Contains => query_to_check.contains(&pattern_to_check),
            FilterRuleType::Regex => {
                if let Some(regex) = self.regex_patterns.get(&rule.pattern) {
                    regex.is_match(&query_to_check)
                } else {
                    false
                }
            }
            FilterRuleType::StartsWith => query_to_check.starts_with(&pattern_to_check),
            FilterRuleType::EndsWith => query_to_check.ends_with(&pattern_to_check),
        }
    }

    /// Compile regex patterns
    fn compile_patterns(&mut self) {
        self.regex_patterns.clear();

        if !self.config.use_regex {
            return;
        }

        // Compile blacklist patterns
        for pattern in &self.config.blacklist_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.regex_patterns.insert(pattern.clone(), regex);
            } else {
                warn!("Failed to compile regex pattern: {}", pattern);
            }
        }

        // Compile whitelist patterns
        for pattern in &self.config.whitelist_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                self.regex_patterns.insert(pattern.clone(), regex);
            } else {
                warn!("Failed to compile regex pattern: {}", pattern);
            }
        }

        // Compile rule patterns
        for rule in &self.rules {
            if rule.rule_type == FilterRuleType::Regex {
                if let Ok(regex) = Regex::new(&rule.pattern) {
                    self.regex_patterns.insert(rule.pattern.clone(), regex);
                } else {
                    warn!(
                        "Failed to compile regex pattern for rule {}: {}",
                        rule.id, rule.pattern
                    );
                }
            }
        }
    }

    /// Initialize default rules
    fn initialize_default_rules(&mut self) {
        // Add default security rules
        self.add_rule(QueryFilterRule {
            id: "block_introspection".to_string(),
            pattern: "__schema".to_string(),
            rule_type: FilterRuleType::Contains,
            action: FilterAction::Block,
            description: "Block introspection queries".to_string(),
            enabled: true,
            priority: 100,
        });

        self.add_rule(QueryFilterRule {
            id: "block_system_fields".to_string(),
            pattern: "system|admin|root|config|settings|debug|test".to_string(),
            rule_type: FilterRuleType::Regex,
            action: FilterAction::Block,
            description: "Block system-related fields".to_string(),
            enabled: true,
            priority: 90,
        });

        self.add_rule(QueryFilterRule {
            id: "log_large_queries".to_string(),
            pattern: r".{1000,}".to_string(),
            rule_type: FilterRuleType::Regex,
            action: FilterAction::LogAndAllow,
            description: "Log large queries".to_string(),
            enabled: true,
            priority: 10,
        });
    }

    /// Add a filter rule
    pub fn add_rule(&mut self, rule: QueryFilterRule) {
        self.rules.push(rule);
        self.compile_patterns();
    }

    /// Remove a filter rule
    pub fn remove_rule(&mut self, rule_id: &str) {
        self.rules.retain(|rule| rule.id != rule_id);
        self.compile_patterns();
    }

    /// Update a filter rule
    pub fn update_rule(&mut self, rule_id: &str, updated_rule: QueryFilterRule) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            *rule = updated_rule;
            self.compile_patterns();
        }
    }

    /// Enable or disable a rule
    pub fn set_rule_enabled(&mut self, rule_id: &str, enabled: bool) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
        }
    }

    /// Add a whitelist pattern
    pub fn add_whitelist_pattern(&mut self, pattern: String) {
        if !self.config.whitelist_patterns.contains(&pattern) {
            self.config.whitelist_patterns.push(pattern);
            self.compile_patterns();
        }
    }

    /// Remove a whitelist pattern
    pub fn remove_whitelist_pattern(&mut self, pattern: &str) {
        self.config.whitelist_patterns.retain(|p| p != pattern);
        self.compile_patterns();
    }

    /// Add a blacklist pattern
    pub fn add_blacklist_pattern(&mut self, pattern: String) {
        if !self.config.blacklist_patterns.contains(&pattern) {
            self.config.blacklist_patterns.push(pattern);
            self.compile_patterns();
        }
    }

    /// Remove a blacklist pattern
    pub fn remove_blacklist_pattern(&mut self, pattern: &str) {
        self.config.blacklist_patterns.retain(|p| p != pattern);
        self.compile_patterns();
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: QueryFilterConfig) {
        self.config = config;
        self.compile_patterns();
    }

    /// Get the current configuration
    pub fn config(&self) -> &QueryFilterConfig {
        &self.config
    }

    /// Get all rules
    pub fn get_rules(&self) -> &[QueryFilterRule] {
        &self.rules
    }

    /// Get rule by ID
    pub fn get_rule(&self, rule_id: &str) -> Option<&QueryFilterRule> {
        self.rules.iter().find(|r| r.id == rule_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blacklist_filtering() {
        let config = QueryFilterConfig {
            enable_blacklist: true,
            enable_whitelist: false,
            blacklist_patterns: vec!["__schema".to_string(), "admin".to_string()],
            ..Default::default()
        };

        let filter = QueryFilter::new(config);

        // Should block blacklisted queries
        assert!(filter
            .validate_query("query { __schema { types { name } } }")
            .is_err());
        assert!(filter.validate_query("query { admin { id } }").is_err());

        // Should allow normal queries
        assert!(filter
            .validate_query("query { series { id name } }")
            .is_ok());
    }

    #[test]
    fn test_whitelist_filtering() {
        let config = QueryFilterConfig {
            enable_blacklist: false,
            enable_whitelist: true,
            whitelist_patterns: vec!["series".to_string(), "dataPoints".to_string()],
            ..Default::default()
        };

        let filter = QueryFilter::new(config);

        // Should allow whitelisted queries
        assert!(filter
            .validate_query("query { series { id name } }")
            .is_ok());
        assert!(filter
            .validate_query("query { dataPoints { value date } }")
            .is_ok());

        // Should block non-whitelisted queries
        assert!(filter
            .validate_query("query { users { id name } }")
            .is_err());
    }

    #[test]
    fn test_regex_filtering() {
        let config = QueryFilterConfig {
            enable_blacklist: true,
            enable_whitelist: false,
            use_regex: true,
            blacklist_patterns: vec![r"__\w+".to_string()],
            ..Default::default()
        };

        let filter = QueryFilter::new(config);

        // Should block regex-matched queries
        assert!(filter
            .validate_query("query { __schema { types { name } } }")
            .is_err());
        assert!(filter.validate_query("query { __type { name } }").is_err());

        // Should allow normal queries
        assert!(filter
            .validate_query("query { series { id name } }")
            .is_ok());
    }

    #[test]
    fn test_case_insensitive_filtering() {
        let config = QueryFilterConfig {
            enable_blacklist: true,
            enable_whitelist: false,
            case_sensitive: false,
            blacklist_patterns: vec!["ADMIN".to_string()],
            ..Default::default()
        };

        let filter = QueryFilter::new(config);

        // Should block case-insensitive matches
        assert!(filter.validate_query("query { admin { id } }").is_err());
        assert!(filter.validate_query("query { Admin { id } }").is_err());
        assert!(filter.validate_query("query { ADMIN { id } }").is_err());
    }

    #[test]
    fn test_custom_rules() {
        let config = QueryFilterConfig::default();
        let mut filter = QueryFilter::new(config);

        // Add a custom rule
        filter.add_rule(QueryFilterRule {
            id: "block_large_queries".to_string(),
            pattern: "dataPoints".to_string(),
            rule_type: FilterRuleType::Contains,
            action: FilterAction::Block,
            description: "Block queries with dataPoints".to_string(),
            enabled: true,
            priority: 50,
        });

        // Should block queries matching the rule
        assert!(filter
            .validate_query("query { series { dataPoints { value } } }")
            .is_err());

        // Should allow other queries
        assert!(filter
            .validate_query("query { series { id name } }")
            .is_ok());
    }
}
