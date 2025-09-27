# EconGraph Backend

The comprehensive backend system for the EconGraph economic data platform, providing a complete suite of services for data collection, processing, analysis, and API access. This backend serves as the foundation for economic data discovery, financial analysis, and collaborative research tools.

## Architecture Overview

The EconGraph backend is built as a modular Rust workspace with 9 specialized crates, each serving a specific domain within the economic data ecosystem:

```
econ-graph-backend/
├── econ-graph-core/          # Core data models and database schema
├── econ-graph-auth/          # Authentication and authorization
├── econ-graph-metrics/       # Metrics collection and monitoring
├── econ-graph-services/      # Business logic and data processing
├── econ-graph-graphql/       # GraphQL API with security features
├── econ-graph-crawler/       # General data acquisition and crawling
├── econ-graph-sec-crawler/   # SEC EDGAR XBRL financial data crawling
├── econ-graph-mcp/           # Model Context Protocol for AI integration
└── econ-graph-backend/       # Main application server and orchestration
```

## Core Components

### **econ-graph-core**
The foundational crate providing core data models, database schema, and shared utilities. All other crates depend on this for type-safe database operations and business logic.

**Key Features:**
- Economic and financial data models with type safety
- Database connection pooling and async operations
- Authentication models and JWT token handling
- Comprehensive error handling and logging
- Test utilities with database containers

### **econ-graph-auth**
Authentication and authorization system with enterprise-grade security features, OAuth integration, and comprehensive user management.

**Key Features:**
- OAuth integration (Google, GitHub, etc.)
- JWT authentication with configurable expiration
- Role-based access control and permissions
- Security middleware and session management
- User lifecycle management

### **econ-graph-metrics**
Comprehensive metrics collection and monitoring capabilities using Prometheus, providing insights into system performance and health.

**Key Features:**
- Crawler performance and error tracking
- Request duration histograms and throughput metrics
- Resource usage monitoring (bandwidth, data collection)
- Rate limiting and retry attempt tracking
- Centralized metrics registry

### **econ-graph-services**
Business logic and service layer providing data processing, external API integrations, and economic data discovery capabilities.

**Key Features:**
- Multi-source data discovery (FRED, BLS, Census, World Bank, etc.)
- Advanced web crawling with politeness and rate limiting
- Global economic analysis and intelligent search
- Asynchronous task processing and job scheduling
- User collaboration and data sharing features

### **econ-graph-graphql**
GraphQL API layer with enterprise-grade security features, performance optimization, and comprehensive monitoring.

**Key Features:**
- Type-safe GraphQL schema for economic data
- Security system with query complexity analysis and rate limiting
- DataLoader-based N+1 query prevention
- JWT-based authentication and role-based access control
- Comprehensive metrics and monitoring

### **econ-graph-crawler**
General data acquisition and crawling functionality with advanced features for systematic data collection from various sources.

**Key Features:**
- Multi-source crawling with intelligent rate limiting
- Robust retry mechanisms with exponential backoff
- Comprehensive data validation and quality checks
- Real-time progress monitoring and status reporting
- Error handling and recovery mechanisms

### **econ-graph-sec-crawler**
Specialized SEC EDGAR XBRL crawler for financial data acquisition with advanced parsing and analysis capabilities.

**Key Features:**
- SEC EDGAR filing crawling and XBRL data extraction
- Advanced XBRL document parsing and financial data extraction
- Automated financial ratio calculation and analysis
- Intelligent rate limiting respecting SEC policies
- Comprehensive data validation and quality assurance

### **econ-graph-mcp**
Model Context Protocol server implementation enabling AI model integration for economic data access through standardized protocols.

**Key Features:**
- Full MCP server implementation with economic data endpoints
- AI model integration and data access control
- Secure authentication for AI model access
- Intelligent data filtering and access control
- Protocol compliance and message handling

### **econ-graph-backend**
Main backend application providing server infrastructure, metrics collection, and comprehensive integration testing capabilities.

**Key Features:**
- Core HTTP server with routing and middleware
- Application performance and health monitoring
- Service orchestration and lifecycle management
- Comprehensive end-to-end testing capabilities
- Centralized configuration and environment handling

## Development Workflow

### **Getting Started**

```bash
# Install dependencies
cargo build

# Run tests
cargo test

# Run specific crate tests
cargo test -p econ-graph-core
cargo test -p econ-graph-services

# Run integration tests
cargo test --test integration
```

### **Database Setup**

```bash
# Start PostgreSQL (using Docker)
docker run -d --name econ-graph-db -p 5432:5432 -e POSTGRES_PASSWORD=password postgres:15

# Run migrations
cd backend
cargo run --bin econ-graph-backend -- --migrate

# Or use diesel CLI
diesel migration run
```

### **Environment Configuration**

```bash
# Copy environment template
cp .env.example .env

# Configure database connection
DATABASE_URL=postgresql://username:password@localhost/econ_graph

# Configure authentication
JWT_SECRET=your-secret-key
GOOGLE_CLIENT_ID=your-google-client-id
GOOGLE_CLIENT_SECRET=your-google-client-secret
```

## Testing Strategy

The backend employs a comprehensive testing strategy across all crates:

### **Test Types**
- **Unit Tests**: Fast, isolated component testing
- **Integration Tests**: Real database and external service testing
- **Security Tests**: Authentication, authorization, and attack prevention
- **Performance Tests**: Load testing and performance validation
- **End-to-End Tests**: Complete workflow validation

### **Test Infrastructure**
- **Database Testing**: TestContainer-based PostgreSQL instances
- **External API Mocking**: Controlled testing of external integrations
- **Security Testing**: Automated security measure validation
- **Performance Monitoring**: Automated performance testing and benchmarking

## API Documentation

### **GraphQL API**
The primary API is exposed through GraphQL with comprehensive schema documentation:

```graphql
# Query economic data
query GetEconomicSeries($symbol: String!) {
  economicSeries(symbol: $symbol) {
    id
    name
    dataPoints {
      date
      value
    }
  }
}

# Search for companies
query SearchCompanies($query: String!) {
  searchCompanies(query: $query) {
    id
    name
    ticker
  }
}
```

### **REST Endpoints**
Additional REST endpoints for specific functionality:

- `GET /health` - System health check
- `GET /metrics` - Prometheus metrics
- `POST /auth/login` - User authentication
- `GET /api/v1/series` - Economic data series

## Monitoring and Observability

### **Metrics Collection**
- **Application Metrics**: Request rates, response times, error rates
- **Business Metrics**: Data collection rates, user activity, search queries
- **Infrastructure Metrics**: Database performance, memory usage, CPU utilization

### **Health Monitoring**
- **System Health**: Database connectivity, external API availability
- **Service Health**: Individual service status and performance
- **Alerting**: Automated alerts for critical issues and performance degradation

## Security Features

### **Authentication & Authorization**
- OAuth 2.0 integration with major providers
- JWT-based stateless authentication
- Role-based access control (RBAC)
- Session management and token refresh

### **API Security**
- GraphQL query complexity analysis
- Rate limiting and DDoS protection
- Input validation and sanitization
- CORS and security headers

### **Data Security**
- Encrypted data transmission (TLS)
- Secure database connections
- API key management and rotation
- Audit logging

## Deployment and Operations

### **Docker Support**
```bash
# Build backend image
docker build -t econ-graph-backend .

# Run with Docker Compose
docker-compose up -d
```

### **Kubernetes Deployment**
```bash
# Deploy to Kubernetes
kubectl apply -f k8s/manifests/

# Monitor deployment
kubectl get pods -l app=econ-graph-backend
```

### **Database Migrations**
```bash
# Run migrations
cargo run --bin econ-graph-backend -- --migrate

# Rollback migrations
diesel migration redo
```

## Contributing

### **Code Standards**
- **Rust**: Follow Rust best practices and clippy recommendations
- **Documentation**: Comprehensive documentation for all public APIs
- **Testing**: Maintain high test coverage with meaningful tests
- **Security**: Security-first development with regular audits

### **Development Process**
1. **Feature Development**: Create feature branches from main
2. **Testing**: Write comprehensive tests for all functionality
3. **Documentation**: Update documentation for any API changes
4. **Code Review**: All changes require peer review
5. **Integration**: Automated testing and deployment validation

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL). See the LICENSE file for complete terms and conditions.

## Support

For technical support, feature requests, or bug reports, please refer to the project documentation or contact the development team.
