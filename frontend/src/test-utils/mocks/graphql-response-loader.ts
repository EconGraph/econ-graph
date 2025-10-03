// GraphQL Response Loader for MSW
// Browser-compatible version that uses static imports instead of file system operations

// Import JSON files with proper typing
import getFinancialDashboardSuccess from './graphql-responses/get_financial_dashboard/success.json';
import getFinancialDashboardError from './graphql-responses/get_financial_dashboard/error.json';
import getFinancialDashboardLoading from './graphql-responses/get_financial_dashboard/loading.json';
import getFinancialStatementSuccess from './graphql-responses/get_financial_statement/success.json';
import getFinancialStatementError from './graphql-responses/get_financial_statement/error.json';
import getFinancialStatementLoading from './graphql-responses/get_financial_statement/loading.json';

// Response mapping for browser compatibility
const RESPONSE_MAP: Record<string, Record<string, any>> = {
  get_financial_dashboard: {
    success: getFinancialDashboardSuccess,
    error: getFinancialDashboardError,
    loading: getFinancialDashboardLoading,
  },
  get_financial_statement: {
    success: getFinancialStatementSuccess,
    error: getFinancialStatementError,
    loading: getFinancialStatementLoading,
  },
};

export interface ResponseScenario {
  operation: string;
  scenario: string;
  response: any;
}

/**
 * Load a GraphQL response from JSON file.
 * @param operation - GraphQL operation name (snake_case).
 * @param scenario - Response scenario (success, error, not_found, etc.).
 * @returns GraphQL response object.
 */
export function loadGraphQLResponse(operation: string, scenario: string): any {
  try {
    const operationResponses = RESPONSE_MAP[operation];
    if (!operationResponses) {
      throw new Error(`Operation ${operation} not found in response map`);
    }

    const response = operationResponses[scenario];
    if (!response) {
      throw new Error(`Scenario ${scenario} not found for operation ${operation}`);
    }

    return response;
  } catch (error) {
    console.warn(`Failed to load GraphQL response: ${operation}/${scenario}`, error);
    return {
      data: null,
      errors: [
        {
          message: `Mock response not found for ${operation}/${scenario}`,
          extensions: { code: 'MOCK_NOT_FOUND' },
        },
      ],
    };
  }
}

/**
 * Get all available scenarios for a GraphQL operation.
 * @param operation - GraphQL operation name.
 * @returns Array of available scenario names.
 */
export function getAvailableScenarios(operation: string): string[] {
  const operationResponses = RESPONSE_MAP[operation];
  if (!operationResponses) {
    return [];
  }
  return Object.keys(operationResponses);
}
