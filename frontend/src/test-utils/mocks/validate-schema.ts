// Schema validation utilities for GraphQL queries
// This ensures our mock queries match the actual schema

import { validate, parse } from 'graphql';
import { schema } from './schema';
import {
  GET_FINANCIAL_DASHBOARD,
  GET_FINANCIAL_STATEMENT,
  GET_TREND_ANALYSIS,
  GET_PEER_COMPARISON,
  GET_BENCHMARK_COMPARISON,
  GET_FINANCIAL_ALERTS,
  GET_FINANCIAL_EXPORT,
} from './graphql/financial-queries';

/**
 * Validate financial GraphQL queries for schema compliance.
 * Checks that all financial queries are properly structured.
 * @returns Validation results for financial queries.
 */
export function validateFinancialQueries() {
  const queries = [
    GET_FINANCIAL_DASHBOARD,
    GET_FINANCIAL_STATEMENT,
    GET_TREND_ANALYSIS,
    GET_PEER_COMPARISON,
    GET_BENCHMARK_COMPARISON,
    GET_FINANCIAL_ALERTS,
    GET_FINANCIAL_EXPORT,
  ];

  const errors: string[] = [];

  queries.forEach((query, index) => {
    try {
      const parsedQuery = parse(query);
      const validationErrors = validate(schema, parsedQuery);

      if (validationErrors.length > 0) {
        errors.push(`Query ${index + 1}: ${validationErrors.map(e => e.message).join(', ')}`);
      }
    } catch (error) {
      errors.push(
        `Query ${index + 1}: Parse error - ${error instanceof Error ? error.message : 'Unknown error'}`
      );
    }
  });

  if (errors.length > 0) {
    throw new Error(`Schema validation failed:\n${errors.join('\n')}`);
  }

  return true;
}

/**
 * Validate a single GraphQL query against the schema.
 * @param query - The GraphQL query string to validate.
 * @param operationName - Optional operation name for the query.
 * @returns True if the query is valid, throws error if invalid.
 */
export function validateQuery(query: string, operationName?: string) {
  try {
    const parsedQuery = parse(query);
    const validationErrors = validate(schema, parsedQuery);

    if (validationErrors.length > 0) {
      throw new Error(
        `Schema validation failed for ${operationName || 'query'}: ${validationErrors.map(e => e.message).join(', ')}`
      );
    }

    return true;
  } catch (error) {
    throw new Error(
      `Query validation failed for ${operationName || 'query'}: ${error instanceof Error ? error.message : 'Unknown error'}`
    );
  }
}
