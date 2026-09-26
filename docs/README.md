# 📚 EconGraph Documentation

Welcome to the comprehensive documentation for EconGraph - the economic data visualization platform.

## 🗺️ Roadmap

**[Roadmap index](./roadmap/README.md)** - Current topic roadmaps, what is built today, open work, and the status of every older plan doc

## 📖 Documentation Structure

### 🏢 [Business Documentation](./business/)
- **[Investor Pitch](./business/INVESTOR_PITCH.md)** - Complete investor presentation and business case
- **[Product Roadmap](./business/ROADMAP.md)** - Product feature phases (see the [roadmap index](./roadmap/README.md) for current status)
- **[Privacy Policy](./business/PRIVACY_POLICY.md)** - Data privacy and protection policies

### 🔧 [Technical Documentation](./technical/)
- **[Admin Security](./technical/ADMIN_SECURITY.md)** - Security configuration and best practices
- **[Frontend Summary](./technical/FRONTEND_SUMMARY.md)** - React frontend architecture overview
- **[Full-text Search](./technical/FULLTEXT_SEARCH.md)** - Search implementation details
- **[MCP Server](./technical/MCP_SERVER.md)** - Model Context Protocol server documentation
- **[MCP CI Pipeline](./technical/MCP_CI_PIPELINE.md)** - CI/CD pipeline for MCP server
- **[Global Analysis Architecture](./technical/GLOBAL_ANALYSIS_ARCHITECTURE.md)** - Technical architecture for global analysis features
- **[Global Analysis Features](./technical/GLOBAL_ANALYSIS_FEATURES.md)** - Comprehensive feature documentation for global analysis
- **[Global Analysis API](./technical/GLOBAL_ANALYSIS_API.md)** - API documentation for global analysis endpoints

#### Backend Technical Docs
- **[BLS API Experimental Findings](./technical/BLS_API_EXPERIMENTAL_FINDINGS.md)** - Bureau of Labor Statistics API research
- **[Census BDS Integration](./technical/CENSUS_BDS_INTEGRATION.md)** - Census Business Dynamics Statistics integration
- **[Census Bureau Integration Summary](./technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md)** - Census API integration overview
- **[Crawler Deployment Guide](./technical/CRAWLER_DEPLOYMENT_GUIDE.md)** - Crawl queue, `crawler` CLI and `crawler-worker` deployment
- **[Crawler Politeness](./technical/CRAWLER_POLITENESS.md)** - Per-source rate limits, retries and tuning
- **[Developer Guide Database](./technical/DEVELOPER_GUIDE_DATABASE.md)** - Database development guidelines
- **[Rate Limit Sources](./technical/RATE_LIMIT_SOURCES.md)** - API rate limiting documentation
- **[Testing Strategy](./technical/TESTING_STRATEGY.md)** - Comprehensive testing approach
- **[World Bank API Experimental Findings](./technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)** - World Bank API research

### 🚀 [Deployment Documentation](./deployment/)
- **[Deployment Restart](./deployment/DEPLOYMENT_RESTART.md)** - Deployment restart procedures
- **[K8s Restart Commands](./deployment/K8S_RESTART_COMMANDS.md)** - Kubernetes restart commands
- **[Chart API Service](./deployment/CHART_API_SERVICE.md)** - Chart API service deployment

### 💻 [Development Documentation](./development/)
- **[Pre-commit Setup](./development/PRECOMMIT_SETUP.md)** - Pre-commit hooks configuration
- **[Vibe Coding](./development/VIBE_CODING.md)** - Development workflow and standards
- **[CI/CD Pipeline](./development/CI_CD_PIPELINE.md)** - Continuous integration and deployment
- **[CI Optimization Notes](./development/CI_OPTIMIZATION_NOTES.md)** - CI performance optimization
- **[Global Analysis Roadmap](./development/GLOBAL_ANALYSIS_ROADMAP.md)** - Comprehensive global analysis feature roadmap

### 🔌 [API Documentation](./api/)
- **[GraphQL API](./api/GRAPHQL_API.md)** - GraphQL API reference and schema

### 🧪 [Testing Documentation](./testing/)
- **[Database Testing](./testing/DATABASE_TESTING.md)** - Database testing strategies and procedures
- **[E2E Test Report](./testing/e2e-test-report.md)** - End-to-end testing results and analysis

### 📊 [Monitoring Documentation](./monitoring/)
- **[Monitoring README](./monitoring/README.md)** - Monitoring setup and configuration
- **[Grafana Dashboards](./monitoring/README.md)** - Grafana dashboard documentation

### 👥 [User Guides](./user-guides/)
- **[Global Analysis User Guide](./user-guides/GLOBAL_ANALYSIS_USER_GUIDE.md)** - Comprehensive user guide for global analysis features

## 🎯 Quick Navigation

### For Developers
- Start with [Development Documentation](./development/) for setup and workflow
- Check [Technical Documentation](./technical/) for architecture details
- Review [Testing Documentation](./testing/) for testing procedures

### For DevOps/Deployment
- See [Deployment Documentation](./deployment/) for deployment procedures
- Check [Monitoring Documentation](./monitoring/) for observability setup

### For Business/Investors
- Review [Business Documentation](./business/) for business case and roadmap
- Check [Investor Pitch](./business/INVESTOR_PITCH.md) for investment details

### For API Users
- See [API Documentation](./api/) for GraphQL API reference
- Check [Global Analysis API](./technical/GLOBAL_ANALYSIS_API.md) for global analysis endpoints

### For End Users
- Start with [User Guides](./user-guides/) for feature usage
- Check [Global Analysis User Guide](./user-guides/GLOBAL_ANALYSIS_USER_GUIDE.md) for interactive map features

### For Security
- Review the [Security Assessment Report](./security/FINAL_COMPREHENSIVE_SECURITY_ASSESSMENT_REPORT.md) for vulnerability analysis
- See [Security Implementation Plan](./projects/SECURITY_IMPLEMENTATION_PLAN.md) for remediation strategy
- Check [Admin Security](./technical/ADMIN_SECURITY.md) for admin interface security

## 🔐 Secrets Management

EconGraph uses a secure secrets management approach:

- **Secrets Repository**: Private repository at `https://github.com/jmalicki/econ-graph-secrets`
- **Encryption Method**: Bitnami Sealed Secrets for Kubernetes
- **Access Control**: Private repository with team-based permissions
- **Integration**: Git submodule in `k8s/secrets/` directory
- **Deployment**: Integrated with deployment scripts in `scripts/deploy/`

For more details, see the [Security Implementation Plan](./projects/SECURITY_IMPLEMENTATION_PLAN.md).

## 📝 Contributing to Documentation

When adding new documentation:

1. **Choose the right category** - Place docs in the most appropriate subdirectory
2. **Follow naming conventions** - Use UPPERCASE_WITH_UNDERSCORES.md for consistency
3. **Update this index** - Add new docs to the appropriate section above
4. **Cross-reference** - Link related documents where appropriate

## 🔍 Finding Documentation

- **By Topic**: Use the category structure above
- **By File**: All docs are in the `docs/` directory with logical subdirectories
- **By Purpose**: Business, Technical, Deployment, Development, API, Testing, Monitoring

---

*This documentation structure provides organized access to all EconGraph documentation. Each category contains relevant documents for different audiences and use cases.*
