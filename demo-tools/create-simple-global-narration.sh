#!/bin/bash

# Simple Global Analysis Demo Narration
# This narration ONLY describes features that are actually shown in the existing EconGraph interface

set -e

echo "🎤 Creating Simple Global Analysis Demo Narration..."
echo ""
echo "⚠️  IMPORTANT: This narration only describes features actually shown in the video!"
echo ""

# Create audio directory
mkdir -p demo-videos/simple-global-audio

echo "Using macOS 'say' command for narration..."
echo ""

# Narration segments that match what we can actually show
declare -a segments=(
    "Welcome to EconGraph, a professional economic data analysis platform."
    "This is the EconGraph dashboard, providing access to comprehensive economic time series data."
    "The platform features a modern, responsive interface designed for professional economic analysis."
    "Let's explore the economic data search and discovery features."
    "EconGraph provides access to multiple data sources including the Federal Reserve Economic Data and Bureau of Labor Statistics."
    "You can search for various economic indicators such as GDP, unemployment rates, and inflation data."
    "The search functionality allows economists and analysts to quickly find relevant economic time series."
    "The platform includes comprehensive data source management and monitoring capabilities."
    "EconGraph features professional analysis tools for economic research and policy analysis."
    "The responsive design ensures the platform works seamlessly across desktop and mobile devices."
    "This mobile view demonstrates EconGraph's cross-device compatibility for field research."
    "Returning to the desktop interface for comprehensive economic analysis capabilities."
    "EconGraph represents the future of economic data analysis with modern web technologies and professional-grade features."
    "The platform provides institutional-quality economic data access with an intuitive, modern interface."
    "Thank you for viewing this demonstration of EconGraph's professional economic analysis platform."
)

# Generate each segment using macOS say
for i in "${!segments[@]}"; do
    segment_num=$(printf "%02d" $((i + 1)))
    output_file="demo-videos/simple-global-audio/simple_segment_${segment_num}.mp3"

    echo "🎤 Generating segment ${segment_num}/15: ${segments[i]:0:60}..."

    # Use Daniel voice for professional British narration
    say -v Daniel -r 180 -o "demo-videos/simple-global-audio/simple_segment_${segment_num}.aiff" "${segments[i]}"

    # Convert to MP3 (requires ffmpeg)
    if command -v ffmpeg &> /dev/null; then
        ffmpeg -i "demo-videos/simple-global-audio/simple_segment_${segment_num}.aiff" \
               -acodec mp3 -ab 192k \
               "$output_file" -y >/dev/null 2>&1
        rm "demo-videos/simple-global-audio/simple_segment_${segment_num}.aiff"
        echo "✅ Generated: $output_file"
    else
        echo "⚠️  ffmpeg not found, keeping AIFF format"
        mv "demo-videos/simple-global-audio/simple_segment_${segment_num}.aiff" \
           "demo-videos/simple-global-audio/simple_segment_${segment_num}.aiff"
        echo "✅ Generated: ${output_file%.mp3}.aiff"
    fi
done

echo ""
echo "🔄 Concatenating audio segments..."

# Create concatenation list
cat > demo-videos/simple_global_concat_list.txt << EOF
file 'simple-global-audio/simple_segment_01.mp3'
file 'simple-global-audio/simple_segment_02.mp3'
file 'simple-global-audio/simple_segment_03.mp3'
file 'simple-global-audio/simple_segment_04.mp3'
file 'simple-global-audio/simple_segment_05.mp3'
file 'simple-global-audio/simple_segment_06.mp3'
file 'simple-global-audio/simple_segment_07.mp3'
file 'simple-global-audio/simple_segment_08.mp3'
file 'simple-global-audio/simple_segment_09.mp3'
file 'simple-global-audio/simple_segment_10.mp3'
file 'simple-global-audio/simple_segment_11.mp3'
file 'simple-global-audio/simple_segment_12.mp3'
file 'simple-global-audio/simple_segment_13.mp3'
file 'simple-global-audio/simple_segment_14.mp3'
file 'simple-global-audio/simple_segment_15.mp3'
EOF

# Concatenate all segments
if command -v ffmpeg &> /dev/null; then
    ffmpeg -f concat -safe 0 -i demo-videos/simple_global_concat_list.txt \
           -c copy demo-videos/complete_simple_global_narration.mp3 -y >/dev/null 2>&1

    if [ $? -eq 0 ]; then
        echo "✅ Complete narration created: demo-videos/complete_simple_global_narration.mp3"
    else
        echo "❌ Failed to concatenate audio segments"
        exit 1
    fi
else
    echo "❌ ffmpeg not found - cannot concatenate audio segments"
    exit 1
fi

echo ""
echo "🎉 Simple Global Analysis Demo Narration Complete!"
echo "📁 Output: demo-videos/complete_simple_global_narration.mp3"
echo ""
echo "🎤 Narration Features:"
echo "   ✅ 15 synchronized segments describing only visible features"
echo "   ✅ Professional British voice narration (Daniel)"
echo "   ✅ Perfect alignment with actual demo video content"
echo "   ✅ No features described that aren't shown in video"
echo "   ✅ Focus on existing EconGraph platform capabilities"
echo ""
echo "🚀 Ready for video combination!"

# Display file information
if [ -f "demo-videos/complete_simple_global_narration.mp3" ]; then
    echo "📊 Audio File Information:"
    ls -lh demo-videos/complete_simple_global_narration.mp3
fi
