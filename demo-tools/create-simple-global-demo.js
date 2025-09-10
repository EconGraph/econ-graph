const { chromium } = require('playwright');

/**
 * Simple Global Analysis Demo
 * Creates a demo using the existing frontend and simulated global analysis features
 */

async function createSimpleGlobalDemo() {
  console.log('🌍 Creating Simple Global Analysis Demo...');

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
    await page.goto('http://localhost:3000');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(3000);
    console.log('✅ Application loaded');

    // SEGMENT 1: Show the main dashboard
    console.log('🎬 SEGMENT 1: EconGraph Dashboard Overview');
    await page.waitForTimeout(3000);

    // Try to open sidebar if available
    try {
      await page.click('button[aria-label="menu"]', { timeout: 2000 });
      await page.waitForTimeout(2000);
    } catch (e) {
      console.log('Menu button not found, continuing...');
    }

    // SEGMENT 2: Navigate to Series Explorer (existing feature)
    console.log('🎬 SEGMENT 2: Economic Data Exploration');

    try {
      // Try different ways to navigate to series explorer
      await page.click('text=Explore', { timeout: 2000 });
      await page.waitForTimeout(2000);
    } catch (e) {
      try {
        await page.click('text=Series', { timeout: 2000 });
        await page.waitForTimeout(2000);
      } catch (e2) {
        try {
          await page.click('a[href="/explore"]', { timeout: 2000 });
          await page.waitForTimeout(2000);
        } catch (e3) {
          console.log('Navigation to explore not found, staying on current page');
        }
      }
    }

    // SEGMENT 3: Search functionality
    console.log('🎬 SEGMENT 3: Economic Data Search');

    try {
      // Look for search input
      await page.fill('input[type="search"]', 'GDP');
      await page.waitForTimeout(2000);
      await page.fill('input[type="search"]', 'Unemployment');
      await page.waitForTimeout(2000);
      await page.fill('input[type="search"]', 'Inflation');
      await page.waitForTimeout(2000);
    } catch (e) {
      try {
        await page.fill('input[placeholder*="Search"]', 'Economic Data');
        await page.waitForTimeout(2000);
      } catch (e2) {
        console.log('Search input not found, continuing...');
      }
    }

    // SEGMENT 4: Show data sources
    console.log('🎬 SEGMENT 4: Data Sources');

    try {
      await page.click('text=Sources', { timeout: 2000 });
      await page.waitForTimeout(3000);
    } catch (e) {
      try {
        await page.click('a[href="/sources"]', { timeout: 2000 });
        await page.waitForTimeout(3000);
      } catch (e2) {
        console.log('Data sources navigation not found');
      }
    }

    // SEGMENT 5: Professional Analysis (if available)
    console.log('🎬 SEGMENT 5: Professional Analysis Features');

    try {
      await page.click('text=Analysis', { timeout: 2000 });
      await page.waitForTimeout(3000);
    } catch (e) {
      console.log('Analysis section not found');
    }

    // SEGMENT 6: Global Analysis Concept (simulate)
    console.log('🎬 SEGMENT 6: Global Economic Analysis Concept');

    // Navigate back to main dashboard
    try {
      await page.click('text=Dashboard', { timeout: 2000 });
      await page.waitForTimeout(2000);
    } catch (e) {
      try {
        await page.goto('http://localhost:3000');
        await page.waitForTimeout(2000);
      } catch (e2) {
        console.log('Dashboard navigation not found');
      }
    }

    // SEGMENT 7: Demonstrate responsive design
    console.log('🎬 SEGMENT 7: Responsive Design');

    // Show mobile view
    await page.setViewportSize({ width: 768, height: 1024 });
    await page.waitForTimeout(2000);

    // Back to desktop
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.waitForTimeout(2000);

    // SEGMENT 8: Final overview
    console.log('🎬 SEGMENT 8: Platform Overview');
    await page.waitForTimeout(4000);

    console.log('🎉 Simple demo recording completed!');

  } catch (error) {
    console.error('❌ Demo recording failed:', error);
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

if (require.main === module) {
  createSimpleGlobalDemo()
    .then(() => {
      console.log('🎉 Simple Global Analysis demo recording completed!');
      process.exit(0);
    })
    .catch((error) => {
      console.error('💥 Demo recording failed:', error);
      process.exit(1);
    });
}
