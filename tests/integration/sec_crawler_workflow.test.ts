/**
 * Integration tests for the complete SEC crawler workflow
 * 
 * Tests cover:
 * - End-to-end workflow from company search to financial data viewing
 * - GraphQL API integration
 * - Database operations
 * - Frontend-backend communication
 * - Error handling and recovery
 * - Performance and scalability
 */

import { test, expect } from '@playwright/test';
import { GraphQLClient } from 'graphql-request';

// GraphQL client setup
const graphqlClient = new GraphQLClient('http://localhost:8000/graphql');

// Test data
const testCompany = {
  name: 'Apple Inc.',
  ticker: 'AAPL',
  cik: '0000320193',
};

const testCrawlConfig = {
  startDate: '2023-01-01',
  endDate: '2023-12-31',
  formTypes: ['10-K', '10-Q'],
  maxDocuments: 10,
  includeAmendments: true,
  includeExhibits: false,
  rateLimit: 5,
  retryAttempts: 3,
  timeout: 300,
};

describe('SEC Crawler Integration Tests', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to admin interface
    await page.goto('http://localhost:3000/admin');
    
    // Wait for page to load
    await page.waitForLoadState('networkidle');
  });

  describe('Company Search Workflow', () => {
    test('should search for companies and display results', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Enter search query
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      
      // Click search button
      await page.click('[data-testid="search-button"]');
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Verify results
      const results = await page.locator('[data-testid="company-result"]');
      await expect(results.first()).toBeVisible();
      
      // Verify company information
      await expect(page.locator('text=Apple Inc.')).toBeVisible();
      await expect(page.locator('text=AAPL')).toBeVisible();
      await expect(page.locator('text=Technology')).toBeVisible();
    });

    test('should handle fuzzy search with misspellings', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Enter misspelled query
      await page.fill('[data-testid="company-search-input"]', 'Appel');
      
      // Click search button
      await page.click('[data-testid="search-button"]');
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Should still find Apple Inc.
      await expect(page.locator('text=Apple Inc.')).toBeVisible();
    });

    test('should filter results by industry', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Enter search query
      await page.fill('[data-testid="company-search-input"]', 'Technology');
      
      // Select industry filter
      await page.selectOption('[data-testid="industry-filter"]', 'Technology Hardware & Equipment');
      
      // Click search button
      await page.click('[data-testid="search-button"]');
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Verify all results are in the selected industry
      const results = await page.locator('[data-testid="company-result"]');
      const count = await results.count();
      
      for (let i = 0; i < count; i++) {
        const industry = await results.nth(i).locator('[data-testid="company-industry"]').textContent();
        expect(industry).toContain('Technology');
      }
    });

    test('should handle empty search results', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Enter non-existent company name
      await page.fill('[data-testid="company-search-input"]', 'NonExistentCompany12345');
      
      // Click search button
      await page.click('[data-testid="search-button"]');
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Should display no results message
      await expect(page.locator('text=No companies found')).toBeVisible();
    });
  });

  describe('SEC Crawl Workflow', () => {
    test('should configure and start SEC crawl', async ({ page }) => {
      // First, search for a company
      await page.click('[data-testid="company-search-tab"]');
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      await page.click('[data-testid="search-button"]');
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Select a company
      await page.click('[data-testid="company-result"]:first-child');
      
      // Navigate to crawler manager
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Configure crawl parameters
      await page.fill('[data-testid="start-date-input"]', testCrawlConfig.startDate);
      await page.fill('[data-testid="end-date-input"]', testCrawlConfig.endDate);
      await page.selectOption('[data-testid="form-types-select"]', testCrawlConfig.formTypes);
      await page.fill('[data-testid="max-documents-input"]', testCrawlConfig.maxDocuments.toString());
      
      // Toggle options
      await page.check('[data-testid="include-amendments-checkbox"]');
      await page.uncheck('[data-testid="include-exhibits-checkbox"]');
      
      // Set advanced options
      await page.fill('[data-testid="rate-limit-input"]', testCrawlConfig.rateLimit.toString());
      await page.fill('[data-testid="retry-attempts-input"]', testCrawlConfig.retryAttempts.toString());
      await page.fill('[data-testid="timeout-input"]', testCrawlConfig.timeout.toString());
      
      // Start crawl
      await page.click('[data-testid="start-crawl-button"]');
      
      // Wait for crawl to start
      await page.waitForSelector('[data-testid="crawl-progress"]');
      
      // Verify crawl is in progress
      await expect(page.locator('[data-testid="crawl-status"]')).toContainText('Crawling in progress');
    });

    test('should monitor crawl progress', async ({ page }) => {
      // Start a crawl (assuming one is already running from previous test)
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Wait for progress updates
      await page.waitForSelector('[data-testid="crawl-progress"]');
      
      // Verify progress bar is visible
      await expect(page.locator('[data-testid="progress-bar"]')).toBeVisible();
      
      // Verify status messages
      await expect(page.locator('[data-testid="crawl-status"]')).toBeVisible();
      
      // Wait for completion (with timeout)
      await page.waitForSelector('[data-testid="crawl-complete"]', { timeout: 60000 });
      
      // Verify completion message
      await expect(page.locator('[data-testid="crawl-result"]')).toContainText('Crawl completed successfully');
    });

    test('should display crawl results', async ({ page }) => {
      // Navigate to crawler manager
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Wait for results to be displayed
      await page.waitForSelector('[data-testid="crawl-results"]');
      
      // Verify result metrics
      await expect(page.locator('[data-testid="documents-processed"]')).toBeVisible();
      await expect(page.locator('[data-testid="documents-skipped"]')).toBeVisible();
      await expect(page.locator('[data-testid="documents-failed"]')).toBeVisible();
      await expect(page.locator('[data-testid="total-size"]')).toBeVisible();
      await expect(page.locator('[data-testid="processing-time"]')).toBeVisible();
      
      // Verify warnings and errors are displayed if present
      const warnings = await page.locator('[data-testid="warnings"]');
      const errors = await page.locator('[data-testid="errors"]');
      
      if (await warnings.count() > 0) {
        await expect(warnings).toBeVisible();
      }
      
      if (await errors.count() > 0) {
        await expect(errors).toBeVisible();
      }
    });

    test('should handle crawl errors gracefully', async ({ page }) => {
      // Navigate to crawler manager
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Configure invalid parameters to trigger error
      await page.fill('[data-testid="start-date-input"]', '2023-12-31');
      await page.fill('[data-testid="end-date-input"]', '2023-01-01'); // Invalid: end before start
      
      // Start crawl
      await page.click('[data-testid="start-crawl-button"]');
      
      // Should display validation error
      await expect(page.locator('[data-testid="validation-error"]')).toBeVisible();
      await expect(page.locator('[data-testid="validation-error"]')).toContainText('Start date must be before end date');
    });
  });

  describe('Financial Data Viewing', () => {
    test('should view financial statements after crawl', async ({ page }) => {
      // Navigate to financial data viewer
      await page.click('[data-testid="financial-data-tab"]');
      
      // Search for company
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      await page.click('[data-testid="search-button"]');
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Select company
      await page.click('[data-testid="company-result"]:first-child');
      
      // Wait for financial data to load
      await page.waitForSelector('[data-testid="financial-statements"]');
      
      // Verify financial statements are displayed
      await expect(page.locator('[data-testid="income-statement"]')).toBeVisible();
      await expect(page.locator('[data-testid="balance-sheet"]')).toBeVisible();
      await expect(page.locator('[data-testid="cash-flow-statement"]')).toBeVisible();
    });

    test('should display financial metrics', async ({ page }) => {
      // Navigate to financial data viewer
      await page.click('[data-testid="financial-data-tab"]');
      
      // Search for company
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      await page.click('[data-testid="search-button"]');
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Select company
      await page.click('[data-testid="company-result"]:first-child');
      
      // Wait for financial data to load
      await page.waitForSelector('[data-testid="financial-statements"]');
      
      // Verify key financial metrics are displayed
      await expect(page.locator('[data-testid="total-revenue"]')).toBeVisible();
      await expect(page.locator('[data-testid="net-income"]')).toBeVisible();
      await expect(page.locator('[data-testid="total-assets"]')).toBeVisible();
      await expect(page.locator('[data-testid="total-liabilities"]')).toBeVisible();
      await expect(page.locator('[data-testid="shareholders-equity"]')).toBeVisible();
    });

    test('should handle missing financial data', async ({ page }) => {
      // Navigate to financial data viewer
      await page.click('[data-testid="financial-data-tab"]');
      
      // Search for company without financial data
      await page.fill('[data-testid="company-search-input"]', 'NonExistentCompany');
      await page.click('[data-testid="search-button"]');
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Should display no data message
      await expect(page.locator('[data-testid="no-financial-data"]')).toBeVisible();
      await expect(page.locator('[data-testid="no-financial-data"]')).toContainText('No financial data available');
    });
  });

  describe('GraphQL API Integration', () => {
    test('should execute company search query', async () => {
      const query = `
        query SearchCompanies($input: CompanySearchInput!) {
          searchCompanies(input: $input) {
            nodes {
              id
              cik
              ticker
              name
              legalName
              industry
              sector
              isActive
            }
            totalCount
            hasNextPage
            hasPreviousPage
          }
        }
      `;
      
      const variables = {
        input: {
          query: 'Apple',
          limit: 10,
          includeInactive: false
        }
      };
      
      const response = await graphqlClient.request(query, variables);
      
      expect(response.searchCompanies).toBeDefined();
      expect(response.searchCompanies.nodes).toBeInstanceOf(Array);
      expect(response.searchCompanies.totalCount).toBeGreaterThanOrEqual(0);
      expect(response.searchCompanies.hasNextPage).toBeDefined();
      expect(response.searchCompanies.hasPreviousPage).toBeDefined();
    });

    test('should execute company query', async () => {
      const query = `
        query GetCompany($id: ID!) {
          company(id: $id) {
            id
            cik
            ticker
            name
            legalName
            industry
            sector
            isActive
          }
        }
      `;
      
      const variables = {
        id: 'test-company-id'
      };
      
      const response = await graphqlClient.request(query, variables);
      
      expect(response.company).toBeDefined();
    });

    test('should execute financial statements query', async () => {
      const query = `
        query GetCompanyFinancialStatements($companyId: ID!, $limit: Int, $offset: Int) {
          companyFinancialStatements(companyId: $companyId, limit: $limit, offset: $offset) {
            nodes {
              id
              statementType
              period
              fiscalYear
              fiscalQuarter
              totalRevenue
              netIncome
              totalAssets
              totalLiabilities
              shareholdersEquity
            }
            totalCount
            hasNextPage
            hasPreviousPage
          }
        }
      `;
      
      const variables = {
        companyId: 'test-company-id',
        limit: 10,
        offset: 0
      };
      
      const response = await graphqlClient.request(query, variables);
      
      expect(response.companyFinancialStatements).toBeDefined();
      expect(response.companyFinancialStatements.nodes).toBeInstanceOf(Array);
      expect(response.companyFinancialStatements.totalCount).toBeGreaterThanOrEqual(0);
      expect(response.companyFinancialStatements.hasNextPage).toBeDefined();
      expect(response.companyFinancialStatements.hasPreviousPage).toBeDefined();
    });

    test('should execute SEC crawl mutation', async () => {
      const mutation = `
        mutation TriggerSecCrawl($input: SecCrawlInput!) {
          triggerSecCrawl(input: $input) {
            success
            message
            documentsProcessed
            documentsSkipped
            documentsFailed
            totalSizeBytes
            processingTimeMs
            errors
            warnings
          }
        }
      `;
      
      const variables = {
        input: {
          companyId: 'test-company-id',
          startDate: '2023-01-01',
          endDate: '2023-12-31',
          formTypes: ['10-K', '10-Q', '8-K'],
          maxDocuments: 100,
          includeAmendments: true,
          includeExhibits: false,
          rateLimit: 10,
          retryAttempts: 3,
          timeout: 300
        }
      };
      
      const response = await graphqlClient.request(mutation, variables);
      
      expect(response.triggerSecCrawl).toBeDefined();
      expect(response.triggerSecCrawl.success).toBeDefined();
      expect(response.triggerSecCrawl.message).toBeDefined();
      expect(response.triggerSecCrawl.documentsProcessed).toBeDefined();
      expect(response.triggerSecCrawl.documentsSkipped).toBeDefined();
      expect(response.triggerSecCrawl.documentsFailed).toBeDefined();
      expect(response.triggerSecCrawl.totalSizeBytes).toBeDefined();
      expect(response.triggerSecCrawl.processingTimeMs).toBeDefined();
      expect(response.triggerSecCrawl.errors).toBeInstanceOf(Array);
      expect(response.triggerSecCrawl.warnings).toBeInstanceOf(Array);
    });

    test('should execute RSS import mutation', async () => {
      const mutation = `
        mutation ImportSecRss($input: SecRssImportInput!) {
          importSecRss(input: $input) {
            success
            message
            itemsProcessed
            itemsSkipped
            itemsFailed
            newCompanies
            updatedCompanies
            processingTimeMs
            errors
            warnings
          }
        }
      `;
      
      const variables = {
        input: {
          feedUrl: 'https://www.sec.gov/Archives/edgar/daily-index/xbrl/companyfacts/',
          maxItems: 1000,
          includeAmendments: true,
          includeExhibits: false,
          rateLimit: 10,
          retryAttempts: 3,
          timeout: 300
        }
      };
      
      const response = await graphqlClient.request(mutation, variables);
      
      expect(response.importSecRss).toBeDefined();
      expect(response.importSecRss.success).toBeDefined();
      expect(response.importSecRss.message).toBeDefined();
      expect(response.importSecRss.itemsProcessed).toBeDefined();
      expect(response.importSecRss.itemsSkipped).toBeDefined();
      expect(response.importSecRss.itemsFailed).toBeDefined();
      expect(response.importSecRss.newCompanies).toBeDefined();
      expect(response.importSecRss.updatedCompanies).toBeDefined();
      expect(response.importSecRss.processingTimeMs).toBeDefined();
      expect(response.importSecRss.errors).toBeInstanceOf(Array);
      expect(response.importSecRss.warnings).toBeInstanceOf(Array);
    });
  });

  describe('Error Handling', () => {
    test('should handle GraphQL errors', async () => {
      const query = `
        query InvalidQuery {
          nonExistentField {
            id
          }
        }
      `;
      
      try {
        await graphqlClient.request(query);
        fail('Should have thrown an error');
      } catch (error) {
        expect(error).toBeDefined();
        expect(error.message).toContain('Cannot query field "nonExistentField"');
      }
    });

    test('should handle network errors', async ({ page }) => {
      // Simulate network error by going offline
      await page.context().setOffline(true);
      
      // Try to search for companies
      await page.click('[data-testid="company-search-tab"]');
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      await page.click('[data-testid="search-button"]');
      
      // Should display network error
      await expect(page.locator('[data-testid="network-error"]')).toBeVisible();
      
      // Restore network
      await page.context().setOffline(false);
    });

    test('should handle server errors', async ({ page }) => {
      // Navigate to admin interface
      await page.goto('http://localhost:3000/admin');
      
      // Simulate server error by modifying request
      await page.route('**/graphql', route => {
        route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'Internal server error' })
        });
      });
      
      // Try to search for companies
      await page.click('[data-testid="company-search-tab"]');
      await page.fill('[data-testid="company-search-input"]', 'Apple');
      await page.click('[data-testid="search-button"]');
      
      // Should display server error
      await expect(page.locator('[data-testid="server-error"]')).toBeVisible();
    });
  });

  describe('Performance', () => {
    test('should handle large result sets efficiently', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Search for a broad term that returns many results
      await page.fill('[data-testid="company-search-input"]', 'Inc');
      await page.click('[data-testid="search-button"]');
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Verify results are displayed efficiently
      const results = await page.locator('[data-testid="company-result"]');
      const count = await results.count();
      
      expect(count).toBeGreaterThan(0);
      
      // Verify pagination is working
      if (count > 10) {
        await expect(page.locator('[data-testid="pagination"]')).toBeVisible();
      }
    });

    test('should handle concurrent requests', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Make multiple rapid searches
      const searchTerms = ['Apple', 'Microsoft', 'Google', 'Amazon', 'Tesla'];
      
      for (const term of searchTerms) {
        await page.fill('[data-testid="company-search-input"]', term);
        await page.click('[data-testid="search-button"]');
        await page.waitForSelector('[data-testid="search-results"]');
      }
      
      // Should handle all requests without errors
      await expect(page.locator('[data-testid="search-results"]')).toBeVisible();
    });

    test('should handle long-running operations', async ({ page }) => {
      // Start a crawl with large parameters
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Configure for large crawl
      await page.fill('[data-testid="start-date-input"]', '2020-01-01');
      await page.fill('[data-testid="end-date-input"]', '2023-12-31');
      await page.fill('[data-testid="max-documents-input"]', '1000');
      
      // Start crawl
      await page.click('[data-testid="start-crawl-button"]');
      
      // Wait for progress updates
      await page.waitForSelector('[data-testid="crawl-progress"]');
      
      // Verify progress is being tracked
      await expect(page.locator('[data-testid="progress-bar"]')).toBeVisible();
      
      // Wait for completion (with extended timeout)
      await page.waitForSelector('[data-testid="crawl-complete"]', { timeout: 300000 });
      
      // Verify completion
      await expect(page.locator('[data-testid="crawl-result"]')).toContainText('Crawl completed successfully');
    });
  });

  describe('Accessibility', () => {
    test('should support keyboard navigation', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Use keyboard to navigate
      await page.keyboard.press('Tab'); // Focus search input
      await page.keyboard.type('Apple');
      await page.keyboard.press('Enter'); // Submit search
      
      // Wait for results
      await page.waitForSelector('[data-testid="search-results"]');
      
      // Verify results are accessible
      await expect(page.locator('[data-testid="company-result"]')).toBeVisible();
    });

    test('should have proper ARIA labels', async ({ page }) => {
      // Navigate to company search
      await page.click('[data-testid="company-search-tab"]');
      
      // Verify ARIA labels are present
      const searchInput = page.locator('[data-testid="company-search-input"]');
      await expect(searchInput).toHaveAttribute('aria-label', 'Search companies');
      
      const searchButton = page.locator('[data-testid="search-button"]');
      await expect(searchButton).toHaveAttribute('aria-label', 'Search');
      
      const results = page.locator('[data-testid="search-results"]');
      await expect(results).toHaveAttribute('role', 'region');
      await expect(results).toHaveAttribute('aria-label', 'Search results');
    });

    test('should announce status changes to screen readers', async ({ page }) => {
      // Navigate to crawler manager
      await page.click('[data-testid="crawler-manager-tab"]');
      
      // Start a crawl
      await page.click('[data-testid="start-crawl-button"]');
      
      // Wait for status updates
      await page.waitForSelector('[data-testid="crawl-status"]');
      
      // Verify status is announced
      const status = page.locator('[data-testid="crawl-status"]');
      await expect(status).toHaveAttribute('role', 'status');
      await expect(status).toHaveAttribute('aria-live', 'polite');
    });
  });
});
