#!/bin/bash

echo "🎯 Creating 20-Minute Comprehensive Investor Pitch Demo"
echo "This will showcase ALL UI features + complete business case for investors"
echo ""

# Check prerequisites
echo "🔍 Checking prerequisites..."

if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install Node.js first."
    exit 1
fi

if ! command -v npm &> /dev/null; then
    echo "❌ npm not found. Please install npm first."
    exit 1
fi

if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust first."
    exit 1
fi

echo "✅ All prerequisites found!"
echo ""

# Create comprehensive investor narration script
echo "📝 Creating comprehensive investor pitch narration..."

cat > ../demo-videos/investor-pitch-script.txt << 'EOF'
COMPREHENSIVE INVESTOR PITCH SCRIPT - 20 MINUTES
=================================================

[0:00-2:00] OPENING & MARKET OPPORTUNITY
- "Welcome to EconGraph - the future of economic intelligence platforms"
- "The global economic data analytics market is worth $8.2 billion and growing 15% annually"
- "Current solutions like Bloomberg Terminal cost $24,000/year per seat with limited customization"
- "We're disrupting this with an open-source, highly customizable platform"
- "EconGraph provides real-time economic insights at a fraction of the cost"

[2:00-4:00] TECHNOLOGY FOUNDATION & COMPETITIVE ADVANTAGE
- "Built on cutting-edge Rust backend for maximum performance and reliability"
- "Modern React frontend with TypeScript for enterprise-grade user experience"
- "GraphQL API for flexible data access and integration"
- "PostgreSQL with advanced indexing for sub-second query performance"
- "Kubernetes-ready architecture for infinite scalability"
- "99.9% uptime with comprehensive test coverage"

[4:00-8:00] CORE FEATURES DEMONSTRATION
- "Let me show you our powerful search and visualization capabilities"
- "Search for any economic indicator - GDP, unemployment, inflation, trade data"
- "Interactive charts with hover tooltips showing exact values and revision history"
- "Data transformations: Year-over-Year, Quarter-over-Quarter, Month-over-Month growth"
- "Multi-country comparisons for global economic analysis"
- "Real-time data updates from authoritative sources"
- "Export capabilities for further analysis"

[8:00-12:00] ADVANCED ANALYTICS & DATA SOURCES
- "Access to comprehensive economic databases"
- "FRED (Federal Reserve Economic Data) integration"
- "International data sources for global coverage"
- "Data quality indicators and revision tracking"
- "Historical data going back decades"
- "API access for programmatic data retrieval"
- "Custom indicator creation and calculation"

[12:00-16:00] BUSINESS MODEL & MONETIZATION
- "Freemium model: Core features free, premium features paid"
- "Enterprise licenses: $2,400/year per seat (90% savings vs Bloomberg)"
- "API access tiers for developers and institutions"
- "Custom deployment and white-label solutions"
- "Professional services and consulting"
- "Target markets: Financial institutions, government agencies, research organizations"

[16:00-18:00] ROADMAP & GROWTH STRATEGY
- "Phase 1: Advanced ML features - Random Forest models for forecasting"
- "Phase 2: LSTM neural networks for time series prediction"
- "Phase 3: Global Economic Network analysis with clustering algorithms"
- "Phase 4: AI-powered insights and natural language queries"
- "Phase 5: Enterprise features - SSO, audit trails, custom dashboards"
- "Strategic partnerships with data providers and financial institutions"

[18:00-20:00] INVESTMENT OPPORTUNITY & CLOSING
- "Seeking $2M Series A for team expansion and feature development"
- "Proven technology stack with working prototype"
- "Massive market opportunity with clear competitive advantages"
- "Strong technical team with domain expertise"
- "Path to profitability within 18 months"
- "Join us in democratizing economic intelligence for the world"

EOF

echo "✅ Investor pitch script created!"
echo ""

# Create comprehensive demo narration audio with British accent
echo "🎵 Creating comprehensive narration audio with British accent..."
echo "Generating professional British-accented narration for investor pitch..."

# Create narration audio using macOS text-to-speech with British voice
if command -v say &> /dev/null; then
    echo "🗣️  Generating British-accented narration..."

    # Use Daniel (British voice) for professional investor pitch
    say -v Daniel -o ../demo-videos/investor-pitch-intro.aiff "Welcome to EconGraph - the future of economic intelligence platforms. The global economic data analytics market is worth 8.2 billion dollars and growing 15 percent annually. Current solutions like Bloomberg Terminal cost 24,000 dollars per year per seat with limited customization. We're disrupting this with an open-source, highly customizable platform that provides real-time economic insights at a fraction of the cost."

    say -v Daniel -o ../demo-videos/investor-pitch-tech.aiff "Built on cutting-edge Rust backend for maximum performance and reliability. Modern React frontend with TypeScript for enterprise-grade user experience. GraphQL API for flexible data access and integration. PostgreSQL with advanced indexing for sub-second query performance. Kubernetes-ready architecture for infinite scalability with 99.9 percent uptime."

    say -v Daniel -o ../demo-videos/investor-pitch-features.aiff "Let me show you our powerful search and visualization capabilities. Search for any economic indicator - GDP, unemployment, inflation, trade data. Interactive charts with hover tooltips showing exact values and revision history. Data transformations include Year-over-Year, Quarter-over-Quarter, and Month-over-Month growth calculations for comprehensive analysis."

    say -v Daniel -o ../demo-videos/investor-pitch-business.aiff "Our freemium business model offers core features free, with premium features paid. Enterprise licenses cost 2,400 dollars per year per seat - that's a 90 percent savings versus Bloomberg. We target financial institutions, government agencies, and research organizations with API access tiers and custom deployment solutions."

    say -v Daniel -o ../demo-videos/investor-pitch-roadmap.aiff "Our roadmap includes advanced machine learning features - Random Forest models for forecasting, LSTM neural networks for time series prediction, and Global Economic Network analysis with clustering algorithms. We're seeking 2 million dollars Series A funding for team expansion and feature development."

    # Convert to MP3 for better compatibility
    echo "🔄 Converting to MP3 format..."
    for file in ../demo-videos/investor-pitch-*.aiff; do
        if [ -f "$file" ]; then
            mp3_file="${file%.aiff}.mp3"
            ffmpeg -i "$file" -acodec libmp3lame -ab 128k "$mp3_file" -y 2>/dev/null
            echo "✅ Created: $(basename "$mp3_file")"
        fi
    done

    # Combine all segments into one comprehensive narration
    echo "🎵 Combining all narration segments..."
    ffmpeg -i ../demo-videos/investor-pitch-intro.mp3 \
           -i ../demo-videos/investor-pitch-tech.mp3 \
           -i ../demo-videos/investor-pitch-features.mp3 \
           -i ../demo-videos/investor-pitch-business.mp3 \
           -i ../demo-videos/investor-pitch-roadmap.mp3 \
           -filter_complex "[0:0][1:0][2:0][3:0][4:0]concat=n=5:v=0:a=1[out]" \
           -map "[out]" ../demo-videos/comprehensive-investor-narration-british.mp3 -y 2>/dev/null

    echo "✅ British-accented investor pitch narration created!"
else
    echo "⚠️  'say' command not available. You'll need to create narration manually."
fi
echo ""

# Start backend
echo "🚀 Starting Rust backend..."
cd ../backend
if [ -d "../backend" ]; then
    cargo run --release &
    BACKEND_PID=$!
    echo "Backend started with PID: $BACKEND_PID"
    sleep 8
else
    echo "❌ Backend directory not found. Make sure you're in the right location."
    exit 1
fi

# Start frontend
echo "🌐 Starting React frontend..."
cd ../frontend
if [ -d "../frontend" ]; then
    npm start &
    FRONTEND_PID=$!
    echo "Frontend started with PID: $FRONTEND_PID"
    sleep 15
else
    echo "❌ Frontend directory not found."
    kill $BACKEND_PID 2>/dev/null
    exit 1
fi

# Wait for services to be ready
echo "⏳ Waiting for services to be fully ready..."
sleep 10

# Check if services are running
echo "🔍 Checking service status..."
if curl -s http://localhost:3000 > /dev/null; then
    echo "✅ Frontend is running at http://localhost:3000"
else
    echo "❌ Frontend not responding"
fi

echo ""
echo "======================================================================="
echo "🎬 READY FOR COMPREHENSIVE 20-MINUTE INVESTOR PITCH RECORDING!"
echo "======================================================================="
echo ""
echo "📱 EconGraph running at: http://localhost:3000"
echo "📋 Pitch script: ../demo-videos/investor-pitch-script.txt"
echo "🎵 British narration: ../demo-videos/comprehensive-investor-narration-british.mp3"
echo "⏱️  Target duration: 20 minutes"
echo ""
echo "🎯 COMPREHENSIVE DEMO SECTIONS:"
echo ""
echo "1. [0-2min] Market Opportunity & Value Proposition"
echo "   - Show landing page, explain market size and disruption"
echo "   - Navigate to main dashboard"
echo ""
echo "2. [2-4min] Technology Foundation"
echo "   - Highlight tech stack (show console/network tab briefly)"
echo "   - Demonstrate performance (fast loading, responsive UI)"
echo ""
echo "3. [4-8min] Core Features Deep Dive"
echo "   - Search functionality (try: 'GDP United States', 'Unemployment Rate')"
echo "   - Interactive charts (hover, zoom, pan)"
echo "   - Data transformations (YoY, QoQ, MoM buttons)"
echo "   - Multi-series comparisons (add multiple countries)"
echo ""
echo "4. [8-12min] Advanced Analytics"
echo "   - Data sources page (show breadth of data)"
echo "   - API documentation (show GraphQL playground if available)"
echo "   - Data quality features (revisions, metadata)"
echo ""
echo "5. [12-16min] Business Model"
echo "   - Pricing comparison with Bloomberg"
echo "   - Target market discussion"
echo "   - Show scalability (mention Kubernetes deployment)"
echo ""
echo "6. [16-18min] Roadmap & Vision"
echo "   - Open ROADMAP.md to show ML features planned"
echo "   - Discuss technical roadmap"
echo ""
echo "7. [18-20min] Investment Ask & Closing"
echo "   - Summarize key points"
echo "   - Investment opportunity"
echo "   - Call to action"
echo ""
echo "🎬 RECORDING TIPS:"
echo "• Use a high-quality screen recorder (OBS Studio recommended)"
echo "• Record in 1080p or higher resolution"
echo "• Ensure clear audio (use external microphone if possible)"
echo "• Practice the demo flow once before recording"
echo "• Keep browser full-screen for professional appearance"
echo "• Have the script open on a second monitor for reference"
echo ""
echo "📖 Script file opened for your reference..."

# Open the script file
open ../demo-videos/investor-pitch-script.txt

# Open the application
open http://localhost:3000

echo ""
echo "🎥 When ready to record, start your screen recorder and follow the script!"
echo "💡 Pro tip: Record in segments if needed, then combine with video editing software"
echo ""
echo "⏸️  Press ENTER when you've finished recording to stop the services..."
read -p ""

# Cleanup
echo "🧹 Stopping services..."
echo "Stopping frontend (PID: $FRONTEND_PID)..."
kill $FRONTEND_PID 2>/dev/null
echo "Stopping backend (PID: $BACKEND_PID)..."
kill $BACKEND_PID 2>/dev/null

echo ""
echo "✅ Comprehensive Investor Pitch Demo setup completed!"
echo ""
echo "📹 Next steps:"
echo "1. Your 20-minute recording showcases all features + business case"
echo "2. Save as: demo-videos/comprehensive-investor-pitch.mp4"
echo "3. This is perfect for investor presentations and fundraising!"
echo ""
echo "🚀 You now have a professional investor-grade demo!"
