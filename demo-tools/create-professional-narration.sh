#!/bin/bash

NARRATION_TEXT="professional-business-impact-narration.txt"
OUTPUT_AUDIO="demo-videos/professional_business_impact_narration.mp3"

echo "🎤 Creating Professional Business Impact Narration"
echo "📝 Using script: $NARRATION_TEXT"
echo "🎵 Output: $OUTPUT_AUDIO"
echo ""

if [ ! -f "$NARRATION_TEXT" ]; then
    echo "❌ Error: Narration script not found: $NARRATION_TEXT"
    exit 1
fi

echo "🎯 Business Impact Narration Features:"
echo "   💰 Cost comparison: Bloomberg ($24k), Thomson Reuters ($22k), CapIQ ($12k)"
echo "   🏢 Institutional-grade capabilities"
echo "   🚀 Open-source competitive advantage"
echo "   📊 Professional terminal-level analysis"
echo "   💡 Transformative business value proposition"
echo ""

# Read the narration text
NARRATION_CONTENT=$(cat "$NARRATION_TEXT")

echo "🎙️ Creating professional narration with business impact focus..."
echo ""

# Use macOS built-in text-to-speech with a professional voice
say -v "Alex" -r 160 -o "$OUTPUT_AUDIO" "$(cat "$NARRATION_TEXT" | grep -v "^\[" | grep -v "^=" | grep -v "^Professional" | grep -v "^Total Duration" | sed '/^$/d')"

if [ $? -eq 0 ]; then
    echo "✅ Professional narration created: $OUTPUT_AUDIO"

    # Get duration
    DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUTPUT_AUDIO" 2>/dev/null)
    if [ ! -z "$DURATION" ]; then
        echo "⏱️  Duration: ${DURATION} seconds"
    fi

    FILE_SIZE=$(du -h "$OUTPUT_AUDIO" | cut -f1)
    echo "📊 File size: ${FILE_SIZE}"
    echo ""
    echo "🎯 Professional Business Impact Narration Complete!"
    echo "   ✅ Bloomberg Terminal cost comparison"
    echo "   ✅ Institutional-grade positioning"
    echo "   ✅ Open-source competitive advantage"
    echo "   ✅ Professional value proposition"
    echo ""
    echo "🎵 Playing narration preview..."
    afplay "$OUTPUT_AUDIO" &
else
    echo "❌ Error creating narration audio"
    exit 1
fi
