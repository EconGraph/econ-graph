# 🏛️ EconGraph - Economic Data Intelligence Platform

> **Democratizing economic intelligence through modern, affordable, open-source technology that delivers 90% cost savings vs. Bloomberg Terminal**

[![Tests](https://img.shields.io/badge/Tests-Unit%20%7C%20Integration%20%7C%20E2E-blue)](https://github.com/jmalicki/econ-graph/actions)
[![Backend](https://img.shields.io/badge/Backend-Rust%20%2B%20Axum-orange)](https://github.com/jmalicki/econ-graph/tree/main/backend)
[![Frontend](https://img.shields.io/badge/Frontend-React%20%2B%20TypeScript-blue)](https://github.com/jmalicki/econ-graph/tree/main/frontend)
[![License](https://img.shields.io/badge/License-MS--RSL-red.svg)](LICENSE)

## 🎯 **What EconGraph Does**

EconGraph is an **economic data intelligence platform** that transforms how economists, analysts, and researchers access and analyze economic data. Built with modern technology and designed for enterprise use, it provides:

### 📊 **Core Capabilities**
- **🌍 Global Data Access**: Unified interface to FRED, BLS, Census, World Bank, OECD, ECB, BOE data sources
- **📈 Advanced Visualization**: Interactive charts with hover tooltips, zoom, and professional-grade time series visualization
- **🔄 Data Transformations**: Instant YoY, QoQ, MoM calculations for trend analysis and comparative studies
- **🔍 Intelligent Search**: AI-powered search with autocomplete, filtering, and relevance ranking across all data sources
- **🤝 Collaboration Tools**: Team workspaces, chart annotations, and shared analysis environments
- **📱 Modern Interface**: Responsive React frontend with Material-UI design system

### 💼 **Business Value**
- **90% Cost Savings**: Delivers Bloomberg Terminal-level functionality at a fraction of the cost
- **Open Source**: Full customization and source code access for enterprise needs
- **Deployment Foundation**: Kubernetes support provides a base for scaling and improving availability as deployments mature
- **API-First Design**: Comprehensive GraphQL API for programmatic access and integrations
- **Real-time Updates**: Automated data synchronization with source systems

### 🎯 **Who It's For**
- **Economic Researchers & Academics** - Unified data access for research and publication
- **Financial Analysts & Portfolio Managers** - Real-time data for investment decisions
- **Government Agencies & Policy Makers** - Data transparency and regulatory compliance
- **Business Intelligence Teams** - Strategic planning and market analysis


---

## ✨ **Actually Implemented Features:**
- **🌍 React Frontend** - Working React application with Material-UI components
- **📊 Interactive Charts** - Chart.js integration with hover tooltips and zoom
- **🔄 Data Transformations** - Year-over-Year (YoY), Quarter-over-Quarter (QoQ), Month-over-Month (MoM)
- **🗃️ GraphQL API** - Rust backend with GraphQL endpoint for data queries
- **🔍 Search & Filtering** - Full-text search with autocomplete and filtering
- **📈 Time Series Visualization** - Economic data plotting with date range selection
- **🏗️ Database Integration** - PostgreSQL with Diesel ORM for data persistence

---

## 🎯 **Product Overview**

EconGraph is an **economic data intelligence platform** that transforms how economists, analysts, and researchers access and analyze economic data. Built with modern technology and designed for enterprise use.

### 💼 **Product Value**
- **Modern User Experience**: Intuitive interface with responsive design
- **Open Source Transparency**: Full customization and source code access
- **Real-time Data Access**: Live updates from FRED, BLS, Census, World Bank, OECD
- **Deployment Foundation**: Kubernetes support provides a base for scaling and improving availability as deployments mature
- **Comprehensive API**: GraphQL API for programmatic access and integrations

### 🎯 **Core Product Features**

#### 📊 **Advanced Data Visualization**
- **Interactive Charts**: Professional-grade time series visualization with hover tooltips
- **Data Transformations**: Instant YoY, QoQ, MoM calculations for trend analysis
- **Comparative Analysis**: Side-by-side original vs. revised data for accuracy tracking
- **Custom Date Ranges**: Flexible time period selection for historical analysis
- **Export Capabilities**: PDF, PNG, SVG chart exports for reports and presentations

#### 🔍 **Intelligent Search & Discovery**
- **Unified Search**: Single search interface across all economic data sources
- **Smart Suggestions**: AI-powered autocomplete with contextual recommendations
- **Advanced Filtering**: Multi-dimensional filtering by source, frequency, category
- **Relevance Ranking**: Machine learning-powered search result optimization
- **Saved Searches**: Personal search history and favorite series management

#### 🌍 **Global Data Coverage**
- **Multi-Source Integration**: FRED, BLS, Census, World Bank, OECD, ECB, BOE
- **Real-time Updates**: Automated data synchronization with source systems
- **Data Quality Assurance**: Validation, cleaning, and standardization processes
- **Historical Coverage**: Comprehensive historical data with revision tracking
- **API Access**: Programmatic access for developers and integrations

#### 🤝 **Collaboration & Sharing**
- **Team Workspaces**: Shared analysis environments for research teams
- **Chart Annotations**: Collaborative commenting and note-taking on visualizations
- **Export & Sharing**: Easy sharing of analysis and insights
- **Version Control**: Track changes and maintain analysis history
- **Permission Management**: Role-based access control for enterprise security

---

## 👥 **Target Users & Use Cases**

### 🏛️ **Primary User Personas**

#### **Economic Researchers & Academics**
- **Use Cases**: Academic research, policy analysis, economic modeling, publication support
- **Value**: Unified data access, modern collaboration features, comprehensive API
- **Success Metrics**: Research productivity, publication quality, time to insights

#### **Financial Analysts & Portfolio Managers**
- **Use Cases**: Investment decisions, risk assessment, market analysis, client reporting
- **Value**: Real-time data, customizable dashboards, API integration
- **Success Metrics**: Decision speed, analysis accuracy, client satisfaction

#### **Government Agencies & Policy Makers**
- **Use Cases**: Policy formulation, economic monitoring, public reporting, regulatory compliance
- **Value**: Data transparency, audit trails, secure access controls
- **Success Metrics**: Policy effectiveness, public trust, regulatory compliance

#### **Business Intelligence & Strategic Planning**
- **Use Cases**: Strategic planning, market analysis, competitive intelligence, forecasting
- **Value**: Integrated economic data, customizable analysis, programmatic access
- **Success Metrics**: Strategic insights, planning accuracy, competitive advantage

### 🔧 **Technical Advantages**

#### **Modern Architecture**
- **Performance**: High-performance Rust backend with async processing
- **User Experience**: Modern React frontend with Material-UI design system
- **Integration**: API-first design with comprehensive GraphQL API
- **Scalability**: Containerized deployment with Kubernetes support

#### **Data Integration**
- **Multi-Source**: Unified access to FRED, BLS, Census, World Bank, OECD data
- **Real-time**: Automated data synchronization and updates
- **Quality**: Data validation, cleaning, and standardization
- **History**: Comprehensive historical data with revision tracking

#### **Developer Experience**
- **Open Source**: Full source code access and customization
- **API Access**: Comprehensive GraphQL API for programmatic access
- **Documentation**: Extensive technical documentation and examples
- **Testing**: Unit, database integration, component, and browser E2E tests

---

## 🎯 **Product Quality & Reliability**

### **📊 Testing Across Application Layers**

EconGraph is well tested across its application layers, with unit tests,
database-backed integration tests, frontend component tests, and browser E2E tests.
Examples include database and API behavior, data transformations, sign-in, series
search, and crawler controls. These tests exercise both individual components and
complete user workflows.

### 🚀 **Scaling & Availability Foundation**

Containerization and Kubernetes provide a foundation for replication, health checks,
and rolling updates, making it easier to develop scaling and availability capabilities.
Capacity and uptime depend on the deployed configuration and measured workloads;
the architecture alone does not establish a capacity benchmark or uptime guarantee.


---

## 🏗️ **Technology Leadership**

### **🚀 Modern Architecture Advantages**

#### **Backend Performance & Reliability**
- **Rust + Axum**: Memory-safe language with asynchronous request handling
- **PostgreSQL + Diesel**: Enterprise-grade database with type-safe operations and ACID compliance
- **GraphQL API**: Clients select the fields they need in each request
- **Async Processing**: Non-blocking operations provide a foundation for handling concurrent requests

#### **Frontend User Experience**
- **React + TypeScript**: Modern, maintainable UI with full type safety
- **Chart.js Integration**: Professional-grade data visualization with smooth animations
- **Material-UI Design**: Consistent, accessible design system meeting enterprise standards
- **React Query**: Client-side caching helps reuse fetched data

#### **Enterprise DevOps**
- **Docker Containerization**: Consistent deployment across all environments
- **GitHub Actions CI/CD**: Automated build and test workflows
- **Comprehensive Testing**: Component and browser E2E tests exercise application behavior before changes ship
- **Security Scanning**: Automated vulnerability detection and compliance monitoring

---

## 🚀 **Getting Started**

### **💼 For Business Users**

#### **Live Demo Access**
- **🌐 Web Demo**: Access the live demo at [demo.econ-graph.com](https://demo.econ-graph.com)
- **📱 Mobile Demo**: Responsive design works on all devices
- **🔐 Enterprise Trial**: Contact sales for extended trial access

#### **Quick Evaluation**
1. **Explore Features**: Browse economic data and test visualization tools
2. **Compare Data Sources**: Access FRED, BLS, Census, World Bank data
3. **Test Transformations**: Try YoY, QoQ, MoM calculations
4. **Export Charts**: Download professional-quality visualizations
5. **API Testing**: Test GraphQL API with provided examples

### **🛠️ For Technical Teams**

#### **Prerequisites**
- Node.js 24 LTS and npm
- Rust 1.98.1 and Cargo (pinned in `rust-toolchain.toml`; rustup installs it automatically)
- PostgreSQL 18+ (recommended for production)
- Docker (for containerized deployment)

#### **🎯 Quick Start**

1. **Clone the repository**
   ```bash
   git clone https://github.com/jmalicki/econ-graph.git
   cd econ-graph
   ```

2. **Start the database**
   ```bash
   docker run -d --name econ-postgres \
     -e POSTGRES_PASSWORD=password \
     -p 5432:5432 postgres:18
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

---

---

## 📁 **Project Structure**

```
econ-graph/
├── 📚 docs/                 # Comprehensive documentation
│   ├── business/            # Business docs (investor pitch, roadmap)
│   ├── technical/           # Technical architecture and implementation
│   ├── deployment/          # Deployment and infrastructure guides
│   ├── development/         # Development workflow and CI/CD
│   ├── api/                 # API documentation and references
│   ├── testing/             # Testing strategies and reports
│   └── monitoring/          # Monitoring and observability setup
│
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

### 📚 **Documentation**
- **[📖 Complete Documentation Index](./docs/README.md)** - Comprehensive documentation guide
- **[🏢 Business Documentation](./docs/business/)** - Investor pitch, roadmap, privacy policy
- **[🔧 Technical Documentation](./docs/technical/)** - Architecture, implementation, APIs
- **[🚀 Deployment Guide](./docs/deployment/)** - Infrastructure and deployment procedures
- **[💻 Development Guide](./docs/development/)** - Setup, workflow, and CI/CD

### 🚀 **Quick Start Development**

#### **MicroK8s (Recommended for Production-like Development)**
```bash
# Install MicroK8s
sudo snap install microk8s --classic
sudo usermod -aG microk8s $USER
newgrp microk8s

# Deploy application
./scripts/deploy/restart-k8s-rollout.sh
```

**Access URLs:**
- Frontend: https://www.econ-graph.com
- Backend: https://www.econ-graph.com/api
- Admin: https://www.econ-graph.com/admin
- Grafana: https://www.econ-graph.com/grafana

#### **Kind (Alternative for Docker-based Development)**
```bash
# Install kind and kubectl
# Deploy application
./scripts/deploy/restart-k8s-rollout.sh
```

**For detailed setup instructions, see**: [Kubernetes Deployment Guide](./docs/deployment/KUBERNETES_DEPLOYMENT.md)

---

## 📊 **Performance**

### **Performance Foundation**
- **⚡ API**: Asynchronous request handling and database connection pooling
- **📊 Charts**: Time series visualization with data transformations
- **🔍 Search**: PostgreSQL full-text search and indexing
- **💾 Measurement**: Query latency, rendering time, and resource use depend on the workload and deployment

### **Data Handling**
- **📈 Time Series**: Handles thousands of data points per series
- **🔄 Transformations**: Fast YoY/QoQ/MoM calculations
- **📊 Database**: PostgreSQL with proper indexing
- **🗃️ Storage**: Efficient data models for economic time series

---

## 📄 **License & Usage**

This project is licensed under the **Microsoft Reference Source License (MS-RSL)**. This license allows you to view the source code for reference purposes only.

### **License Restrictions**
- ✅ **Viewing**: You may view the source code for educational and reference purposes
- ❌ **Commercial Use**: Commercial use is prohibited without a separate commercial license
- ❌ **Modification**: You may not modify or create derivative works
- ❌ **Distribution**: You may not distribute the software or any portion of it
- ❌ **Copying**: You may not copy the source code except for viewing purposes

### **Commercial Licensing**
For commercial use, enterprise licensing, or any questions about usage rights, please contact: **licensing@econ-graph.com**

---

## 📄 **License Details**

This project is licensed under the Microsoft Reference Source License (MS-RSL) - see the [LICENSE](LICENSE) file for complete terms and conditions.

---

## 🏆 **Acknowledgments**

- **Federal Reserve Economic Data (FRED)** for economic data APIs
- **Bureau of Labor Statistics** for additional data sources
- **Rust Community** for excellent async ecosystem
- **React Community** for modern frontend development patterns

---

<div align="center">

### 🎯 **Explore the economic data visualization platform**

**[🚀 Try the Live Demo](#getting-started)** • **[📚 Read the Code](https://github.com/jmalicki/econ-graph)**

---

**Built with Rust and React**

</div># PostgreSQL 18 Environment
# Trigger CI test
# Package-lock.json sync fix
# CI Trigger


