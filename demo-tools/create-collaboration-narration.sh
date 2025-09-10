#!/bin/bash

# Collaboration-Focused Demo Narration Audio Generator
# Creates high-quality voice narration for collaboration features

set -e

echo "🎤 Creating Collaboration Demo Narration Audio..."

# Create audio directory
mkdir -p demo-videos/collaboration-audio

# Professional voice settings for macOS 'say' command
VOICE="Alex"  # Professional male voice
RATE="175"    # Slightly slower for clarity
OUTPUT_FORMAT="aiff"  # High quality audio format

# Collaboration-focused narration script
declare -a COLLABORATION_SEGMENTS=(
    "Welcome to EconGraph's Revolutionary Collaboration Platform - where economic analysis meets real-time teamwork. Today we'll explore how teams collaborate on economic research like never before."
    "Let's start with our Bloomberg Terminal-inspired collaboration workspace. Notice the real-time collaboration panel showing active team members working on this analysis."
    "Here we see live collaboration indicators - green dots show who's currently online, their roles, and their permission levels for this economic analysis."
    "Now let's demonstrate real-time chart annotations. I'll add a professional annotation marking the COVID-19 economic impact period with contextual analysis."
    "Watch as I create a vertical line annotation at March 2020. The system provides multiple annotation types: vertical lines, data points, range boxes, and trend lines."
    "I'm adding a detailed description explaining the economic significance of this date. Notice the professional color coding and tagging system for organizing annotations."
    "The annotation appears instantly on the chart with professional styling. Team members can see this annotation in real-time, enabling collaborative economic analysis."
    "Now let's explore the comment threading system. Team members can discuss annotations directly, creating threaded conversations about specific economic events."
    "I'm adding a comment to this annotation discussing the Federal Reserve's policy response. Notice the real-time timestamp and author attribution."
    "The collaboration panel shows all team annotations with filtering options: view all annotations, only mine, or just the pinned important ones."
    "Let's demonstrate the annotation management features. Users can toggle visibility, pin important annotations, and organize by tags for complex analysis workflows."
    "The permission system ensures secure collaboration. Team leads can control who can view, comment, edit, or administer collaborative charts and annotations."
    "Now I'll show chart sharing capabilities. Users can invite team members with specific permission levels: viewer access, commenting rights, or full editing privileges."
    "The collaboration history tracks all changes, comments, and annotations with full audit trails - essential for institutional compliance and research integrity."
    "Multiple team members can work simultaneously on the same economic analysis, with real-time synchronization ensuring everyone sees the latest insights and discussions."
    "The system supports professional workflows: analysts can add technical annotations, economists can provide contextual comments, and managers can pin critical insights."
    "All collaboration features work seamlessly across desktop, tablet, and mobile devices, enabling economic teams to collaborate from anywhere in the world."
    "This represents the future of collaborative economic analysis - combining Bloomberg Terminal-level functionality with modern real-time collaboration technology."
    "Thank you for exploring EconGraph's collaboration platform. Transform your economic analysis workflow with professional-grade real-time collaboration tools."
)

# Generate audio files for each segment
for i in "${!COLLABORATION_SEGMENTS[@]}"; do
    segment_num=$(printf "%02d" $((i + 1)))
    output_file="demo-videos/collaboration-audio/collab_segment_${segment_num}.aiff"

    echo "  🎙️  Generating collaboration segment ${segment_num}/19..."

    # Use macOS 'say' command to generate high-quality speech
    say -v "$VOICE" -r "$RATE" -o "$output_file" "${COLLABORATION_SEGMENTS[$i]}"

    # Convert to high-quality MP3 for better compression
    mp3_file="demo-videos/collaboration-audio/collab_segment_${segment_num}.mp3"
    ffmpeg -i "$output_file" -codec:a libmp3lame -b:a 192k "$mp3_file" -y >/dev/null 2>&1

    # Remove the AIFF file to save space
    rm "$output_file"

    echo "    ✅ Created: $mp3_file"
done

echo ""
echo "🎉 Collaboration demo narration audio generation complete!"
echo "📁 Audio segments saved in: demo-videos/collaboration-audio/"
echo "🎵 Format: High-quality MP3 (192kbps)"
echo "🎤 Voice: Professional male voice (Alex)"
echo "📊 Segments: 19 collaboration-focused narration segments ready"
echo ""
echo "Next step: Create collaboration demo video with synchronized audio..."
