# GraphQL Security Guide

## Overview

This document provides comprehensive security guidance for the EconGraph GraphQL API. It covers security best practices, implementation details, and operational procedures to ensure the API remains secure and resilient against various attack vectors.

## Table of Contents

1. [Security Architecture](#security-architecture)
2. [Authentication & Authorization](#authentication--authorization)
3. [Query Security](#query-security)
4. [Input Validation](#input-validation)
5. [Rate Limiting](#rate-limiting)
6. [Monitoring & Alerting](#monitoring--alerting)
7. [Security Configuration](#security-configuration)
8. [Incident Response](#incident-response)
9. [Best Practices](#best-practices)
10. [Security Checklist](#security-checklist)

## Security Architecture

### Defense in Depth

The GraphQL API implements a multi-layered security approach:

```
┌─────────────────────────────────────────────────────────────┐
│                    Client Request                           │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│                 Rate Limiting                               │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│              Input Validation                               │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│            Query Analysis & Filtering                       │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│            Authentication & Authorization                   │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│              Query Execution                                │
└─────────────────────┬───────────────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────┐
│              Response & Logging                             │
└─────────────────────────────────────────────────────────────┘
```

### Security Components

1. **Query Complexity Analysis**: Prevents expensive queries
2. **Query Depth Limiting**: Prevents deeply nested queries
3. **Rate Limiting**: Prevents abuse and DoS attacks
4. **Input Validation**: Prevents injection attacks
5. **Authentication**: Verifies user identity
6. **Authorization**: Controls access to resources
7. **Monitoring**: Tracks security events and anomalies
8. **Alerting**: Notifies of potential threats

## Authentication & Authorization

### Authentication

The API uses JWT-based authentication with the following features:

- **Token Validation**: Signature verification and expiration checking
- **Secure Storage**: Tokens are not stored in localStorage
- **Token Rotation**: Regular token refresh mechanism
- **Multi-Factor Authentication**: Support for MFA (planned)

### Authorization

Role-based access control (RBAC) with fine-grained permissions:

#### User Roles

1. **Super Admin**: Full system access
2. **Admin**: Administrative functions and user management
3. **Analyst**: Data analysis and chart creation
4. **Viewer**: Read-only access to economic data
5. **Guest**: Limited read access

#### Permissions

```rust
pub enum Permission {
    // Data access permissions
    ReadEconomicData,
    WriteEconomicData,
    DeleteEconomicData,
    
    // User management permissions
    ReadUsers,
    CreateUsers,
    UpdateUsers,
    DeleteUsers,
    
    // System administration permissions
    ReadSystemConfig,
    UpdateSystemConfig,
    ManageCrawlers,
    ViewLogs,
    ManageSecurity,
    
    // Chart and annotation permissions
    CreateCharts,
    UpdateCharts,
    DeleteCharts,
    ShareCharts,
    CreateAnnotations,
    UpdateAnnotations,
    DeleteAnnotations,
    
    // API access permissions
    AccessGraphQL,
    AccessREST,
    AccessMCP,
    
    // Monitoring permissions
    ViewMetrics,
    ViewHealth,
    ViewSecurityEvents,
}
```

### Authorization Examples

```rust
// Require specific permission
ctx.require_permission(&Permission::ReadEconomicData)?;

// Require any of multiple permissions
ctx.require_any_permission(&[
    Permission::CreateCharts,
    Permission::UpdateCharts,
])?;

// Require all permissions
ctx.require_all_permissions(&[
    Permission::ReadEconomicData,
    Permission::CreateCharts,
])?;
```

## Query Security

### Query Complexity Analysis

Prevents expensive queries that could overload the system:

```rust
// Configuration
let config = SecurityConfig {
    max_complexity: 1000,
    // ... other settings
};

// Field complexity values
field_complexity.insert("dataPoints".to_string(), 10);
field_complexity.insert("series".to_string(), 5);
field_complexity.insert("users".to_string(), 3);
```

### Query Depth Limiting

Prevents deeply nested queries that could cause stack overflow:

```rust
// Configuration
let config = SecurityConfig {
    max_depth: 10,
    // ... other settings
};
```

### Query Size Limiting

Prevents extremely large queries:

```rust
// Configuration
let config = SecurityConfig {
    max_query_size: 10000, // 10KB
    // ... other settings
};
```

### Introspection Protection

Controls access to schema introspection:

```rust
// Configuration
let config = SecurityConfig {
    protect_introspection: true,
    // ... other settings
};
```

### Query Filtering

Whitelist/blacklist specific query patterns:

```rust
// Configuration
let config = SecurityConfig {
    query_filter: QueryFilterConfig {
        enable_blacklist: true,
        blacklist_patterns: vec![
            "__schema".to_string(),
            "__type".to_string(),
            "system".to_string(),
            "admin".to_string(),
        ],
        // ... other settings
    },
    // ... other settings
};
```

## Input Validation

### String Validation

```rust
// Validate string input
let validator = InputValidator::new(ValidationConfig::default());
let validated = validator.validate_string(input, "field_name")?;
```

### Email Validation

```rust
// Validate email format
let validated = validator.validate_with_rule(email, "email")?;
```

### UUID Validation

```rust
// Validate UUID format
let validated = validator.validate_with_rule(uuid, "uuid")?;
```

### Custom Validation Rules

```rust
// Add custom validation rule
let rule = ValidationRule {
    name: "custom_rule".to_string(),
    pattern: Some(r"^[A-Z]+$".to_string()),
    error_message: "Only uppercase letters allowed".to_string(),
    enabled: true,
    // ... other settings
};
validator.add_rule(rule);
```

### Sanitization

Automatic sanitization of inputs:

- **HTML Sanitization**: Prevents XSS attacks
- **SQL Injection Prevention**: Blocks SQL injection attempts
- **XSS Prevention**: Removes malicious scripts

## Rate Limiting

### Configuration

```rust
let config = SecurityConfig {
    rate_limit: RateLimitConfig {
        requests_per_minute: 60,
        requests_per_hour: 1000,
        requests_per_day: 10000,
        enabled: true,
    },
    // ... other settings
};
```

### Rate Limit Status

```rust
// Check rate limit status
let status = rate_limiter.get_rate_limit_status("127.0.0.1").await;
if status.is_exceeded() {
    return Err("Rate limit exceeded");
}
```

### Rate Limit Reset

```rust
// Reset rate limits for a client
rate_limiter.reset_rate_limit("127.0.0.1").await;
```

## Monitoring & Alerting

### Security Events

The system tracks various security events:

```rust
pub enum SecurityEvent {
    RateLimitExceeded { client_ip: String, requests_per_minute: u32, timestamp: DateTime<Utc> },
    ComplexityExceeded { client_ip: String, complexity: u32, max_complexity: u32, query: String, timestamp: DateTime<Utc> },
    DepthExceeded { client_ip: String, depth: u32, max_depth: u32, query: String, timestamp: DateTime<Utc> },
    QuerySizeExceeded { client_ip: String, size: usize, max_size: usize, timestamp: DateTime<Utc> },
    IntrospectionBlocked { client_ip: String, query: String, timestamp: DateTime<Utc> },
    QueryFiltered { client_ip: String, query: String, reason: String, timestamp: DateTime<Utc> },
}
```

### Alert Configuration

```rust
let config = MonitoringConfig {
    alert_thresholds: AlertThresholds {
        rate_limit_violations_per_minute: 10,
        complexity_violations_per_hour: 50,
        auth_failures_per_hour: 20,
        authz_violations_per_hour: 30,
        suspicious_queries_per_hour: 25,
        system_errors_per_hour: 100,
    },
    notifications: NotificationConfig {
        enable_email: true,
        enable_webhook: true,
        enable_slack: true,
        email_recipients: vec!["security@example.com".to_string()],
        webhook_urls: vec!["https://hooks.slack.com/services/...".to_string()],
        cooldown_period: 300, // 5 minutes
    },
    // ... other settings
};
```

### Security Metrics

```rust
// Get security metrics
let metrics = monitor.get_security_metrics().await;
println!("Rate limited requests: {}", metrics.rate_limited_requests);
println!("Complexity blocked requests: {}", metrics.complexity_blocked_requests);
println!("Suspicious queries: {}", metrics.filtered_requests);
```

## Security Configuration

### Default Configuration

```rust
let config = SecurityConfig {
    max_complexity: 1000,
    max_depth: 10,
    max_query_size: 10000,
    query_timeout: 30,
    protect_introspection: true,
    rate_limit: RateLimitConfig {
        requests_per_minute: 60,
        requests_per_hour: 1000,
        requests_per_day: 10000,
        enabled: true,
    },
    query_filter: QueryFilterConfig {
        enable_whitelist: false,
        enable_blacklist: true,
        whitelist_patterns: Vec::new(),
        blacklist_patterns: vec![
            "__schema".to_string(),
            "__type".to_string(),
            "__typename".to_string(),
        ],
    },
};
```

### Environment-Specific Configuration

#### Development

```rust
let config = SecurityConfig {
    max_complexity: 2000, // More lenient for development
    max_depth: 15,
    protect_introspection: false, // Allow introspection in dev
    rate_limit: RateLimitConfig {
        requests_per_minute: 120, // Higher limits for development
        enabled: false, // Disable rate limiting in dev
    },
    // ... other settings
};
```

#### Production

```rust
let config = SecurityConfig {
    max_complexity: 500, // Stricter limits for production
    max_depth: 8,
    protect_introspection: true,
    rate_limit: RateLimitConfig {
        requests_per_minute: 30, // Lower limits for production
        enabled: true,
    },
    // ... other settings
};
```

## Incident Response

### Security Incident Types

1. **Rate Limit Violations**: Excessive request frequency
2. **Complexity Attacks**: Malicious complex queries
3. **Authentication Failures**: Failed login attempts
4. **Authorization Violations**: Unauthorized access attempts
5. **Suspicious Queries**: Potentially malicious query patterns
6. **System Errors**: Unexpected system behavior

### Response Procedures

#### 1. Detection

- Automated monitoring detects security events
- Alerts are triggered based on configured thresholds
- Security team is notified via multiple channels

#### 2. Assessment

- Analyze the security event
- Determine severity and impact
- Identify the source and scope

#### 3. Containment

- Block malicious IP addresses
- Rate limit suspicious clients
- Disable affected user accounts if necessary

#### 4. Eradication

- Remove malicious queries from the system
- Patch any identified vulnerabilities
- Update security configurations

#### 5. Recovery

- Restore normal operations
- Monitor for recurring issues
- Update incident response procedures

#### 6. Lessons Learned

- Document the incident
- Analyze root causes
- Improve security measures
- Update training and procedures

### Emergency Contacts

- **Security Team**: security@example.com
- **On-Call Engineer**: +1-555-0123
- **Management**: management@example.com

## Best Practices

### Development

1. **Security-First Design**: Consider security from the beginning
2. **Input Validation**: Validate all inputs at the boundaries
3. **Error Handling**: Don't leak sensitive information in errors
4. **Logging**: Log security-relevant events
5. **Testing**: Include security tests in your test suite

### Operations

1. **Monitoring**: Continuously monitor security metrics
2. **Alerting**: Set up appropriate alert thresholds
3. **Incident Response**: Have clear procedures for security incidents
4. **Regular Updates**: Keep security configurations up to date
5. **Access Control**: Regularly review and update user permissions

### Configuration

1. **Environment-Specific**: Use different configurations for different environments
2. **Documentation**: Document all security configurations
3. **Version Control**: Keep security configurations in version control
4. **Testing**: Test security configurations before deployment
5. **Review**: Regularly review and update security settings

### Monitoring

1. **Real-Time**: Monitor security events in real-time
2. **Trends**: Analyze security trends over time
3. **Anomalies**: Detect unusual patterns and behaviors
4. **Reporting**: Generate regular security reports
5. **Improvement**: Use monitoring data to improve security

## Security Checklist

### Pre-Deployment

- [ ] Security configuration reviewed and approved
- [ ] Rate limiting configured appropriately
- [ ] Input validation implemented
- [ ] Authentication and authorization tested
- [ ] Query complexity limits set
- [ ] Introspection protection enabled
- [ ] Monitoring and alerting configured
- [ ] Incident response procedures documented
- [ ] Security team notified of deployment

### Post-Deployment

- [ ] Security metrics monitored
- [ ] Alert thresholds verified
- [ ] Rate limiting functioning correctly
- [ ] Input validation working as expected
- [ ] Authentication and authorization operational
- [ ] Query security measures active
- [ ] Monitoring systems operational
- [ ] Incident response procedures tested
- [ ] Security documentation updated

### Regular Maintenance

- [ ] Security configurations reviewed monthly
- [ ] User permissions audited quarterly
- [ ] Security metrics analyzed weekly
- [ ] Incident response procedures updated as needed
- [ ] Security training conducted annually
- [ ] Penetration testing performed annually
- [ ] Security documentation updated regularly
- [ ] Emergency contacts verified quarterly

## Conclusion

This security guide provides comprehensive coverage of GraphQL API security for the EconGraph platform. By following these guidelines and implementing the recommended security measures, you can ensure that your GraphQL API remains secure and resilient against various attack vectors.

Remember that security is an ongoing process, not a one-time implementation. Regular monitoring, updates, and improvements are essential to maintaining a secure system.

For questions or concerns about security, please contact the security team at security@example.com.
