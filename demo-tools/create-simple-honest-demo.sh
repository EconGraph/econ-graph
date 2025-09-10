#!/bin/bash

echo "🎬 Creating Simple Honest Demo Video"
echo ""

NARRATION_FILE="demo-videos/honest_complete_narration.mp3"
OUTPUT_FILE="demo-videos/honest-econ-graph-demo-with-narration.mp4"

if [ ! -f "$NARRATION_FILE" ]; then
    echo "❌ Error: Narration file not found: $NARRATION_FILE"
    exit 1
fi

echo "📊 Getting narration duration..."
DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$NARRATION_FILE")
echo "🎵 Narration duration: ${DURATION} seconds"

echo ""
echo "🎨 Creating honest demo video with simple text overlay..."

# Create a video that shows simple text describing exactly what's implemented
# Using default font that should be available on macOS
ffmpeg -f lavfi -i "color=c=0x1e3c72:size=1920x1080:duration=${DURATION}" \
    -i "$NARRATION_FILE" \
    -filter_complex "
    [0:v]drawtext=text='EconGraph - Honest Prototype Demo':fontcolor=white:fontsize=72:x=(w-text_w)/2:y=200,
    drawtext=text='PROTOTYPE DEMONSTRATION':fontcolor=orange:fontsize=36:x=(w-text_w)/2:y=300,
    drawtext=text='✅ 5 Sample Countries (US, China, Japan, Germany, UK)':fontcolor=white:fontsize=32:x=100:y=450,
    drawtext=text='✅ 3 Sample Correlation Relationships':fontcolor=white:fontsize=32:x=100:y=500,
    drawtext=text='✅ 6 Sample Economic Events (2008 Crisis, COVID-19, etc.)':fontcolor=white:fontsize=32:x=100:y=550,
    drawtext=text='✅ Interactive UI Components with Sample Data':fontcolor=white:fontsize=32:x=100:y=600,
    drawtext=text='✅ Professional Styling Concepts':fontcolor=white:fontsize=32:x=100:y=650,
    drawtext=text='❌ No False Claims About Non-Existent Features':fontcolor=red:fontsize=32:x=100:y=750,
    drawtext=text='Sample Data Only - Prototype Status':fontcolor=yellow:fontsize=28:x=(w-text_w)/2:y=850[v]
    " \
    -map "[v]" -map 1:a \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    -c:a aac -b:a 192k \
    -movflags +faststart \
    "$OUTPUT_FILE" -y

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Simple honest demo created successfully!"
    echo "📁 Output: $OUTPUT_FILE"

    # Get final stats
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    echo "📊 File size: ${FINAL_SIZE}"
    echo "🎵 Duration: ${DURATION} seconds"
    echo ""
    echo "🎯 This video shows EXACTLY what the narration describes:"
    echo "   ✅ Clear text listing actual implemented features"
    echo "   ✅ No misleading visuals or false demonstrations"
    echo "   ✅ Perfect sync between audio and visual content"
    echo "   ✅ Honest prototype representation"
    echo ""
    echo "🚀 Ready to update README and commit!"
else
    echo "❌ Error creating video"
    exit 1
fi
