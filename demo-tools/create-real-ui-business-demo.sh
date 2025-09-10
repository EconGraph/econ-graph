#!/bin/bash

echo "🎬 Creating REAL UI Business Demo"
echo "   📱 Shows actual React application running"
echo "   💼 Highlights business value and use cases"
echo "   🎯 Real interactions with working features"
echo "   🚫 No cursor visible in recording"
echo ""

# Check if we're in the right directory
if [ ! -f "frontend/package.json" ] || [ ! -f "backend/Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the econ-graph root directory"
    exit 1
fi

echo "🚀 REAL UI BUSINESS DEMO SETUP"
echo ""

# Check prerequisites
echo "📋 Checking prerequisites..."

# Check Node.js
if ! command -v node &> /dev/null; then
    echo "❌ Node.js not found. Please install Node.js 18+ first."
    exit 1
fi

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo not found. Please install Rust first."
    exit 1
fi

# Check PostgreSQL (optional for demo)
if ! command -v psql &> /dev/null; then
    echo "⚠️  PostgreSQL not found locally. We'll use sample data for demo."
    USE_SAMPLE_DATA=true
else
    USE_SAMPLE_DATA=false
fi

echo "✅ Prerequisites checked"
echo ""

# Create business demo script
echo "📝 Creating business demo script..."

cat > temp_business_demo_script.md << 'EOF'
# 🎯 REAL UI BUSINESS DEMO SCRIPT (3-4 minutes)

## 📱 **What You'll Demonstrate:**
- **REAL React application** running at localhost:3000
- **Actual working features** with live interactions
- **Business value** for economists and analysts
- **Professional UI** with Material-UI components

## 🎬 **Recording Setup:**
1. Start backend: `cargo run` (in backend/)
2. Start frontend: `npm start` (in frontend/)
3. Open QuickTime → New Screen Recording
4. Record ONLY the browser window (avoid showing cursor)
5. Follow this script for narration

---

## 🎤 **DEMO SCRIPT & ACTIONS:**

### [0-15 seconds] **Opening & Value Proposition**
**SAY:** "This is EconGraph - a working economic data visualization prototype. What you're seeing is the actual React application running locally, not a mockup or slideshow."

**DO:**
- Show the main dashboard loading
- Point out the professional Material-UI interface
- Highlight the navigation menu

### [15-45 seconds] **Core Business Value**
**SAY:** "For economists and analysts, this solves real problems. Instead of using multiple tools or complex spreadsheets, you get interactive charts, data transformations, and search all in one place."

**DO:**
- Click on "Data Sources" to show available economic data
- Demonstrate the search functionality by typing "GDP" or "inflation"
- Show the autocomplete suggestions working

### [45-75 seconds] **Interactive Charts Demo**
**SAY:** "Here's where it gets powerful. These aren't static charts - they're fully interactive with real data transformations."

**DO:**
- Navigate to a chart view (Dashboard or specific series)
- Show hover tooltips on data points
- Demonstrate date range selection
- Switch between YoY, QoQ, MoM transformations
- Show zoom and pan functionality

### [75-105 seconds] **Technical Capabilities**
**SAY:** "Under the hood, this is a full-stack application: React frontend, Rust backend with GraphQL API, and PostgreSQL database. It's production-ready architecture, not just a prototype."

**DO:**
- Open browser dev tools briefly to show GraphQL queries
- Navigate between different pages to show routing works
- Demonstrate responsive design by resizing browser window

### [105-135 seconds] **Search & Data Management**
**SAY:** "The full-text search makes finding economic indicators fast and intuitive. This is the kind of user experience economists deserve."

**DO:**
- Use the global search bar
- Filter results by source or frequency
- Show search results with relevance ranking
- Click through to view specific data series

### [135-165 seconds] **Business Impact & ROI**
**SAY:** "For organizations, this replaces expensive tools and reduces analysis time. A team of economists can collaborate on data analysis instead of fighting with clunky interfaces."

**DO:**
- Show multiple data series comparison
- Demonstrate export functionality if available
- Navigate smoothly between features to show integration

### [165-195 seconds] **Future Vision (Roadmap)**
**SAY:** "This is just the beginning. Our roadmap includes machine learning models, LSTM forecasting, interactive world maps, and advanced analytics - all built on this solid foundation."

**DO:**
- Show the current features working smoothly
- Emphasize the professional UI and responsive design
- Maybe open the GitHub repository to show the roadmap

### [195-210 seconds] **Closing**
**SAY:** "EconGraph: a working prototype today, with a vision for advanced economic analysis tomorrow. Built with React, Rust, and real attention to user experience."

**DO:**
- Return to the main dashboard
- Show the overall application one final time
- End with the EconGraph logo or title

---

## 💡 **KEY BUSINESS POINTS TO EMPHASIZE:**

### ✅ **Current Value:**
- "Replaces multiple tools with one integrated platform"
- "Reduces time from data to insight"
- "Professional UI that economists actually want to use"
- "Full-stack application ready for deployment"

### 🚀 **Future Potential:**
- "Roadmap includes ML and advanced analytics"
- "Foundation for sophisticated economic modeling"
- "Scalable architecture for enterprise deployment"
- "Open source with customization potential"

### 🎯 **Target Audience:**
- "Economic research teams"
- "Financial institutions"
- "Government agencies"
- "Academic researchers"

---

## 🎬 **RECORDING TIPS:**

1. **Hide Cursor:** Use QuickTime's cursor hiding option
2. **Smooth Movements:** Move deliberately, not too fast
3. **Show Real Interactions:** Click buttons, use search, navigate pages
4. **Professional Pace:** Don't rush, let features load properly
5. **Business Focus:** Always connect features to business value

## 🎯 **SUCCESS CRITERIA:**
- ✅ Shows REAL React app running (not mockups)
- ✅ Demonstrates actual working features
- ✅ Highlights business value throughout
- ✅ Professional presentation quality
- ✅ No cursor visible in recording
- ✅ Smooth, confident navigation

EOF

echo "✅ Business demo script created: temp_business_demo_script.md"
echo ""

# Setup the application for demo
echo "🚀 Setting up the REAL application for demo..."

# Check if frontend dependencies are installed
if [ ! -d "frontend/node_modules" ]; then
    echo "📦 Installing frontend dependencies..."
    cd frontend
    npm install
    cd ..
fi

# Create a demo launch script
cat > temp_launch_demo.sh << 'EOF'
#!/bin/bash

echo "🚀 Launching REAL EconGraph Application for Business Demo"
echo ""

# Function to cleanup on exit
cleanup() {
    echo ""
    echo "🛑 Shutting down demo applications..."
    kill $BACKEND_PID 2>/dev/null
    kill $FRONTEND_PID 2>/dev/null
    exit
}

trap cleanup SIGINT SIGTERM

# Start backend
echo "🦀 Starting Rust backend..."
cd backend
cargo run &
BACKEND_PID=$!
cd ..

# Wait for backend to start
echo "⏳ Waiting for backend to initialize..."
sleep 5

# Start frontend
echo "⚛️ Starting React frontend..."
cd frontend
npm start &
FRONTEND_PID=$!
cd ..

echo ""
echo "✅ REAL ECONGRAPH APPLICATION RUNNING!"
echo ""
echo "🌐 Frontend: http://localhost:3000"
echo "🔗 Backend API: http://localhost:8000"
echo ""
echo "🎬 READY FOR BUSINESS DEMO RECORDING:"
echo "   1. Open http://localhost:3000 in your browser"
echo "   2. Start QuickTime screen recording (hide cursor)"
echo "   3. Follow the business demo script"
echo "   4. Record the REAL UI interactions"
echo ""
echo "📄 Demo script: temp_business_demo_script.md"
echo ""
echo "Press Ctrl+C to stop both applications when demo is complete"

# Wait for user to stop
wait
EOF

chmod +x temp_launch_demo.sh

echo "✅ Demo launch script created: temp_launch_demo.sh"
echo ""

# Final instructions
echo "🎯 READY TO CREATE REAL UI BUSINESS DEMO!"
echo ""
echo "📋 NEXT STEPS:"
echo ""
echo "1. 📖 READ THE SCRIPT:"
echo "   open temp_business_demo_script.md"
echo ""
echo "2. 🚀 LAUNCH THE REAL APPLICATION:"
echo "   ./temp_launch_demo.sh"
echo ""
echo "3. 🌐 OPEN BROWSER:"
echo "   http://localhost:3000"
echo ""
echo "4. 📹 START RECORDING:"
echo "   QuickTime Player → File → New Screen Recording"
echo "   ✅ Hide cursor option"
echo "   ✅ Record only browser window"
echo ""
echo "5. 🎤 FOLLOW THE SCRIPT:"
echo "   Demonstrate real features while explaining business value"
echo "   Show actual UI interactions (clicking, searching, navigating)"
echo ""
echo "6. 💾 SAVE AS:"
echo "   demo-videos/real-ui-business-demo.mp4"
echo ""
echo "🎯 RESULT: Professional demo showing REAL working app + business case!"
echo ""
echo "Ready to launch the real application? (y/n)"
read -r response

if [[ "$response" =~ ^[Yy]$ ]]; then
    echo ""
    echo "🚀 Launching REAL EconGraph application..."
    ./temp_launch_demo.sh
else
    echo ""
    echo "📋 When you're ready:"
    echo "   1. Read: temp_business_demo_script.md"
    echo "   2. Run: ./temp_launch_demo.sh"
    echo "   3. Record the REAL UI demo!"
fi
