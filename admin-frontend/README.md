# EconGraph Admin Interface

## Overview

The EconGraph Admin Interface is a secure, role-based administrative dashboard that provides comprehensive system management capabilities for the EconGraph platform. It integrates with our existing monitoring infrastructure and provides a unified interface for system administration.

## Architecture

### Core Components

```
admin-frontend/
├── src/
│   ├── components/
│   │   └── layout/
│   │       └── AdminLayout.tsx          # Main layout with navigation
│   ├── contexts/
│   │   ├── AuthContext.tsx             # Authentication context
│   │   └── SecurityContext.tsx         # Security and session management
│   ├── pages/
│   │   ├── MonitoringPage.tsx          # Grafana dashboard integration
│   │   ├── SystemHealthPage.tsx        # System health metrics
│   │   └── UserManagementPage.tsx      # User administration
│   └── __tests__/                      # Comprehensive test suite
```

### Integration with Existing Infrastructure

The admin interface leverages our existing monitoring and security infrastructure:

- **Grafana Dashboards**: Direct integration with our pre-configured dashboards
- **Prometheus Metrics**: Real-time system metrics and alerting
- **Authentication System**: Built on our existing auth infrastructure
- **Security Context**: Comprehensive audit logging and session management

## Features

### 1. AdminLayout Component

**Purpose**: Provides consistent admin interface layout with role-based navigation and security indicators.

**Key Features**:
- Role-based navigation menu with access control
- Security warnings and session time display
- Real-time security event notifications
- Responsive design with mobile support
- User profile management and logout functionality

**Navigation Items**:
- **Dashboard**: System overview (read_only+)
- **System Health**: Health metrics and service status (read_only+)
- **Monitoring**: Grafana dashboard integration (read_only+)
- **Crawler Management**: Data crawler administration (admin+)
- **Database Management**: Database administration (super_admin+)
- **User Management**: User administration (super_admin+)
- **Security**: Security monitoring and audit (admin+)
- **Audit Logs**: System audit trail (read_only+)
- **System Config**: System configuration (super_admin+)

### 2. MonitoringPage Component

**Purpose**: Integrates with our existing Grafana dashboards to provide comprehensive system monitoring.

**Integration Points**:
- **EconGraph Platform Overview**: `http://localhost:30001/d/econgraph-overview/econgraph-platform-overview`
- **Database Statistics**: `http://localhost:30001/d/database-statistics/database-statistics`
- **Crawler Status**: `http://localhost:30001/d/crawler-status/crawler-status`

**Features**:
- Direct links to Grafana dashboards
- Embedded dashboard views (when available)
- System status overview with aggregated metrics
- Service health indicators
- Real-time alert notifications

### 3. SystemHealthPage Component

**Purpose**: Provides immediate system health visibility with links to detailed Grafana dashboards.

**Health Metrics**:
- System uptime and availability
- API response times
- Database connection status
- Memory and disk utilization
- Active user counts

**Service Monitoring**:
- Backend API service status
- PostgreSQL database health
- Data crawler status
- Grafana monitoring service
- NGINX proxy status

**Quick Actions**:
- Direct links to specific Grafana dashboards
- Platform overview access
- Performance metrics
- Crawler status monitoring
- Security event viewing

### 4. UserManagementPage Component

**Purpose**: Comprehensive user administration interface for super_admin role.

**User Management Features**:
- View all registered users
- Edit user roles and permissions
- Suspend/activate user accounts
- Delete user accounts
- View active sessions
- Force user logout

**Session Management**:
- Real-time online user tracking
- Session information (IP, user agent, duration)
- Force logout capabilities
- Session timeout management

**Search and Filtering**:
- Search users by name or email
- Filter by role (read_only, admin, super_admin)
- Filter by status (active, inactive, suspended)
- Combined search and filter capabilities

## Security Features

### Role-Based Access Control (RBAC)

**Role Hierarchy**:
- **read_only**: View-only access to dashboards and logs
- **admin**: Full system monitoring and crawler management
- **super_admin**: Complete system administration including user management

**Access Control Implementation**:
```typescript
const roleHierarchy = {
  'read_only': ['read_only'],
  'admin': ['read_only', 'admin'],
  'super_admin': ['read_only', 'admin', 'super_admin'],
};
```

### Security Context

**Features**:
- Session timeout management
- Security event tracking
- Access control validation
- Audit logging integration

**Security Events**:
- Failed login attempts
- Permission denied events
- Suspicious activity detection
- Session timeout warnings

### Audit Logging

All administrative actions are logged with:
- User identification
- Action performed
- Timestamp
- IP address
- User agent
- Result status

## Integration with Existing Systems

### Grafana Integration

The admin interface seamlessly integrates with our existing Grafana monitoring stack:

**Dashboard URLs**:
- Platform Overview: `/d/econgraph-overview/econgraph-platform-overview`
- Database Statistics: `/d/database-statistics/database-statistics`
- Crawler Status: `/d/crawler-status/crawler-status`

**Embedded Views**:
- Real-time dashboard panels
- Direct links to full Grafana interface
- Custom time range filtering
- Metric-specific deep links

### Prometheus Metrics Integration

**System Metrics**:
- Service availability (`up{job="econgraph-backend"}`)
- API response times (`http_request_duration_seconds`)
- Database connections (`pg_stat_activity_count`)
- Resource utilization (`container_memory_usage_bytes`)

**Custom Metrics**:
- Queue processing rates (`econgraph_queue_items`)
- Crawler performance (`econgraph_crawl_errors_total`)
- User activity (`econgraph_active_users`)

### Authentication Integration

**Backend Integration**:
- JWT token validation
- Role-based permission checking
- Session management
- Secure logout handling

**Security Features**:
- Automatic session timeout
- Secure token storage
- CSRF protection
- XSS prevention

## Development and Testing

### Test Coverage

Comprehensive test suite covering:
- Component rendering and behavior
- Role-based access control
- User interactions and form handling
- Integration with Grafana dashboards
- Security context functionality
- Error handling and edge cases

### Test Structure

```
__tests__/
├── AdminLayout.test.tsx           # Layout and navigation tests
├── MonitoringPage.test.tsx        # Grafana integration tests
├── SystemHealthPage.test.tsx      # Health metrics tests
└── UserManagementPage.test.tsx    # User management tests
```

### Testing Approach

- **Unit Tests**: Individual component testing
- **Integration Tests**: Context and service integration
- **Access Control Tests**: Role-based permission validation
- **UI Tests**: User interaction and form handling
- **Mock Data**: Realistic test data matching production scenarios

## Deployment and Configuration

### Environment Configuration

**Required Environment Variables**:
- `REACT_APP_API_URL`: Backend API endpoint
- `REACT_APP_GRAFANA_URL`: Grafana dashboard URL
- `REACT_APP_ADMIN_PORT`: Admin interface port

### Build Process

```bash
# Install dependencies
npm install

# Run tests
npm test

# Build for production
npm run build

# Start development server
npm start
```

### Docker Integration

The admin interface is containerized and deployed alongside the main application:

```dockerfile
FROM node:18-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production

FROM nginx:alpine
COPY --from=builder /app/build /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 3000
```

## Security Considerations

### Access Control

- All routes protected by role-based middleware
- Session timeout enforcement
- Secure token handling
- CSRF protection

### Data Protection

- No sensitive data stored in frontend
- Secure API communication
- Input validation and sanitization
- XSS prevention

### Audit Requirements

- All admin actions logged
- User session tracking
- Security event monitoring
- Compliance reporting

## Monitoring and Observability

### Health Checks

- Component rendering status
- API connectivity
- Grafana dashboard availability
- User session validity

### Metrics Collection

- Admin interface usage statistics
- User activity patterns
- Performance metrics
- Error rates and types

### Alerting

- Admin interface downtime
- Authentication failures
- Permission violations
- System health degradation

## Future Enhancements

### Planned Features

1. **Advanced User Management**:
   - Bulk user operations
   - User group management
   - Advanced permission models

2. **Enhanced Monitoring**:
   - Custom dashboard creation
   - Alert configuration
   - Performance optimization

3. **System Administration**:
   - Configuration management
   - Backup and recovery
   - System maintenance tools

4. **Security Enhancements**:
   - Multi-factor authentication
   - Advanced audit logging
   - Security policy management

### Integration Opportunities

- **CI/CD Pipeline Integration**: Automated testing and deployment
- **External Monitoring**: Integration with external monitoring services
- **API Management**: Advanced API monitoring and management
- **Compliance Tools**: Integration with compliance and audit tools

## Troubleshooting

### Common Issues

1. **Authentication Failures**:
   - Check JWT token validity
   - Verify backend API connectivity
   - Validate user permissions

2. **Grafana Integration Issues**:
   - Verify Grafana service availability
   - Check dashboard URLs and IDs
   - Validate CORS configuration

3. **Performance Issues**:
   - Monitor component rendering
   - Check API response times
   - Validate resource utilization

### Debug Mode

Enable debug logging by setting:
```bash
REACT_APP_DEBUG=true
```

This provides detailed logging for:
- Authentication flow
- API requests and responses
- Component rendering
- Error handling

## Contributing

### Development Guidelines

1. **Code Style**: Follow existing patterns and ESLint configuration
2. **Testing**: Maintain comprehensive test coverage
3. **Documentation**: Update documentation for new features
4. **Security**: Follow security best practices
5. **Performance**: Optimize for performance and user experience

### Pull Request Process

1. Create feature branch from main
2. Implement changes with tests
3. Update documentation
4. Run full test suite
5. Submit pull request with detailed description

---

This admin interface provides a comprehensive, secure, and user-friendly way to manage the EconGraph platform while leveraging our existing monitoring and security infrastructure.
