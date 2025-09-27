/**
 * GraphQL Schema Validation Tests
 *
 * This test suite ensures that all GraphQL mock responses conform to the backend schema.
 * It validates that mock data structures match the expected GraphQL types and fields.
 */

import { describe, it, expect } from 'vitest';
import { loadGraphQLResponse } from '../mocks/graphql-response-loader';
import { MockScenarios } from '../mocks/simpleServer';

// Import all GraphQL queries we're testing
import {
  GET_FINANCIAL_STATEMENT,
  GET_FINANCIAL_DASHBOARD,
  GET_FINANCIAL_RATIOS,
  GET_FINANCIAL_ALERTS,
  GET_RATIO_BENCHMARKS,
} from '../mocks/graphql/financial-queries';

// Schema validation helper
const validateGraphQLResponse = (response: any, expectedFields: string[]) => {
  const missingFields = expectedFields.filter(field => {
    const fieldPath = field.split('.');
    let current = response;
    for (const path of fieldPath) {
      if (current && typeof current === 'object' && path in current) {
        current = current[path];
      } else {
        return true; // Field is missing
      }
    }
    return false;
  });

  return {
    isValid: missingFields.length === 0,
    missingFields,
  };
};

describe('GraphQL Schema Validation', () => {
  describe('GetFinancialStatement', () => {
    it('should return valid financial statement data structure', () => {
      const response = loadGraphQLResponse('get_financial_statement', MockScenarios.SUCCESS);

      // Validate top-level structure
      expect(response).toHaveProperty('data');
      expect(response.data).toHaveProperty('financialStatement');

      const statement = response.data.financialStatement;

      // Validate required fields based on GraphQL query
      const requiredFields = [
        'id',
        'type',
        'period',
        'data',
        'lineItems',
      ];

      const validation = validateGraphQLResponse(statement, requiredFields);
      expect(validation.isValid).toBe(true);
      expect(validation.missingFields).toEqual([]);

      // Validate lineItems structure
      expect(Array.isArray(statement.lineItems)).toBe(true);
      if (statement.lineItems.length > 0) {
        const lineItem = statement.lineItems[0];
        const lineItemFields = ['id', 'name', 'value', 'category'];
        const lineItemValidation = validateGraphQLResponse(lineItem, lineItemFields);
        expect(lineItemValidation.isValid).toBe(true);
      }
    });

    it('should handle error scenarios correctly', () => {
      const errorResponse = loadGraphQLResponse('get_financial_statement', MockScenarios.ERROR);

      expect(errorResponse).toHaveProperty('errors');
      expect(Array.isArray(errorResponse.errors)).toBe(true);
      expect(errorResponse.errors.length).toBeGreaterThan(0);
      expect(errorResponse.errors[0]).toHaveProperty('message');
    });

    it('should handle not found scenarios correctly', () => {
      const notFoundResponse = loadGraphQLResponse('get_financial_statement', MockScenarios.NOT_FOUND);

      // Should either have errors or null data
      const hasErrors = notFoundResponse.errors && notFoundResponse.errors.length > 0;
      const hasNullData = notFoundResponse.data?.financialStatement === null;

      expect(hasErrors || hasNullData).toBe(true);
    });
  });

  describe('GetFinancialDashboard', () => {
    it('should return valid financial dashboard data structure', () => {
      const response = loadGraphQLResponse('get_financial_dashboard', MockScenarios.SUCCESS);

      expect(response).toHaveProperty('data');
      expect(response.data).toHaveProperty('company');

      const company = response.data.company;
      const requiredFields = [
        'id',
        'name',
        'ticker',
        'industry',
        'sector',
        'financialStatements',
      ];

      const validation = validateGraphQLResponse(company, requiredFields);
      expect(validation.isValid).toBe(true);
      expect(validation.missingFields).toEqual([]);

      // Validate financialStatements array
      expect(Array.isArray(company.financialStatements)).toBe(true);
      if (company.financialStatements.length > 0) {
        const statement = company.financialStatements[0];
        const statementFields = ['id', 'type', 'period', 'data'];
        const statementValidation = validateGraphQLResponse(statement, statementFields);
        expect(statementValidation.isValid).toBe(true);
      }
    });
  });

  describe('GetFinancialRatios', () => {
    it('should return valid financial ratios data structure', () => {
      const response = loadGraphQLResponse('get_financial_ratios', MockScenarios.SUCCESS);

      expect(response).toHaveProperty('data');
      expect(response.data).toHaveProperty('financialRatios');

      const ratios = response.data.financialRatios;
      expect(Array.isArray(ratios)).toBe(true);

      if (ratios.length > 0) {
        const ratio = ratios[0];
        const ratioFields = [
          'id',
          'statementId',
          'ratioName',
          'ratioDisplayName',
          'value',
          'category',
          'formula',
          'interpretation',
          'benchmarkPercentile',
          'periodEndDate',
          'fiscalYear',
          'fiscalQuarter',
          'calculatedAt',
          'dataQualityScore',
        ];

        const validation = validateGraphQLResponse(ratio, ratioFields);
        expect(validation.isValid).toBe(true);
        expect(validation.missingFields).toEqual([]);
      }
    });
  });

  describe('GetFinancialAlerts', () => {
    it('should return valid financial alerts data structure', () => {
      const response = loadGraphQLResponse('get_financial_alerts', MockScenarios.SUCCESS);

      expect(response).toHaveProperty('data');
      expect(response.data).toHaveProperty('financialAlerts');

      const alerts = response.data.financialAlerts;
      expect(Array.isArray(alerts)).toBe(true);

      if (alerts.length > 0) {
        const alert = alerts[0];
        const alertFields = [
          'id',
          'companyId',
          'alertType',
          'severity',
          'title',
          'description',
          'triggeredAt',
          'isResolved',
          'resolvedAt',
          'createdAt',
          'updatedAt',
        ];

        const validation = validateGraphQLResponse(alert, alertFields);
        expect(validation.isValid).toBe(true);
        expect(validation.missingFields).toEqual([]);
      }
    });
  });

  describe('GetRatioBenchmarks', () => {
    it('should return valid ratio benchmarks data structure', () => {
      const response = loadGraphQLResponse('get_ratio_benchmarks', MockScenarios.SUCCESS);

      expect(response).toHaveProperty('data');
      expect(response.data).toHaveProperty('ratioBenchmarks');

      const benchmarks = response.data.ratioBenchmarks;
      const benchmarkFields = [
        'ratioName',
        'industry',
        'percentile25',
        'percentile50',
        'percentile75',
        'percentile90',
        'sampleSize',
        'lastUpdated',
        'benchmarkText',
        'performanceLevel',
      ];

      const validation = validateGraphQLResponse(benchmarks, benchmarkFields);
      expect(validation.isValid).toBe(true);
      expect(validation.missingFields).toEqual([]);
    });
  });

  describe('Data Type Validation', () => {
    it('should have correct data types for financial statement', () => {
      const response = loadGraphQLResponse('get_financial_statement', MockScenarios.SUCCESS);
      const statement = response.data.financialStatement;

      expect(typeof statement.id).toBe('string');
      expect(typeof statement.type).toBe('string');
      expect(typeof statement.period).toBe('string');
      expect(typeof statement.data).toBe('object');
      expect(Array.isArray(statement.lineItems)).toBe(true);
    });

    it('should have correct data types for financial ratios', () => {
      const response = loadGraphQLResponse('get_financial_ratios', MockScenarios.SUCCESS);
      const ratios = response.data.financialRatios;

      if (ratios.length > 0) {
        const ratio = ratios[0];
        expect(typeof ratio.id).toBe('string');
        expect(typeof ratio.ratioName).toBe('string');
        expect(typeof ratio.value).toBe('number');
        expect(typeof ratio.category).toBe('string');
        expect(typeof ratio.benchmarkPercentile).toBe('number');
      }
    });

    it('should have correct data types for financial alerts', () => {
      const response = loadGraphQLResponse('get_financial_alerts', MockScenarios.SUCCESS);
      const alerts = response.data.financialAlerts;

      if (alerts.length > 0) {
        const alert = alerts[0];
        expect(typeof alert.id).toBe('string');
        expect(typeof alert.alertType).toBe('string');
        expect(typeof alert.severity).toBe('string');
        expect(typeof alert.title).toBe('string');
        expect(typeof alert.isResolved).toBe('boolean');
      }
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty arrays gracefully', () => {
      const response = loadGraphQLResponse('get_financial_ratios', MockScenarios.EMPTY);
      const ratios = response.data.financialRatios;

      expect(Array.isArray(ratios)).toBe(true);
      expect(ratios.length).toBe(0);
    });

    it('should handle loading states correctly', () => {
      const response = loadGraphQLResponse('get_financial_statement', MockScenarios.LOADING);

      // Loading state should either have null data or specific loading indicators
      const hasNullData = response.data?.financialStatement === null;
      const hasLoadingIndicator = response.data?.loading === true;

      expect(hasNullData || hasLoadingIndicator).toBe(true);
    });
  });

  describe('Schema Consistency', () => {
    it('should maintain consistent field naming across all responses', () => {
      const responses = [
        loadGraphQLResponse('get_financial_statement', MockScenarios.SUCCESS),
        loadGraphQLResponse('get_financial_dashboard', MockScenarios.SUCCESS),
        loadGraphQLResponse('get_financial_ratios', MockScenarios.SUCCESS),
        loadGraphQLResponse('get_financial_alerts', MockScenarios.SUCCESS),
        loadGraphQLResponse('get_ratio_benchmarks', MockScenarios.SUCCESS),
      ];

      responses.forEach(response => {
        // All responses should have data property
        expect(response).toHaveProperty('data');

        // All responses should not have both data and errors simultaneously
        if (response.data) {
          expect(response.errors).toBeUndefined();
        }
      });
    });

    it('should have consistent error structure', () => {
      const errorResponses = [
        loadGraphQLResponse('get_financial_statement', MockScenarios.ERROR),
        loadGraphQLResponse('get_financial_dashboard', MockScenarios.ERROR),
        loadGraphQLResponse('get_financial_ratios', MockScenarios.ERROR),
      ];

      errorResponses.forEach(response => {
        expect(response).toHaveProperty('errors');
        expect(Array.isArray(response.errors)).toBe(true);
        expect(response.errors.length).toBeGreaterThan(0);

        response.errors.forEach((error: any) => {
          expect(error).toHaveProperty('message');
          expect(typeof error.message).toBe('string');
        });
      });
    });
  });
});
