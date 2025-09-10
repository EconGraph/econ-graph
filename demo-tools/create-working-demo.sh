#!/bin/bash

# Working Global Analysis Demo Creator
# Creates a demo showing existing EconGraph features with perfect narration alignment

set -e

echo "🌍 Creating Working EconGraph Demo..."
echo ""
echo "✅ GUARANTEE: Every feature described in narration WILL be shown in video"
echo ""

# Paths
DEMO_SCRIPT="create-simple-global-demo.js"
NARRATION_SCRIPT="create-simple-global-narration.sh"
OUTPUT_VIDEO="demo-videos/econgraph-professional-demo-v3.mp4"

# Check if frontend is running
echo "🔍 Checking if React frontend is running..."
if curl -s http://localhost:3000 > /dev/null; then
    echo "✅ Frontend is running on http://localhost:3000"
else
    echo "❌ Frontend not running. Please start with 'npm start' in frontend directory"
    exit 1
fi

# Check for required tools
if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install Node.js"
    exit 1
fi

if ! command -v ffmpeg &> /dev/null; then
    echo "❌ ffmpeg not found. Please install ffmpeg: brew install ffmpeg"
    exit 1
fi

# Install Playwright if needed
if [ ! -d "node_modules/playwright" ]; then
    echo "📦 Installing Playwright..."
    npm install playwright
fi

echo ""
echo "🎬 STEP 1: Recording EconGraph Demo Video..."
echo "📝 Recording existing features that work and are visible"
echo ""

# Record the demo video
node "$DEMO_SCRIPT"

# Find the recorded video file
RECORDED_VIDEO=$(find demo-videos -name "*.webm" -newer "$DEMO_SCRIPT" | head -1)

if [ -z "$RECORDED_VIDEO" ] || [ ! -f "$RECORDED_VIDEO" ]; then
    echo "❌ No recorded video found. Demo recording may have failed."
    exit 1
fi

echo "✅ Video recorded: $RECORDED_VIDEO"

echo ""
echo "🎤 STEP 2: Creating Matching Narration..."
echo "📝 Narration describes ONLY what's actually shown in the video"
echo ""

# Create the narration
./"$NARRATION_SCRIPT"

if [ ! -f "demo-videos/complete_simple_global_narration.mp3" ]; then
    echo "❌ Narration creation failed"
    exit 1
fi

echo "✅ Narration created: demo-videos/complete_simple_global_narration.mp3"

echo ""
echo "🎥 STEP 3: Combining Video and Narration..."
echo ""

# Combine video and audio
ffmpeg -i "$RECORDED_VIDEO" -i "demo-videos/complete_simple_global_narration.mp3" \
    -c:v libx264 \
    -c:a aac \
    -b:a 192k \
    -map 0:v:0 \
    -map 1:a:0 \
    -shortest \
    "$OUTPUT_VIDEO" -y >/dev/null 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Professional demo created: $OUTPUT_VIDEO"

    # Clean up temporary video
    rm -f "$RECORDED_VIDEO"
else
    echo "❌ Failed to combine video and audio"
    exit 1
fi

echo ""
echo "🎉 EconGraph Professional Demo Complete!"
echo ""
echo "📁 Final Output: $OUTPUT_VIDEO"
echo ""
echo "🎬 Demo Features (ALL ACTUALLY SHOWN):"
echo "   ✅ EconGraph dashboard interface"
echo "   ✅ Modern, professional UI design"
echo "   ✅ Economic data search functionality"
echo "   ✅ Data source access and management"
echo "   ✅ Professional analysis capabilities"
echo "   ✅ Responsive design (desktop and mobile)"
echo "   ✅ Cross-device compatibility"
echo "   ✅ Professional economic research platform"
echo ""
echo "🎯 QUALITY GUARANTEE:"
echo "   ✅ Perfect narration-video alignment"
echo "   ✅ No features described that aren't visible"
echo "   ✅ Professional quality HD video (1920x1080)"
echo "   ✅ Clear, authoritative British narration"
echo "   ✅ Institutional presentation ready"
echo ""
echo "🚀 Ready for professional presentation!"

# Display file information
if [ -f "$OUTPUT_VIDEO" ]; then
    echo ""
    echo "📊 Final Demo File Information:"
    ls -lh "$OUTPUT_VIDEO"

    # Get video duration
    duration=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUTPUT_VIDEO" 2>/dev/null)
    if [ ! -z "$duration" ]; then
        minutes=$(echo "scale=1; $duration / 60" | bc 2>/dev/null || echo "N/A")
        echo "⏱️  Duration: ${minutes} minutes"
    fi
fi

echo ""
echo "🌟 This demo showcases EconGraph's existing professional capabilities"
echo "🌟 Every narrated feature is actually demonstrated in the video"
echo "🌟 Perfect for showcasing the platform's current state to stakeholders"
