# GraphQL Security API Reference

## Overview

This document provides a comprehensive API reference for the GraphQL security system, including all public interfaces, data structures, and configuration options.

## Core Types

### SecurityConfig

```rust
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Maximum query complexity allowed
    pub max_complexity: u32,
    /// Maximum query depth allowed
    pub max_depth: u32,
    /// Maximum query size in bytes
    pub max_query_size: usize,
    /// Query timeout in seconds
    pub query_timeout: u64,
    /// Whether to protect introspection
    pub protect_introspection: bool,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Query filter configuration
    pub query_filter: QueryFilterConfig,
}
```

**Default Values:**
- `max_complexity`: 1000
- `max_depth`: 10
- `max_query_size`: 10000
- `query_timeout`: 30
- `protect_introspection`: true

### SecurityMiddleware

```rust
pub struct SecurityMiddleware {
    /// Query complexity analyzer
    pub complexity_analyzer: complexity::ComplexityAnalyzer,
    /// Query depth limiter
    pub depth_limiter: depth_limit::DepthLimiter,
    /// Rate limiter
    pub rate_limiter: rate_limit::RateLimiter,
    /// Query analyzer
    pub query_analyzer: query_analysis::QueryAnalyzer,
    /// Introspection protector
    pub introspection_protector: introspection::IntrospectionProtector,
    /// Timeout manager
    pub timeout_manager: timeout::TimeoutManager,
    /// Query filter
    pub query_filter: whitelist::QueryFilter,
    /// Configuration
    pub config: SecurityConfig,
}
```

## Security Modules

### 1. Complexity Analysis

#### ComplexityAnalyzer

```rust
pub struct ComplexityAnalyzer {
    max_complexity: u32,
    field_complexity: HashMap<String, u32>,
    list_complexity_multiplier: f64,
}
```

**Methods:**

```rust
impl ComplexityAnalyzer {
    /// Create a new complexity analyzer
    pub fn new(max_complexity: u32) -> Self
    
    /// Calculate the complexity of a GraphQL query
    pub fn calculate_complexity(&self, query: &str) -> Result<u32, String>
    
    /// Set custom complexity for a field
    pub fn set_field_complexity(&mut self, field_name: String, complexity: u32)
    
    /// Set the list complexity multiplier
    pub fn set_list_complexity_multiplier(&mut self, multiplier: f64)
    
    /// Get the maximum allowed complexity
    pub fn get_max_complexity(&self) -> u32
}
```

**Example Usage:**

```rust
let mut analyzer = ComplexityAnalyzer::new(1000);
analyzer.set_field_complexity("users".to_string(), 10);
analyzer.set_list_complexity_multiplier(2.0);

let query = "query { users { id name } }";
let complexity = analyzer.calculate_complexity(query)?;
println!("Query complexity: {}", complexity);
```

### 2. Depth Limiting

#### DepthLimiter

```rust
pub struct DepthLimiter {
    max_depth: u32,
}
```

**Methods:**

```rust
impl DepthLimiter {
    /// Create a new depth limiter
    pub fn new(max_depth: u32) -> Self
    
    /// Calculate the depth of a GraphQL query
    pub fn calculate_depth(&self, query: &str) -> Result<u32, String>
    
    /// Check if a query exceeds the maximum depth
    pub fn check_depth(&self, query: &str) -> Result<(), String>
    
    /// Get the maximum allowed depth
    pub fn get_max_depth(&self) -> u32
}
```

**Example Usage:**

```rust
let limiter = DepthLimiter::new(10);
let query = "query { users { posts { comments { author { name } } } } }";
let depth = limiter.calculate_depth(query)?;
println!("Query depth: {}", depth);
```

### 3. Rate Limiting

#### RateLimiter

```rust
pub struct RateLimiter {
    config: RateLimitConfig,
    entries: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute
    pub requests_per_minute: u32,
    /// Maximum requests per hour
    pub requests_per_hour: u32,
    /// Maximum requests per day
    pub requests_per_day: u32,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}
```

**Methods:**

```rust
impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self
    
    /// Check if a request is within rate limits
    pub async fn check_rate_limit(&self, client_ip: &str) -> Result<(), String>
    
    /// Get rate limit status for a client
    pub async fn get_rate_limit_status(&self, client_ip: &str) -> RateLimitStatus
    
    /// Get rate limiting statistics
    pub async fn get_statistics(&self) -> RateLimitStatistics
    
    /// Clean up expired entries
    pub async fn cleanup(&self)
}
```

**Status Types:**

```rust
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub requests_per_day: u32,
    pub limit_per_minute: u32,
    pub limit_per_hour: u32,
    pub limit_per_day: u32,
}

#[derive(Debug, Clone)]
pub struct RateLimitStatistics {
    pub total_ips: usize,
    pub active_ips: usize,
    pub total_requests: u32,
    pub config: RateLimitConfig,
}
```

**Example Usage:**

```rust
let config = RateLimitConfig {
    requests_per_minute: 60,
    requests_per_hour: 1000,
    requests_per_day: 10000,
    enabled: true,
};

let rate_limiter = RateLimiter::new(config);
rate_limiter.check_rate_limit("127.0.0.1").await?;

let status = rate_limiter.get_rate_limit_status("127.0.0.1").await;
println!("Rate limit status: {:?}", status);
```

### 4. Input Validation

#### InputValidator

```rust
pub struct InputValidator {
    config: InputValidationConfig,
    blocked_patterns: Vec<Regex>,
    allowed_patterns: Vec<Regex>,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct InputValidationConfig {
    /// Maximum string length
    pub max_string_length: usize,
    /// Minimum string length
    pub min_string_length: usize,
    /// Allowed characters regex
    pub allowed_characters: Option<String>,
    /// Blocked characters regex
    pub blocked_characters: Option<String>,
    /// Enable HTML sanitization
    pub enable_html_sanitization: bool,
    /// Enable SQL injection prevention
    pub enable_sql_injection_prevention: bool,
    /// Enable XSS prevention
    pub enable_xss_prevention: bool,
    /// Custom validation rules
    pub custom_rules: HashMap<String, InputValidationRule>,
}
```

**Validation Rule:**

```rust
#[derive(Debug, Clone)]
pub struct InputValidationRule {
    /// Regex pattern for validation
    pub regex: Option<String>,
    /// Minimum length
    pub min_length: Option<usize>,
    /// Maximum length
    pub max_length: Option<usize>,
    /// Error message for validation failure
    pub error_message: Option<String>,
    /// Whether to allow empty values
    pub allow_empty: bool,
}
```

**Methods:**

```rust
impl InputValidator {
    /// Create a new input validator
    pub fn new(config: InputValidationConfig) -> Result<Self, String>
    
    /// Validate a string input
    pub fn validate(&self, value: &str, field_type: &str) -> Result<String, String>
    
    /// Validate a JSON value
    pub fn validate_json(&self, value: &serde_json::Value) -> Result<serde_json::Value, String>
    
    /// Add a custom validation rule
    pub fn add_rule(&mut self, field_type: String, rule: InputValidationRule) -> Result<(), String>
    
    /// Remove a custom validation rule
    pub fn remove_rule(&mut self, field_type: &str)
}
```

**Example Usage:**

```rust
let config = InputValidationConfig {
    max_string_length: 1000,
    min_string_length: 1,
    enable_html_sanitization: true,
    enable_sql_injection_prevention: true,
    enable_xss_prevention: true,
    ..Default::default()
};

let validator = InputValidator::new(config)?;
let sanitized = validator.validate("<script>alert('xss')</script>", "string")?;
println!("Sanitized input: {}", sanitized);
```

### 5. Introspection Protection

#### IntrospectionProtector

```rust
pub struct IntrospectionProtector {
    config: IntrospectionConfig,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct IntrospectionConfig {
    /// Whether introspection is enabled
    pub enabled: bool,
    /// Blocked introspection fields
    pub blocked_fields: HashSet<String>,
    /// Allowed introspection fields (if not empty, only these are allowed)
    pub allowed_fields: HashSet<String>,
    /// Whether to block introspection in production
    pub block_in_production: bool,
}
```

**Methods:**

```rust
impl IntrospectionProtector {
    /// Create a new introspection protector
    pub fn new(config: IntrospectionConfig) -> Self
    
    /// Check if a query contains introspection
    pub fn contains_introspection(&self, query: &str) -> bool
    
    /// Validate introspection access
    pub fn validate_introspection(&self, query: &str) -> Result<(), String>
    
    /// Block specific introspection fields
    pub fn block_field(&mut self, field_name: String)
    
    /// Allow specific introspection fields
    pub fn allow_field(&mut self, field_name: String)
}
```

**Example Usage:**

```rust
let config = IntrospectionConfig {
    enabled: true,
    blocked_fields: ["__schema".to_string(), "__type".to_string()].into_iter().collect(),
    allowed_fields: HashSet::new(),
    block_in_production: true,
};

let protector = IntrospectionProtector::new(config);
let query = "query { __schema { types { name } } }";
protector.validate_introspection(query)?;
```

### 6. Query Analysis

#### QueryAnalyzer

```rust
pub struct QueryAnalyzer {
    config: QueryAnalysisConfig,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct QueryAnalysisConfig {
    /// Whether to analyze query structure
    pub analyze_structure: bool,
    /// Whether to analyze field usage
    pub analyze_fields: bool,
    /// Whether to analyze variable usage
    pub analyze_variables: bool,
    /// Whether to detect suspicious patterns
    pub detect_suspicious_patterns: bool,
}
```

**Analysis Result:**

```rust
#[derive(Debug, Clone)]
pub struct QueryAnalysisResult {
    /// Number of fields in the query
    pub field_count: usize,
    /// Number of variables in the query
    pub variable_count: usize,
    /// Depth of the query
    pub depth: u32,
    /// Whether the query contains suspicious patterns
    pub suspicious_patterns: Vec<String>,
    /// Fields used in the query
    pub fields: HashSet<String>,
    /// Variables used in the query
    pub variables: HashSet<String>,
}
```

**Methods:**

```rust
impl QueryAnalyzer {
    /// Create a new query analyzer
    pub fn new(config: QueryAnalysisConfig) -> Self
    
    /// Analyze a GraphQL query
    pub fn analyze_query(&self, query: &str) -> Result<QueryAnalysisResult, String>
    
    /// Check if a query is suspicious
    pub fn is_suspicious(&self, query: &str) -> Result<bool, String>
    
    /// Get query statistics
    pub fn get_statistics(&self, query: &str) -> Result<QueryStatistics, String>
}
```

**Example Usage:**

```rust
let config = QueryAnalysisConfig {
    analyze_structure: true,
    analyze_fields: true,
    analyze_variables: true,
    detect_suspicious_patterns: true,
};

let analyzer = QueryAnalyzer::new(config);
let result = analyzer.analyze_query("query { users { id name } }")?;
println!("Query analysis: {:?}", result);
```

### 7. Timeout Management

#### TimeoutManager

```rust
pub struct TimeoutManager {
    config: TimeoutConfig,
    active_queries: Arc<RwLock<HashMap<String, Instant>>>,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Default timeout in seconds
    pub default_timeout: u64,
    /// Maximum timeout in seconds
    pub max_timeout: u64,
    /// Whether timeouts are enabled
    pub enabled: bool,
}
```

**Methods:**

```rust
impl TimeoutManager {
    /// Create a new timeout manager
    pub fn new(config: TimeoutConfig) -> Self
    
    /// Execute a future with timeout
    pub async fn execute_with_timeout<F, T>(&self, future: F) -> Result<T, TimeoutError>
    where
        F: Future<Output = T>,
    
    /// Set a custom timeout for a query
    pub fn set_query_timeout(&self, query_id: String, timeout: Duration)
    
    /// Cancel a query timeout
    pub fn cancel_query_timeout(&self, query_id: String)
    
    /// Get timeout statistics
    pub fn get_statistics(&self) -> TimeoutStatistics
}
```

**Error Types:**

```rust
#[derive(Debug, thiserror::Error)]
pub enum TimeoutError {
    #[error("Query timed out after {duration:?}")]
    QueryTimeout { duration: Duration },
    #[error("Timeout configuration error: {message}")]
    ConfigurationError { message: String },
    #[error("Timeout management error: {message}")]
    ManagementError { message: String },
}
```

**Example Usage:**

```rust
let config = TimeoutConfig {
    default_timeout: 30,
    max_timeout: 300,
    enabled: true,
};

let timeout_manager = TimeoutManager::new(config);
let result = timeout_manager.execute_with_timeout(async {
    // Long-running operation
    tokio::time::sleep(Duration::from_secs(5)).await;
    "completed"
}).await?;
```

### 8. Query Filtering

#### QueryFilter

```rust
pub struct QueryFilter {
    config: QueryFilterConfig,
    whitelist_patterns: Vec<Regex>,
    blacklist_patterns: Vec<Regex>,
}
```

**Configuration:**

```rust
#[derive(Debug, Clone)]
pub struct QueryFilterConfig {
    /// Enable query whitelisting
    pub enable_whitelist: bool,
    /// Enable query blacklisting
    pub enable_blacklist: bool,
    /// Whitelisted query patterns
    pub whitelist_patterns: Vec<String>,
    /// Blacklisted query patterns
    pub blacklist_patterns: Vec<String>,
    /// Case sensitive matching
    pub case_sensitive: bool,
    /// Use regex patterns
    pub use_regex: bool,
    /// Allow partial matches
    pub allow_partial_matches: bool,
}
```

**Methods:**

```rust
impl QueryFilter {
    /// Create a new query filter
    pub fn new(config: QueryFilterConfig) -> Result<Self, String>
    
    /// Check if a query is allowed
    pub fn is_allowed(&self, query: &str) -> Result<bool, String>
    
    /// Add a whitelist pattern
    pub fn add_whitelist_pattern(&mut self, pattern: String) -> Result<(), String>
    
    /// Add a blacklist pattern
    pub fn add_blacklist_pattern(&mut self, pattern: String) -> Result<(), String>
    
    /// Remove a whitelist pattern
    pub fn remove_whitelist_pattern(&mut self, pattern: &str)
    
    /// Remove a blacklist pattern
    pub fn remove_blacklist_pattern(&mut self, pattern: &str)
}
```

**Example Usage:**

```rust
let config = QueryFilterConfig {
    enable_whitelist: true,
    enable_blacklist: false,
    whitelist_patterns: vec!["query.*users".to_string()],
    blacklist_patterns: vec![],
    case_sensitive: false,
    use_regex: true,
    allow_partial_matches: true,
};

let filter = QueryFilter::new(config)?;
let is_allowed = filter.is_allowed("query { users { id name } }")?;
println!("Query allowed: {}", is_allowed);
```

## Authorization System

### Permission Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

### UserRole Enum

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UserRole {
    SuperAdmin,
    Admin,
    Analyst,
    Viewer,
    Guest,
}
```

**Methods:**

```rust
impl UserRole {
    /// Create a role from string
    pub fn from_str(role: &str) -> Option<Self>
    
    /// Get permissions for this role
    pub fn get_permissions(&self) -> HashSet<Permission>
    
    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool
}
```

### GraphQLContext

```rust
pub struct GraphQLContext {
    /// Current user
    pub user: Option<User>,
    /// User role
    pub user_role: Option<UserRole>,
    /// User permissions
    pub permissions: HashSet<Permission>,
    /// Client IP address
    pub client_ip: Option<String>,
    /// Request timestamp
    pub request_timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID
    pub request_id: String,
    /// Security middleware
    pub security: Arc<SecurityMiddleware>,
}
```

**Methods:**

```rust
impl GraphQLContext {
    /// Create a new context
    pub fn new(user: Option<User>) -> Self
    
    /// Create a new context with client information
    pub fn new_with_client_info(user: Option<User>, client_ip: Option<String>) -> Self
    
    /// Check if the user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool
    
    /// Require a specific permission
    pub fn require_permission(&self, permission: &Permission) -> Result<(), String>
    
    /// Check if the user has any of the specified permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool
    
    /// Require any of the specified permissions
    pub fn require_any_permission(&self, permissions: &[Permission]) -> Result<(), String>
    
    /// Check if the user has all of the specified permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool
    
    /// Require all of the specified permissions
    pub fn require_all_permissions(&self, permissions: &[Permission]) -> Result<(), String>
    
    /// Check if the user is an admin
    pub fn require_admin(&self) -> Result<(), String>
    
    /// Check if the user is a super admin
    pub fn require_super_admin(&self) -> Result<(), String>
    
    /// Check if the user can manage another user
    pub fn can_manage_user(&self, target_user: &User) -> bool
    
    /// Log an authorization decision
    pub fn log_authorization_decision(&self, permission: &Permission, granted: bool)
}
```

**Helper Methods:**

```rust
impl GraphQLContext {
    /// Check if the user can read economic data
    pub fn can_read_economic_data(&self) -> bool
    
    /// Check if the user can write economic data
    pub fn can_write_economic_data(&self) -> bool
    
    /// Check if the user can delete economic data
    pub fn can_delete_economic_data(&self) -> bool
    
    /// Check if the user can create charts
    pub fn can_create_charts(&self) -> bool
    
    /// Check if the user can update charts
    pub fn can_update_charts(&self) -> bool
    
    /// Check if the user can delete charts
    pub fn can_delete_charts(&self) -> bool
    
    /// Check if the user can share charts
    pub fn can_share_charts(&self) -> bool
    
    /// Check if the user can create annotations
    pub fn can_create_annotations(&self) -> bool
    
    /// Check if the user can update annotations
    pub fn can_update_annotations(&self) -> bool
    
    /// Check if the user can delete annotations
    pub fn can_delete_annotations(&self) -> bool
    
    /// Check if the user can view logs
    pub fn can_view_logs(&self) -> bool
    
    /// Check if the user can view metrics
    pub fn can_view_metrics(&self) -> bool
    
    /// Check if the user can view health status
    pub fn can_view_health(&self) -> bool
    
    /// Check if the user can view security events
    pub fn can_view_security_events(&self) -> bool
}
```

## Security Events

### SecurityEvent Enum

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SecurityEvent {
    RateLimitExceeded {
        client_ip: String,
        requests_per_minute: u32,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ComplexityExceeded {
        client_ip: String,
        complexity: u32,
        max_complexity: u32,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    DepthExceeded {
        client_ip: String,
        depth: u32,
        max_depth: u32,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    SizeExceeded {
        client_ip: String,
        size: usize,
        max_size: usize,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    IntrospectionBlocked {
        client_ip: String,
        query: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    QueryFiltered {
        client_ip: String,
        query: String,
        reason: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    AuthorizationFailed {
        client_ip: String,
        user_id: Option<String>,
        permission: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    InputValidationFailed {
        client_ip: String,
        field: String,
        value: String,
        error: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    QueryTimeout {
        client_ip: String,
        query: String,
        duration: Duration,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}
```

### SecurityEventHandler Trait

```rust
pub trait SecurityEventHandler: Send + Sync {
    /// Handle a security event
    fn handle_event(&self, event: SecurityEvent);
}
```

### SecurityMetrics

```rust
#[derive(Debug, Default, Clone)]
pub struct SecurityMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests blocked by rate limiting
    pub rate_limited_requests: u64,
    /// Requests blocked by complexity
    pub complexity_blocked_requests: u64,
    /// Requests blocked by depth
    pub depth_blocked_requests: u64,
    /// Requests blocked by size
    pub size_blocked_requests: u64,
    /// Requests blocked by introspection
    pub introspection_blocked_requests: u64,
    /// Requests blocked by filtering
    pub filtered_requests: u64,
    /// Average query complexity
    pub average_complexity: f64,
    /// Average query depth
    pub average_depth: f64,
    /// Average query size
    pub average_size: f64,
}
```

## Error Types

### SecurityError

```rust
#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },
    
    #[error("Query complexity exceeded: {complexity} > {max_complexity}")]
    ComplexityExceeded { complexity: u32, max_complexity: u32 },
    
    #[error("Query depth exceeded: {depth} > {max_depth}")]
    DepthExceeded { depth: u32, max_depth: u32 },
    
    #[error("Query size exceeded: {size} > {max_size}")]
    SizeExceeded { size: usize, max_size: usize },
    
    #[error("Input validation failed: {field} - {message}")]
    InputValidationFailed { field: String, message: String },
    
    #[error("Authorization failed: {permission:?}")]
    AuthorizationFailed { permission: Permission },
    
    #[error("Query timeout: {duration:?}")]
    QueryTimeout { duration: Duration },
    
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
    
    #[error("Introspection blocked: {query}")]
    IntrospectionBlocked { query: String },
    
    #[error("Query filtered: {reason}")]
    QueryFiltered { reason: String },
}
```

## Configuration Examples

### Basic Configuration

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
        whitelist_patterns: vec![],
        blacklist_patterns: vec!["__schema".to_string()],
        case_sensitive: false,
        use_regex: true,
        allow_partial_matches: true,
    },
};
```

### Production Configuration

```rust
let config = SecurityConfig {
    max_complexity: 500,
    max_depth: 8,
    max_query_size: 5000,
    query_timeout: 15,
    protect_introspection: true,
    rate_limit: RateLimitConfig {
        requests_per_minute: 30,
        requests_per_hour: 500,
        requests_per_day: 5000,
        enabled: true,
    },
    query_filter: QueryFilterConfig {
        enable_whitelist: true,
        enable_blacklist: true,
        whitelist_patterns: vec![
            "query.*users".to_string(),
            "query.*economicData".to_string(),
        ],
        blacklist_patterns: vec![
            "__schema".to_string(),
            "__type".to_string(),
            "mutation.*delete".to_string(),
        ],
        case_sensitive: false,
        use_regex: true,
        allow_partial_matches: false,
    },
};
```

### Development Configuration

```rust
let config = SecurityConfig {
    max_complexity: 2000,
    max_depth: 15,
    max_query_size: 20000,
    query_timeout: 60,
    protect_introspection: false,
    rate_limit: RateLimitConfig {
        requests_per_minute: 120,
        requests_per_hour: 2000,
        requests_per_day: 20000,
        enabled: false,
    },
    query_filter: QueryFilterConfig {
        enable_whitelist: false,
        enable_blacklist: false,
        whitelist_patterns: vec![],
        blacklist_patterns: vec![],
        case_sensitive: false,
        use_regex: false,
        allow_partial_matches: true,
    },
};
```

## Integration Examples

### Basic Integration

```rust
use econ_graph_graphql::security::{SecurityConfig, SecurityMiddleware};

// Create security configuration
let config = SecurityConfig::default();

// Create security middleware
let security = SecurityMiddleware::new(config);

// Use in GraphQL context
let context = GraphQLContext {
    user: Some(user),
    user_role: Some(UserRole::Admin),
    permissions: UserRole::Admin.get_permissions(),
    client_ip: Some("127.0.0.1".to_string()),
    request_timestamp: chrono::Utc::now(),
    request_id: uuid::Uuid::new_v4().to_string(),
    security: Arc::new(security),
};
```

### Advanced Integration

```rust
use econ_graph_graphql::security::{
    SecurityConfig, SecurityMiddleware, RateLimitConfig, QueryFilterConfig,
    InputValidationConfig, IntrospectionConfig, TimeoutConfig,
};

// Create comprehensive security configuration
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
        enable_whitelist: true,
        enable_blacklist: true,
        whitelist_patterns: vec!["query.*users".to_string()],
        blacklist_patterns: vec!["__schema".to_string()],
        case_sensitive: false,
        use_regex: true,
        allow_partial_matches: true,
    },
};

// Create security middleware
let security = SecurityMiddleware::new(config);

// Validate request
let request = Request {
    query: "query { users { id name } }".to_string(),
    variables: HashMap::new(),
    client_ip: "127.0.0.1".to_string(),
    context: GraphQLContext::new_with_client_info(Some(user), Some("127.0.0.1".to_string())),
};

security.validate_request(&request).await?;
```

## Best Practices

### 1. Configuration Management

- Use environment variables for sensitive configuration
- Implement configuration validation
- Provide sensible defaults
- Document all configuration options

### 2. Error Handling

- Use specific error types
- Provide clear error messages
- Log security events
- Implement graceful degradation

### 3. Performance Optimization

- Cache expensive operations
- Use async operations where possible
- Monitor performance overhead
- Implement resource limits

### 4. Security Monitoring

- Log all security events
- Implement alerting for critical events
- Monitor security metrics
- Regular security audits

### 5. Testing

- Unit tests for all security modules
- Integration tests for end-to-end scenarios
- Performance tests for overhead measurement
- Security tests for vulnerability assessment

## Conclusion

This API reference provides comprehensive documentation for all public interfaces in the GraphQL security system. The system is designed to be flexible, performant, and secure while providing extensive configuration options and monitoring capabilities.
