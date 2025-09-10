#!/bin/bash

echo "🎬 Creating Automated UI Demo (Real Interface, No Cursor)"
echo ""

NARRATION_FILE="demo-videos/ultra_comprehensive_complete_narration.mp3"
DEMO_HTML="demo-videos/ultra-comprehensive-global-analysis-demo.html"
OUTPUT_FILE="demo-videos/real-ui-demo.mp4"

if [ ! -f "$NARRATION_FILE" ]; then
    NARRATION_FILE="demo-videos/comprehensive_complete_narration.mp3"
fi

if [ ! -f "$DEMO_HTML" ]; then
    DEMO_HTML="demo-videos/comprehensive-global-analysis-demo.html"
fi

echo "📊 Getting narration duration..."
DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$NARRATION_FILE")
DURATION_INT=$(echo "$DURATION" | cut -d. -f1)
MINUTES=$(echo "scale=1; $DURATION / 60" | bc)
echo "🎵 Duration: ${MINUTES} minutes (${DURATION} seconds)"

echo ""
echo "🎬 Creating video from actual interface screenshots..."

# Create Node.js script to take screenshots of the real interface
cat > temp_screenshot_generator.js << 'EOF'
const puppeteer = require('puppeteer');
const fs = require('fs');
const path = require('path');

async function generateScreenshots() {
    console.log('🚀 Launching browser for screenshot generation...');

    const browser = await puppeteer.launch({
        headless: true,
        defaultViewport: null,
        args: [
            '--no-default-browser-check',
            '--disable-extensions',
            '--hide-scrollbars',
            '--disable-web-security',
            '--allow-file-access-from-files'
        ]
    });

    const page = await browser.newPage();

    await page.setViewport({
        width: 1920,
        height: 1080,
        deviceScaleFactor: 1
    });

    const demoPath = path.resolve(process.argv[2]);
    const fileUrl = `file://${demoPath}`;

    console.log(`📄 Loading interface: ${fileUrl}`);
    await page.goto(fileUrl, { waitUntil: 'networkidle0' });

    // Hide cursor completely
    await page.addStyleTag({
        content: `
            *, *:hover, *:active, *:focus {
                cursor: none !important;
            }
        `
    });

    const duration = parseInt(process.argv[3] || 60);
    const screenshotsDir = 'temp_screenshots';

    if (!fs.existsSync(screenshotsDir)) {
        fs.mkdirSync(screenshotsDir);
    }

    console.log(`📸 Taking screenshots for ${duration} seconds...`);

    // Take screenshots at 2 FPS (every 0.5 seconds)
    const totalScreenshots = duration * 2;

    for (let i = 0; i < totalScreenshots; i++) {
        // Simulate some interactions to show the interface is dynamic
        if (i % 10 === 0) {
            await page.evaluate((frameNum) => {
                // Update some data values to show dynamism
                const dataElements = document.querySelectorAll('.data-value, .metric-value');
                dataElements.forEach((el, index) => {
                    if (el.textContent.includes('$') && index % 3 === frameNum % 3) {
                        const currentValue = parseFloat(el.textContent.replace(/[^0-9.-]/g, ''));
                        if (!isNaN(currentValue)) {
                            const variation = 0.98 + Math.random() * 0.04;
                            const newValue = (currentValue * variation).toFixed(2);
                            el.textContent = el.textContent.replace(/[\d.-]+/, newValue);
                        }
                    }
                });

                // Highlight different sections
                const sections = document.querySelectorAll('.country, .chart, .dashboard-panel, .map-container');
                sections.forEach((section, index) => {
                    if (index === (frameNum % sections.length)) {
                        section.style.boxShadow = '0 0 10px rgba(76, 175, 80, 0.5)';
                        section.style.transform = 'scale(1.02)';
                    } else {
                        section.style.boxShadow = '';
                        section.style.transform = '';
                    }
                });
            }, i);
        }

        const filename = `${screenshotsDir}/frame_${String(i).padStart(6, '0')}.png`;
        await page.screenshot({
            path: filename,
            fullPage: false
        });

        if (i % 20 === 0) {
            console.log(`📸 Progress: ${Math.round((i / totalScreenshots) * 100)}%`);
        }

        await page.waitForTimeout(500); // 0.5 second intervals
    }

    console.log('✅ Screenshots completed');
    await browser.close();

    return screenshotsDir;
}

generateScreenshots().then(screenshotsDir => {
    console.log(`📁 Screenshots saved to: ${screenshotsDir}`);
}).catch(console.error);
EOF

# Install puppeteer if needed
if [ ! -d "node_modules/puppeteer" ]; then
    echo "📦 Installing puppeteer..."
    npm install puppeteer
fi

echo "📸 Generating screenshots from real interface..."
node temp_screenshot_generator.js "$DEMO_HTML" "$DURATION_INT"

if [ $? -eq 0 ] && [ -d "temp_screenshots" ]; then
    echo "🎬 Creating video from real interface screenshots..."

    # Create video from screenshots with narration
    ffmpeg -framerate 2 -i temp_screenshots/frame_%06d.png -i "$NARRATION_FILE" \
        -c:v libx264 -preset medium -crf 18 -pix_fmt yuv420p \
        -c:a aac -b:a 256k \
        -movflags +faststart \
        -t "$DURATION" \
        "$OUTPUT_FILE" -y

    # Clean up
    rm -rf temp_screenshots
    rm -f temp_screenshot_generator.js

    if [ $? -eq 0 ]; then
        FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
        echo ""
        echo "🎉 REAL UI DEMO COMPLETED SUCCESSFULLY!"
        echo "📁 Output: $OUTPUT_FILE"
        echo "📊 File size: ${FINAL_SIZE}"
        echo "🎵 Duration: ${MINUTES} minutes"
        echo ""
        echo "✅ This video shows the ACTUAL interface UI (no cursor)"
        echo "✅ Real interface elements and interactions"
        echo "✅ Dynamic data updates and visual feedback"
        echo "✅ Professional business narration"
        echo ""
        echo "🎯 The video now shows the REAL UI, not just text overlays!"
    else
        echo "❌ Error creating video from screenshots"
        exit 1
    fi
else
    echo "❌ Error generating screenshots"
    exit 1
fi
