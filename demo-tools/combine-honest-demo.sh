#!/bin/bash

echo "🎬 Combining Honest Demo Video with Narration"
echo ""

# File paths
VIDEO_FILE="honest-demo-recording.mov"
AUDIO_FILE="demo-videos/honest_complete_narration.mp3"
OUTPUT_FILE="demo-videos/honest-econ-graph-demo-with-narration.mp4"

# Check if files exist
if [ ! -f "$VIDEO_FILE" ]; then
    echo "❌ Error: Video file not found: $VIDEO_FILE"
    echo "Please record the demo first using ./create-honest-demo-video.sh"
    exit 1
fi

if [ ! -f "$AUDIO_FILE" ]; then
    echo "❌ Error: Audio file not found: $AUDIO_FILE"
    exit 1
fi

echo "📊 Analyzing video and audio durations..."

# Get video duration in seconds
VIDEO_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$VIDEO_FILE")
VIDEO_INT=$(echo "$VIDEO_DURATION" | cut -d. -f1)

# Get audio duration in seconds
AUDIO_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$AUDIO_FILE")
AUDIO_INT=$(echo "$AUDIO_DURATION" | cut -d. -f1)

echo "🎥 Video duration: ${VIDEO_DURATION} seconds"
echo "🎵 Audio duration: ${AUDIO_DURATION} seconds"

# Combine video and audio
echo ""
echo "🔗 Combining video with honest narration..."

if [ "$AUDIO_INT" -gt "$VIDEO_INT" ]; then
    echo "🎵 Audio is longer than video - extending video with fade to black"
    # Audio is longer, so extend video with black frames and fade, replace audio
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

elif [ "$VIDEO_INT" -gt "$AUDIO_INT" ]; then
    echo "🎥 Video is longer than audio - trimming video to match audio"
    # Video is longer, so trim video to match audio duration
    ffmpeg -i "$VIDEO_FILE" -i "$AUDIO_FILE" \
        -filter_complex "
        [0:v]scale=1920:1080[scaled];
        [scaled]trim=duration=${AUDIO_DURATION}[trimmed];
        [trimmed]fade=t=out:st=$((AUDIO_INT-3)):d=3[final_video]
        " \
        -map "[final_video]" -map 1:a \
        -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
        -c:a aac -b:a 192k \
        -movflags +faststart \
        "$OUTPUT_FILE" -y

else
    echo "⚖️ Video and audio durations match - simple combine"
    # Durations match, simple combination
    ffmpeg -i "$VIDEO_FILE" -i "$AUDIO_FILE" \
        -filter_complex "[0:v]scale=1920:1080[scaled]" \
        -map "[scaled]" -map 1:a \
        -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p \
        -c:a aac -b:a 192k \
        -movflags +faststart \
        "$OUTPUT_FILE" -y
fi

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Honest demo video created successfully!"
    echo "📁 Output: $OUTPUT_FILE"
    echo ""

    # Get final video info
    FINAL_DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUTPUT_FILE")
    FINAL_SIZE=$(du -h "$OUTPUT_FILE" | cut -f1)

    echo "📊 Final video stats:"
    echo "   Duration: ${FINAL_DURATION} seconds"
    echo "   File size: ${FINAL_SIZE}"
    echo ""
    echo "🎬 This video contains HONEST descriptions of implemented features:"
    echo "   ✅ 5 countries with sample data"
    echo "   ✅ 3 correlation relationships"
    echo "   ✅ 6 major economic events"
    echo "   ✅ Prototype UI demonstration"
    echo "   ✅ No false claims about non-existent features"
    echo ""
    echo "🚀 Ready to update documentation and commit!"

else
    echo "❌ Error creating video. Check the logs above."
    exit 1
fi
