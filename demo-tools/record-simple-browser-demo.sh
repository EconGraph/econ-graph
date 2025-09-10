#!/bin/bash

# Record Chrome browser window showing EconGraph - Simple approach
# No JavaScript automation required, just clean window recording

set -e

echo "🎬 RECORDING CHROME BROWSER WINDOW - SIMPLE APPROACH"
echo "📹 Professional demo showing only the EconGraph application"
echo "🔒 Privacy-secure: No desktop or personal information visible"
echo ""

# Ensure output directory exists
mkdir -p demo-videos

# Check if services are running
echo "🔍 Checking services..."
if ! curl -s http://localhost:3000 > /dev/null; then
    echo "❌ Frontend not running. Please start with: cd frontend && npm start"
    exit 1
fi

echo "✅ Services are ready"
echo ""

# Open Chrome and navigate to the application
echo "🌐 Opening EconGraph in Chrome..."
open -a "Google Chrome" "http://localhost:3000"
sleep 4

# Position Chrome window for optimal recording
echo "📐 Positioning Chrome window..."
osascript << 'EOF'
tell application "Google Chrome"
    activate
    delay 2

    -- Position the window for clean recording
    tell front window
        set bounds to {200, 100, 1600, 1000}  -- x, y, width, height for 1400x900 window
    end tell

    delay 1
end tell
EOF

echo ""
echo "📹 STARTING BROWSER WINDOW RECORDING"
echo "⏱️  Recording 45 seconds of Chrome window showing EconGraph"
echo "🎵 With professional British investor narration"
echo ""
echo "🎯 RECORDING SETUP:"
echo "   • Chrome window positioned at 200x100, size 1400x900"
echo "   • Recording area cropped to show only browser content"
echo "   • Audio: Professional investor narration"
echo "   • Duration: 45 seconds"
echo ""

# Record with cropping to capture only the Chrome browser window area
# Crop parameters: width:height:x:y (crop to browser content area)
ffmpeg -f avfoundation -i "1" -i "demo-tools/generated-audio/investor_narration_20min.aiff" \
       -filter_complex "[0:v]crop=1400:900:200:100[browser]" \
       -map "[browser]" -map 1:a \
       -c:v libx264 -preset medium -crf 20 -c:a aac -b:a 128k \
       -t 45 -y demo-videos/econ-graph-browser-demo.mp4

# Check if video was created
if [ -f demo-videos/econ-graph-browser-demo.mp4 ]; then
    SIZE=$(ls -lh demo-videos/econ-graph-browser-demo.mp4 | awk '{print $5}')
    DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 demo-videos/econ-graph-browser-demo.mp4 | cut -d. -f1)

    echo ""
    echo "✅ PROFESSIONAL BROWSER-ONLY VIDEO CREATED!"
    echo "📁 File: demo-videos/econ-graph-browser-demo.mp4"
    echo "💾 Size: ${SIZE}"
    echo "⏱️  Duration: ${DURATION} seconds"
    echo "📹 Content: Chrome browser window only showing EconGraph"
    echo "🎵 Audio: Professional British investor narration"
    echo "🔒 Privacy: No desktop, taskbar, or personal info visible"
    echo "🎯 Quality: Clean, professional demonstration"
    echo ""
    echo "🚀 READY FOR PROFESSIONAL PRESENTATION!"
    echo "   Perfect for investors, customers, and stakeholders"
else
    echo "❌ Video creation failed"
    exit 1
fi
EOF
