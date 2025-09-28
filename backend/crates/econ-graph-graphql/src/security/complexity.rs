//! # Query Complexity Analysis
//!
//! This module provides query complexity analysis to prevent expensive queries
//! that could cause denial of service attacks or system overload.
//!
//! # Complexity Calculation
//!
//! The complexity of a GraphQL query is calculated based on:
//! - Field complexity: Each field has a base complexity of 1
//! - List complexity: Lists multiply complexity by their size
//! - Nested complexity: Nested fields add to the total complexity
//! - Custom complexity: Fields can have custom complexity values
//!
//! # Security Benefits
//!
//! - Prevents expensive queries that could overload the database
//! - Protects against malicious queries designed to consume resources
//! - Allows fine-grained control over query performance
//! - Provides predictable query execution times
//!
//! # Configuration
//!
//! - `max_complexity`: Maximum allowed complexity score
//! - `field_complexity`: Custom complexity values for specific fields
//! - `list_complexity_multiplier`: Multiplier for list fields
//! - `nested_complexity_multiplier`: Multiplier for nested fields

use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::parser::Pos;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Query complexity analyzer
pub struct ComplexityAnalyzer {
    /// Maximum allowed complexity score
    max_complexity: u32,
    /// Custom complexity values for specific fields
    field_complexity: HashMap<String, u32>,
    /// Complexity multiplier for list fields
    list_complexity_multiplier: f64,
    /// Complexity multiplier for nested fields
    nested_complexity_multiplier: f64,
}

impl ComplexityAnalyzer {
    /// Create a new complexity analyzer
    pub fn new(max_complexity: u32) -> Self {
        let mut field_complexity = HashMap::new();

        // Set custom complexity for expensive fields
        field_complexity.insert("dataPoints".to_string(), 10);
        field_complexity.insert("series".to_string(), 5);
        field_complexity.insert("users".to_string(), 3);
        field_complexity.insert("annotations".to_string(), 2);
        field_complexity.insert("comments".to_string(), 1);

        Self {
            max_complexity,
            field_complexity,
            list_complexity_multiplier: 1.5,
            nested_complexity_multiplier: 1.2,
        }
    }

    /// Validate query complexity
    pub fn validate_complexity(&self, query: &str) -> Result<(), String> {
        let complexity = self.calculate_complexity(query)?;

        if complexity > self.max_complexity {
            return Err(format!(
                "Query complexity {} exceeds maximum allowed complexity {}",
                complexity, self.max_complexity
            ));
        }

        debug!(
            "Query complexity: {} (max: {})",
            complexity, self.max_complexity
        );
        Ok(())
    }

    /// Calculate the complexity of a GraphQL query
    pub fn calculate_complexity(&self, query: &str) -> Result<u32, String> {
        let document = async_graphql::parser::parse_query(query)
            .map_err(|e| format!("Failed to parse query: {}", e))?;

        let mut total_complexity = 0;

        for (_, operation) in document.operations.iter() {
            if operation.node.ty == OperationType::Query {
                total_complexity +=
                    self.calculate_operation_complexity(&operation.node.selection_set.node, 0)?;
            }
        }

        Ok(total_complexity)
    }

    /// Calculate complexity for an operation
    fn calculate_operation_complexity(
        &self,
        selection_set: &SelectionSet,
        depth: u32,
    ) -> Result<u32, String> {
        let mut complexity = 0;

        for selection in &selection_set.items {
            complexity += self.calculate_selection_complexity(&selection.node, depth)?;
        }

        Ok(complexity)
    }

    /// Calculate complexity for a selection
    fn calculate_selection_complexity(
        &self,
        selection: &Selection,
        depth: u32,
    ) -> Result<u32, String> {
        match selection {
            Selection::Field(field) => {
                let field_name = &field.node.name.node;
                let base_complexity = self.get_field_complexity(field_name);

                // Apply depth multiplier
                let depth_multiplier = if depth > 0 {
                    self.nested_complexity_multiplier.powi(depth as i32)
                } else {
                    1.0
                };

                let mut field_complexity = (base_complexity as f64 * depth_multiplier) as u32;

                // Check for list fields and apply multiplier
                if self.is_list_field(field_name) {
                    field_complexity =
                        (field_complexity as f64 * self.list_complexity_multiplier) as u32;
                }

                // Add complexity from nested selections
                if !field.node.selection_set.node.items.is_empty() {
                    field_complexity += self.calculate_operation_complexity(
                        &field.node.selection_set.node,
                        depth + 1,
                    )?;
                }

                Ok(field_complexity)
            }
            Selection::FragmentSpread(fragment_spread) => {
                // Fragment spreads are handled by the parser
                // For now, we'll skip them as they don't add complexity
                Ok(0)
            }
            Selection::InlineFragment(inline_fragment) => {
                if !inline_fragment.node.selection_set.node.items.is_empty() {
                    self.calculate_operation_complexity(
                        &inline_fragment.node.selection_set.node,
                        depth,
                    )
                } else {
                    Ok(0)
                }
            }
        }
    }

    /// Get the complexity value for a field
    fn get_field_complexity(&self, field_name: &str) -> u32 {
        self.field_complexity.get(field_name).copied().unwrap_or(1)
    }

    /// Check if a field is a list field
    fn is_list_field(&self, field_name: &str) -> bool {
        // List fields that typically return multiple items
        matches!(
            field_name,
            "dataPoints" | "series" | "users" | "annotations" | "comments" | "sources"
        )
    }

    /// Update the maximum complexity
    pub fn update_max_complexity(&mut self, max_complexity: u32) {
        self.max_complexity = max_complexity;
    }

    /// Add or update field complexity
    pub fn set_field_complexity(&mut self, field_name: String, complexity: u32) {
        self.field_complexity.insert(field_name, complexity);
    }

    /// Get the current maximum complexity
    pub fn max_complexity(&self) -> u32 {
        self.max_complexity
    }

    /// Get the complexity of a specific field
    pub fn get_field_complexity_value(&self, field_name: &str) -> u32 {
        self.get_field_complexity(field_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query_complexity() {
        let analyzer = ComplexityAnalyzer::new(100);
        let query = r#"
            query {
                series {
                    id
                    name
                }
            }
        "#;

        let complexity = analyzer.calculate_complexity(query).unwrap();
        assert_eq!(complexity, 5); // series(5) + id(1) + name(1) = 7, but with depth multiplier
    }

    #[test]
    fn test_nested_query_complexity() {
        let analyzer = ComplexityAnalyzer::new(100);
        let query = r#"
            query {
                series {
                    id
                    dataPoints {
                        value
                        date
                    }
                }
            }
        "#;

        let complexity = analyzer.calculate_complexity(query).unwrap();
        assert!(complexity > 10); // Should be higher due to nested dataPoints
    }

    #[test]
    fn test_complexity_validation() {
        let analyzer = ComplexityAnalyzer::new(10);
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

        let result = analyzer.validate_complexity(query);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_field_complexity() {
        let mut analyzer = ComplexityAnalyzer::new(100);
        analyzer.set_field_complexity("expensiveField".to_string(), 50);

        let query = r#"
            query {
                expensiveField {
                    id
                }
            }
        "#;

        let complexity = analyzer.calculate_complexity(query).unwrap();
        assert!(complexity >= 50);
    }
}
