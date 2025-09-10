const { chromium } = require('playwright');
const path = require('path');

/**
 * Global Economic Network Analysis Demo
 * This script demonstrates ONLY features that are actually implemented and visible
 * Every narrated feature will be shown in the video
 */

async function createGlobalAnalysisDemo() {
  console.log('🌍 Starting Global Economic Network Analysis Demo Recording...');

  const browser = await chromium.launch({
    headless: false,
    args: ['--no-sandbox', '--disable-web-security']
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
    console.log('📱 Navigating to EconGraph application...');

    // Navigate to the application (assuming it's running on localhost:3000)
    await page.goto('http://localhost:3000');
    await page.waitForLoadState('networkidle');

    // Wait for initial load
    await page.waitForTimeout(3000);
    console.log('✅ Application loaded');

    // SEGMENT 1: Navigate to Global Analysis
    console.log('🎬 SEGMENT 1: Navigation to Global Analysis');

    // Click on Global Analysis in sidebar
    await page.click('text=Global Analysis');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);
    console.log('✅ Global Analysis page loaded');

    // SEGMENT 2: Overview of Global Analysis Interface
    console.log('🎬 SEGMENT 2: Global Analysis Interface Overview');

    // Show the main header and tabs
    await page.waitForSelector('h3:has-text("Global Economic Network Analysis")');
    await page.waitForTimeout(2000);

    // Highlight the different tabs available
    await page.hover('[role="tab"]:has-text("Network Map")');
    await page.waitForTimeout(1000);
    await page.hover('[role="tab"]:has-text("Multi-Country Dashboard")');
    await page.waitForTimeout(1000);
    await page.hover('[role="tab"]:has-text("Global Events")');
    await page.waitForTimeout(1000);
    await page.hover('[role="tab"]:has-text("Impact Analysis")');
    await page.waitForTimeout(2000);
    console.log('✅ Interface overview complete');

    // SEGMENT 3: Interactive World Map Network Visualization
    console.log('🎬 SEGMENT 3: Interactive World Map');

    // Make sure we're on the Network Map tab (should be default)
    await page.click('[role="tab"]:has-text("Network Map")');
    await page.waitForTimeout(2000);

    // Wait for the map to load
    await page.waitForSelector('svg', { timeout: 10000 });
    await page.waitForTimeout(3000);

    // Show the economic indicator selector
    await page.click('text=Economic Indicator');
    await page.waitForTimeout(1000);
    await page.click('text=GDP Growth');
    await page.waitForTimeout(2000);

    // Adjust the minimum correlation slider
    const slider = await page.locator('input[type="range"]').first();
    await slider.fill('0.5');
    await page.waitForTimeout(2000);

    // Toggle connections on/off to show the feature
    await page.click('text=Show Connections');
    await page.waitForTimeout(2000);
    await page.click('text=Show Connections');
    await page.waitForTimeout(2000);

    // Try different economic indicators
    await page.click('text=Economic Indicator');
    await page.waitForTimeout(500);
    await page.click('text=Trade Flows');
    await page.waitForTimeout(3000);

    await page.click('text=Economic Indicator');
    await page.waitForTimeout(500);
    await page.click('text=Inflation');
    await page.waitForTimeout(3000);
    console.log('✅ Network Map demonstration complete');

    // SEGMENT 4: Multi-Country Dashboard
    console.log('🎬 SEGMENT 4: Multi-Country Dashboard');

    // Switch to Multi-Country Dashboard tab
    await page.click('[role="tab"]:has-text("Multi-Country Dashboard")');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);

    // Show country selection
    await page.click('input[placeholder="Search countries..."]');
    await page.type('input[placeholder="Search countries..."]', 'France');
    await page.waitForTimeout(1000);

    // Select France if available in dropdown
    try {
      await page.click('text=France', { timeout: 2000 });
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('France not found in dropdown, continuing...');
    }

    // Clear search and try another country
    await page.fill('input[placeholder="Search countries..."]', '');
    await page.type('input[placeholder="Search countries..."]', 'Canada');
    await page.waitForTimeout(1000);

    try {
      await page.click('text=Canada', { timeout: 2000 });
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('Canada not found in dropdown, continuing...');
    }

    // Show different indicator tabs
    await page.click('text=Inflation');
    await page.waitForTimeout(2000);
    await page.click('text=Unemployment');
    await page.waitForTimeout(2000);
    await page.click('text=GDP');
    await page.waitForTimeout(2000);

    // Toggle sync charts
    const syncSwitch = await page.locator('text=Sync Charts').locator('..').locator('input[type="checkbox"]');
    try {
      await syncSwitch.click();
      await page.waitForTimeout(1000);
      await syncSwitch.click();
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('Sync Charts toggle not found, continuing...');
    }
    console.log('✅ Multi-Country Dashboard demonstration complete');

    // SEGMENT 5: Global Events Explorer
    console.log('🎬 SEGMENT 5: Global Events Explorer');

    // Switch to Global Events tab
    await page.click('[role="tab"]:has-text("Global Events")');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);

    // Show event type filter
    await page.click('text=Event Type');
    await page.waitForTimeout(1000);
    await page.click('text=Financial Crisis');
    await page.waitForTimeout(2000);

    await page.click('text=Event Type');
    await page.waitForTimeout(1000);
    await page.click('text=All Events');
    await page.waitForTimeout(2000);

    // Adjust impact score slider
    const impactSlider = await page.locator('input[type="range"]').first();
    try {
      await impactSlider.fill('30');
      await page.waitForTimeout(2000);
      await impactSlider.fill('70');
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('Impact slider not found, continuing...');
    }

    // Toggle show recovered countries
    try {
      await page.click('text=Show Recovered Countries');
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('Show Recovered Countries toggle not found, continuing...');
    }

    // Try to expand an event if any are visible
    try {
      await page.click('button[aria-label="expand more"]', { timeout: 2000 });
      await page.waitForTimeout(3000);
    } catch (e) {
      console.log('No expandable events found, continuing...');
    }
    console.log('✅ Global Events Explorer demonstration complete');

    // SEGMENT 6: Impact Analysis (Placeholder)
    console.log('🎬 SEGMENT 6: Impact Analysis Preview');

    // Switch to Impact Analysis tab
    await page.click('[role="tab"]:has-text("Impact Analysis")');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);

    // Show the placeholder content
    await page.waitForTimeout(4000);
    console.log('✅ Impact Analysis preview complete');

    // SEGMENT 7: Return to Network Map for Final Overview
    console.log('🎬 SEGMENT 7: Final Network Map Overview');

    // Return to Network Map
    await page.click('[role="tab"]:has-text("Network Map")');
    await page.waitForTimeout(2000);

    // Final demonstration of controls
    await page.click('text=Economic Indicator');
    await page.waitForTimeout(500);
    await page.click('text=GDP Growth');
    await page.waitForTimeout(2000);

    // Final view of the complete interface
    await page.waitForTimeout(4000);
    console.log('✅ Final overview complete');

    console.log('🎉 Demo recording completed successfully!');

  } catch (error) {
    console.error('❌ Demo recording failed:', error);
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

if (require.main === module) {
  createGlobalAnalysisDemo()
    .then(() => {
      console.log('🎉 Global Analysis demo recording completed!');
      process.exit(0);
    })
    .catch((error) => {
      console.error('💥 Demo recording failed:', error);
      process.exit(1);
    });
}
