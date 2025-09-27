// GraphQL Response Loader for MSW
// Loads JSON response files for different GraphQL operations and scenarios

import fs from 'fs';
import path from 'path';

// eslint-disable-next-line no-undef
const RESPONSES_DIR = path.join(__dirname, 'graphql-responses');

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
  const filePath = path.join(RESPONSES_DIR, operation, `${scenario}.json`);

  try {
    const fileContent = fs.readFileSync(filePath, 'utf-8');
    return JSON.parse(fileContent);
  } catch (error) {
    console.warn(`Failed to load GraphQL response: ${operation}/${scenario}`, error);
    return {
      data: null,
      errors: [{ message: `Mock response not found: ${operation}/${scenario}` }],
    };
  }
}

/**
 * Get all available scenarios for a GraphQL operation.
 * @param operation - GraphQL operation name.
 * @returns Array of available scenario names.
 */
export function getAvailableScenarios(operation: string): string[] {
  const operationDir = path.join(RESPONSES_DIR, operation);

  try {
    const files = fs.readdirSync(operationDir);
    return files.filter(file => file.endsWith('.json')).map(file => file.replace('.json', ''));
  } catch (error) {
    console.warn(`Failed to read scenarios for operation: ${operation}`, error);
    return [];
  }
}

/**
 * Get all available GraphQL operations.
 * @returns Array of operation names.
 */
export function getAvailableOperations(): string[] {
  try {
    const dirs = fs.readdirSync(RESPONSES_DIR);
    return dirs.filter(dir => {
      const dirPath = path.join(RESPONSES_DIR, dir);
      return fs.statSync(dirPath).isDirectory();
    });
  } catch (error) {
    console.warn('Failed to read GraphQL operations directory', error);
    return [];
  }
}

/**
 * Create a response handler for MSW that uses the specified scenario.
 * @param operation - GraphQL operation name.
 * @param scenario - Response scenario to use.
 * @returns MSW response handler.
 */
export function createResponseHandler(operation: string, scenario = 'success') {
  return () => {
    const response = loadGraphQLResponse(operation, scenario);
    // eslint-disable-next-line no-undef
    return new Response(JSON.stringify(response), {
      status: response.errors ? 400 : 200,
      headers: {
        'Content-Type': 'application/json',
      },
    });
  };
}

/**
 * Validate that all response files are valid JSON.
 * @returns Array of validation errors.
 */
export function validateResponseFiles(): string[] {
  const errors: string[] = [];
  const operations = getAvailableOperations();

  operations.forEach(operation => {
    const scenarios = getAvailableScenarios(operation);

    scenarios.forEach(scenario => {
      try {
        loadGraphQLResponse(operation, scenario);
      } catch (error) {
        errors.push(`${operation}/${scenario}: ${error}`);
      }
    });
  });

  return errors;
}
