#!/bin/bash

echo "🎬 Creating Comprehensive VIDEO Demo (6:44 with British Narration)"
echo "This will help you record the actual UI while the narration plays"
echo ""

# Check if the narration exists
if [ ! -f "../demo-videos/comprehensive-20-minute-demo-british.mp3" ]; then
    echo "❌ Narration file not found. Please run create-20-minute-comprehensive-demo.sh first."
    exit 1
fi

# Get narration duration
DURATION=$(ffprobe -v quiet -show_entries format=duration -of csv=p=0 ../demo-videos/comprehensive-20-minute-demo-british.mp3 2>/dev/null)
MINUTES=$((${DURATION%.*}/60))
SECONDS=$((${DURATION%.*}%60))

echo "✅ Found comprehensive narration: ${MINUTES}m ${SECONDS}s"
echo ""

# Check if frontend is running
if ! curl -s http://localhost:3000 > /dev/null; then
    echo "🌐 Starting React frontend..."
    cd ../frontend
    npm start &
    FRONTEND_PID=$!
    echo "Frontend starting with PID: $FRONTEND_PID"
    sleep 15
    cd ../demo-tools
else
    echo "✅ Frontend already running at http://localhost:3000"
fi

echo ""
echo "======================================================================="
echo "🎬 COMPREHENSIVE VIDEO DEMO RECORDING GUIDE"
echo "======================================================================="
echo ""
echo "📱 EconGraph: http://localhost:3000"
echo "🎵 Narration: ../demo-videos/comprehensive-20-minute-demo-british.mp3"
echo "⏱️  Duration: ${MINUTES}m ${SECONDS}s"
echo ""
echo "🎯 RECORDING INSTRUCTIONS:"
echo ""
echo "1. 📹 START SCREEN RECORDING (QuickTime/OBS):"
echo "   - Record your entire screen or browser window"
echo "   - Make sure to capture audio from the narration"
echo "   - Resolution: 1080p or higher for professional quality"
echo ""
echo "2. 🎵 PLAY THE NARRATION:"
echo "   - The narration file will open automatically"
echo "   - Start playing it when you begin screen recording"
echo ""
echo "3. 🖱️  FOLLOW THE NARRATION WITH UI INTERACTIONS:"
echo ""
echo "   [0:00-1:17] MARKET OPPORTUNITY"
echo "   → Show EconGraph landing page"
echo "   → Navigate through main interface"
echo "   → Highlight professional design"
echo ""
echo "   [1:17-2:40] TECHNOLOGY FOUNDATION"
echo "   → Open browser developer tools briefly"
echo "   → Show network requests (GraphQL)"
echo "   → Demonstrate responsive design"
echo "   → Show fast loading times"
echo ""
echo "   [2:40-4:12] CORE FEATURES DEMO"
echo "   → Search for 'GDP United States' - show it works!"
echo "   → Click on Real GDP link - verify it shows GDP (not CPI!)"
echo "   → Hover over chart points - show tooltips"
echo "   → Apply Year-over-Year transformation"
echo "   → Notice Y-axis shows clear units"
echo "   → Search for 'Unemployment Rate'"
echo "   → Add multiple series for comparison"
echo ""
echo "   [4:12-5:05] DATA SOURCES & QUALITY"
echo "   → Navigate to Data Sources page"
echo "   → Show breadth of available data"
echo "   → Demonstrate search functionality"
echo "   → Show data quality indicators"
echo ""
echo "   [5:05-5:46] BUSINESS MODEL"
echo "   → Show pricing information or About page"
echo "   → Demonstrate enterprise-ready features"
echo "   → Show scalability indicators"
echo ""
echo "   [5:46-6:18] COMPETITIVE ADVANTAGES"
echo "   → Show open source nature (GitHub link)"
echo "   → Demonstrate modern UX vs old interfaces"
echo "   → Show customization capabilities"
echo ""
echo "   [6:18-6:44] INVESTMENT OPPORTUNITY"
echo "   → Show future ML features planned"
echo "   → Return to main app showing current solid foundation"
echo ""
echo "4. 🎬 RECORDING TIPS:"
echo "   - Keep browser full-screen for professional look"
echo "   - Move mouse smoothly and deliberately"
echo "   - Click clearly on buttons and links"
echo "   - Let the narration guide your timing"
echo "   - Show the fixes: series links work, y-axis labels clear"
echo ""

# Open the narration and browser
echo "🎵 Opening narration and EconGraph app..."
open ../demo-videos/comprehensive-20-minute-demo-british.mp3
sleep 2
open http://localhost:3000

echo ""
echo "======================================================================="
echo "🎬 READY TO RECORD YOUR COMPREHENSIVE VIDEO DEMO!"
echo "======================================================================="
echo ""
echo "📹 Start your screen recording now and play the narration!"
echo "🎯 Follow the timing guide above for perfect synchronization"
echo "⏱️  Total recording time: ${MINUTES}m ${SECONDS}s"
echo ""
echo "When finished, you'll have:"
echo "✅ Professional video showing ACTUAL UI"
echo "✅ British narration explaining business case"
echo "✅ Complete investor presentation"
echo "✅ Technical demonstration with working features"
echo ""
echo "💡 Save your recording as: demo-videos/comprehensive-video-demo.mp4"
echo ""
echo "⏸️  Press ENTER when you've finished recording..."
read -p ""

# Cleanup if we started the frontend
if [ ! -z "$FRONTEND_PID" ]; then
    echo "🧹 Stopping frontend..."
    kill $FRONTEND_PID 2>/dev/null
fi

echo ""
echo "✅ Comprehensive VIDEO demo recording session completed!"
echo ""
echo "🎬 You now have a professional video combining:"
echo "   📹 Real UI interactions"
echo "   🎵 Professional British narration"
echo "   💼 Complete business case"
echo "   🔧 Working features demonstration"
echo ""
echo "🚀 Perfect for investor presentations and demo days!"
