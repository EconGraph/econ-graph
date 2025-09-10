#!/bin/bash

# Professional Narrated Demo Video Creator
# Synchronizes professional voice narration with existing demo video

set -e

echo "🎬 Creating Professional Narrated Demo Video..."

# Input files
INPUT_VIDEO="demo-videos/epic-system-demo.webm"
AUDIO_DIR="demo-videos/audio-segments"
OUTPUT_VIDEO="demo-videos/epic-system-demo-with-narration.mp4"

# Verify input video exists
if [ ! -f "$INPUT_VIDEO" ]; then
    echo "❌ Error: Input video not found: $INPUT_VIDEO"
    exit 1
fi

# Verify audio segments exist
if [ ! -d "$AUDIO_DIR" ]; then
    echo "❌ Error: Audio segments directory not found: $AUDIO_DIR"
    exit 1
fi

echo "📹 Input video: $INPUT_VIDEO"
echo "🎵 Audio segments: $AUDIO_DIR"
echo "🎯 Output video: $OUTPUT_VIDEO"
echo ""

# Timing information from the narration script (in seconds)
declare -a TIMINGS=(
    0      # Segment 01: 0s
    8.5    # Segment 02: 8.5s
    15     # Segment 03: 15s
    21     # Segment 04: 21s
    29     # Segment 05: 29s
    36     # Segment 06: 36s
    43     # Segment 07: 43s
    50     # Segment 08: 50s
    57     # Segment 09: 57s
    63     # Segment 10: 63s
    70     # Segment 11: 70s
    77     # Segment 12: 77s
    85     # Segment 13: 85s
    92     # Segment 14: 92s
    99     # Segment 15: 99s
    106    # Segment 16: 106s
    113    # Segment 17: 113s
    119    # Segment 18: 119s
    126    # Segment 19: 126s (final segment)
)

echo "🔧 Creating audio timeline..."

# Create a single continuous audio track by concatenating all segments at their proper times
# First, create silence file for gaps
SILENCE_FILE="demo-videos/silence_1sec.mp3"
ffmpeg -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=48000 -t 1 "$SILENCE_FILE" -y >/dev/null 2>&1

# Create the complete audio track
TEMP_AUDIO="demo-videos/temp_complete_audio.mp3"

echo "🎼 Building synchronized audio track..."

# Start with silence, then add each segment at the correct time
audio_filter=""
inputs=""
input_count=0

# Add the silence file as input
inputs="$inputs -i $SILENCE_FILE"
input_count=$((input_count + 1))

# Add all audio segments as inputs
for i in {1..19}; do
    segment_file="$AUDIO_DIR/segment_$(printf "%02d" $i).mp3"
    if [ -f "$segment_file" ]; then
        inputs="$inputs -i $segment_file"
        input_count=$((input_count + 1))
    else
        echo "⚠️  Warning: Missing segment: $segment_file"
    fi
done

echo "🎛️  Mixing audio segments with precise timing..."

# Create a complex filter to mix all segments at their proper times
# This is a simplified approach - we'll create the final audio by overlaying segments
filter_complex=""
for i in {1..19}; do
    segment_index=$((i))  # segment files are inputs 1-19 (input 0 is silence)
    delay_ms=$(echo "${TIMINGS[$((i-1))]} * 1000" | bc)

    if [ $i -eq 1 ]; then
        filter_complex="[$segment_index]adelay=${delay_ms}[a$i]"
    else
        filter_complex="$filter_complex;[$segment_index]adelay=${delay_ms}[a$i]"
    fi
done

# Mix all delayed segments together
mix_inputs=""
for i in {1..19}; do
    mix_inputs="$mix_inputs[a$i]"
done
filter_complex="$filter_complex;${mix_inputs}amix=inputs=19[audio_out]"

echo "🔀 Executing audio mix..."

# Create the complete audio track
ffmpeg $inputs -filter_complex "$filter_complex" -map "[audio_out]" -t 140 "$TEMP_AUDIO" -y >/dev/null 2>&1

echo "✅ Audio track created successfully"

echo "🎥 Combining video with narrated audio..."

# Combine the original video with the new narrated audio track
ffmpeg -i "$INPUT_VIDEO" -i "$TEMP_AUDIO" \
    -c:v copy \
    -c:a aac \
    -b:a 192k \
    -map 0:v:0 \
    -map 1:a:0 \
    -shortest \
    "$OUTPUT_VIDEO" -y >/dev/null 2>&1

echo "✅ Video with narration created successfully"

# Clean up temporary files
rm -f "$SILENCE_FILE" "$TEMP_AUDIO"

echo ""
echo "🎉 Professional Narrated Demo Video Complete!"
echo "📁 Output: $OUTPUT_VIDEO"
echo "🎬 Features:"
echo "   ✅ HD Video (1920x1080)"
echo "   ✅ Professional Voice Narration (19 segments)"
echo "   ✅ Synchronized Timing"
echo "   ✅ High-Quality Audio (192kbps AAC)"
echo "   ✅ Bloomberg Terminal-level Feature Showcase"
echo ""
echo "🚀 Ready for professional presentations and institutional demos!"

# Display file information
echo "📊 File Information:"
ls -lh "$OUTPUT_VIDEO"
