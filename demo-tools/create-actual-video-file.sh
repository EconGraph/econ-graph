#!/bin/bash

# Create actual MP4 video file for GitHub repository
# This generates a real video file that can be committed and shared

set -e

echo "🎬 Creating actual EconGraph investor demo video file..."
echo "📹 Output: demo-videos/econ-graph-investor-demo-20min.mp4"

# Create output directory
mkdir -p demo-videos

# Create a slide-based video with the audio narration
echo "🎨 Generating video slides with key investor points..."

# Create slide images using ImageMagick (if available) or fallback to simple approach
if command -v convert &> /dev/null; then
    echo "📊 Creating professional slides..."

    # Slide 1: Title
    convert -size 1920x1080 xc:white \
        -font Arial -pointsize 72 -fill "#1976d2" \
        -gravity center -annotate +0-200 "EconGraph" \
        -pointsize 48 -fill "#424242" \
        -annotate +0-100 "Next-Generation Economic Data Platform" \
        -pointsize 36 -fill "#666666" \
        -annotate +0+50 "Professional Investor Demonstration" \
        -pointsize 24 -fill "#888888" \
        -annotate +0+150 "Market Opportunity: \$8.2B | 90% Cost Savings vs Bloomberg" \
        /tmp/slide1.png

    # Slide 2: Technology Stack
    convert -size 1920x1080 xc:white \
        -font Arial -pointsize 64 -fill "#1976d2" \
        -gravity center -annotate +0-300 "Modern Technology Stack" \
        -pointsize 40 -fill "#424242" \
        -annotate +0-150 "• React + TypeScript Frontend" \
        -annotate +0-50 "• Rust + Axum Backend" \
        -annotate +0+50 "• GraphQL API" \
        -annotate +0+150 "• PostgreSQL Database" \
        -pointsize 32 -fill "#666666" \
        -annotate +0+300 "High Performance | Memory Safe | Scalable" \
        /tmp/slide2.png

    # Slide 3: Business Model
    convert -size 1920x1080 xc:white \
        -font Arial -pointsize 64 -fill "#1976d2" \
        -gravity center -annotate +0-300 "Business Opportunity" \
        -pointsize 40 -fill "#424242" \
        -annotate +0-150 "• \$8.2B Financial Analytics Market" \
        -annotate +0-50 "• 90% Cost Savings vs Bloomberg" \
        -annotate +0+50 "• \$2M Series A Funding" \
        -annotate +0+150 "• AI-Assisted Development" \
        -pointsize 32 -fill "#666666" \
        -annotate +0+300 "Enterprise Ready | Scalable | Cost Effective" \
        /tmp/slide3.png

    # Create video from slides and audio
    echo "🎵 Combining slides with professional narration..."
    ffmpeg -loop 1 -i /tmp/slide1.png -loop 1 -i /tmp/slide2.png -loop 1 -i /tmp/slide3.png \
           -i demo-tools/generated-audio/investor_narration_20min.aiff \
           -filter_complex "[0:v]trim=duration=160[v1];[1:v]trim=duration=160[v2];[2:v]trim=duration=160[v3];[v1][v2][v3]concat=n=3:v=1:a=0[outv]" \
           -map "[outv]" -map 3:a \
           -c:v libx264 -c:a aac -b:a 128k -r 30 -t 480 \
           demo-videos/econ-graph-investor-demo-20min.mp4 2>/dev/null

    # Clean up temp files
    rm -f /tmp/slide*.png

else
    echo "📹 Creating video with audio narration..."
    # Create a simple video with black background and audio
    ffmpeg -f lavfi -i "color=black:size=1920x1080:rate=30" \
           -i demo-tools/generated-audio/investor_narration_20min.aiff \
           -c:v libx264 -c:a aac -b:a 128k -t 480 \
           demo-videos/econ-graph-investor-demo-20min.mp4 2>/dev/null
fi

# Get video duration
if [ -f demo-videos/econ-graph-investor-demo-20min.mp4 ]; then
    DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 demo-videos/econ-graph-investor-demo-20min.mp4 2>/dev/null || echo "unknown")
    SIZE=$(ls -lh demo-videos/econ-graph-investor-demo-20min.mp4 | awk '{print $5}')

    echo ""
    echo "✅ Video created successfully!"
    echo "📁 File: demo-videos/econ-graph-investor-demo-20min.mp4"
    echo "⏱️  Duration: ${DURATION} seconds"
    echo "💾 Size: ${SIZE}"
    echo ""
    echo "🚀 Ready to commit to GitHub!"
else
    echo "❌ Video creation failed"
    exit 1
fi
