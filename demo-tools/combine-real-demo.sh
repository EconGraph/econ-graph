#!/bin/bash

# Combine Real Demo Recording with Ultra-Comprehensive Narration
echo "🎬 Combining Real Demo Recording with Ultra-Comprehensive Narration..."

# Check if the real demo recording exists
if [ ! -f "real-demo-recording.mov" ]; then
    echo "❌ Error: real-demo-recording.mov not found!"
    echo "Please record the demo first using: ./create-real-working-demo.sh"
    exit 1
fi

# Video and audio file paths
VIDEO_FILE="real-demo-recording.mov"
AUDIO_FILE="demo-videos/ultra_comprehensive_complete_narration.mp3"
OUTPUT_FILE="demo-videos/ultra-comprehensive-global-analysis-demo-with-narration.mp4"

echo "📹 Video file: $VIDEO_FILE"
echo "🎵 Audio file: $AUDIO_FILE"
echo "🎯 Output file: $OUTPUT_FILE"

# Get video and audio durations
echo "⏱️ Analyzing file durations..."
VIDEO_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$VIDEO_FILE")
AUDIO_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$AUDIO_FILE")

echo "📺 Video duration: ${VIDEO_DURATION}s"
echo "🎤 Audio duration: ${AUDIO_DURATION}s"

# Convert to integers for comparison
VIDEO_INT=$(printf "%.0f" "$VIDEO_DURATION")
AUDIO_INT=$(printf "%.0f" "$AUDIO_DURATION")

echo "🔄 Processing real demo with ultra-comprehensive narration..."

# Audio is longer, so extend video with fade to black
echo "🎵 Audio is longer than video - extending video with fade to black"
ffmpeg -i "$VIDEO_FILE" -i "$AUDIO_FILE" \
    -filter_complex "
    [0:v]scale=1920:1080[scaled];
    [scaled]fade=t=out:st=$((VIDEO_INT-3)):d=3[faded];
    [faded]tpad=stop_mode=clone:stop_duration=$((AUDIO_INT-VIDEO_INT))[extended];
    [extended]fade=t=in:st=$((VIDEO_INT)):d=2[final_video]
    " \
    -map "[final_video]" -map 1:a \
    -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
    -c:a aac -b:a 192k \
    -movflags +faststart \
    "$OUTPUT_FILE" -y

if [ $? -eq 0 ]; then
    echo "✅ Real Working Demo Video Created Successfully!"

    # Get final file size and duration
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)
    FINAL_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUTPUT_FILE")
    FINAL_DURATION_MIN=$(echo "scale=1; $FINAL_DURATION / 60" | bc -l)

    echo "📁 Output file: $OUTPUT_FILE"
    echo "💾 File size: $FINAL_SIZE"
    echo "⏱️ Duration: ${FINAL_DURATION_MIN} minutes"
    echo "🎬 Resolution: 1920x1080 HD"
    echo "🎤 Audio: 192kbps AAC with ultra-comprehensive narration"
    echo "📊 Video: H.264 with REAL working demo content"

    echo ""
    echo "🏆 REAL WORKING DEMO COMPLETE!"
    echo "🎯 This video now has ACTUAL EconGraph interface content!"
    echo "📺 Ready to upload to GitHub and showcase real features!"

else
    echo "❌ Error creating real working demo video"
    exit 1
fi
