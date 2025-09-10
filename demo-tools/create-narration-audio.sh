#!/bin/bash

# Professional Demo Narration Audio Generator
# Creates high-quality voice narration for EconGraph demo video

set -e

echo "🎤 Creating Professional Demo Narration Audio..."

# Create audio directory
mkdir -p demo-videos/audio-segments

# Professional voice settings for macOS 'say' command
VOICE="Alex"  # Professional male voice
RATE="175"    # Slightly slower for clarity
OUTPUT_FORMAT="aiff"  # High quality audio format

# Narration script segments with timing
declare -a SEGMENTS=(
    "Welcome to EconGraph Professional - the Bloomberg Terminal-level economic analysis platform. Today we'll demonstrate the comprehensive professional chart analytics and enterprise authentication features."
    "Let's start by exploring the modern, responsive interface. Notice the professional header with integrated search functionality and authentication controls."
    "First, let's demonstrate our enterprise OAuth authentication system. We'll click the Sign In button to access professional features."
    "The login dialog showcases our multi-provider authentication - supporting Google OAuth, Facebook OAuth, and secure email registration with professional form validation."
    "After authentication, users gain access to the Professional Analysis dashboard - our Bloomberg Terminal-inspired interface for institutional-grade economic research."
    "Here we see the key metrics dashboard showing real-time economic indicators, active comparison series, chart annotations, and collaborative team members."
    "Now let's explore the professional chart analytics. This advanced charting system provides Bloomberg Terminal-level technical analysis capabilities."
    "We can enable technical analysis indicators including Simple Moving Averages, Exponential Moving Averages, and Bollinger Bands for volatility analysis."
    "The system also features economic cycle detection, automatically identifying peaks and troughs in economic data with confidence scoring."
    "Economic events are automatically annotated on the chart, showing major events like the COVID-19 pandemic, Federal Reserve policy changes, and economic recoveries."
    "Let's demonstrate the real-time collaboration features. The collaboration panel allows teams to add annotations, comments, and share insights in real-time."
    "Users can create professional annotations with different types - vertical lines, data points, range boxes, and trend lines - all with customizable colors and descriptions."
    "The comment threading system enables economic discussions directly on chart annotations, with role-based permissions for team collaboration."
    "Multi-series comparison allows analysts to overlay different economic indicators, with real-time correlation analysis showing statistical relationships."
    "The user profile system provides complete preference management, theme customization, and role-based access control for enterprise security."
    "All features are mobile-responsive, providing the same professional experience across desktop, tablet, and mobile devices with accessibility compliance."
    "The search functionality integrates with authentication, providing personalized search results and access to professional analysis features."
    "EconGraph Professional represents a complete transformation into an enterprise-ready economic analysis platform with institutional-grade capabilities."
    "Thank you for watching this demonstration of EconGraph Professional - your Bloomberg Terminal-level economic analysis platform. Ready for institutional use."
)

# Generate audio files for each segment
for i in "${!SEGMENTS[@]}"; do
    segment_num=$(printf "%02d" $((i + 1)))
    output_file="demo-videos/audio-segments/segment_${segment_num}.aiff"

    echo "  🎙️  Generating segment ${segment_num}/19..."

    # Use macOS 'say' command to generate high-quality speech
    say -v "$VOICE" -r "$RATE" -o "$output_file" "${SEGMENTS[$i]}"

    # Convert to high-quality MP3 for better compression
    mp3_file="demo-videos/audio-segments/segment_${segment_num}.mp3"
    ffmpeg -i "$output_file" -codec:a libmp3lame -b:a 192k "$mp3_file" -y >/dev/null 2>&1

    # Remove the AIFF file to save space
    rm "$output_file"

    echo "    ✅ Created: $mp3_file"
done

echo ""
echo "🎉 Professional narration audio generation complete!"
echo "📁 Audio segments saved in: demo-videos/audio-segments/"
echo "🎵 Format: High-quality MP3 (192kbps)"
echo "🎤 Voice: Professional male voice (Alex)"
echo "📊 Segments: 19 narration segments ready for synchronization"
echo ""
echo "Next step: Synchronize audio with video timing..."
