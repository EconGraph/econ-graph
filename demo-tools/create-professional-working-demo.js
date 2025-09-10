const { chromium } = require('playwright');
const path = require('path');

async function createProfessionalDemo() {
  console.log('🎬 Creating Professional Global Economic Network Analysis Demo...');

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
    // Load the HTML demo file
    const demoPath = path.resolve('./demo-videos/working-global-analysis-demo.html');
    console.log('📍 Loading demo page:', demoPath);
    await page.goto(`file://${demoPath}`);
    await page.waitForTimeout(3000);

    // Introduction - Show the main title and overview
    console.log('🌍 Introduction: Global Economic Network Analysis System...');
    await page.waitForSelector('h1');
    await page.waitForTimeout(5000);

    // Network Map Tab (already active)
    console.log('🗺️ Demonstrating Interactive Global Economic Network Map...');
    await page.waitForTimeout(3000);

    // Interact with controls
    console.log('🎛️ Showing professional controls...');
    const indicatorSelect = page.locator('select').first();
    await indicatorSelect.click();
    await page.waitForTimeout(1000);
    await indicatorSelect.selectOption('Inflation Rate');
    await page.waitForTimeout(2000);

    // Adjust correlation threshold
    console.log('📊 Adjusting correlation threshold...');
    const slider = page.locator('input[type="range"]').first();
    await slider.fill('0.8');
    await page.waitForTimeout(2000);

    // Hover over countries to show interaction
    console.log('🌍 Demonstrating country interactions...');
    const countries = page.locator('.country-node');
    await countries.first().hover();
    await page.waitForTimeout(1500);
    await countries.nth(1).hover();
    await page.waitForTimeout(1500);
    await countries.nth(2).hover();
    await page.waitForTimeout(1500);

    // Show feature cards
    console.log('✨ Highlighting key features...');
    await page.locator('.feature-grid').first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Switch to Multi-Country Dashboard
    console.log('📊 Switching to Multi-Country Dashboard...');
    await page.locator('text=📊 Multi-Country Dashboard').click();
    await page.waitForTimeout(3000);

    // Show dashboard controls
    console.log('🎯 Demonstrating dashboard controls...');
    const countrySelect = page.locator('select[multiple]').first();
    await countrySelect.click();
    await page.waitForTimeout(2000);

    // Show metric cards
    console.log('📈 Showing economic metrics...');
    await page.locator('.dashboard-grid').scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    // Show dashboard features
    console.log('💼 Highlighting dashboard features...');
    await page.locator('.feature-grid').nth(1).scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Switch to Global Events Explorer
    console.log('📅 Switching to Global Events Explorer...');
    await page.locator('text=📅 Global Events Explorer').click();
    await page.waitForTimeout(3000);

    // Show event controls
    console.log('🔍 Demonstrating event filtering...');
    const eventSelect = page.locator('select[multiple]').nth(1);
    await eventSelect.click();
    await page.waitForTimeout(1500);

    // Adjust impact score
    console.log('⚖️ Adjusting impact score threshold...');
    const impactSlider = page.locator('input[type="range"]').nth(1);
    await impactSlider.fill('4');
    await page.waitForTimeout(2000);

    // Show timeline events
    console.log('📖 Exploring major economic events...');
    const timelineItems = page.locator('.timeline-item');
    await timelineItems.first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);
    await timelineItems.nth(1).scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);
    await timelineItems.nth(2).scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Show global statistics
    console.log('📊 Showing revolutionary achievement statistics...');
    await page.locator('.stats-grid').scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    // Final overview - scroll to conclusion
    console.log('🏆 Final overview of revolutionary platform...');
    await page.locator('text=World\'s First Open-Source').scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    // Return to network map for finale
    console.log('🌍 Returning to network map for finale...');
    await page.locator('text=🗺️ Economic Network Map').click();
    await page.waitForTimeout(2000);
    await page.locator('.world-map').scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    console.log('✅ Professional demo recording completed successfully!');

  } catch (error) {
    console.error('❌ Error during demo recording:', error);
    await page.screenshot({ path: 'error-screenshot.png', fullPage: true });
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

createProfessionalDemo().catch(console.error);
