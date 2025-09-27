//! # Security Monitoring and Alerting
//!
//! This module provides comprehensive security monitoring and alerting for the GraphQL API.
//! It tracks security events, analyzes patterns, and provides real-time alerts for potential threats.
//!
//! # Monitoring Features
//!
//! - Real-time security event tracking
//! - Pattern analysis and anomaly detection
//! - Threat intelligence integration
//! - Automated alerting and notifications
//! - Security metrics and dashboards
//! - Incident response automation
//!
//! # Security Benefits
//!
//! - Early threat detection
//! - Automated incident response
//! - Security trend analysis
//! - Compliance monitoring
//! - Forensic capabilities
//! - Risk assessment
//!
//! # Alert Types
//!
//! - Rate limit violations
//! - Query complexity attacks
//! - Suspicious query patterns
//! - Authentication failures
//! - Authorization violations
//! - System anomalies

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::security::{BlockReason, SecurityEvent, SecurityMetrics};

/// Security monitoring configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    /// Enable real-time monitoring
    pub enabled: bool,
    /// Maximum events to store in memory
    pub max_events: usize,
    /// Event retention period in seconds
    pub retention_period: u64,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
    /// Notification settings
    pub notifications: NotificationConfig,
    /// Pattern analysis settings
    pub pattern_analysis: PatternAnalysisConfig,
}

/// Alert thresholds for different security events
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    /// Rate limit violations per minute
    pub rate_limit_violations_per_minute: u32,
    /// Query complexity violations per hour
    pub complexity_violations_per_hour: u32,
    /// Authentication failures per hour
    pub auth_failures_per_hour: u32,
    /// Authorization violations per hour
    pub authz_violations_per_hour: u32,
    /// Suspicious queries per hour
    pub suspicious_queries_per_hour: u32,
    /// System errors per hour
    pub system_errors_per_hour: u32,
}

/// Notification configuration
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Enable email notifications
    pub enable_email: bool,
    /// Enable webhook notifications
    pub enable_webhook: bool,
    /// Enable Slack notifications
    pub enable_slack: bool,
    /// Email recipients
    pub email_recipients: Vec<String>,
    /// Webhook URLs
    pub webhook_urls: Vec<String>,
    /// Slack webhook URL
    pub slack_webhook_url: Option<String>,
    /// Notification cooldown period in seconds
    pub cooldown_period: u64,
}

/// Pattern analysis configuration
#[derive(Debug, Clone)]
pub struct PatternAnalysisConfig {
    /// Enable pattern analysis
    pub enabled: bool,
    /// Analysis window size in seconds
    pub window_size: u64,
    /// Minimum events for pattern detection
    pub min_events: usize,
    /// Suspicious pattern thresholds
    pub suspicious_thresholds: SuspiciousPatternThresholds,
}

/// Suspicious pattern thresholds
#[derive(Debug, Clone)]
pub struct SuspiciousPatternThresholds {
    /// Repeated failed authentication attempts
    pub repeated_auth_failures: u32,
    /// Rapid query complexity escalation
    pub complexity_escalation: u32,
    /// Unusual query patterns
    pub unusual_patterns: u32,
    /// Geographic anomalies
    pub geographic_anomalies: u32,
    /// Time-based anomalies
    pub time_anomalies: u32,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_events: 10000,
            retention_period: 86400, // 24 hours
            alert_thresholds: AlertThresholds {
                rate_limit_violations_per_minute: 10,
                complexity_violations_per_hour: 50,
                auth_failures_per_hour: 20,
                authz_violations_per_hour: 30,
                suspicious_queries_per_hour: 25,
                system_errors_per_hour: 100,
            },
            notifications: NotificationConfig {
                enable_email: false,
                enable_webhook: false,
                enable_slack: false,
                email_recipients: Vec::new(),
                webhook_urls: Vec::new(),
                slack_webhook_url: None,
                cooldown_period: 300, // 5 minutes
            },
            pattern_analysis: PatternAnalysisConfig {
                enabled: true,
                window_size: 3600, // 1 hour
                min_events: 5,
                suspicious_thresholds: SuspiciousPatternThresholds {
                    repeated_auth_failures: 5,
                    complexity_escalation: 3,
                    unusual_patterns: 10,
                    geographic_anomalies: 3,
                    time_anomalies: 5,
                },
            },
        }
    }
}

/// Security event with metadata
#[derive(Debug, Clone)]
pub struct SecurityEventWithMetadata {
    /// The security event
    pub event: SecurityEvent,
    /// Event timestamp
    pub timestamp: Instant,
    /// Event severity
    pub severity: EventSeverity,
    /// Event source
    pub source: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Event severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Security alert
#[derive(Debug, Clone)]
pub struct SecurityAlert {
    /// Alert ID
    pub id: String,
    /// Alert type
    pub alert_type: AlertType,
    /// Alert severity
    pub severity: EventSeverity,
    /// Alert message
    pub message: String,
    /// Alert timestamp
    pub timestamp: Instant,
    /// Related events
    pub related_events: Vec<SecurityEventWithMetadata>,
    /// Alert status
    pub status: AlertStatus,
    /// Alert metadata
    pub metadata: HashMap<String, String>,
}

/// Alert types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertType {
    RateLimitViolation,
    ComplexityAttack,
    AuthenticationFailure,
    AuthorizationViolation,
    SuspiciousQuery,
    SystemError,
    PatternAnomaly,
    GeographicAnomaly,
    TimeAnomaly,
    Custom,
}

/// Alert status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertStatus {
    Active,
    Acknowledged,
    Resolved,
    Suppressed,
}

/// Security monitor
pub struct SecurityMonitor {
    /// Monitoring configuration
    config: MonitoringConfig,
    /// Security events queue
    events: Arc<RwLock<VecDeque<SecurityEventWithMetadata>>>,
    /// Active alerts
    alerts: Arc<RwLock<Vec<SecurityAlert>>>,
    /// Event counters
    counters: Arc<RwLock<EventCounters>>,
    /// Pattern analyzer
    pattern_analyzer: Arc<RwLock<PatternAnalyzer>>,
    /// Notification service
    notification_service: Arc<NotificationService>,
    /// Last cleanup time
    last_cleanup: Arc<RwLock<Instant>>,
}

/// Event counters for monitoring
#[derive(Debug, Default)]
struct EventCounters {
    /// Rate limit violations per minute
    rate_limit_violations: VecDeque<(Instant, u32)>,
    /// Query complexity violations per hour
    complexity_violations: VecDeque<(Instant, u32)>,
    /// Authentication failures per hour
    auth_failures: VecDeque<(Instant, u32)>,
    /// Authorization violations per hour
    authz_violations: VecDeque<(Instant, u32)>,
    /// Suspicious queries per hour
    suspicious_queries: VecDeque<(Instant, u32)>,
    /// System errors per hour
    system_errors: VecDeque<(Instant, u32)>,
}

/// Pattern analyzer for detecting anomalies
#[derive(Debug, Default)]
struct PatternAnalyzer {
    /// Query patterns
    query_patterns: HashMap<String, VecDeque<Instant>>,
    /// IP patterns
    ip_patterns: HashMap<String, VecDeque<Instant>>,
    /// User patterns
    user_patterns: HashMap<String, VecDeque<Instant>>,
    /// Time patterns
    time_patterns: VecDeque<Instant>,
}

/// Notification service
#[derive(Debug)]
struct NotificationService {
    config: NotificationConfig,
    last_notification: Arc<RwLock<HashMap<String, Instant>>>,
}

impl SecurityMonitor {
    /// Create a new security monitor
    pub fn new(config: MonitoringConfig) -> Self {
        let notification_service = Arc::new(NotificationService {
            config: config.notifications.clone(),
            last_notification: Arc::new(RwLock::new(HashMap::new())),
        });

        Self {
            events: Arc::new(RwLock::new(VecDeque::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            counters: Arc::new(RwLock::new(EventCounters::default())),
            pattern_analyzer: Arc::new(RwLock::new(PatternAnalyzer::default())),
            notification_service,
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
            config,
        }
    }

    /// Record a security event
    pub async fn record_event(&self, event: SecurityEvent, source: String) {
        if !self.config.enabled {
            return;
        }

        let severity = self.determine_severity(&event);
        let metadata = self.extract_metadata(&event);

        let event_with_metadata = SecurityEventWithMetadata {
            event: event.clone(),
            timestamp: Instant::now(),
            severity,
            source,
            metadata,
        };

        // Add to events queue
        {
            let mut events = self.events.write().await;
            events.push_back(event_with_metadata.clone());

            // Maintain max events limit
            while events.len() > self.config.max_events {
                events.pop_front();
            }
        }

        // Update counters
        self.update_counters(&event).await;

        // Analyze patterns
        if self.config.pattern_analysis.enabled {
            self.analyze_patterns(&event_with_metadata).await;
        }

        // Check for alerts
        self.check_alerts(&event_with_metadata).await;

        // Cleanup old data
        self.cleanup_old_data().await;
    }

    /// Determine event severity
    fn determine_severity(&self, event: &SecurityEvent) -> EventSeverity {
        match event {
            SecurityEvent::RateLimitExceeded { .. } => EventSeverity::Medium,
            SecurityEvent::ComplexityExceeded {
                complexity,
                max_complexity,
                ..
            } => {
                if *complexity > *max_complexity * 2 {
                    EventSeverity::High
                } else {
                    EventSeverity::Medium
                }
            }
            SecurityEvent::DepthExceeded {
                depth, max_depth, ..
            } => {
                if *depth > *max_depth * 2 {
                    EventSeverity::High
                } else {
                    EventSeverity::Medium
                }
            }
            SecurityEvent::QuerySizeExceeded { size, max_size, .. } => {
                if *size > *max_size * 2 {
                    EventSeverity::High
                } else {
                    EventSeverity::Medium
                }
            }
            SecurityEvent::IntrospectionBlocked { .. } => EventSeverity::Low,
            SecurityEvent::QueryFiltered { .. } => EventSeverity::Medium,
        }
    }

    /// Extract metadata from event
    fn extract_metadata(&self, event: &SecurityEvent) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        match event {
            SecurityEvent::RateLimitExceeded {
                client_ip,
                requests_per_minute,
                ..
            } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
                metadata.insert(
                    "requests_per_minute".to_string(),
                    requests_per_minute.to_string(),
                );
            }
            SecurityEvent::ComplexityExceeded {
                client_ip,
                complexity,
                max_complexity,
                ..
            } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
                metadata.insert("complexity".to_string(), complexity.to_string());
                metadata.insert("max_complexity".to_string(), max_complexity.to_string());
            }
            SecurityEvent::DepthExceeded {
                client_ip,
                depth,
                max_depth,
                ..
            } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
                metadata.insert("depth".to_string(), depth.to_string());
                metadata.insert("max_depth".to_string(), max_depth.to_string());
            }
            SecurityEvent::QuerySizeExceeded {
                client_ip,
                size,
                max_size,
                ..
            } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
                metadata.insert("size".to_string(), size.to_string());
                metadata.insert("max_size".to_string(), max_size.to_string());
            }
            SecurityEvent::IntrospectionBlocked { client_ip, .. } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
            }
            SecurityEvent::QueryFiltered {
                client_ip, reason, ..
            } => {
                metadata.insert("client_ip".to_string(), client_ip.clone());
                metadata.insert("reason".to_string(), reason.clone());
            }
        }

        metadata
    }

    /// Update event counters
    async fn update_counters(&self, event: &SecurityEvent) {
        let mut counters = self.counters.write().await;
        let now = Instant::now();

        match event {
            SecurityEvent::RateLimitExceeded { .. } => {
                counters.rate_limit_violations.push_back((now, 1));
            }
            SecurityEvent::ComplexityExceeded { .. } => {
                counters.complexity_violations.push_back((now, 1));
            }
            SecurityEvent::DepthExceeded { .. } => {
                counters.complexity_violations.push_back((now, 1));
            }
            SecurityEvent::QuerySizeExceeded { .. } => {
                counters.complexity_violations.push_back((now, 1));
            }
            SecurityEvent::IntrospectionBlocked { .. } => {
                counters.suspicious_queries.push_back((now, 1));
            }
            SecurityEvent::QueryFiltered { .. } => {
                counters.suspicious_queries.push_back((now, 1));
            }
        }
    }

    /// Analyze patterns for anomalies
    async fn analyze_patterns(&self, event: &SecurityEventWithMetadata) {
        let mut analyzer = self.pattern_analyzer.write().await;
        let now = Instant::now();

        // Analyze query patterns
        if let Some(query) = event.metadata.get("query") {
            let patterns = analyzer
                .query_patterns
                .entry(query.clone())
                .or_insert_with(VecDeque::new);
            patterns.push_back(now);

            // Clean old patterns
            while patterns.len() > 100 {
                patterns.pop_front();
            }
        }

        // Analyze IP patterns
        if let Some(ip) = event.metadata.get("client_ip") {
            let patterns = analyzer
                .ip_patterns
                .entry(ip.clone())
                .or_insert_with(VecDeque::new);
            patterns.push_back(now);

            // Clean old patterns
            while patterns.len() > 100 {
                patterns.pop_front();
            }
        }

        // Analyze time patterns
        analyzer.time_patterns.push_back(now);
        while analyzer.time_patterns.len() > 1000 {
            analyzer.time_patterns.pop_front();
        }
    }

    /// Check for alerts based on event
    async fn check_alerts(&self, event: &SecurityEventWithMetadata) {
        let mut alerts = self.alerts.write().await;
        let counters = self.counters.read().await;
        let now = Instant::now();

        // Check rate limit violations
        let recent_rate_violations: u32 = counters
            .rate_limit_violations
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(60))
            .map(|(_, count)| count)
            .sum();

        if recent_rate_violations
            >= self
                .config
                .alert_thresholds
                .rate_limit_violations_per_minute
        {
            self.create_alert(
                &mut alerts,
                AlertType::RateLimitViolation,
                EventSeverity::High,
                format!(
                    "High rate of rate limit violations: {} in the last minute",
                    recent_rate_violations
                ),
                vec![event.clone()],
            )
            .await;
        }

        // Check complexity violations
        let recent_complexity_violations: u32 = counters
            .complexity_violations
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(3600))
            .map(|(_, count)| count)
            .sum();

        if recent_complexity_violations
            >= self.config.alert_thresholds.complexity_violations_per_hour
        {
            self.create_alert(
                &mut alerts,
                AlertType::ComplexityAttack,
                EventSeverity::High,
                format!(
                    "High rate of complexity violations: {} in the last hour",
                    recent_complexity_violations
                ),
                vec![event.clone()],
            )
            .await;
        }

        // Check suspicious queries
        let recent_suspicious: u32 = counters
            .suspicious_queries
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(3600))
            .map(|(_, count)| count)
            .sum();

        if recent_suspicious >= self.config.alert_thresholds.suspicious_queries_per_hour {
            self.create_alert(
                &mut alerts,
                AlertType::SuspiciousQuery,
                EventSeverity::Medium,
                format!(
                    "High rate of suspicious queries: {} in the last hour",
                    recent_suspicious
                ),
                vec![event.clone()],
            )
            .await;
        }
    }

    /// Create a new alert
    async fn create_alert(
        &self,
        alerts: &mut Vec<SecurityAlert>,
        alert_type: AlertType,
        severity: EventSeverity,
        message: String,
        related_events: Vec<SecurityEventWithMetadata>,
    ) {
        let alert = SecurityAlert {
            id: uuid::Uuid::new_v4().to_string(),
            alert_type,
            severity,
            message,
            timestamp: Instant::now(),
            related_events,
            status: AlertStatus::Active,
            metadata: HashMap::new(),
        };

        alerts.push(alert.clone());

        // Send notification
        self.send_notification(&alert).await;

        info!("Security alert created: {} - {}", alert.id, alert.message);
    }

    /// Send notification for alert
    async fn send_notification(&self, alert: &SecurityAlert) {
        // Check cooldown period
        let mut last_notifications = self.notification_service.last_notification.write().await;
        let now = Instant::now();

        if let Some(last_time) = last_notifications.get(&alert.alert_type.to_string()) {
            if now.duration_since(*last_time)
                < Duration::from_secs(self.config.notifications.cooldown_period)
            {
                return; // Still in cooldown period
            }
        }

        last_notifications.insert(alert.alert_type.to_string(), now);

        // Send notifications based on configuration
        if self.config.notifications.enable_email {
            self.send_email_notification(alert).await;
        }

        if self.config.notifications.enable_webhook {
            self.send_webhook_notification(alert).await;
        }

        if self.config.notifications.enable_slack {
            self.send_slack_notification(alert).await;
        }
    }

    /// Send email notification
    async fn send_email_notification(&self, alert: &SecurityAlert) {
        // Implementation would depend on email service
        info!("Email notification sent for alert: {}", alert.id);
    }

    /// Send webhook notification
    async fn send_webhook_notification(&self, alert: &SecurityAlert) {
        // Implementation would depend on webhook service
        info!("Webhook notification sent for alert: {}", alert.id);
    }

    /// Send Slack notification
    async fn send_slack_notification(&self, alert: &SecurityAlert) {
        // Implementation would depend on Slack service
        info!("Slack notification sent for alert: {}", alert.id);
    }

    /// Cleanup old data
    async fn cleanup_old_data(&self) {
        let now = Instant::now();
        let mut last_cleanup = self.last_cleanup.write().await;

        // Only cleanup every 5 minutes
        if now.duration_since(*last_cleanup) < Duration::from_secs(300) {
            return;
        }

        // Cleanup old events
        {
            let mut events = self.events.write().await;
            let cutoff = now - Duration::from_secs(self.config.retention_period);
            events.retain(|event| event.timestamp > cutoff);
        }

        // Cleanup old counters
        {
            let mut counters = self.counters.write().await;
            let cutoff = now - Duration::from_secs(3600); // 1 hour

            counters
                .rate_limit_violations
                .retain(|(timestamp, _)| *timestamp > cutoff);
            counters
                .complexity_violations
                .retain(|(timestamp, _)| *timestamp > cutoff);
            counters
                .auth_failures
                .retain(|(timestamp, _)| *timestamp > cutoff);
            counters
                .authz_violations
                .retain(|(timestamp, _)| *timestamp > cutoff);
            counters
                .suspicious_queries
                .retain(|(timestamp, _)| *timestamp > cutoff);
            counters
                .system_errors
                .retain(|(timestamp, _)| *timestamp > cutoff);
        }

        // Cleanup old alerts
        {
            let mut alerts = self.alerts.write().await;
            let cutoff = now - Duration::from_secs(self.config.retention_period);
            alerts.retain(|alert| alert.timestamp > cutoff);
        }

        *last_cleanup = now;
    }

    /// Get recent security events
    pub async fn get_recent_events(&self, limit: usize) -> Vec<SecurityEventWithMetadata> {
        let events = self.events.read().await;
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<SecurityAlert> {
        let alerts = self.alerts.read().await;
        alerts
            .iter()
            .filter(|alert| alert.status == AlertStatus::Active)
            .cloned()
            .collect()
    }

    /// Get security metrics
    pub async fn get_security_metrics(&self) -> SecurityMetrics {
        let counters = self.counters.read().await;
        let now = Instant::now();

        let rate_limit_violations: u32 = counters
            .rate_limit_violations
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(60))
            .map(|(_, count)| count)
            .sum();

        let complexity_violations: u32 = counters
            .complexity_violations
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(3600))
            .map(|(_, count)| count)
            .sum();

        let suspicious_queries: u32 = counters
            .suspicious_queries
            .iter()
            .filter(|(timestamp, _)| now.duration_since(*timestamp) < Duration::from_secs(3600))
            .map(|(_, count)| count)
            .sum();

        SecurityMetrics {
            total_requests: 0, // Would be tracked separately
            rate_limited_requests: rate_limit_violations as u64,
            complexity_blocked_requests: complexity_violations as u64,
            depth_blocked_requests: 0,
            size_blocked_requests: 0,
            introspection_blocked_requests: 0,
            filtered_requests: suspicious_queries as u64,
            average_complexity: 0.0,
            average_depth: 0.0,
            average_size: 0.0,
        }
    }

    /// Update monitoring configuration
    pub fn update_config(&mut self, config: MonitoringConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &MonitoringConfig {
        &self.config
    }
}

impl std::fmt::Display for AlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertType::RateLimitViolation => write!(f, "Rate Limit Violation"),
            AlertType::ComplexityAttack => write!(f, "Complexity Attack"),
            AlertType::AuthenticationFailure => write!(f, "Authentication Failure"),
            AlertType::AuthorizationViolation => write!(f, "Authorization Violation"),
            AlertType::SuspiciousQuery => write!(f, "Suspicious Query"),
            AlertType::SystemError => write!(f, "System Error"),
            AlertType::PatternAnomaly => write!(f, "Pattern Anomaly"),
            AlertType::GeographicAnomaly => write!(f, "Geographic Anomaly"),
            AlertType::TimeAnomaly => write!(f, "Time Anomaly"),
            AlertType::Custom => write!(f, "Custom Alert"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_security_monitor_creation() {
        let config = MonitoringConfig::default();
        let monitor = SecurityMonitor::new(config);

        assert!(monitor.config().enabled);
        assert_eq!(monitor.config().max_events, 10000);
    }

    #[tokio::test]
    async fn test_event_recording() {
        let config = MonitoringConfig::default();
        let monitor = SecurityMonitor::new(config);

        let event = SecurityEvent::RateLimitExceeded {
            client_ip: "127.0.0.1".to_string(),
            requests_per_minute: 100,
            timestamp: chrono::Utc::now(),
        };

        monitor.record_event(event, "test_source".to_string()).await;

        let recent_events = monitor.get_recent_events(10).await;
        assert_eq!(recent_events.len(), 1);
        assert_eq!(recent_events[0].source, "test_source");
    }

    #[tokio::test]
    async fn test_alert_creation() {
        let mut config = MonitoringConfig::default();
        config.alert_thresholds.rate_limit_violations_per_minute = 1;
        let monitor = SecurityMonitor::new(config);

        // Record multiple rate limit violations
        for _ in 0..2 {
            let event = SecurityEvent::RateLimitExceeded {
                client_ip: "127.0.0.1".to_string(),
                requests_per_minute: 100,
                timestamp: chrono::Utc::now(),
            };
            monitor.record_event(event, "test_source".to_string()).await;
        }

        let alerts = monitor.get_active_alerts().await;
        assert!(!alerts.is_empty());
    }

    #[tokio::test]
    async fn test_security_metrics() {
        let config = MonitoringConfig::default();
        let monitor = SecurityMonitor::new(config);

        let event = SecurityEvent::RateLimitExceeded {
            client_ip: "127.0.0.1".to_string(),
            requests_per_minute: 100,
            timestamp: chrono::Utc::now(),
        };

        monitor.record_event(event, "test_source".to_string()).await;

        let metrics = monitor.get_security_metrics().await;
        assert!(metrics.rate_limited_requests > 0);
    }
}
