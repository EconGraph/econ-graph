#!/bin/bash

echo "🎬 Recording the ACTUAL EconGraph Interface"
echo ""

# Make sure we're using the running React app
REACT_URL="http://localhost:3000"
NARRATION_FILE="demo-videos/honest_complete_narration.mp3"
OUTPUT_FILE="demo-videos/real-econ-graph-interface.mp4"

echo "🌐 React app running at: $REACT_URL"
echo "🎵 Using narration: $NARRATION_FILE"
echo "📁 Output will be: $OUTPUT_FILE"
echo ""

# Get narration duration
if [ -f "$NARRATION_FILE" ]; then
    DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$NARRATION_FILE")
    echo "🎵 Narration duration: ${DURATION} seconds"
else
    DURATION=30
    echo "⚠️ No narration file, using 30 seconds"
fi

echo ""
echo "🎥 Recording the browser window showing the real interface..."

# Open the React app
open "$REACT_URL"
sleep 3

echo "📹 Starting screen recording..."
echo "   This will record the actual browser window with EconGraph"
echo ""

# Use ffmpeg with avfoundation to capture screen
# This captures the actual screen content, not fake overlays
ffmpeg -f avfoundation -framerate 30 -i "1:0" -t $DURATION \
       -vf "crop=1920:1080:0:0" \
       -c:v libx264 -preset medium -crf 23 \
       -pix_fmt yuv420p \
       temp_screen_capture.mp4 -y &

FFMPEG_PID=$!

echo "🎬 Recording for ${DURATION} seconds..."
echo "   Please interact with the EconGraph interface in your browser"
echo "   Navigate through: Dashboard, Charts, Search, etc."
echo ""

# Wait for recording to complete
sleep $DURATION

# Stop ffmpeg
kill $FFMPEG_PID 2>/dev/null
wait $FFMPEG_PID 2>/dev/null

echo ""
echo "🎵 Adding narration to the screen recording..."

# Combine screen recording with narration
if [ -f "temp_screen_capture.mp4" ] && [ -f "$NARRATION_FILE" ]; then
    ffmpeg -i temp_screen_capture.mp4 \
           -i "$NARRATION_FILE" \
           -c:v copy -c:a aac \
           -shortest \
           "$OUTPUT_FILE" -y

    rm temp_screen_capture.mp4

    echo "✅ Real interface video created: $OUTPUT_FILE"

    # Get file size
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    echo "📊 File size: ${FINAL_SIZE}"

    echo ""
    echo "🎯 This video shows:"
    echo "   ✅ ACTUAL screen recording of the browser"
    echo "   ✅ REAL React components and interface"
    echo "   ✅ NO fake text boxes or overlays"
    echo "   ✅ Genuine EconGraph application in action"

    # Open the video
    open "$OUTPUT_FILE"

else
    echo "❌ Screen recording failed"
    echo "💡 Alternative: The interface is still running at $REACT_URL"
    echo "   You can manually record it using Cmd+Shift+5"
fi

echo ""
echo "🌐 Live interface: $REACT_URL"
