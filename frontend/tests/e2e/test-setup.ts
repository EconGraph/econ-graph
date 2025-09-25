import { test as base, expect } from '@playwright/test';

// Extend the base test to add network logging (only if debugging is enabled)
export const test = base.extend({
  page: async ({ page }, use) => {
    // Only add network logging if PLAYWRIGHT_DEBUG_NETWORK is enabled
    const debugNetwork = process.env.PLAYWRIGHT_DEBUG_NETWORK === 'true';

    if (debugNetwork) {
      // Add network request logging to every test
      page.on('request', (request) => {
        console.log(`🌐 Test Network Request: ${request.method()} ${request.url()}`);
      });

      page.on('response', (response) => {
        const status = response.status();
        const url = response.url();
        console.log(`📡 Test Network Response: ${status} ${url}`);
      });

      page.on('requestfailed', (request) => {
        const failure = request.failure();
        const url = request.url();
        const errorText = failure?.errorText || 'Unknown error';

        console.log(`💥 Test Network Failure: ${url}`);
        console.log(`   Error: ${errorText}`);

        if (errorText.includes('net::ERR_CONNECTION_REFUSED')) {
          console.log(`   🚨 INVALID PORT DETECTED: Connection refused to ${url}`);
        }
      });
    }

    await use(page);
  },
});

export { expect };
