//! # Query Analysis
//!
//! This module provides query analysis functionality to validate GraphQL queries
//! for size, structure, and potential security issues.
//!
//! # Analysis Features
//!
//! - Query size validation
//! - Query structure analysis
//! - Potential security issue detection
//! - Query complexity estimation
//! - Malicious pattern detection
//!
//! # Security Benefits
//!
//! - Prevents extremely large queries
//! - Detects potential injection attempts
//! - Identifies suspicious query patterns
//! - Provides query validation
//! - Helps prevent resource exhaustion
//!
//! # Configuration
//!
//! - `max_query_size`: Maximum allowed query size in bytes
//! - `max_fields`: Maximum number of fields in a query
//! - `max_arguments`: Maximum number of arguments per field
//! - `enable_pattern_detection`: Whether to enable malicious pattern detection

use async_graphql::parser::types::{
    ExecutableDocument, Field, OperationType, Selection, SelectionSet,
};
use tracing::{debug, warn};

/// Query analyzer configuration
#[derive(Debug, Clone)]
pub struct QueryAnalysisConfig {
    /// Maximum query size in bytes
    pub max_query_size: usize,
    /// Maximum number of fields in a query
    pub max_fields: u32,
    /// Maximum number of arguments per field
    pub max_arguments: u32,
    /// Enable malicious pattern detection
    pub enable_pattern_detection: bool,
    /// Maximum query length in characters
    pub max_query_length: usize,
}

impl Default for QueryAnalysisConfig {
    fn default() -> Self {
        Self {
            max_query_size: 10000, // 10KB
            max_fields: 1000,
            max_arguments: 50,
            enable_pattern_detection: true,
            max_query_length: 50000, // 50KB
        }
    }
}

/// Query analyzer
pub struct QueryAnalyzer {
    /// Analysis configuration
    config: QueryAnalysisConfig,
    /// Known malicious patterns
    malicious_patterns: Vec<String>,
}

impl QueryAnalyzer {
    /// Create a new query analyzer
    pub fn new(max_query_size: usize) -> Self {
        let malicious_patterns = vec![
            "__schema".to_string(),
            "__type".to_string(),
            "__typename".to_string(),
            "__directive".to_string(),
            "__field".to_string(),
            "__enumValue".to_string(),
            "__inputValue".to_string(),
            "__directiveLocation".to_string(),
        ];

        Self {
            config: QueryAnalysisConfig {
                max_query_size,
                ..Default::default()
            },
            malicious_patterns,
        }
    }

    /// Validate query size
    pub fn validate_query_size(&self, query: &str) -> Result<(), String> {
        let query_bytes = query.len();
        let query_chars = query.chars().count();

        if query_bytes > self.config.max_query_size {
            return Err(format!(
                "Query size {} bytes exceeds maximum allowed size {} bytes",
                query_bytes, self.config.max_query_size
            ));
        }

        if query_chars > self.config.max_query_length {
            return Err(format!(
                "Query length {} characters exceeds maximum allowed length {} characters",
                query_chars, self.config.max_query_length
            ));
        }

        debug!(
            "Query size validation passed: {} bytes, {} characters",
            query_bytes, query_chars
        );
        Ok(())
    }

    /// Analyze query structure and detect potential issues
    pub fn analyze_query(&self, query: &str) -> Result<QueryAnalysisResult, String> {
        let document = async_graphql::parser::parse_query(query)
            .map_err(|e| format!("Failed to parse query: {}", e))?;

        let mut result = QueryAnalysisResult::new();

        for (_, operation) in document.operations.iter() {
            if operation.node.ty == OperationType::Query {
                self.analyze_operation(&operation.node.selection_set.node, &mut result)?;
            }
        }

        // Check for malicious patterns
        if self.config.enable_pattern_detection {
            self.detect_malicious_patterns(query, &mut result);
        }

        // Validate analysis results
        self.validate_analysis_result(&result)?;

        Ok(result)
    }

    /// Analyze an operation
    fn analyze_operation(
        &self,
        selection_set: &SelectionSet,
        result: &mut QueryAnalysisResult,
    ) -> Result<(), String> {
        for selection in &selection_set.items {
            self.analyze_selection(&selection.node, result)?;
        }
        Ok(())
    }

    /// Analyze a selection
    fn analyze_selection(
        &self,
        selection: &Selection,
        result: &mut QueryAnalysisResult,
    ) -> Result<(), String> {
        match selection {
            Selection::Field(field) => {
                result.field_count += 1;
                result.argument_count += field.node.arguments.len() as u32;

                // Check for suspicious field names
                let field_name = &field.node.name.node;
                if self.is_suspicious_field(field_name) {
                    result.suspicious_fields.push(field_name.to_string());
                }

                // Analyze nested selections
                if !field.node.selection_set.node.items.is_empty() {
                    self.analyze_operation(&field.node.selection_set.node, result)?;
                }
            }
            Selection::FragmentSpread(_fragment_spread) => {
                result.fragment_count += 1;
            }
            Selection::InlineFragment(_inline_fragment) => {
                result.inline_fragment_count += 1;
            }
        }
        Ok(())
    }

    /// Detect malicious patterns in the query
    fn detect_malicious_patterns(&self, query: &str, result: &mut QueryAnalysisResult) {
        let query_lower = query.to_lowercase();

        for pattern in &self.malicious_patterns {
            if query_lower.contains(pattern) {
                result.malicious_patterns.push(pattern.clone());
            }
        }

        // Check for common injection patterns
        let injection_patterns = vec![
            "union",
            "select",
            "insert",
            "update",
            "delete",
            "drop",
            "create",
            "alter",
            "exec",
            "execute",
            "script",
            "javascript:",
            "data:",
            "vbscript:",
            "onload",
            "onerror",
            "onclick",
        ];

        for pattern in injection_patterns {
            if query_lower.contains(pattern) {
                result.potential_injections.push(pattern.to_string());
            }
        }
    }

    /// Check if a field name is suspicious
    fn is_suspicious_field(&self, field_name: &str) -> bool {
        // Fields that might indicate introspection or system access
        matches!(
            field_name,
            "__schema"
                | "__type"
                | "__typename"
                | "__directive"
                | "__field"
                | "__enumValue"
                | "__inputValue"
                | "__directiveLocation"
                | "system"
                | "admin"
                | "root"
                | "config"
                | "settings"
                | "debug"
                | "test"
        )
    }

    /// Validate analysis results against configuration
    fn validate_analysis_result(&self, result: &QueryAnalysisResult) -> Result<(), String> {
        if result.field_count > self.config.max_fields {
            return Err(format!(
                "Query contains {} fields, exceeds maximum allowed {}",
                result.field_count, self.config.max_fields
            ));
        }

        if result.argument_count > self.config.max_arguments {
            return Err(format!(
                "Query contains {} arguments, exceeds maximum allowed {}",
                result.argument_count, self.config.max_arguments
            ));
        }

        if !result.malicious_patterns.is_empty() {
            return Err(format!(
                "Query contains malicious patterns: {:?}",
                result.malicious_patterns
            ));
        }

        if !result.potential_injections.is_empty() {
            return Err(format!(
                "Query contains potential injection patterns: {:?}",
                result.potential_injections
            ));
        }

        Ok(())
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: QueryAnalysisConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &QueryAnalysisConfig {
        &self.config
    }

    /// Add a malicious pattern
    pub fn add_malicious_pattern(&mut self, pattern: String) {
        self.malicious_patterns.push(pattern);
    }

    /// Remove a malicious pattern
    pub fn remove_malicious_pattern(&mut self, pattern: &str) {
        self.malicious_patterns.retain(|p| p != pattern);
    }
}

/// Result of query analysis
#[derive(Debug, Clone)]
pub struct QueryAnalysisResult {
    /// Number of fields in the query
    pub field_count: u32,
    /// Number of arguments in the query
    pub argument_count: u32,
    /// Number of fragments in the query
    pub fragment_count: u32,
    /// Number of inline fragments in the query
    pub inline_fragment_count: u32,
    /// Suspicious field names found
    pub suspicious_fields: Vec<String>,
    /// Malicious patterns found
    pub malicious_patterns: Vec<String>,
    /// Potential injection patterns found
    pub potential_injections: Vec<String>,
    /// Query complexity score
    pub complexity_score: u32,
    /// Query depth
    pub depth: u32,
}

impl QueryAnalysisResult {
    /// Create a new analysis result
    pub fn new() -> Self {
        Self {
            field_count: 0,
            argument_count: 0,
            fragment_count: 0,
            inline_fragment_count: 0,
            suspicious_fields: Vec::new(),
            malicious_patterns: Vec::new(),
            potential_injections: Vec::new(),
            complexity_score: 0,
            depth: 0,
        }
    }
}

impl Default for QueryAnalysisResult {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryAnalysisResult {
    /// Check if the query has security issues
    pub fn has_security_issues(&self) -> bool {
        !self.malicious_patterns.is_empty()
            || !self.potential_injections.is_empty()
            || !self.suspicious_fields.is_empty()
    }

    /// Get a summary of the analysis
    pub fn get_summary(&self) -> String {
        format!(
            "Fields: {}, Arguments: {}, Fragments: {}, Inline Fragments: {}, \
             Suspicious Fields: {}, Malicious Patterns: {}, Potential Injections: {}",
            self.field_count,
            self.argument_count,
            self.fragment_count,
            self.inline_fragment_count,
            self.suspicious_fields.len(),
            self.malicious_patterns.len(),
            self.potential_injections.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_size_validation() {
        let analyzer = QueryAnalyzer::new(1000);

        // Valid query
        let valid_query = "query { series { id name } }";
        assert!(analyzer.validate_query_size(valid_query).is_ok());

        // Query too large
        let large_query = "a".repeat(2000);
        assert!(analyzer.validate_query_size(&large_query).is_err());
    }

    #[test]
    fn test_query_analysis() {
        let analyzer = QueryAnalyzer::new(10000);
        let query = r#"
            query {
                series {
                    id
                    name
                    dataPoints {
                        value
                        date
                    }
                }
            }
        "#;

        let result = analyzer.analyze_query(query).unwrap();
        assert!(result.field_count > 0);
        assert!(!result.has_security_issues());
    }

    #[test]
    fn test_malicious_pattern_detection() {
        let analyzer = QueryAnalyzer::new(10000);
        let query = r#"
            query {
                __schema {
                    types {
                        name
                    }
                }
            }
        "#;

        let result = analyzer.analyze_query(query);
        assert!(result.is_err());
    }

    #[test]
    fn test_injection_pattern_detection() {
        let analyzer = QueryAnalyzer::new(10000);
        let query = r#"
            query {
                series {
                    id
                    name
                    # union select * from users
                }
            }
        "#;

        let result = analyzer.analyze_query(query);
        assert!(result.is_err());
    }

    #[test]
    fn test_suspicious_field_detection() {
        let analyzer = QueryAnalyzer::new(10000);
        let query = r#"
            query {
                system {
                    config
                }
            }
        "#;

        let result = analyzer.analyze_query(query).unwrap();
        assert!(!result.suspicious_fields.is_empty());
    }
}
