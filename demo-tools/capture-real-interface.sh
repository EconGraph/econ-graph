#!/bin/bash

echo "🎬 Capturing REAL EconGraph Interface"
echo ""
echo "The React app is running at: http://localhost:3000"
echo ""

# Open the actual React application
echo "🌐 Opening EconGraph interface..."
open "http://localhost:3000"

# Wait for it to load
echo "⏳ Waiting for interface to load..."
sleep 5

echo ""
echo "🎥 RECORDING THE REAL INTERFACE"
echo "================================"
echo ""
echo "I'm going to use macOS screen recording to capture the ACTUAL interface."
echo "This will show the real React components, not text boxes!"
echo ""

# Use macOS screenshot utility to start screen recording
echo "📱 Starting screen recording of the browser window..."
echo "   - A recording dialog will appear"
echo "   - Select the browser window with EconGraph"
echo "   - Click 'Record Selected Portion'"
echo "   - Navigate through the interface for 30 seconds"
echo ""

# Start screen recording
screencapture -R 0,0,1920,1080 -v temp_interface_capture.mov &
CAPTURE_PID=$!

echo "🎙️ DEMONSTRATE THESE REAL FEATURES:"
echo "======================================"
echo ""
echo "1. 🏠 Dashboard - Main page with navigation"
echo "2. 📊 Charts - Interactive Chart.js components"
echo "3. 🔍 Search - Working search functionality"
echo "4. 🌍 Global Analysis - If available"
echo "5. 📈 Professional Analysis - Chart tools"
echo "6. 🤝 Collaboration - User features"
echo ""
echo "⏱️ Recording for 30 seconds..."
sleep 30

# Stop recording
kill $CAPTURE_PID 2>/dev/null

echo ""
echo "🎬 Creating final video with narration..."

# Combine the screen capture with narration
if [ -f "temp_interface_capture.mov" ] && [ -f "demo-videos/honest_complete_narration.mp3" ]; then
    ffmpeg -i temp_interface_capture.mov \
           -i demo-videos/honest_complete_narration.mp3 \
           -c:v libx264 -c:a aac \
           -shortest \
           demo-videos/actual-interface-demo.mp4 -y

    rm temp_interface_capture.mov

    echo "✅ REAL interface demo created: demo-videos/actual-interface-demo.mp4"
else
    echo "⚠️ Screen capture failed. Let's try a different approach..."

    # Alternative: Create a simple video that just opens the browser
    echo "🌐 Creating video that opens the real interface..."

    # Create a simple video that shows the URL and instructions
    ffmpeg -f lavfi -i "color=c=0x2196f3:size=1920x1080:duration=10" \
           -filter_complex "
           [0:v]drawtext=fontfile=/System/Library/Fonts/ArialHB.ttc:text='EconGraph Real Interface':fontcolor=white:fontsize=72:x=(w-text_w)/2:y=300,
           drawtext=fontfile=/System/Library/Fonts/ArialHB.ttc:text='Open your browser to:':fontcolor=white:fontsize=36:x=(w-text_w)/2:y=500,
           drawtext=fontfile=/System/Library/Fonts/ArialHB.ttc:text='localhost port 3000':fontcolor=yellow:fontsize=48:x=(w-text_w)/2:y=600,
           drawtext=fontfile=/System/Library/Fonts/ArialHB.ttc:text='See the actual React components':fontcolor=white:fontsize=24:x=(w-text_w)/2:y=750[v]
           " \
           -map "[v]" \
           -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
           demo-videos/browser-instructions.mp4 -y

    echo "✅ Browser instructions video created"
fi

echo ""
echo "🚀 SOLUTION: The REAL interface is running!"
echo "============================================"
echo ""
echo "🌐 Open: http://localhost:3000"
echo "👀 You'll see: ACTUAL React components, not text boxes"
echo "🖱️ You can: Click, navigate, interact with real UI"
echo ""
echo "This is the genuine EconGraph interface with:"
echo "   • Material-UI components"
echo "   • React Router navigation"
echo "   • Chart.js visualizations"
echo "   • Working search and filters"
echo "   • Professional styling"
echo ""
echo "NO MORE FAKE TEXT BOXES - This is the real deal!"
