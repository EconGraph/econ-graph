# EconGraph Backend

The comprehensive backend system for the EconGraph economic data platform, providing a complete suite of services for data collection, processing, analysis, and API access. This backend serves as the foundation for economic data discovery, financial analysis, and collaborative research tools.

## Architecture Overview

The EconGraph backend is built as a modular Rust workspace with 11 specialized crates, each serving a specific domain within the economic data ecosystem:

```
backend/crates/
├── econ-graph-core/          # Core data models and database schema
├── econ-graph-auth/          # Authentication and authorization
├── econ-graph-metrics/       # Metrics collection and monitoring
├── econ-graph-services/      # Business logic (search, series, analysis, queue stats)
├── econ-graph-graphql/       # GraphQL API with security features
├── econ-graph-crawler/       # Source adapters, crawl_queue worker, `crawler` CLI
├── econ-graph-crawler-worker/ # Deployed `crawler-worker` binary (worker + SEC handler)
├── econ-graph-sec-crawler/   # SEC EDGAR XBRL financial data crawling
├── econ-graph-mcp/           # Model Context Protocol for AI integration
├── econ-graph-backend/       # Main application server and orchestration
└── econ-graph-flags-build/   # Compile-time feature flag support
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
Business logic and service layer used by the GraphQL and MCP APIs.

**Key Features:**
- Global economic analysis and intelligent search
- Series queries
- `crawl_queue` statistics
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
All data collection goes through the Postgres `crawl_queue`: jobs are enqueued (GraphQL
`triggerCrawl`, or `crawler enqueue|discover`), and `crawler-worker` drains the queue.

**Key Features:**
- One source adapter per provider (FRED, BLS, BEA, Census, World Bank, FHFA)
- Shared `HttpFetcher`: per-source rate limits and concurrency, timeouts, retries honouring `Retry-After`
- Queue worker with retry/fail by error kind and per-source pause
- `crawler discover|enqueue|status|sources|fetch` operator CLI

See the [crawler README](crates/econ-graph-crawler/README.md) and
[crawler deployment guide](../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md).

### **econ-graph-sec-crawler**
Specialized SEC EDGAR XBRL crawler for financial data acquisition with advanced parsing and analysis capabilities.

**Key Features:**
- SEC EDGAR filing crawling and XBRL data extraction
- Advanced XBRL document parsing and financial data extraction (behind the `xbrl-parser` cargo feature)
- Automated financial ratio calculation and analysis (behind the `xbrl-parser` cargo feature)
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
- Core HTTP server with routing and middleware (warp routes served by hyper, HTTP/1.1 only; HTTP/2 is refused on every route, including the NodePort)
- Application performance and health monitoring
- Service orchestration and lifecycle management
- Comprehensive end-to-end testing capabilities
- Centralized configuration and environment handling

## Development Workflow

### **Local database and server**

Run these commands from the repository root. Docker must be running.

```bash
docker run -d --name econ-graph-db \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=econ_graph \
  -p 127.0.0.1:5432:5432 postgres:18

# Wait until this reports "accepting connections" before starting the server.
docker exec econ-graph-db pg_isready -U postgres -d econ_graph

cd backend
export DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph
cargo run -p econ-graph-backend --bin econ-graph-backend
```

The server applies pending migrations during startup and listens on port **9876**
unless `BACKEND_PORT` overrides it. Verify startup with
`curl --fail "http://localhost:${BACKEND_PORT:-9876}/health"`. The database is initially empty;
see the [crawler deployment guide](../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md)
for data loading.

For the public frontend, open a second terminal at the repository root:

```bash
cd frontend
npm ci
BACKEND_URL=http://localhost:9876 npm run dev
```

Vite serves the frontend at `http://localhost:3000`. Its explicit `BACKEND_URL`
override aligns the proxy with the backend's default port.

### **Environment Configuration**

The backend reads exported variables and optionally loads a `.env` file.
There is no checked-in `.env.example`; the exported `DATABASE_URL` above is sufficient
for the local anonymous server.

To enable sign-in, first configure a reachable Keycloak realm and API audience,
then export these settings before starting the backend:

```bash
export OIDC_ISSUER=http://localhost:8081/realms/econ-graph
export OIDC_AUDIENCE=econ-graph-api
export OIDC_JWKS_URL=http://localhost:8081/realms/econ-graph/protocol/openid-connect/certs
```

These URLs are example identity-provider settings, not a Keycloak installation.
The backend verifies provider access tokens and does not issue its own.
With `OIDC_ISSUER` unset, callers are anonymous and protected operations remain
unavailable.

### **Build and test**

Run from `backend/` with the `econ-graph-db` container above running and Docker
available. Create a separate, disposable test database once. Database-backed tests
**drop and recreate its public schema**, so never point them at application data.
The subshell below keeps the application's exported `DATABASE_URL` unchanged.

```bash
cargo build --workspace
# One-time creation; skip this command if econ_graph_test already exists.
docker exec econ-graph-db createdb -U postgres econ_graph_test
(
  export DATABASE_URL=postgresql://postgres:password@localhost:5432/econ_graph_test
  cargo test --workspace
  cargo test -p econ-graph-core
  cargo test -p econ-graph-services
)
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

Enable the explorer with `ENABLE_GRAPHQL_PLAYGROUND=true` when starting the server,
then visit `http://localhost:9876/playground` to inspect the current schema.

A minimal connectivity query:

```graphql
query {
  __typename
}
```

See the [GraphQL API reference](../docs/api/GRAPHQL_API.md) for data queries.


### **REST Endpoints**
Additional REST endpoints for specific functionality:

- `GET /health` - System health check
- `GET /metrics` - Prometheus metrics
- `GET /playground` - GraphQL explorer, only when `ENABLE_GRAPHQL_PLAYGROUND=true`

Economic series queries use `POST /graphql`; authentication uses identity-provider
access tokens. This server does not expose `/auth/login` or `/api/v1/series`.

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

# Run the repository's Compose configuration (from backend/)
docker compose -f ../docker-compose.yml up -d
```

### **Kubernetes Deployment**
```bash
# Deploy to Kubernetes (from backend/)
kubectl apply -f ../k8s/manifests/

# Monitor deployment
kubectl get pods -l app=econ-graph-backend
```

### **Database Migrations**

From `backend/`, with `DATABASE_URL` exported as above. Normal server startup applies
pending migrations automatically; there is no `--migrate` server mode.

```bash
# Apply pending migrations without starting the server (requires diesel CLI)
diesel migration run

# Undo the latest migration (destructive; use only on disposable development data)
diesel migration revert
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


