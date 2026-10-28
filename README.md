# 🏛️ EconGraph - Economic Data Intelligence Platform

> **Economic intelligence made accessible: discover, visualize, and analyze public economic data in one place.**

[![Tests](https://img.shields.io/badge/Tests-Unit%20%7C%20Integration%20%7C%20E2E-blue)](https://github.com/EconGraph/econ-graph/actions)
[![Backend](https://img.shields.io/badge/Backend-Rust%20%2B%20Warp-orange)](https://github.com/EconGraph/econ-graph/tree/main/backend)
[![Frontend](https://img.shields.io/badge/Frontend-React%20%2B%20TypeScript-blue)](https://github.com/EconGraph/econ-graph/tree/main/frontend)
[![License](https://img.shields.io/badge/License-MS--RSL-red.svg)](LICENSE)

## 🎯 **What EconGraph Does**

EconGraph brings economic data discovery, interactive charts, and analysis into a
single responsive interface for economists, analysts, and researchers. It offers
a focused alternative to switching between source websites and spreadsheets for
economic-series research, with a GraphQL API for integrating data into your own
analysis tools.

### 📊 **Core Capabilities**

- **Global data access**: Explore series from FRED, BLS, Census, World Bank, BEA,
  and FHFA through a unified interface.
- **Interactive visualization**: Inspect time series with hover tooltips and custom
  date ranges. Tooltips include revision dates when available.
- **Data transformations**: Compare year-over-year (YoY), quarter-over-quarter
  (QoQ), and month-over-month (MoM) changes to study trends.
- **Search and discovery**: Find series with typo-tolerant search, abbreviation
  expansion, autocomplete, and relevance ranking. Filter by source and frequency.
  Search uses PostgreSQL full-text matching and trigram similarity.
- **Annotations and comments**: Signed-in users can add public or private notes to
  series charts and discuss public annotations in comment threads.
- **Data exports**: Export Series Explorer results as CSV or JSON and download
  series observations as CSV for further analysis.
- **Programmatic access**: Query series and observations through the GraphQL API.
  Crawler jobs fetch source data and track revisions; data freshness depends on
  source releases and the configured crawl schedule.

Source code is available for reference under the repository's stated license
restrictions. See [License & Usage](#license--usage) for the terms and commercial
licensing contact.

### 👥 **Who It's For**

- **Economic researchers and academics**: Find historical series, examine trends,
  and retrieve data for economic modeling and publication.
- **Financial analysts and portfolio managers**: Monitor economic indicators for
  investment research, risk assessment, and client reporting.
- **Government agencies and policy makers**: Explore economic indicators for
  policy analysis, monitoring, and public reporting.
- **Business intelligence teams**: Bring economic data into strategic planning,
  market analysis, and existing tools through the API.

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

## 🏗️ **Architecture**

The Rust backend uses Warp for asynchronous HTTP handling and exposes a GraphQL
API, so clients can select the fields they need. PostgreSQL stores economic data
and application state, with Diesel providing typed database operations and
connection pooling.

The React and TypeScript frontend uses Material-UI for its responsive interface,
Chart.js for time series visualization, and React Query to cache fetched data.
Docker images and Kubernetes deployment scripts support containerized operation.
GitHub Actions runs automated builds, tests, and dependency and container security
scans; see the [development documentation](docs/development/) for the workflow.

---

<a id="getting-started"></a>

## 🚀 **Getting Started**

### **💼 For Business Users**

#### **Live Demo Access**

- **🌐 Web**: [econgraph.com](https://econgraph.com)
- **📱 Mobile Demo**: Responsive layouts for desktop and mobile screens
- **✉️ Contact**: support@econgraph.com

#### **Quick Evaluation**

1. **Explore Features**: Browse economic data and test visualization tools
2. **Compare Data Sources**: Access FRED, BLS, Census, World Bank data
3. **Test Transformations**: Try YoY, QoQ, MoM calculations
4. **Export Data**: Download Explorer results as CSV or JSON, or series observations as CSV
5. **API Testing**: Test GraphQL API with provided examples

#### Series CSV exports

The Series Explorer's **Export as CSV** and the series page's **Download CSV**
produce UTF-8 files with a byte-order mark so Excel recognizes non-ASCII titles.
Text beginning with `=`, `+`, `-`, `@`, tab, carriage return, or line feed gets a
leading apostrophe to prevent spreadsheet formula evaluation. This also applies
to the Explorer's ID columns: an external ID such as `-ABC` exports as `'-ABC`.
For exact source IDs in automated matching, use **Export as JSON**, which preserves
them unchanged. Numeric observation values remain numbers in series CSV files.

### **🛠️ For Technical Teams**

#### **Prerequisites**

- Node.js 26 and npm (Active LTS from October 28, 2026; version in `.nvmrc`)
  - With nvm: run `nvm install && nvm use` from the repository root.
  - CI reads `.nvmrc`; `node scripts/check-node-version.mjs` checks package engines and Docker images against it.
- Rust 1.99.1 and Cargo, using the 2024 edition (pinned in `rust-toolchain.toml`; rustup installs it automatically)
- PostgreSQL 18+ (recommended for production)
- Docker (for containerized deployment)

#### **🎯 Quick Start**

1. **Clone the repository**
   ```bash
   git clone https://github.com/EconGraph/econ-graph.git
   cd econ-graph
   ```

2. **Start the database**
   ```bash
   docker run -d --name econ-postgres \
     -e POSTGRES_USER=postgres \
     -e POSTGRES_PASSWORD=password \
     -e POSTGRES_DB=econ_graph \
     -p 127.0.0.1:5432:5432 postgres:18
   # Wait for "accepting connections" before launching the backend.
   docker exec econ-postgres pg_isready -U postgres -d econ_graph
   ```

3. **Launch the backend**
   ```bash
   cd backend
   # One local database is both the app and the data database.
   export APP_DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph
   export DATA_DATABASE_URL=$APP_DATABASE_URL
   # Apply the data migrations (the backend applies only the app migrations).
   cargo run -p econ-graph-crawler --bin crawler -- migrate
   cargo run -p econ-graph-backend --bin econ-graph-backend
   # Startup applies the app migrations; backend runs on http://localhost:9876
   ```

4. **Start the frontend in a second terminal** (from the repository root)
   ```bash
   cd frontend
   npm ci
   BACKEND_URL=http://localhost:9876 npm run dev
   # Frontend running on http://localhost:3000
   ```

5. **🎉 Open your browser** to `http://localhost:3000`

This starts an empty local database. The backend treats callers as anonymous while
`OIDC_ISSUER` is unset; protected operations require a configured identity provider.
Vite's development frontend defaults to the Keycloak realm at
`http://localhost:8081/realms/econ-graph`. Sign-in requires that realm to be reachable
and matching frontend (`VITE_OIDC_ISSUER`, `VITE_OIDC_CLIENT_ID`) and backend
(`OIDC_ISSUER`, `OIDC_AUDIENCE`, optionally `OIDC_JWKS_URL`) settings. See the
[backend development setup](backend/README.md#development-workflow) for configuration
and the [crawler deployment guide](docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md) to load data.
The explicit `BACKEND_URL` aligns Vite's proxy with the backend's default port.

---

## 📁 **Project Structure**

```
econ-graph/
├── 📚 docs/                 # Comprehensive documentation
│   ├── business/            # Privacy policy
│   ├── technical/           # Technical architecture and implementation
│   ├── deployment/          # Deployment and infrastructure guides
│   ├── development/         # Development workflow and CI/CD
│   ├── api/                 # API documentation and references
│   ├── testing/             # Testing strategies and reports
│   └── monitoring/          # Monitoring and observability setup
│
├── 🦀 backend/              # Rust backend with Warp + PostgreSQL
│   ├── crates/              # Rust workspace: core, services, GraphQL, auth, crawlers
│   │   └── econ-graph-backend/src/main.rs  # HTTP server and routes
│   ├── migrations/          # Database schema migrations
│   └── Cargo.toml           # Workspace members and shared dependencies
│
├── ⚛️ frontend/             # React frontend with TypeScript
│   ├── src/
│   │   ├── components/      # Reusable UI components
│   │   ├── pages/           # Application pages and routes
│   │   ├── hooks/           # Custom React hooks for data fetching
│   │   └── utils/           # Utility functions and GraphQL client
│   └── __tests__/           # Frontend test suites
│
├── 🏗️ terraform/           # Infrastructure as Code
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
- **[🏢 Business Documentation](./docs/business/)** - Privacy policy
- **[🔧 Technical Documentation](./docs/technical/)** - Architecture, implementation, APIs
- **[🚀 Deployment Guide](./docs/deployment/)** - Infrastructure and deployment procedures
- **[💻 Development Guide](./docs/development/)** - Setup, workflow, and CI/CD

### 🚀 **Kubernetes Development**

Choose the setup guide for your cluster type:

- **MicroK8s**: Follow the [MicroK8s deployment guide](docs/deployment/MICROK8S_DEPLOYMENT.md)
  for prerequisites, installation, required addons, and deployment with
  `scripts/deploy/restart-k8s-rollout.sh`. That script targets MicroK8s.
- **kind (Kubernetes in Docker)**: Follow the prerequisites and manual setup in the
  [kind deployment guide](k8s/README.md#prerequisites) to install the pinned kind
  version, create the cluster with Terraform, build images, and deploy with
  `scripts/deploy/deploy.sh`.

See the [Kubernetes deployment guide](docs/deployment/KUBERNETES_DEPLOYMENT.md)
for additional configuration and troubleshooting.

---

<a id="license--usage"></a>

## 📄 **License & Usage**

The [LICENSE](LICENSE) file is titled **Microsoft Reference Source License
(MS-RSL)**. Its Use Restrictions section states that the source is for reference
purposes only and prohibits commercial use, modification or derivative works,
distribution, copying except for viewing, use in a product or service, and
competitive reverse engineering or creation of competing products or services.
Read the full license for its grants, conditions, and restrictions.

For commercial use, enterprise licensing, or questions about usage rights,
contact **support@econgraph.com**.

---

## 🏆 **Acknowledgments**

- **Federal Reserve Economic Data (FRED)** for economic data APIs
- **Bureau of Labor Statistics** for additional data sources
- **Rust Community** for excellent async ecosystem
- **React Community** for modern frontend development patterns

---

<div align="center">

### 🎯 **Explore the economic data visualization platform**

**[🚀 Try the Live Demo](#getting-started)** • **[📚 Read the Code](https://github.com/EconGraph/econ-graph)**

---

**Built with Rust and React**

</div>
