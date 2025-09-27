//! # Query Depth Limiting
//!
//! This module provides query depth limiting to prevent deeply nested queries
//! that could cause stack overflow or excessive resource consumption.
//!
//! # Depth Calculation
//!
//! The depth of a GraphQL query is calculated by counting the maximum nesting
//! level of fields in the query. For example:
//!
//! ```graphql
//! query {
//!   user {          # depth 1
//!     posts {       # depth 2
//!       comments {  # depth 3
//!         author {  # depth 4
//!           name    # depth 5
//!         }
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! # Security Benefits
//!
//! - Prevents stack overflow attacks
//! - Limits resource consumption
//! - Provides predictable query execution
//! - Protects against malicious deep queries
//!
//! # Configuration
//!
//! - `max_depth`: Maximum allowed query depth
//! - `count_fragments`: Whether to count fragment depth
//! - `count_inline_fragments`: Whether to count inline fragment depth

use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use tracing::{debug, warn};

/// Query depth limiter
pub struct DepthLimiter {
    /// Maximum allowed query depth
    max_depth: u32,
    /// Whether to count fragment depth
    count_fragments: bool,
    /// Whether to count inline fragment depth
    count_inline_fragments: bool,
}

impl DepthLimiter {
    /// Create a new depth limiter
    pub fn new(max_depth: u32) -> Self {
        Self {
            max_depth,
            count_fragments: true,
            count_inline_fragments: true,
        }
    }

    /// Validate query depth
    pub fn validate_depth(&self, query: &str) -> Result<(), String> {
        let depth = self.calculate_depth(query)?;

        if depth > self.max_depth {
            return Err(format!(
                "Query depth {} exceeds maximum allowed depth {}",
                depth, self.max_depth
            ));
        }

        debug!("Query depth: {} (max: {})", depth, self.max_depth);
        Ok(())
    }

    /// Calculate the maximum depth of a GraphQL query
    pub fn calculate_depth(&self, query: &str) -> Result<u32, String> {
        let document = async_graphql::parser::parse_query(query)
            .map_err(|e| format!("Failed to parse query: {}", e))?;

        let mut max_depth = 0;

        for (_, operation) in document.operations.iter() {
            if operation.node.ty == OperationType::Query {
                let depth =
                    self.calculate_operation_depth(&operation.node.selection_set.node, 0)?;
                max_depth = max_depth.max(depth);
            }
        }

        Ok(max_depth)
    }

    /// Calculate depth for an operation
    fn calculate_operation_depth(
        &self,
        selection_set: &SelectionSet,
        current_depth: u32,
    ) -> Result<u32, String> {
        let mut max_depth = current_depth;

        for selection in &selection_set.items {
            let depth = self.calculate_selection_depth(&selection.node, current_depth)?;
            max_depth = max_depth.max(depth);
        }

        Ok(max_depth)
    }

    /// Calculate depth for a selection
    fn calculate_selection_depth(
        &self,
        selection: &Selection,
        current_depth: u32,
    ) -> Result<u32, String> {
        match selection {
            Selection::Field(field) => {
                let mut depth = current_depth + 1;

                // Add depth from nested selections
                if !field.node.selection_set.node.items.is_empty() {
                    depth =
                        self.calculate_operation_depth(&field.node.selection_set.node, depth)?;
                }

                Ok(depth)
            }
            Selection::FragmentSpread(_fragment_spread) => {
                // Fragment spreads don't add depth
                Ok(current_depth)
            }
            Selection::InlineFragment(inline_fragment) => {
                if self.count_inline_fragments {
                    let mut depth = current_depth + 1;

                    if !inline_fragment.node.selection_set.node.items.is_empty() {
                        depth = self.calculate_operation_depth(
                            &inline_fragment.node.selection_set.node,
                            depth,
                        )?;
                    }

                    Ok(depth)
                } else {
                    Ok(current_depth)
                }
            }
        }
    }

    /// Update the maximum depth
    pub fn update_max_depth(&mut self, max_depth: u32) {
        self.max_depth = max_depth;
    }

    /// Set whether to count fragment depth
    pub fn set_count_fragments(&mut self, count_fragments: bool) {
        self.count_fragments = count_fragments;
    }

    /// Set whether to count inline fragment depth
    pub fn set_count_inline_fragments(&mut self, count_inline_fragments: bool) {
        self.count_inline_fragments = count_inline_fragments;
    }

    /// Get the current maximum depth
    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query_depth() {
        let limiter = DepthLimiter::new(10);
        let query = r#"
            query {
                series {
                    id
                    name
                }
            }
        "#;

        let depth = limiter.calculate_depth(query).unwrap();
        assert_eq!(depth, 2); // series -> id/name
    }

    #[test]
    fn test_nested_query_depth() {
        let limiter = DepthLimiter::new(10);
        let query = r#"
            query {
                series {
                    dataPoints {
                        value
                        date
                    }
                }
            }
        "#;

        let depth = limiter.calculate_depth(query).unwrap();
        assert_eq!(depth, 3); // series -> dataPoints -> value/date
    }

    #[test]
    fn test_deep_query_depth() {
        let limiter = DepthLimiter::new(10);
        let query = r#"
            query {
                series {
                    dataPoints {
                        value
                        date
                        metadata {
                            source {
                                name
                                description
                            }
                        }
                    }
                }
            }
        "#;

        let depth = limiter.calculate_depth(query).unwrap();
        assert_eq!(depth, 5); // series -> dataPoints -> metadata -> source -> name/description
    }

    #[test]
    fn test_depth_validation() {
        let limiter = DepthLimiter::new(3);
        let query = r#"
            query {
                series {
                    dataPoints {
                        value
                        date
                        metadata {
                            source {
                                name
                            }
                        }
                    }
                }
            }
        "#;

        let result = limiter.validate_depth(query);
        assert!(result.is_err());
    }

    #[test]
    fn test_inline_fragment_depth() {
        let mut limiter = DepthLimiter::new(10);
        limiter.set_count_inline_fragments(true);

        let query = r#"
            query {
                series {
                    ... on EconomicSeries {
                        dataPoints {
                            value
                        }
                    }
                }
            }
        "#;

        let depth = limiter.calculate_depth(query).unwrap();
        assert_eq!(depth, 3); // series -> inline fragment -> dataPoints -> value
    }

    #[test]
    fn test_inline_fragment_depth_disabled() {
        let mut limiter = DepthLimiter::new(10);
        limiter.set_count_inline_fragments(false);

        let query = r#"
            query {
                series {
                    ... on EconomicSeries {
                        dataPoints {
                            value
                        }
                    }
                }
            }
        "#;

        let depth = limiter.calculate_depth(query).unwrap();
        assert_eq!(depth, 2); // series -> dataPoints -> value (inline fragment not counted)
    }
}
