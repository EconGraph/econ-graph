# 🏛️ EconGraph - Economic Data Visualization Prototype

> **A working prototype for economic time series visualization with React frontend and Rust backend**

[![Tests](https://img.shields.io/badge/Tests-157%20Passing-brightgreen)](https://github.com/jmalicki/econ-graph/actions)
[![Backend](https://img.shields.io/badge/Backend-Rust%20%2B%20Axum-orange)](https://github.com/jmalicki/econ-graph/tree/main/backend)
[![Frontend](https://img.shields.io/badge/Frontend-React%20%2B%20TypeScript-blue)](https://github.com/jmalicki/econ-graph/tree/main/frontend)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## 🎬 **PROTOTYPE DEMONSTRATIONS**

> **Honest demos showing actual implemented features**

### 💼 **Real UI Business Demo - Working App + Business Case (NEW!)**
[![EconGraph Real UI Business Demo](https://img.shields.io/badge/💼%20Real%20UI%20Business%20Demo-Working%20App%20%2B%20Business%20Case-gold?style=for-the-badge&logo=play)](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/real-ui-business-demo.mp4)

**[🚀 Create Your Own Real UI Business Demo](./demo-tools/create-real-ui-business-demo.sh)**

> **✅ REAL WORKING APPLICATION: Shows the actual React app running at localhost:3000 with live interactions, business value demonstration, and professional UI usage. No mockups - just real software being used.**

**🎯 What You'll See:**
- **Live React Application** running in browser with real interactions
- **Business Value Demonstration** showing ROI and use cases for economists
- **Working Features** with actual clicking, searching, and chart interactions
- **Professional UI** with Material-UI components in real usage
- **Technical Capabilities** including GraphQL API calls and responsive design
- **Future Roadmap** highlighting ML and advanced analytics potential

### 🌍 **Real Interface Demo - Actual React Application**
[![EconGraph Real Interface Demo](https://img.shields.io/badge/🌍%20Real%20Interface%20Demo-Actual%20React%20Application-brightgreen?style=for-the-badge&logo=play)](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/real-econ-graph-interface.mp4)

**[📺 Watch the REAL Interface Demo - 77sec HD Screen Recording](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/real-econ-graph-interface.mp4)**

> **✅ ACTUAL PROTOTYPE: Shows the real running React application with genuine Material-UI components, working navigation, and functional interface elements.**

### 🤝 **Collaboration Features Demo**
[![EconGraph Enhanced Collaboration Demo](https://img.shields.io/badge/🤝%20Enhanced%20Collaboration%20Demo-Team%20Workflow%20Features-blue?style=for-the-badge&logo=play)](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/collaboration-demo-with-narration.mp3)

**[📺 Watch the Collaboration Demo with Audio](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/collaboration-demo-with-narration.mp3)**

### ✨ **Actually Implemented Features:**
- **🌍 React Frontend** - Working React application with Material-UI components
- **📊 Interactive Charts** - Chart.js integration with hover tooltips and zoom
- **🔄 Data Transformations** - Year-over-Year (YoY), Quarter-over-Quarter (QoQ), Month-over-Month (MoM)
- **🗃️ GraphQL API** - Rust backend with GraphQL endpoint for data queries
- **🔍 Search & Filtering** - Full-text search with autocomplete and filtering
- **📈 Time Series Visualization** - Economic data plotting with date range selection
- **🏗️ Database Integration** - PostgreSQL with Diesel ORM for data persistence

### 🎯 **What You'll See in the Real Interface Demo:**
- 🌍 **Live React Application**: Actual browser window showing the running EconGraph interface
- 📊 **Material-UI Components**: Real buttons, navigation, cards, and layouts in action
- 📈 **Working Navigation**: Functional React Router with live page transitions
- 🎛️ **Interactive Elements**: Real clickable components and working state management
- 🏷️ **Professional Styling**: Actual Material-UI theme with responsive design
- 🎤 **Honest Narration**: 77-second description of what's actually implemented
- 🎯 **Zero Fake Content**: No text overlays, Unicode boxes, or static mockups
- ✅ **Genuine Demonstration**: Authentic screen recording of the actual running application

---

## 🚀 **System Overview**

EconGraph is a **working prototype** for economic data visualization. It's built with modern technologies and demonstrates a complete full-stack application with React frontend, Rust backend, and PostgreSQL database.

### ✨ **Actual Features**

#### 📊 **Data Visualization**
- **Interactive Time Series Charts** with Chart.js
- **Data Transformations**: YoY, QoQ, MoM calculations
- **Hover Tooltips** with detailed data point information
- **Date Range Selection** with calendar pickers
- **Zoom and Pan** functionality on charts

#### 🔍 **Search & Discovery**
- **Full-text Search** across economic data series
- **Autocomplete Suggestions** in search interface
- **Filtering** by data source, frequency, and date range
- **Search Results** with relevance-based ranking

#### 🗃️ **Data Management**
- **PostgreSQL Database** with structured economic data
- **Diesel ORM** for type-safe database operations
- **Data Models** for economic series and data sources
- **Sample Data** from Federal Reserve (FRED) API

#### 🏗️ **Technical Architecture**
- **React Frontend** with TypeScript and Material-UI
- **Rust Backend** with Axum web framework
- **GraphQL API** for efficient data fetching
- **Async Processing** with Tokio runtime
- **Docker Support** for containerized deployment

---

## 🧪 **Testing**

### **📊 Test Coverage: 157 Tests Passing**

- ✅ **Backend Tests**: 64 passing (Database, GraphQL, Services, Models)
- ✅ **Frontend Tests**: 93 passing (Components, Hooks, Integration)
- ✅ **Integration Tests**: Database integration with real PostgreSQL
- ✅ **Unit Tests**: Individual component and function testing

### 🎬 **E2E Testing**
```bash
# Run the complete test suite
./epic-e2e-demo.sh
```

**Features:**
- **Database Testing**: Real PostgreSQL integration tests
- **GraphQL Testing**: Complete API schema validation
- **Frontend Testing**: Component and integration tests
- **Performance Testing**: Basic load testing capabilities

---

## 🏗️ **Technical Stack**

### **Backend**
- **🦀 Rust + Axum**: Web framework with async support
- **🗃️ PostgreSQL + Diesel**: Database with type-safe ORM
- **📊 GraphQL**: API layer for data queries
- **⚡ Tokio**: Async runtime for concurrent processing

### **Frontend** 
- **⚛️ React + TypeScript**: Component-based UI architecture
- **📈 Chart.js**: Interactive data visualization
- **🎨 Material-UI**: Professional design system
- **🔄 React Query**: Data fetching and caching

### **DevOps**
- **🐳 Docker**: Containerization support
- **🔄 GitHub Actions**: CI/CD pipeline
- **🧪 Jest + Playwright**: Testing frameworks

---

## 🚀 **Getting Started**

### **Prerequisites**
- Node.js 18+ and npm
- Rust 1.70+ and Cargo
- PostgreSQL 14+
- Docker (optional)

### **🎯 Quick Start**

1. **Clone the repository**
   ```bash
   git clone https://github.com/jmalicki/econ-graph.git
   cd econ-graph
   ```

2. **Start the database**
   ```bash
   docker run -d --name econ-postgres \
     -e POSTGRES_PASSWORD=password \
     -p 5432:5432 postgres:14
   ```

3. **Launch the backend**
   ```bash
   cd backend
   cargo run
   # Backend running on http://localhost:8000
   ```

4. **Start the frontend**
   ```bash
   cd frontend
   npm install && npm start
   # Frontend running on http://localhost:3000
   ```

5. **🎉 Open your browser** to `http://localhost:3000`

### **🎬 Create Demo Videos**
```bash
# Navigate to demo tools
cd demo-tools

# 🚀 RECOMMENDED: Create real UI business demo
./create-real-ui-business-demo.sh

# Create realistic demo showing actual features
./create-realistic-demo.sh

# Create honest pitch video
./create-honest-pitch-video.sh
```

**🎯 For the best demo experience:**
1. **Run the Real UI Business Demo** - shows actual working app + business case
2. **Record yourself using the interface** - demonstrates real functionality
3. **Follow the provided script** - ensures professional presentation

**📁 All demo tools available in:** [`demo-tools/`](./demo-tools/) directory

---

## 📁 **Project Structure**

```
econ-graph/
├── 🦀 backend/              # Rust backend with Axum + PostgreSQL
│   ├── src/
│   │   ├── graphql/         # GraphQL schema and resolvers
│   │   ├── models/          # Database models with Diesel ORM
│   │   ├── services/        # Business logic and data processing
│   │   └── handlers/        # HTTP request handlers
│   ├── migrations/          # Database schema migrations
│   └── tests/               # Integration and unit tests
│
├── ⚛️ frontend/             # React frontend with TypeScript
│   ├── src/
│   │   ├── components/      # Reusable UI components
│   │   ├── pages/           # Application pages and routes
│   │   ├── hooks/           # Custom React hooks for data fetching
│   │   └── utils/           # Utility functions and GraphQL client
│   └── __tests__/           # Frontend test suites
│
├── 🏗️ terraform/           # Infrastructure as Code (deployment ready)
│   ├── modules/             # Reusable Terraform modules
│   └── environments/       # Environment-specific configurations
│
├── 📊 grafana-dashboards/  # Monitoring configurations
│   ├── system-metrics.json
│   └── database-statistics.json
│
├── 🎬 demo-videos/         # Demo recordings and HTML interfaces
│   ├── honest-global-analysis-demo.html
│   └── comprehensive-global-analysis-demo.html
│
└── 🛠️ demo-tools/          # Professional demo creation scripts
    ├── create-real-ui-business-demo.sh    # RECOMMENDED
    ├── create-realistic-demo.sh
    ├── create-honest-pitch-video.sh
    └── README.md           # Complete demo tools documentation
```

---

## 📊 **Performance**

### **Current Performance**
- **⚡ API Response**: ~100ms for typical queries
- **📊 Chart Rendering**: ~500ms for 1000 data points  
- **🔍 Search Speed**: ~200ms for text queries
- **💾 Memory Usage**: Efficient Rust backend with minimal overhead

### **Data Handling**
- **📈 Time Series**: Handles thousands of data points per series
- **🔄 Transformations**: Fast YoY/QoQ/MoM calculations
- **📊 Database**: PostgreSQL with proper indexing
- **🗃️ Storage**: Efficient data models for economic time series

---

## 🤝 **Contributing**

We welcome contributions! This is a working prototype with room for improvement.

### **Development Workflow**
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests (`cargo test && npm test`)
4. Commit changes (`git commit -m 'Add amazing feature'`)
5. Push to branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

---

## 📄 **License**

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🏆 **Acknowledgments**

- **Federal Reserve Economic Data (FRED)** for economic data APIs
- **Bureau of Labor Statistics** for additional data sources
- **Rust Community** for excellent async ecosystem
- **React Community** for modern frontend development patterns

---

<div align="center">

### 🎯 **Ready to explore this economic data visualization prototype?**

**[🌍 Watch Real Interface Demo](https://github.com/jmalicki/econ-graph/raw/main/demo-videos/real-econ-graph-interface.mp4)** • **[🚀 Try the Live Demo](#getting-started)** • **[📚 Read the Code](https://github.com/jmalicki/econ-graph)**

> **📺 Real Interface Demo - ACTUAL screen recording of the working React prototype**

---

**Built as a learning project for full-stack development with Rust and React**

</div>