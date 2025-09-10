const { chromium } = require('playwright');
const path = require('path');

async function createUltraComprehensiveDemo() {
  console.log('🎬 Starting Ultra-Comprehensive Global Analysis Demo Recording...');

  const browser = await chromium.launch({
    headless: false,
    args: [
      '--disable-web-security',
      '--disable-features=VizDisplayCompositor',
      '--force-device-scale-factor=1'
    ]
  });

  const context = await browser.newContext({
    viewport: { width: 1920, height: 1080 },
    deviceScaleFactor: 1,
  });

  const page = await context.newPage();

  // Start recording
  await page.video.saveAs(path.join(__dirname, 'demo-videos', `ultra-comprehensive-demo-${Date.now()}.webm`));

  // Load the ultra-comprehensive demo HTML
  const htmlPath = path.join(__dirname, 'demo-videos', 'ultra-comprehensive-global-analysis-demo.html');
  await page.goto(`file://${htmlPath}`);

  console.log('📺 Demo page loaded, starting comprehensive recording...');

  // Wait for page to fully load
  await page.waitForTimeout(3000);

  // === INTRODUCTION & OVERVIEW (20 seconds) ===
  console.log('🌟 Recording: Introduction and competitive positioning...');

  // Hover over competitive banner
  await page.hover('.competitive-banner');
  await page.waitForTimeout(2000);

  // Show navigation tabs
  await page.hover('.nav-tab:nth-child(1)');
  await page.waitForTimeout(1000);
  await page.hover('.nav-tab:nth-child(2)');
  await page.waitForTimeout(1000);
  await page.hover('.nav-tab:nth-child(3)');
  await page.waitForTimeout(1000);
  await page.hover('.nav-tab:nth-child(4)');
  await page.waitForTimeout(2000);

  // === GLOBAL NETWORK MAP - COMPREHENSIVE EXPLORATION (60 seconds) ===
  console.log('🌍 Recording: Global Network Map comprehensive features...');

  // Ensure we're on network tab
  await page.click('.nav-tab:nth-child(1)');
  await page.waitForTimeout(2000);

  // Explore feature cards in detail
  const featureCards = await page.locator('.feature-grid .feature-card').all();
  for (let i = 0; i < Math.min(4, featureCards.length); i++) {
    await featureCards[i].hover();
    await page.waitForTimeout(2500); // Extended hover time for detailed view
  }

  // Advanced controls comprehensive demonstration
  console.log('🎛️ Demonstrating advanced network controls...');

  // Correlation threshold adjustment with multiple values
  const thresholdSlider = page.locator('#correlation-threshold');
  await thresholdSlider.hover();
  await page.waitForTimeout(1000);
  await thresholdSlider.fill('0.3');
  await page.waitForTimeout(2000);
  await thresholdSlider.fill('0.6');
  await page.waitForTimeout(2000);
  await thresholdSlider.fill('0.8');
  await page.waitForTimeout(2000);

  // Significance level changes
  await page.selectOption('#significance-level', '0.001');
  await page.waitForTimeout(1500);
  await page.selectOption('#significance-level', '0.05');
  await page.waitForTimeout(1500);
  await page.selectOption('#significance-level', '0.01');
  await page.waitForTimeout(1500);

  // Economic indicators comprehensive cycling
  const indicators = ['gdp', 'inflation', 'unemployment', 'interest', 'trade', 'debt'];
  for (let i = 0; i < indicators.length; i++) {
    await page.selectOption('#primary-indicator', indicators[i]);
    await page.waitForTimeout(1500);
  }

  // Time period adjustments
  await page.fill('#start-year', '2010');
  await page.waitForTimeout(1000);
  await page.fill('#end-year', '2023');
  await page.waitForTimeout(1000);

  // Action buttons demonstration
  await page.hover('button[onclick="updateAnalysis()"]');
  await page.waitForTimeout(1000);
  await page.click('button[onclick="updateAnalysis()"]');
  await page.waitForTimeout(3000); // Wait for loading simulation

  await page.hover('button[onclick="exportNetwork()"]');
  await page.waitForTimeout(1000);
  await page.click('button[onclick="exportNetwork()"]');
  await page.waitForTimeout(1500);

  // Network visualization interaction
  console.log('🔗 Demonstrating network visualization interactions...');

  // Hover over multiple network nodes
  const networkNodes = [
    'circle[fill="#3b82f6"]', // North America nodes
    'circle[fill="#10b981"]', // Europe nodes
    'circle[fill="#f59e0b"]', // Asia nodes
  ];

  for (const nodeSelector of networkNodes) {
    const nodes = await page.locator(nodeSelector).all();
    for (let i = 0; i < Math.min(3, nodes.length); i++) {
      await nodes[i].hover();
      await page.waitForTimeout(2000); // Extended hover for tooltip
    }
  }

  // Hover over correlation links
  const links = await page.locator('.correlation-link').all();
  for (let i = 0; i < Math.min(5, links.length); i++) {
    await links[i].hover();
    await page.waitForTimeout(1500);
  }

  // Statistics banner interaction
  await page.hover('.stats-banner');
  await page.waitForTimeout(2000);

  const statItems = await page.locator('.stat-item').all();
  for (let i = 0; i < Math.min(6, statItems.length); i++) {
    await statItems[i].hover();
    await page.waitForTimeout(1000);
  }

  // === MULTI-COUNTRY DASHBOARD - COMPREHENSIVE ANALYSIS (50 seconds) ===
  console.log('📊 Recording: Multi-Country Dashboard comprehensive features...');

  await page.click('.nav-tab:nth-child(2)');
  await page.waitForTimeout(3000);

  // Dashboard configuration comprehensive demonstration
  console.log('🎯 Demonstrating dashboard configuration...');

  // Country selection interactions
  await page.hover('#primary-countries');
  await page.waitForTimeout(1500);

  // Action buttons for country management
  await page.hover('button[onclick="addCountry()"]');
  await page.waitForTimeout(1000);
  await page.click('button[onclick="addCountry()"]');
  await page.waitForTimeout(1500);

  await page.hover('button[onclick="selectG7()"]');
  await page.waitForTimeout(1000);
  await page.click('button[onclick="selectG7()"]');
  await page.waitForTimeout(2000);

  await page.hover('button[onclick="selectBRICS()"]');
  await page.waitForTimeout(1000);
  await page.click('button[onclick="selectBRICS()"]');
  await page.waitForTimeout(2000);

  // Chart type comprehensive cycling
  const chartTypes = ['line', 'bar', 'area', 'scatter', 'heatmap', 'radar'];
  for (const chartType of chartTypes) {
    await page.selectOption('#chart-type', chartType);
    await page.waitForTimeout(1500);
  }

  // Time range adjustments
  const timeRanges = ['1y', '3y', '5y', '10y', 'max'];
  for (const timeRange of timeRanges) {
    await page.selectOption('#time-range', timeRange);
    await page.waitForTimeout(1200);
  }

  // Export and sharing buttons
  const exportButtons = [
    'button[onclick="exportDashboard()"]',
    'button[onclick="generateReport()"]',
    'button[onclick="shareAnalysis()"]',
    'button[onclick="scheduleUpdate()"]'
  ];

  for (const buttonSelector of exportButtons) {
    await page.hover(buttonSelector);
    await page.waitForTimeout(1000);
    await page.click(buttonSelector);
    await page.waitForTimeout(1500);
  }

  // Metric cards comprehensive interaction
  console.log('💹 Demonstrating metric cards and charts...');

  const metricCards = await page.locator('.metric-card').all();
  for (let i = 0; i < Math.min(6, metricCards.length); i++) {
    await metricCards[i].hover();
    await page.waitForTimeout(2000); // Extended hover for detailed view

    // Hover over chart containers within metric cards
    const chartContainer = metricCards[i].locator('.chart-container');
    if (await chartContainer.count() > 0) {
      await chartContainer.hover();
      await page.waitForTimeout(1500);
    }
  }

  // Dashboard statistics interaction
  await page.hover('.stats-banner');
  await page.waitForTimeout(2000);

  // === GLOBAL EVENTS EXPLORER - COMPREHENSIVE ANALYSIS (60 seconds) ===
  console.log('📈 Recording: Global Events Explorer comprehensive features...');

  await page.click('.nav-tab:nth-child(3)');
  await page.waitForTimeout(3000);

  // Advanced filtering comprehensive demonstration
  console.log('🔍 Demonstrating advanced event filtering...');

  // Event type filtering
  const eventTypes = ['all', 'financial', 'pandemic', 'political', 'trade', 'natural', 'conflict'];
  for (const eventType of eventTypes) {
    await page.selectOption('#event-type', eventType);
    await page.waitForTimeout(1500);
  }

  // Impact severity filtering
  const severityLevels = ['all', 'critical', 'high', 'medium', 'low'];
  for (const severity of severityLevels) {
    await page.selectOption('#impact-severity', severity);
    await page.waitForTimeout(1500);
  }

  // Date range adjustments
  await page.fill('#start-date', '2008-01-01');
  await page.waitForTimeout(1000);
  await page.fill('#end-date', '2024-12-31');
  await page.waitForTimeout(1000);

  // Analysis action buttons
  const analysisButtons = [
    'button[onclick="filterEvents()"]',
    'button[onclick="compareEvents()"]',
    'button[onclick="impactAnalysis()"]',
    'button[onclick="recoveryTracking()"]'
  ];

  for (const buttonSelector of analysisButtons) {
    await page.hover(buttonSelector);
    await page.waitForTimeout(1000);
    await page.click(buttonSelector);
    await page.waitForTimeout(2000);
  }

  // Event items comprehensive interaction
  console.log('🏦 Demonstrating major economic events...');

  const eventItems = await page.locator('.event-item').all();
  for (let i = 0; i < Math.min(6, eventItems.length); i++) {
    await eventItems[i].hover();
    await page.waitForTimeout(3000); // Extended hover for comprehensive view

    // Hover over event severity badges
    const severityBadge = eventItems[i].locator('.event-severity');
    if (await severityBadge.count() > 0) {
      await severityBadge.hover();
      await page.waitForTimeout(1000);
    }

    // Hover over country impact tags
    const countryImpacts = await eventItems[i].locator('.country-impact').all();
    for (let j = 0; j < Math.min(3, countryImpacts.length); j++) {
      await countryImpacts[j].hover();
      await page.waitForTimeout(1000);
    }
  }

  // Events statistics interaction
  await page.hover('.stats-banner');
  await page.waitForTimeout(2000);

  // === ADVANCED ANALYTICS - COMPREHENSIVE DEMONSTRATION (50 seconds) ===
  console.log('🔬 Recording: Advanced Analytics comprehensive features...');

  await page.click('.nav-tab:nth-child(4)');
  await page.waitForTimeout(3000);

  // Advanced analytics feature cards
  console.log('🤖 Demonstrating machine learning and statistical features...');

  const analyticsCards = await page.locator('.feature-grid .feature-card').all();
  for (let i = 0; i < Math.min(4, analyticsCards.length); i++) {
    await analyticsCards[i].hover();
    await page.waitForTimeout(3000); // Extended hover for comprehensive view
  }

  // Analytics controls comprehensive demonstration
  console.log('🔬 Demonstrating advanced analytics controls...');

  // Analysis type cycling
  const analysisTypes = ['forecasting', 'correlation', 'clustering', 'anomaly', 'causality', 'network'];
  for (const analysisType of analysisTypes) {
    await page.selectOption('#analysis-type', analysisType);
    await page.waitForTimeout(1500);
  }

  // Model type cycling
  const modelTypes = ['lstm', 'arima', 'var', 'rf', 'svm'];
  for (const modelType of modelTypes) {
    await page.selectOption('#model-type', modelType);
    await page.waitForTimeout(1500);
  }

  // Forecast horizon adjustments
  const forecastHorizons = ['3m', '6m', '1y', '2y', '5y'];
  for (const horizon of forecastHorizons) {
    await page.selectOption('#forecast-horizon', horizon);
    await page.waitForTimeout(1200);
  }

  // Confidence level adjustments
  const confidenceLevels = ['90', '95', '99'];
  for (const confidence of confidenceLevels) {
    await page.selectOption('#confidence-level', confidence);
    await page.waitForTimeout(1200);
  }

  // Execution buttons comprehensive demonstration
  const executionButtons = [
    'button[onclick="runAnalysis()"]',
    'button[onclick="validateModel()"]',
    'button[onclick="exportResults()"]',
    'button[onclick="scheduleAnalysis()"]'
  ];

  for (const buttonSelector of executionButtons) {
    await page.hover(buttonSelector);
    await page.waitForTimeout(1000);
    await page.click(buttonSelector);
    await page.waitForTimeout(2500); // Extended wait for analysis simulation
  }

  // Analytics visualization interaction
  await page.hover('.visualization-container');
  await page.waitForTimeout(3000);

  // Analytics statistics interaction
  await page.hover('.stats-banner');
  await page.waitForTimeout(2000);

  // === FINAL COMPREHENSIVE OVERVIEW (15 seconds) ===
  console.log('🏆 Recording: Final comprehensive overview...');

  // Navigate through all sections quickly to show completeness
  for (let i = 1; i <= 4; i++) {
    await page.click(`.nav-tab:nth-child(${i})`);
    await page.waitForTimeout(2000);

    // Scroll to show full section
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight / 2));
    await page.waitForTimeout(1500);
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.waitForTimeout(1500);
  }

  // Final hover over header for brand emphasis
  await page.hover('.header h1');
  await page.waitForTimeout(3000);

  console.log('✅ Ultra-comprehensive demo recording completed!');

  // Get the video file path
  const videoPath = await page.video().path();
  console.log(`📹 Video saved to: ${videoPath}`);

  await browser.close();

  return path.basename(videoPath);
}

// Run the demo creation
createUltraComprehensiveDemo()
  .then(videoFile => {
    console.log(`🎬 Ultra-Comprehensive Demo Recording Complete!`);
    console.log(`📁 Video file: ${videoFile}`);
    console.log(`⏱️ Estimated duration: ~5 minutes of comprehensive feature demonstration`);
    console.log(`🏆 Status: Ready for ultra-comprehensive narration creation`);
  })
  .catch(error => {
    console.error('❌ Error creating ultra-comprehensive demo:', error);
  });
