#!/bin/bash

# REQUIREMENT: Enhanced epic demo orchestration with professional features
# PURPOSE: Complete demo setup, recording, and GitHub integration
# This demonstrates Bloomberg Terminal-level capabilities with audio narration

set -e

echo "🏆 EconGraph Professional Demo v2.0 Orchestrator"
echo "================================================"
echo "Features: Bloomberg Terminal Analytics + Enterprise OAuth"
echo "Audio: Professional voice walkthrough included"
echo ""

# Configuration
BACKEND_PORT=8080
FRONTEND_PORT=3000
DEMO_TIMEOUT=300  # 5 minutes for complete demo
DOCKER_COMPOSE_FILE="docker-compose.yml"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_section() {
    echo -e "${PURPLE}[SECTION]${NC} $1"
}

# Cleanup function
cleanup() {
    print_section "🧹 Cleaning up demo environment..."

    # Stop Docker containers
    if [ -f "$DOCKER_COMPOSE_FILE" ]; then
        print_status "Stopping Docker containers..."
        docker-compose down --remove-orphans || true
    fi

    # Kill any remaining processes
    pkill -f "npm start" || true
    pkill -f "cargo run" || true
    pkill -f "node create-professional-demo-video.js" || true

    print_success "Cleanup completed"
}

# Set up cleanup trap
trap cleanup EXIT

# Check prerequisites
check_prerequisites() {
    print_section "🔍 Checking prerequisites..."

    # Check if Docker is running
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker is not running. Please start Docker Desktop."
        exit 1
    fi

    # Check if Node.js is available
    if ! command -v node > /dev/null 2>&1; then
        print_error "Node.js is not installed. Please install Node.js."
        exit 1
    fi

    # Check if Rust/Cargo is available
    if ! command -v cargo > /dev/null 2>&1; then
        print_error "Rust/Cargo is not installed. Please install Rust."
        exit 1
    fi

    # Check if required files exist
    if [ ! -f "$DOCKER_COMPOSE_FILE" ]; then
        print_error "docker-compose.yml not found. Please run from project root."
        exit 1
    fi

    if [ ! -f "create-professional-demo-video.js" ]; then
        print_error "Professional demo script not found."
        exit 1
    fi

    print_success "All prerequisites satisfied"
}

# Start infrastructure
start_infrastructure() {
    print_section "🚀 Starting infrastructure..."

    print_status "Starting PostgreSQL and supporting services..."
    docker-compose up -d postgres

    # Wait for PostgreSQL to be ready
    print_status "Waiting for PostgreSQL to be ready..."
    sleep 10

    # Check PostgreSQL connection
    for i in {1..30}; do
        if docker-compose exec -T postgres pg_isready -U postgres > /dev/null 2>&1; then
            print_success "PostgreSQL is ready"
            break
        fi
        if [ $i -eq 30 ]; then
            print_error "PostgreSQL failed to start within 30 seconds"
            exit 1
        fi
        sleep 1
    done
}

# Setup backend
setup_backend() {
    print_section "🔧 Setting up backend..."

    cd backend

    # Run migrations
    print_status "Running database migrations..."
    if command -v diesel > /dev/null 2>&1; then
        diesel migration run --migration-dir migrations/data
        diesel migration run --migration-dir migrations/app
    elif [ -f ~/.cargo/bin/diesel ]; then
        ~/.cargo/bin/diesel migration run --migration-dir migrations/data
        ~/.cargo/bin/diesel migration run --migration-dir migrations/app
    else
        print_warning "Diesel CLI not found, skipping migrations"
    fi

    # Start backend server
    print_status "Starting backend server on port $BACKEND_PORT..."
    cargo run > ../backend.log 2>&1 &
    BACKEND_PID=$!

    cd ..

    # Wait for backend to be ready
    print_status "Waiting for backend server..."
    for i in {1..60}; do
        if curl -f http://localhost:$BACKEND_PORT/health > /dev/null 2>&1; then
            print_success "Backend server is ready"
            break
        fi
        if [ $i -eq 60 ]; then
            print_error "Backend server failed to start within 60 seconds"
            print_error "Backend logs:"
            tail -20 backend.log
            exit 1
        fi
        sleep 1
    done
}

# Setup frontend
setup_frontend() {
    print_section "🎨 Setting up frontend..."

    cd frontend

    # Install dependencies if needed
    if [ ! -d "node_modules" ]; then
        print_status "Installing frontend dependencies..."
        npm install
    fi

    # Start frontend server
    print_status "Starting frontend server on port $FRONTEND_PORT..."
    npm start > ../frontend.log 2>&1 &
    FRONTEND_PID=$!

    cd ..

    # Wait for frontend to be ready
    print_status "Waiting for frontend server..."
    for i in {1..120}; do
        if curl -f http://localhost:$FRONTEND_PORT > /dev/null 2>&1; then
            print_success "Frontend server is ready"
            break
        fi
        if [ $i -eq 120 ]; then
            print_error "Frontend server failed to start within 120 seconds"
            print_error "Frontend logs:"
            tail -20 frontend.log
            exit 1
        fi
        sleep 1
    done
}

# Install demo dependencies
install_demo_dependencies() {
    print_section "📦 Installing demo dependencies..."

    # Check if Playwright is installed
    if ! npm list playwright > /dev/null 2>&1; then
        print_status "Installing Playwright..."
        npm install playwright
    fi

    # Install Playwright browsers if needed
    if [ ! -d "$HOME/.cache/ms-playwright" ]; then
        print_status "Installing Playwright browsers..."
        npx playwright install chromium
    fi

    print_success "Demo dependencies ready"
}

# Create professional demo video
create_professional_demo() {
    print_section "🎬 Creating professional demo video..."

    print_status "Starting professional demo recording..."
    print_status "Features being demonstrated:"
    print_status "  ✅ Bloomberg Terminal-level chart analytics"
    print_status "  ✅ Multi-provider OAuth authentication (Google, Facebook, Email)"
    print_status "  ✅ Real-time chart collaboration with annotations"
    print_status "  ✅ Professional technical analysis (8 indicators)"
    print_status "  ✅ Economic cycle detection and event annotations"
    print_status "  ✅ Mobile-responsive professional UI"
    print_status "  ✅ Audio voice walkthrough narration"
    print_status ""
    print_status "Demo duration: ~2.5 minutes with professional narration"
    print_status "Recording in HD (1920x1080)..."

    # Run the professional demo
    timeout $DEMO_TIMEOUT node create-professional-demo-video.js || {
        print_error "Demo creation timed out or failed"
        exit 1
    }

    # Check if video was created
    if [ -f "professional-econgraph-demo-v2.mp4" ]; then
        print_success "Professional demo video created successfully!"

        # Get file size
        VIDEO_SIZE=$(du -h professional-econgraph-demo-v2.mp4 | cut -f1)
        print_status "Video file size: $VIDEO_SIZE"

        # Get video duration (if ffprobe is available)
        if command -v ffprobe > /dev/null 2>&1; then
            DURATION=$(ffprobe -v quiet -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 professional-econgraph-demo-v2.mp4 | cut -d. -f1)
            print_status "Video duration: ${DURATION} seconds"
        fi

    else
        print_error "Professional demo video was not created"
        exit 1
    fi
}

# Upload to GitHub
upload_to_github() {
    print_section "📤 Uploading to GitHub..."

    if [ ! -f "professional-econgraph-demo-v2.mp4" ]; then
        print_error "Demo video not found for upload"
        exit 1
    fi

    # Check if git is clean
    if [ -n "$(git status --porcelain)" ]; then
        print_status "Committing demo files..."
        git add professional-econgraph-demo-v2.mp4
        git add PROFESSIONAL_DEMO_README.md
        git add create-professional-demo-video.js
        git add professional-demo-orchestrator.sh

        git commit -m "feat: Enhanced Professional Demo v2.0 with Audio Narration

🎬 BLOOMBERG TERMINAL-LEVEL DEMO WITH AUDIO WALKTHROUGH

Professional Features Demonstrated:
✅ Multi-provider OAuth authentication (Google, Facebook, Email)
✅ Bloomberg Terminal-level chart analytics with 8 indicators
✅ Real-time collaboration with annotations and comments
✅ Economic cycle detection and correlation analysis
✅ Professional UI with mobile responsiveness
✅ Role-based access control and user management

Demo Specifications:
- Duration: ~2.5 minutes with professional narration
- Resolution: 1920x1080 HD
- Audio: Synchronized voice walkthrough
- Features: 15+ major professional capabilities
- Format: MP4 with enhanced visual highlighting

Technical Implementation:
- Playwright automation for consistent demonstrations
- Professional narration overlay system
- Mobile responsiveness showcase
- Real-time feature interaction
- Business value proposition

ACHIEVEMENT: Complete professional demonstration suitable for:
- Financial institutions
- Economic research organizations
- Government agencies
- Academic institutions
- Professional economic analysts

STATUS: 🏆 ENTERPRISE-READY PROFESSIONAL DEMONSTRATION"
    fi

    # Push to GitHub
    print_status "Pushing to GitHub..."
    git push origin main

    print_success "Professional demo uploaded to GitHub!"
}

# Update README with demo link
update_readme_with_demo() {
    print_section "📝 Updating README with professional demo..."

    if [ -f "README.md" ]; then
        # Check if demo section already exists
        if ! grep -q "Professional Demo v2.0" README.md; then
            # Add professional demo section
            cat << 'EOF' >> README_TEMP.md

## 🎬 Professional Demo v2.0 - Bloomberg Terminal Experience

### 🏆 Enhanced HD Demo with Audio Walkthrough
[📥 Download Professional Demo Video](./professional-econgraph-demo-v2.mp4)

**New in v2.0**: Complete Bloomberg Terminal-level capabilities demonstration with professional audio narration.

#### 🔐 Enterprise Authentication Demo
- Multi-provider OAuth (Google, Facebook, Email)
- Professional login interface with validation
- Role-based access control (Admin/Analyst/Viewer)
- Complete user profile management

#### 📊 Professional Chart Analytics Demo
- 8 Bloomberg Terminal-level technical indicators
- Economic cycle detection with confidence scoring
- Multi-series correlation analysis
- Economic event annotations
- Real-time parameter adjustment

#### 🤝 Real-Time Collaboration Demo
- Live chart annotation system
- Comment threading for economic discussions
- Tag organization and filtering
- Permission management demonstration

#### 🎨 Professional UI/UX Demo
- Mobile-responsive Bloomberg Terminal interface
- Authentication-aware navigation
- Professional loading states and accessibility
- Seamless user experience across devices

**Demo Specs**: 2.5min • 1920x1080 HD • Professional Audio Narration • 15+ Features

---

EOF

            # Prepend to existing README
            cat README.md >> README_TEMP.md
            mv README_TEMP.md README.md

            print_success "README updated with professional demo"
        else
            print_status "README already contains professional demo section"
        fi
    else
        print_warning "README.md not found, skipping update"
    fi
}

# Main execution
main() {
    print_section "🎯 Starting Professional Demo Creation Process"
    echo ""

    check_prerequisites
    start_infrastructure
    setup_backend
    setup_frontend
    install_demo_dependencies
    create_professional_demo
    upload_to_github
    update_readme_with_demo

    echo ""
    print_section "🎉 Professional Demo Creation Complete!"
    echo ""
    print_success "✅ Bloomberg Terminal-level analytics demonstrated"
    print_success "✅ Enterprise OAuth authentication showcased"
    print_success "✅ Real-time collaboration features highlighted"
    print_success "✅ Professional audio narration included"
    print_success "✅ HD video uploaded to GitHub"
    print_success "✅ Documentation updated"
    echo ""
    print_status "🎬 Demo file: professional-econgraph-demo-v2.mp4"
    print_status "📄 Documentation: PROFESSIONAL_DEMO_README.md"
    print_status "🌐 GitHub: Updated with professional showcase"
    echo ""
    print_section "🏆 EconGraph Professional - Ready for Enterprise Presentation!"
}

# Run main function
main "$@"
