//! # Introspection Protection
//!
//! This module provides introspection protection to control access to GraphQL
//! schema introspection queries.
//!
//! # Introspection Queries
//!
//! GraphQL introspection allows clients to query the schema itself, including:
//! - Available types and fields
//! - Query and mutation definitions
//! - Directive definitions
//! - Enum values and descriptions
//!
//! # Security Considerations
//!
//! While introspection is useful for development, it can be a security risk in production:
//! - Exposes internal schema structure
//! - Reveals field names and types
//! - Can help attackers understand the API
//! - May expose sensitive information
//!
//! # Protection Strategies
//!
//! - Complete blocking of introspection queries
//! - Selective blocking of sensitive introspection fields
//! - Role-based access control for introspection
//! - Environment-based configuration
//!
//! # Configuration
//!
//! - `enabled`: Whether introspection protection is enabled
//! - `block_all`: Whether to block all introspection queries
//! - `allowed_fields`: List of allowed introspection fields
//! - `blocked_fields`: List of blocked introspection fields

use async_graphql::parser::types::{
    ExecutableDocument, Field, OperationType, Selection, SelectionSet,
};
use tracing::{debug, warn};

/// Introspection protection configuration
#[derive(Debug, Clone)]
pub struct IntrospectionConfig {
    /// Whether introspection protection is enabled
    pub enabled: bool,
    /// Whether to block all introspection queries
    pub block_all: bool,
    /// Allowed introspection fields
    pub allowed_fields: Vec<String>,
    /// Blocked introspection fields
    pub blocked_fields: Vec<String>,
    /// Allow introspection in development
    pub allow_in_development: bool,
}

impl Default for IntrospectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            block_all: true,
            allowed_fields: Vec::new(),
            blocked_fields: vec![
                "__schema".to_string(),
                "__type".to_string(),
                "__typename".to_string(),
                "__directive".to_string(),
                "__field".to_string(),
                "__enumValue".to_string(),
                "__inputValue".to_string(),
                "__directiveLocation".to_string(),
            ],
            allow_in_development: false,
        }
    }
}

/// Introspection protector
pub struct IntrospectionProtector {
    /// Protection configuration
    config: IntrospectionConfig,
    /// Environment detection
    is_development: bool,
}

impl IntrospectionProtector {
    /// Create a new introspection protector
    pub fn new(enabled: bool) -> Self {
        Self {
            config: IntrospectionConfig {
                enabled,
                ..Default::default()
            },
            is_development: Self::detect_development_environment(),
        }
    }

    /// Validate introspection queries
    pub fn validate_introspection(&self, query: &str) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        // Allow introspection in development if configured
        if self.is_development && self.config.allow_in_development {
            debug!("Allowing introspection in development environment");
            return Ok(());
        }

        // Block all introspection if configured
        if self.config.block_all {
            if self.contains_introspection_query(query) {
                return Err("Introspection queries are not allowed".to_string());
            }
        } else {
            // Selective blocking
            self.validate_selective_introspection(query)?;
        }

        debug!("Introspection validation passed");
        Ok(())
    }

    /// Check if query contains introspection
    fn contains_introspection_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();

        // Check for common introspection fields
        let introspection_fields = vec![
            "__schema",
            "__type",
            "__typename",
            "__directive",
            "__field",
            "__enumvalue",
            "__inputvalue",
            "__directivelocation",
        ];

        for field in introspection_fields {
            if query_lower.contains(field) {
                return true;
            }
        }

        false
    }

    /// Validate selective introspection
    fn validate_selective_introspection(&self, query: &str) -> Result<(), String> {
        let document = async_graphql::parser::parse_query(query)
            .map_err(|e| format!("Failed to parse query: {}", e))?;

        for (_, operation) in document.operations.iter() {
            if operation.node.ty == OperationType::Query {
                self.validate_operation_introspection(&operation.node.selection_set.node)?;
            }
        }

        Ok(())
    }

    /// Validate operation introspection
    fn validate_operation_introspection(&self, selection_set: &SelectionSet) -> Result<(), String> {
        for selection in &selection_set.items {
            self.validate_selection_introspection(&selection.node)?;
        }
        Ok(())
    }

    /// Validate selection introspection
    fn validate_selection_introspection(&self, selection: &Selection) -> Result<(), String> {
        match selection {
            Selection::Field(field) => {
                let field_name = &field.node.name.node;

                // Check if field is blocked
                if self.config.blocked_fields.contains(&field_name.to_string()) {
                    return Err(format!(
                        "Introspection field '{}' is not allowed",
                        field_name
                    ));
                }

                // Check if field is allowed (if allowlist is used)
                if !self.config.allowed_fields.is_empty()
                    && !self.config.allowed_fields.contains(&field_name.to_string())
                {
                    return Err(format!(
                        "Introspection field '{}' is not in the allowed list",
                        field_name
                    ));
                }

                // Validate nested selections
                if !field.node.selection_set.node.items.is_empty() {
                    self.validate_operation_introspection(&field.node.selection_set.node)?;
                }
            }
            Selection::FragmentSpread(_fragment_spread) => {
                // Fragment spreads don't contain introspection fields directly
            }
            Selection::InlineFragment(inline_fragment) => {
                if !inline_fragment.node.selection_set.node.items.is_empty() {
                    self.validate_operation_introspection(
                        &inline_fragment.node.selection_set.node,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Detect if we're in a development environment
    fn detect_development_environment() -> bool {
        // Check common environment variables
        std::env::var("RUST_ENV").unwrap_or_default() == "development"
            || std::env::var("ENVIRONMENT").unwrap_or_default() == "development"
            || std::env::var("NODE_ENV").unwrap_or_default() == "development"
            || std::env::var("DEBUG").unwrap_or_default() == "true"
            || cfg!(debug_assertions)
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: IntrospectionConfig) {
        self.config = config;
    }

    /// Get the current configuration
    pub fn config(&self) -> &IntrospectionConfig {
        &self.config
    }

    /// Set whether to allow introspection in development
    pub fn set_allow_in_development(&mut self, allow: bool) {
        self.config.allow_in_development = allow;
    }

    /// Add a blocked introspection field
    pub fn add_blocked_field(&mut self, field: String) {
        if !self.config.blocked_fields.contains(&field) {
            self.config.blocked_fields.push(field);
        }
    }

    /// Remove a blocked introspection field
    pub fn remove_blocked_field(&mut self, field: &str) {
        self.config.blocked_fields.retain(|f| f != field);
    }

    /// Add an allowed introspection field
    pub fn add_allowed_field(&mut self, field: String) {
        if !self.config.allowed_fields.contains(&field) {
            self.config.allowed_fields.push(field);
        }
    }

    /// Remove an allowed introspection field
    pub fn remove_allowed_field(&mut self, field: &str) {
        self.config.allowed_fields.retain(|f| f != field);
    }

    /// Check if introspection is allowed in current environment
    pub fn is_introspection_allowed(&self) -> bool {
        if !self.config.enabled {
            return true;
        }

        if self.is_development && self.config.allow_in_development {
            return true;
        }

        false
    }
}

/// Introspection query patterns for detection
pub struct IntrospectionPatterns;

impl IntrospectionPatterns {
    /// Common introspection query patterns
    pub const SCHEMA_QUERY: &'static str = r#"
        query IntrospectionQuery {
            __schema {
                queryType { name }
                mutationType { name }
                subscriptionType { name }
                types {
                    ...FullType
                }
                directives {
                    name
                    description
                    locations
                    args {
                        ...InputValue
                    }
                }
            }
        }
    "#;

    pub const TYPE_QUERY: &'static str = r#"
        query IntrospectionTypeQuery($name: String!) {
            __type(name: $name) {
                name
                kind
                description
                fields {
                    name
                    description
                    type {
                        ...TypeRef
                    }
                }
            }
        }
    "#;

    pub const SIMPLE_SCHEMA_QUERY: &'static str = r#"
        query {
            __schema {
                types {
                    name
                }
            }
        }
    "#;

    /// Check if a query matches common introspection patterns
    pub fn is_introspection_pattern(query: &str) -> bool {
        let query_lower = query.to_lowercase();

        // Check for common introspection patterns
        query_lower.contains("__schema")
            || query_lower.contains("__type")
            || query_lower.contains("__typename")
            || query_lower.contains("__directive")
            || query_lower.contains("__field")
            || query_lower.contains("__enumvalue")
            || query_lower.contains("__inputvalue")
            || query_lower.contains("__directivelocation")
    }

    /// Get a list of all introspection fields
    pub fn get_all_introspection_fields() -> Vec<&'static str> {
        vec![
            "__schema",
            "__type",
            "__typename",
            "__directive",
            "__field",
            "__enumValue",
            "__inputValue",
            "__directiveLocation",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_introspection_protection_enabled() {
        let protector = IntrospectionProtector::new(true);
        let query = r#"
            query {
                __schema {
                    types {
                        name
                    }
                }
            }
        "#;

        let result = protector.validate_introspection(query);
        assert!(result.is_err());
    }

    #[test]
    fn test_introspection_protection_disabled() {
        let protector = IntrospectionProtector::new(false);
        let query = r#"
            query {
                __schema {
                    types {
                        name
                    }
                }
            }
        "#;

        let result = protector.validate_introspection(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_normal_query_allowed() {
        let protector = IntrospectionProtector::new(true);
        let query = r#"
            query {
                series {
                    id
                    name
                }
            }
        "#;

        let result = protector.validate_introspection(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_selective_introspection() {
        let mut protector = IntrospectionProtector::new(true);
        protector.config.block_all = false;
        protector.add_allowed_field("__typename".to_string());

        let query = r#"
            query {
                __typename
            }
        "#;

        let result = protector.validate_introspection(query);
        assert!(result.is_ok());
    }

    #[test]
    fn test_introspection_pattern_detection() {
        assert!(IntrospectionPatterns::is_introspection_pattern(
            "query { __schema { types { name } } }"
        ));
        assert!(!IntrospectionPatterns::is_introspection_pattern(
            "query { series { id name } }"
        ));
    }
}
