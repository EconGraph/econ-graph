/**
 * Global Setup for XBRL Financial Integration Tests
 *
 * This setup ensures the test environment is properly configured for
 * XBRL financial statement integration testing, including mock data
 * setup and API endpoint configuration.
 */

import { chromium, FullConfig } from '@playwright/test';

async function globalSetup(config: FullConfig) {
  console.log('🔧 Setting up XBRL Financial Integration Test Environment...');

  // Set up test environment variables
  process.env.NODE_ENV = 'test';
  process.env.REACT_APP_API_BASE_URL = 'http://localhost:8000';
  process.env.REACT_APP_ENABLE_XBRL_FEATURES = 'true';
  process.env.REACT_APP_ENABLE_FINANCIAL_ANALYSIS = 'true';
  process.env.REACT_APP_MOCK_API_RESPONSES = 'true';

  // Launch browser for setup verification
  const browser = await chromium.launch();
  const page = await browser.newPage();

  try {
    // Verify the frontend application is accessible
    console.log('🌐 Verifying frontend application accessibility...');
    await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });

    // Check if XBRL features are enabled
    const xbrlFeaturesEnabled = await page.evaluate(() => {
      return (
        window.location.search.includes('enableXbrl=true') ||
        localStorage.getItem('enableXbrlFeatures') === 'true'
      );
    });

    if (!xbrlFeaturesEnabled) {
      console.log('⚠️  XBRL features not enabled, enabling for tests...');
      await page.evaluate(() => {
        localStorage.setItem('enableXbrlFeatures', 'true');
        localStorage.setItem('enableFinancialAnalysis', 'true');
      });
    }

    // Verify financial components are loaded
    console.log('📊 Verifying financial components availability...');
    const hasFinancialComponents = await page.evaluate(() => {
      return (
        typeof window !== 'undefined' &&
        window.document.querySelector('[data-testid="financial-dashboard"]') !== null
      );
    });

    if (!hasFinancialComponents) {
      console.log('📦 Financial components not found, ensuring proper loading...');
      // Wait for components to load
      await page.waitForTimeout(2000);
    }

    console.log('✅ XBRL Financial Integration Test Environment Setup Complete');
  } catch (error) {
    console.error('❌ Failed to setup XBRL Financial Integration Test Environment:', error);
    throw error;
  } finally {
    await browser.close();
  }
}

export default globalSetup;
