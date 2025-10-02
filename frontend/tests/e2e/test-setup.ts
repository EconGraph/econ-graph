import { test as base, expect } from '@playwright/test';

// Extend the base test to add network logging
export const test = base.extend({
  page: async ({ page }, use) => {
    // Add network request logging to every test
    page.on('request', request => {
      console.log(`🌐 Test Network Request: ${request.method()} ${request.url()}`);
    });

    page.on('response', response => {
      const status = response.status();
      const url = response.url();
      console.log(`📡 Test Network Response: ${status} ${url}`);
    });

    page.on('requestfailed', request => {
      const failure = request.failure();
      const url = request.url();
      const errorText = failure?.errorText || 'Unknown error';

      console.log(`💥 Test Network Failure: ${url}`);
      console.log(`   Error: ${errorText}`);

      if (errorText.includes('net::ERR_CONNECTION_REFUSED')) {
        console.log(`   🚨 INVALID PORT DETECTED: Connection refused to ${url}`);
      }
    });

    await use(page);
  },
});

export { expect };
