import { chromium, FullConfig } from '@playwright/test';

async function globalSetup(config: FullConfig) {
  console.log('🔧 Playwright Global Setup - Network Error Interception');

  // Create a browser context to test network connectivity
  const browser = await chromium.launch();
  const context = await browser.newContext();

  // Add network error interception
  context.on('request', (request) => {
    console.log(`🌐 Playwright Network Request: ${request.method()} ${request.url()}`);
  });

  context.on('response', (response) => {
    const status = response.status();
    const url = response.url();
    if (status >= 400) {
      console.log(`❌ Playwright Network Error: ${status} ${url}`);
    } else {
      console.log(`✅ Playwright Network Success: ${status} ${url}`);
    }
  });

  // Add request failed interception with detailed error analysis
  context.on('requestfailed', (request) => {
    const failure = request.failure();
    const url = request.url();
    const errorText = failure?.errorText || 'Unknown error';

    console.log(`💥 Playwright Network Failure: ${url}`);
    console.log(`   Error: ${errorText}`);

    // Analyze specific error types
    if (errorText.includes('net::ERR_CONNECTION_REFUSED')) {
      console.log(`   🚨 INVALID PORT DETECTED: Connection refused to ${url}`);
      console.log(`   🔍 This usually means nothing is listening on that port`);
    } else if (errorText.includes('net::ERR_CONNECTION_TIMED_OUT')) {
      console.log(`   ⏰ CONNECTION TIMEOUT: ${url} - service may be down`);
    } else if (errorText.includes('net::ERR_NAME_NOT_RESOLVED')) {
      console.log(`   🌐 DNS ERROR: Cannot resolve hostname in ${url}`);
    } else {
      console.log(`   ❓ OTHER ERROR: ${errorText}`);
    }
  });

  // Test basic connectivity
  console.log('🔧 Testing basic network connectivity...');
  try {
    const page = await context.newPage();

    // Test frontend connectivity
    console.log('  - Testing frontend connectivity...');
    const frontendResponse = await page.goto(process.env.FRONTEND_URL || 'http://localhost:3000', {
      timeout: 10000,
      waitUntil: 'networkidle'
    });
    console.log(`    Frontend response: ${frontendResponse?.status()}`);

    // Test backend connectivity
    console.log('  - Testing backend connectivity...');
    const backendResponse = await page.request.get(process.env.BACKEND_URL + '/health' || 'http://localhost:51249/health');
    console.log(`    Backend response: ${backendResponse.status()}`);

  } catch (error) {
    console.error('❌ Network connectivity test failed:', error);
  }

  await browser.close();
}

export default globalSetup;
