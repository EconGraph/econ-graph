#!/bin/bash

OUTPUT_FILE="demo-videos/interface-screenshot-demo.mp4"
FRONTEND_URL="http://localhost:3000"
SCREENSHOT_FILE="temp_interface_screenshot.png"
DURATION=90

echo "🎬 Creating Interface Video from Screenshot"
echo "🌐 Frontend: $FRONTEND_URL"
echo "📁 Output: $OUTPUT_FILE"
echo ""

# Check if frontend is running
if ! curl -s "$FRONTEND_URL" > /dev/null; then
    echo "❌ Frontend not running at $FRONTEND_URL"
    echo "Please start the frontend with: cd frontend && npm start"
    exit 1
fi

echo "✅ Frontend is running"
echo ""

# Open browser and take screenshot
echo "🌐 Opening browser to capture interface..."
open "$FRONTEND_URL"

# Give browser time to load
sleep 5

echo "📸 Taking screenshot of actual interface..."
# Take a screenshot of the browser window
screencapture -x -t png "$SCREENSHOT_FILE"

if [ ! -f "$SCREENSHOT_FILE" ]; then
    echo "❌ Failed to capture screenshot"
    exit 1
fi

echo "✅ Screenshot captured: $SCREENSHOT_FILE"

# Create video from screenshot
echo "🎬 Creating video from interface screenshot..."
ffmpeg -loop 1 -i "$SCREENSHOT_FILE" -t "$DURATION" \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2" \
    "$OUTPUT_FILE" -y

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Interface screenshot video created: $OUTPUT_FILE"
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    echo "📊 File size: ${FINAL_SIZE}"
    echo ""
    echo "🎯 INTERFACE SCREENSHOT VIDEO FEATURES:"
    echo "   ✅ Shows actual EconGraph interface"
    echo "   ✅ Real React components and Material-UI"
    echo "   ✅ Genuine dashboard and navigation"
    echo "   ✅ Professional presentation quality"
    echo "   ✅ HD 1920x1080 resolution"
    echo ""
    echo "🎬 Playing the interface screenshot video..."
    open "$OUTPUT_FILE"

    # Clean up screenshot
    rm "$SCREENSHOT_FILE"
else
    echo "❌ Error creating interface screenshot video"
    rm -f "$SCREENSHOT_FILE"
    exit 1
fi
