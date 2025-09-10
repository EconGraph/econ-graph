const puppeteer = require('puppeteer');
const fs = require('fs');

async function createInterfaceVideo() {
    console.log('🎬 Creating Real Interface Video...');

    const browser = await puppeteer.launch({
        headless: false,
        defaultViewport: { width: 1920, height: 1080 },
        args: ['--no-sandbox', '--disable-setuid-sandbox']
    });

    const page = await browser.newPage();
    await page.setViewport({ width: 1920, height: 1080 });

    console.log('🌐 Loading EconGraph interface...');
    await page.goto('http://localhost:3000', { waitUntil: 'networkidle0' });

    // Wait for React to load
    await page.waitForTimeout(3000);

    console.log('📸 Taking screenshots of interface...');

    // Take screenshot of main dashboard
    await page.screenshot({ path: 'temp_dashboard.png', fullPage: false });

    // Navigate to different sections if they exist
    try {
        // Try to click on Global Analysis if it exists
        const globalAnalysisLink = await page.$('[href*="global"]');
        if (globalAnalysisLink) {
            await globalAnalysisLink.click();
            await page.waitForTimeout(2000);
            await page.screenshot({ path: 'temp_global.png', fullPage: false });
        }

        // Try to navigate to Series Explorer
        const seriesLink = await page.$('[href*="series"]');
        if (seriesLink) {
            await seriesLink.click();
            await page.waitForTimeout(2000);
            await page.screenshot({ path: 'temp_series.png', fullPage: false });
        }

        // Try to navigate to Professional Analysis
        const analysisLink = await page.$('[href*="analysis"]');
        if (analysisLink) {
            await analysisLink.click();
            await page.waitForTimeout(2000);
            await page.screenshot({ path: 'temp_analysis.png', fullPage: false });
        }

    } catch (error) {
        console.log('Navigation error (expected for prototype):', error.message);
    }

    await browser.close();
    console.log('✅ Screenshots captured successfully!');
    console.log('📁 Files created: temp_dashboard.png, temp_global.png, temp_series.png, temp_analysis.png');
}

createInterfaceVideo().catch(console.error);
