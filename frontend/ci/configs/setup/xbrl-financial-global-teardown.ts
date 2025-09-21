/**
 * Global Teardown for XBRL Financial Integration Tests
 * 
 * This teardown cleans up the test environment after XBRL financial
 * integration tests are complete, ensuring proper cleanup of resources
 * and test data.
 */

import { chromium, FullConfig } from '@playwright/test';

async function globalTeardown(config: FullConfig) {
  console.log('🧹 Cleaning up XBRL Financial Integration Test Environment...');

  try {
    // Launch browser for cleanup verification
    const browser = await chromium.launch();
    const page = await browser.newPage();

    // Navigate to the application to perform cleanup
    await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });

    // Clear test data from localStorage
    console.log('🗑️  Clearing test data from localStorage...');
    await page.evaluate(() => {
      localStorage.removeItem('testXbrlData');
      localStorage.removeItem('testFinancialData');
      localStorage.removeItem('testBenchmarkData');
      localStorage.removeItem('enableXbrlFeatures');
      localStorage.removeItem('enableFinancialAnalysis');
    });

    // Clear any cached financial data
    console.log('🗑️  Clearing cached financial data...');
    await page.evaluate(() => {
      if ('caches' in window) {
        caches.keys().then(names => {
          names.forEach(name => {
            if (name.includes('financial') || name.includes('xbrl')) {
              caches.delete(name);
            }
          });
        });
      }
    });

    // Reset any test-specific state
    console.log('🔄 Resetting test state...');
    await page.evaluate(() => {
      // Reset any global test state
      if (typeof window !== 'undefined') {
        (window as any).__TEST_STATE__ = {};
        (window as any).__XBRL_TEST_DATA__ = null;
        (window as any).__FINANCIAL_TEST_DATA__ = null;
      }
    });

    await browser.close();

    console.log('✅ XBRL Financial Integration Test Environment Cleanup Complete');

  } catch (error) {
    console.error('❌ Failed to cleanup XBRL Financial Integration Test Environment:', error);
    // Don't throw error during teardown to avoid masking test failures
  }
}

export default globalTeardown;
