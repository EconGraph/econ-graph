const { chromium } = require('playwright');
const path = require('path');

async function createComprehensiveDemo() {
  console.log('🎬 Creating Comprehensive Global Economic Network Analysis Demo...');

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
    // Load the comprehensive HTML demo file
    const demoPath = path.resolve('./demo-videos/comprehensive-global-analysis-demo.html');
    console.log('📍 Loading comprehensive demo page:', demoPath);
    await page.goto(`file://${demoPath}`);
    await page.waitForTimeout(4000);

    // === INTRODUCTION SEQUENCE ===
    console.log('🌍 Introduction: Revolutionary Platform Overview...');
    await page.waitForSelector('h1');
    await page.waitForTimeout(6000);

    // Show competitive comparison
    console.log('💰 Highlighting competitive advantage...');
    await page.locator('.competitive-comparison').scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    // === NETWORK MAP DETAILED DEMONSTRATION ===
    console.log('🗺️ Network Map: Comprehensive Feature Showcase...');
    await page.waitForTimeout(3000);

    // Show advanced controls panel
    console.log('🎛️ Demonstrating professional analysis controls...');
    await page.locator('.controls-panel').first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Interact with multiple indicators
    console.log('📊 Testing different economic indicators...');
    const indicatorSelect = page.locator('#indicatorSelect');
    await indicatorSelect.click();
    await page.waitForTimeout(1000);
    await indicatorSelect.selectOption('inflation');
    await page.waitForTimeout(2500);

    await indicatorSelect.click();
    await page.waitForTimeout(1000);
    await indicatorSelect.selectOption('unemployment');
    await page.waitForTimeout(2500);

    await indicatorSelect.click();
    await page.waitForTimeout(1000);
    await indicatorSelect.selectOption('trade');
    await page.waitForTimeout(2500);

    // Demonstrate correlation threshold adjustment
    console.log('🎯 Adjusting correlation thresholds...');
    const correlationSlider = page.locator('#correlationSlider');
    await correlationSlider.fill('0.4');
    await page.waitForTimeout(2000);
    await correlationSlider.fill('0.8');
    await page.waitForTimeout(2000);
    await correlationSlider.fill('0.65');
    await page.waitForTimeout(2000);

    // Show time period selection
    console.log('📅 Exploring different time periods...');
    const timePeriod = page.locator('#timePeriod');
    await timePeriod.selectOption('5y');
    await page.waitForTimeout(2000);
    await timePeriod.selectOption('20y');
    await page.waitForTimeout(2000);
    await timePeriod.selectOption('10y');
    await page.waitForTimeout(2000);

    // Statistical significance demonstration
    console.log('📈 Statistical significance configuration...');
    const significance = page.locator('#significance');
    await significance.selectOption('0.001');
    await page.waitForTimeout(2000);
    await significance.selectOption('0.01');
    await page.waitForTimeout(2000);

    // Comprehensive country interaction demonstration
    console.log('🌍 Interactive country exploration...');
    await page.locator('.world-map').scrollIntoViewIfNeeded();
    await page.waitForTimeout(2000);

    // Hover over multiple countries with detailed tooltips
    const countries = page.locator('.country-node');
    console.log('🇺🇸 Exploring United States...');
    await countries.first().hover();
    await page.waitForTimeout(3000);

    console.log('🇨🇳 Exploring China...');
    await countries.nth(1).hover();
    await page.waitForTimeout(3000);

    console.log('🇯🇵 Exploring Japan...');
    await countries.nth(2).hover();
    await page.waitForTimeout(3000);

    console.log('🇩🇪 Exploring Germany...');
    await countries.nth(3).hover();
    await page.waitForTimeout(3000);

    console.log('🇬🇧 Exploring United Kingdom...');
    await countries.nth(4).hover();
    await page.waitForTimeout(3000);

    console.log('🇮🇳 Exploring India...');
    await countries.nth(5).hover();
    await page.waitForTimeout(3000);

    // Show feature showcase
    console.log('✨ Network Analysis Features Overview...');
    await page.locator('.feature-showcase').first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(5000);

    // === MULTI-COUNTRY DASHBOARD DETAILED DEMONSTRATION ===
    console.log('📊 Multi-Country Dashboard: Professional Analysis Suite...');
    await page.locator('text=📊 Multi-Country Dashboard').click();
    await page.waitForTimeout(3000);

    // Show dashboard controls
    console.log('🎯 Dashboard configuration controls...');
    await page.locator('.controls-panel').nth(1).scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Comprehensive country selection
    console.log('🌐 Multi-country selection demonstration...');
    const countrySelection = page.locator('#countrySelection');
    await countrySelection.click();
    await page.waitForTimeout(1500);

    // Add more countries
    await page.keyboard.press('Control+a'); // Select all for demo
    await page.waitForTimeout(2000);
    await page.locator('body').click(); // Click outside to close
    await page.waitForTimeout(2000);

    // Comparison metric changes
    console.log('📈 Testing different comparison metrics...');
    const comparisonMetric = page.locator('#comparisonMetric');
    await comparisonMetric.selectOption('inflation');
    await page.waitForTimeout(2500);
    await comparisonMetric.selectOption('unemployment');
    await page.waitForTimeout(2500);
    await comparisonMetric.selectOption('trade');
    await page.waitForTimeout(2500);
    await comparisonMetric.selectOption('gdp');
    await page.waitForTimeout(2000);

    // Chart type demonstration
    console.log('📊 Chart type variations...');
    const chartType = page.locator('#chartType');
    await chartType.selectOption('line');
    await page.waitForTimeout(2000);
    await chartType.selectOption('area');
    await page.waitForTimeout(2000);
    await chartType.selectOption('scatter');
    await page.waitForTimeout(2000);
    await chartType.selectOption('bar');
    await page.waitForTimeout(2000);

    // Show comprehensive dashboard metrics
    console.log('📊 Dashboard metrics showcase...');
    await page.locator('#dashboardGrid').scrollIntoViewIfNeeded();
    await page.waitForTimeout(5000);

    // Dashboard features overview
    console.log('💼 Dashboard features deep dive...');
    await page.locator('.feature-showcase').nth(1).scrollIntoViewIfNeeded();
    await page.waitForTimeout(5000);

    // === GLOBAL EVENTS EXPLORER COMPREHENSIVE DEMONSTRATION ===
    console.log('📅 Global Events Explorer: Crisis Analysis Platform...');
    await page.locator('text=📅 Global Events Explorer').click();
    await page.waitForTimeout(3000);

    // Advanced event filtering
    console.log('🔍 Advanced event filtering capabilities...');
    await page.locator('.controls-panel').nth(2).scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Event type selection
    console.log('📋 Event category filtering...');
    const eventTypes = page.locator('#eventTypes');
    await eventTypes.click();
    await page.waitForTimeout(2000);

    // Impact severity adjustment
    console.log('⚖️ Impact severity threshold testing...');
    const impactSlider = page.locator('#impactSlider');
    await impactSlider.fill('5');
    await page.waitForTimeout(2000);
    await impactSlider.fill('2');
    await page.waitForTimeout(2000);
    await impactSlider.fill('3');
    await page.waitForTimeout(2000);

    // Time range exploration
    console.log('📅 Historical time range analysis...');
    const eventTimeRange = page.locator('#eventTimeRange');
    await eventTimeRange.selectOption('5y');
    await page.waitForTimeout(2000);
    await eventTimeRange.selectOption('all');
    await page.waitForTimeout(2000);
    await eventTimeRange.selectOption('25y');
    await page.waitForTimeout(2000);

    // Recovery status filtering
    console.log('🔄 Recovery status analysis...');
    const recoveryFilter = page.locator('#recoveryFilter');
    await recoveryFilter.selectOption('recovered');
    await page.waitForTimeout(2000);
    await recoveryFilter.selectOption('ongoing');
    await page.waitForTimeout(2000);
    await recoveryFilter.selectOption('all');
    await page.waitForTimeout(2000);

    // Comprehensive timeline exploration
    console.log('📖 Detailed timeline exploration...');
    const timelineItems = page.locator('.timeline-item');

    console.log('💰 2008 Financial Crisis analysis...');
    await timelineItems.first().scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    console.log('🦠 COVID-19 pandemic impact...');
    await timelineItems.nth(1).scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    console.log('🏛️ Brexit economic transition...');
    await timelineItems.nth(2).scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    console.log('📦 US-China trade war effects...');
    await timelineItems.nth(3).scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    console.log('🛢️ Russia-Ukraine conflict impact...');
    await timelineItems.nth(4).scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    // === REVOLUTIONARY ACHIEVEMENT SHOWCASE ===
    console.log('🏆 Revolutionary platform achievement statistics...');
    await page.locator('.stats-grid').scrollIntoViewIfNeeded();
    await page.waitForTimeout(6000);

    // Final achievement banner
    console.log('🌟 Platform achievement and competitive positioning...');
    await page.locator('.achievement-banner').scrollIntoViewIfNeeded();
    await page.waitForTimeout(6000);

    // === COMPREHENSIVE FEATURE TOUR FINALE ===
    console.log('🔄 Comprehensive feature tour finale...');

    // Return to network map for final showcase
    console.log('🗺️ Returning to network map for comprehensive finale...');
    await page.locator('text=🗺️ Economic Network Map').click();
    await page.waitForTimeout(2000);

    // Final network interaction
    console.log('🌍 Final network visualization showcase...');
    await page.locator('.world-map').scrollIntoViewIfNeeded();
    await page.waitForTimeout(3000);

    // Demonstrate final correlation adjustment
    await correlationSlider.fill('0.7');
    await page.waitForTimeout(2000);

    // Final country interactions
    console.log('🌐 Final country network exploration...');
    await countries.first().hover();
    await page.waitForTimeout(2000);
    await countries.nth(1).hover();
    await page.waitForTimeout(2000);
    await countries.nth(2).hover();
    await page.waitForTimeout(2000);

    // Scroll to show the competitive advantage one final time
    console.log('💎 Final competitive advantage showcase...');
    await page.locator('h1').scrollIntoViewIfNeeded();
    await page.waitForTimeout(4000);

    console.log('✅ Comprehensive professional demo recording completed successfully!');

  } catch (error) {
    console.error('❌ Error during comprehensive demo recording:', error);
    await page.screenshot({ path: 'comprehensive-demo-error.png', fullPage: true });
    throw error;
  } finally {
    await context.close();
    await browser.close();
  }
}

createComprehensiveDemo().catch(console.error);
