#!/bin/bash

OUTPUT_AUDIO="demo-videos/professional_business_impact_narration.mp3"

echo "🎤 Creating Professional Business Impact Narration (Fixed)"
echo "🎵 Output: $OUTPUT_AUDIO"
echo ""

# Create the narration text directly
NARRATION_TEXT="Welcome to EconGraph, the revolutionary open-source economic analysis platform that delivers Bloomberg Terminal-level capabilities at a fraction of the cost. While Bloomberg costs twenty-four thousand dollars annually, and Thomson Reuters twenty-two thousand, EconGraph provides institutional-grade analysis completely free. Here's our professional dashboard, featuring real-time economic indicators with collaboration tools that rival premium financial terminals. Notice the clean Material-UI interface and responsive design that outperforms traditional systems. Let's explore our breakthrough Global Economic Network Analysis. This interactive world map visualizes economic correlations between countries - a feature typically found only in premium terminals costing thousands per month. The network visualization shows economic interconnections with D3.js-powered graphics. Node sizes represent economic centrality, colors indicate health, and connections show correlation strength. This level of analysis previously required expensive Bloomberg or CapIQ subscriptions. Switch to our Multi-Country Dashboard for comparative analysis across major economies. This Bloomberg Terminal-style interface provides GDP, inflation, and trade relationship analysis with synchronized charts and real-time correlation calculations. Our Global Events Explorer tracks major economic crises including the 2008 financial crisis, COVID-19 pandemic, and Brexit impacts. This comprehensive event analysis typically costs institutions twelve thousand dollars annually through S&P Capital IQ. The business impact is transformative. Financial institutions can save hundreds of thousands annually by replacing expensive terminal subscriptions. Research teams gain access to institutional-grade tools without budget constraints. EconGraph's open-source architecture enables customization impossible with proprietary systems. Built with Rust backend for performance and React frontend for modern user experience, delivering enterprise capabilities with startup agility. For economic research, policy analysis, and institutional decision-making, EconGraph provides the comprehensive tools previously exclusive to premium financial terminals. Experience Bloomberg Terminal-level analysis, completely free and open-source. Transform your economic analysis workflow with EconGraph - the future of accessible, professional economic data visualization."

echo "🎙️ Creating professional narration with business impact focus..."

# Use macOS built-in text-to-speech with a professional voice
say -v "Alex" -r 160 -o "$OUTPUT_AUDIO" "$NARRATION_TEXT"

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
else
    echo "❌ Error creating narration audio"
    exit 1
fi
