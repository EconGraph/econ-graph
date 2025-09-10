#!/bin/bash

NARRATION_FILE="demo-videos/professional_business_impact_narration.mp3"
OUTPUT_FILE="demo-videos/professional-business-impact-demo.mp4"
TEMP_SCREEN_CAPTURE="temp_professional_screen_capture.mp4"
FRONTEND_URL="http://localhost:3000"

echo "🏢 Creating PROFESSIONAL BUSINESS IMPACT VIDEO"
echo "💰 Bloomberg Terminal ($24k) vs EconGraph (FREE) - VIDEO DEMO"
echo ""
echo "🌐 React app running at: $FRONTEND_URL"
echo "🎵 Using narration: $NARRATION_FILE"
echo "📁 Output will be: $OUTPUT_FILE"
echo ""

if [ ! -f "$NARRATION_FILE" ]; then
    echo "❌ Error: Professional narration file not found: $NARRATION_FILE"
    exit 1
fi

echo "🎵 Professional narration duration..."
DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$NARRATION_FILE")
echo "🎵 Narration duration: ${DURATION} seconds"
echo ""

echo "🏢 PROFESSIONAL VIDEO DEMONSTRATION PLAN:"
echo "   💼 0-8s: Dashboard - Bloomberg Terminal-level interface"
echo "   🌍 8-25s: Global Analysis - Network Map (Premium feature)"
echo "   📊 25-45s: Multi-Country Dashboard - Institutional analysis"
echo "   📅 45-55s: Global Events Explorer - Crisis tracking"
echo "   💰 55-90s: Professional features showcasing business value"
echo ""

# Open the browser to the frontend URL
open "$FRONTEND_URL"

# Give the browser some time to load
sleep 5

echo "🎬 PROFESSIONAL VIDEO RECORDING INSTRUCTIONS:"
echo ""
echo "⏰ 0-8 seconds: Dashboard (Bloomberg Terminal Comparison)"
echo "   - Show professional dashboard with economic indicators"
echo "   - Highlight Material-UI professional interface"
echo "   - Demonstrate institutional-grade presentation"
echo ""
echo "⏰ 8-25 seconds: Global Economic Network Analysis"
echo "   - Click hamburger menu → 'Global Analysis'"
echo "   - Show the Network Map tab (should be default)"
echo "   - Demonstrate D3.js world map visualization"
echo "   - Show premium terminal-level capabilities"
echo ""
echo "⏰ 25-45 seconds: Multi-Country Dashboard"
echo "   - Click 'Multi-Country Dashboard' tab"
echo "   - Show Bloomberg Terminal-style comparative analysis"
echo "   - Demonstrate institutional-grade interface"
echo ""
echo "⏰ 45-55 seconds: Global Events Explorer"
echo "   - Click 'Global Events' tab"
echo "   - Show 2008 crisis, COVID-19, Brexit tracking"
echo "   - Demonstrate S&P CapIQ-level functionality"
echo ""
echo "⏰ 55-90 seconds: Professional Value Focus"
echo "   - Navigate through features showing business value"
echo "   - Emphasize professional presentation quality"
echo "   - Show responsive design and capabilities"
echo ""
echo "💰 NARRATION HIGHLIGHTS COST SAVINGS:"
echo "   - Bloomberg Terminal: $24,000/year"
echo "   - Thomson Reuters: $22,000/year"
echo "   - S&P CapIQ: $12,000/year"
echo "   - EconGraph: FREE with same capabilities"
echo ""
echo "Press ENTER when ready to start PROFESSIONAL VIDEO recording..."
read

# Record the screen for the duration of the narration
echo "🎥 Recording professional business impact demonstration..."
ffmpeg -f avfoundation -i "1:0" -t "$DURATION" -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p "$TEMP_SCREEN_CAPTURE" -y

if [ $? -ne 0 ]; then
    echo "❌ Error during professional video recording."
    exit 1
fi

echo ""
echo "🎵 Combining screen recording with professional narration..."
ffmpeg -i "$TEMP_SCREEN_CAPTURE" -i "$NARRATION_FILE" \
    -c:v copy -c:a aac -b:a 192k -map 0:v:0 -map 1:a:0 \
    -shortest "$OUTPUT_FILE" -y

if [ $? -eq 0 ]; then
    echo "✅ Professional business impact VIDEO created: $OUTPUT_FILE"
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    echo "📊 File size: ${FINAL_SIZE}"
    echo ""
    echo "🏢 PROFESSIONAL BUSINESS IMPACT VIDEO FEATURES:"
    echo "   ✅ ACTUAL screen recording with professional narration"
    echo "   ✅ Bloomberg Terminal cost comparison ($24k vs FREE)"
    echo "   ✅ Shows REAL Global Analysis features mentioned"
    echo "   ✅ Network Map, Multi-Country Dashboard, Events Explorer"
    echo "   ✅ Professional Material-UI interface demonstration"
    echo "   ✅ Institutional-grade business value positioning"
    echo ""
    echo "💰 BUSINESS IMPACT DEMONSTRATED:"
    echo "   📈 Hundreds of thousands in annual savings"
    echo "   🏢 Enterprise capabilities at zero cost"
    echo "   🚀 Open-source competitive advantage"
    echo "   📊 Professional presentation quality"
    echo ""
    echo "🌐 Live interface: $FRONTEND_URL"
    echo "📋 Features shown: Dashboard → Global Analysis → Professional Tools"
else
    echo "❌ Error combining screen recording with narration."
    exit 1
fi

# Clean up temporary file
rm "$TEMP_SCREEN_CAPTURE"

echo ""
echo "🎬 Opening the PROFESSIONAL BUSINESS IMPACT VIDEO..."
open "$OUTPUT_FILE"
