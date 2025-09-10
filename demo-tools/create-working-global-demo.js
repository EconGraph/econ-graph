const { chromium } = require('playwright');

async function createGlobalAnalysisDemo() {
  console.log('🎬 Starting Global Economic Network Analysis Demo Recording...');

  const browser = await chromium.launch({
    headless: false,
    args: ['--disable-web-security', '--allow-running-insecure-content']
  });

  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    recordVideo: {
      dir: './demo-videos/',
      size: { width: 1920, height: 1080 }
    }
  });

  const page = await context.newPage();

  try {
    // Navigate to the application
    console.log('📍 Navigating to EconGraph application...');
    await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });
    await page.waitForTimeout(2000);

    // Show the main dashboard first
    console.log('🏠 Showing main dashboard...');
    await page.waitForSelector('text=EconGraph', { timeout: 30000 });
    await page.waitForTimeout(5000);

    // Navigate to Global Analysis
    console.log('🌍 Navigating to Global Analysis...');
    const globalAnalysisLink = page.locator('text=Global Analysis').first();
    await globalAnalysisLink.click();
    await page.waitForTimeout(3000);

    // Show the Global Analysis page
    console.log('📊 Demonstrating Global Analysis features...');
    await page.waitForSelector('text=Interactive Global Economic Network Map', { timeout: 10000 });
    await page.waitForTimeout(4000);

    // Interact with the network map controls
    console.log('🎛️ Interacting with network map controls...');
    const indicatorSelect = page.locator('text=Economic Indicator').first();
    if (await indicatorSelect.isVisible()) {
      await indicatorSelect.click();
      await page.waitForTimeout(1000);
      await page.locator('text=Inflation Rate').first().click();
      await page.waitForTimeout(2000);
    }

    // Adjust correlation threshold
    console.log('📈 Adjusting correlation threshold...');
    const slider = page.locator('input[type="range"]').first();
    if (await slider.isVisible()) {
      await slider.fill('0.7');
      await page.waitForTimeout(2000);
    }

    // Click on a country in the map
    console.log('🗺️ Clicking on countries in the network map...');
    const countries = page.locator('circle.country');
    const countryCount = await countries.count();
    if (countryCount > 0) {
      await countries.first().click();
      await page.waitForTimeout(3000);
    }

    // Switch to Multi-Country Dashboard tab
    console.log('📊 Switching to Multi-Country Dashboard...');
    const dashboardTab = page.locator('text=Multi-Country Dashboard').first();
    await dashboardTab.click();
    await page.waitForTimeout(3000);

    // Show dashboard features
    console.log('💼 Demonstrating dashboard features...');
    const countrySelect = page.locator('text=Select Countries to Compare').first();
    if (await countrySelect.isVisible()) {
      await countrySelect.click();
      await page.waitForTimeout(1000);
      const option = page.locator('text=China (CN)').first();
      if (await option.isVisible()) {
        await option.click();
        await page.waitForTimeout(2000);
      }
      // Click outside to close the dropdown
      await page.locator('body').click();
      await page.waitForTimeout(2000);
    }

    // Switch between dashboard tabs
    console.log('🔄 Switching between dashboard views...');
    const chartTab = page.locator('text=Comparison Charts').first();
    if (await chartTab.isVisible()) {
      await chartTab.click();
      await page.waitForTimeout(3000);
    }

    const tableTab = page.locator('text=Data Table').first();
    if (await tableTab.isVisible()) {
      await tableTab.click();
      await page.waitForTimeout(3000);
    }

    // Switch to Global Events Explorer
    console.log('📅 Switching to Global Events Explorer...');
    const eventsTab = page.locator('text=Global Events Explorer').first();
    await eventsTab.click();
    await page.waitForTimeout(3000);

    // Interact with event filters
    console.log('🔍 Demonstrating event filtering...');
    const eventTypeSelect = page.locator('text=Event Types').first();
    if (await eventTypeSelect.isVisible()) {
      await eventTypeSelect.click();
      await page.waitForTimeout(1000);
      const financialCrisis = page.locator('text=Financial Crisis').first();
      if (await financialCrisis.isVisible()) {
        await financialCrisis.click();
        await page.waitForTimeout(2000);
      }
      // Click outside to close
      await page.locator('body').click();
      await page.waitForTimeout(2000);
    }

    // Expand an event to show details
    console.log('📖 Expanding event details...');
    const expandButton = page.locator('button[aria-label="expand"]').first();
    if (await expandButton.isVisible()) {
      await expandButton.click();
      await page.waitForTimeout(3000);
    }

    // Scroll to show the global impact statistics
    console.log('📊 Showing global impact statistics...');
    await page.locator('text=Global Impact Statistics').scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Final overview
    console.log('🎯 Final overview of the Global Analysis System...');
    await page.locator('text=Interactive Global Economic Network Map').first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(2000);

    // Switch back to network map for finale
    const networkTab = page.locator('text=Economic Network Map').first();
    if (await networkTab.isVisible()) {
      await networkTab.click();
      await page.waitForTimeout(4000);
    }

    console.log('✅ Demo recording completed successfully!');

  } catch (error) {
    console.error('❌ Error during demo recording:', error);
    // Take a screenshot for debugging
    await page.screenshot({ path: 'error-screenshot.png', fullPage: true });
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

createGlobalAnalysisDemo().catch(console.error);
