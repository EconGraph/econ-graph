/**
 * REQUIREMENT: Admin audit and security models for admin UI support
 * PURPOSE: Provide audit logging and security event tracking for admin interface
 * This enables comprehensive admin functionality with audit trails and security monitoring
 */
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::schema::{audit_logs, security_events};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

/// Who sent a request, as recorded on audit log entries and security events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo {
    /// The client's address (the backend decides which address that is: the peer, or the
    /// `X-Forwarded-For` entry its trusted proxies vouch for).
    pub ip_address: Option<String>,
    /// The `User-Agent` header, cut to [`ClientInfo::USER_AGENT_MAX_CHARS`] characters.
    pub user_agent: Option<String>,
}

impl ClientInfo {
    /// Longest `User-Agent` kept, in characters; the rest is dropped.
    pub const USER_AGENT_MAX_CHARS: usize = 512;

    /// Client info with `user_agent` trimmed and cut to [`Self::USER_AGENT_MAX_CHARS`]
    /// characters; a blank one counts as missing.
    pub fn new(ip_address: Option<String>, user_agent: Option<&str>) -> Self {
        let user_agent = user_agent
            .map(str::trim)
            .filter(|ua| !ua.is_empty())
            .map(|ua| ua.chars().take(Self::USER_AGENT_MAX_CHARS).collect());
        Self {
            ip_address,
            user_agent,
        }
    }
}

/// Audit log entries that could not be written since the process started.
static AUDIT_LOG_WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);
/// Security events that could not be written since the process started.
static SECURITY_EVENT_WRITE_FAILURES: AtomicU64 = AtomicU64::new(0);

/// How many audit log entries could not be written since the process started. The backend
/// exports it as a Prometheus counter so a broken audit trail can be alerted on.
pub fn audit_log_write_failures() -> u64 {
    AtomicU64::load(&AUDIT_LOG_WRITE_FAILURES, Ordering::Relaxed)
}

/// How many security events could not be written since the process started.
pub fn security_event_write_failures() -> u64 {
    AtomicU64::load(&SECURITY_EVENT_WRITE_FAILURES, Ordering::Relaxed)
}

/// Audit log entry model for tracking admin actions
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = audit_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub details: Option<Value>,
    pub created_at: DateTime<Utc>,
}

/// New audit log entry for insertion
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = audit_logs)]
pub struct NewAuditLog {
    pub user_id: Uuid,
    pub user_name: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub details: Option<Value>,
}

/// Security event model for tracking security incidents
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = security_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SecurityEvent {
    pub id: Uuid,
    pub event_type: String,
    pub user_id: Option<Uuid>,
    pub user_email: Option<String>,
    pub severity: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub description: String,
    pub metadata: Option<Value>,
    pub resolved: Option<bool>,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// New security event for insertion
#[derive(Debug, Serialize, Deserialize, Insertable)]
#[diesel(table_name = security_events)]
pub struct NewSecurityEvent {
    pub event_type: String,
    pub user_id: Option<Uuid>,
    pub user_email: Option<String>,
    pub severity: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub description: String,
    pub metadata: Option<Value>,
}

/// Security event update for resolution
#[derive(Debug, Serialize, Deserialize, AsChangeset)]
#[diesel(table_name = security_events)]
pub struct SecurityEventUpdate {
    pub resolved: Option<bool>,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl AuditLog {
    /// Create a new audit log entry
    pub async fn create(
        pool: &DatabasePool,
        user_id: Uuid,
        user_name: String,
        action: String,
        resource_type: String,
        resource_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        details: Option<Value>,
    ) -> AppResult<AuditLog> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let new_log = NewAuditLog {
            user_id,
            user_name,
            action,
            resource_type,
            resource_id,
            ip_address,
            user_agent,
            details,
        };

        let log = diesel::insert_into(audit_logs::table)
            .values(&new_log)
            .get_result::<AuditLog>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(log)
    }

    /// Get audit logs matching `filter`, newest first
    pub async fn get_logs(
        pool: &DatabasePool,
        filter: &AuditLogFilter,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AuditLog>> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let logs = filter
            .apply(audit_logs::table.into_boxed())
            .order((audit_logs::created_at.desc(), audit_logs::id.desc()))
            .limit(limit)
            .offset(offset)
            .get_results::<AuditLog>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    /// Get total count of audit logs matching `filter`
    pub async fn get_count(pool: &DatabasePool, filter: &AuditLogFilter) -> AppResult<i64> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let count = filter
            .apply(audit_logs::table.into_boxed())
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(count)
    }
}

/// Which audit log entries to read; `None` leaves that column unfiltered.
#[derive(Debug, Clone, Default)]
pub struct AuditLogFilter {
    pub user_id: Option<Uuid>,
    pub action: Option<String>,
    pub resource_type: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl AuditLogFilter {
    fn apply<'a>(
        &self,
        mut query: audit_logs::BoxedQuery<'a, diesel::pg::Pg>,
    ) -> audit_logs::BoxedQuery<'a, diesel::pg::Pg> {
        if let Some(user_id) = self.user_id {
            query = query.filter(audit_logs::user_id.eq(user_id));
        }
        if let Some(action) = &self.action {
            query = query.filter(audit_logs::action.eq(action.clone()));
        }
        if let Some(resource_type) = &self.resource_type {
            query = query.filter(audit_logs::resource_type.eq(resource_type.clone()));
        }
        if let Some(after) = self.created_after {
            query = query.filter(audit_logs::created_at.gt(after));
        }
        if let Some(before) = self.created_before {
            query = query.filter(audit_logs::created_at.lt(before));
        }
        query
    }
}

impl NewAuditLog {
    /// Insert the entry, logging rather than returning a failure: the action it records has
    /// already happened, so a failed write must not turn it into an error for the caller.
    ///
    /// The trade-off: the write is best-effort and runs after the action's own statement,
    /// not in one transaction with it (none of the admin mutations runs in a transaction), so
    /// a database failure in between can leave a committed action with no audit row. Each
    /// such failure is logged at error level and counted in [`audit_log_write_failures`],
    /// which the backend exports for alerting.
    pub async fn record(self, pool: &DatabasePool) {
        let written = async {
            let mut conn = pool.get().await.map_err(|e| e.to_string())?;
            diesel::insert_into(audit_logs::table)
                .values(&self)
                .execute(&mut conn)
                .await
                .map_err(|e| e.to_string())
        }
        .await;
        if let Err(why) = written {
            AUDIT_LOG_WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
            tracing::error!(
                "could not write the audit log entry for {} by {}: {why}",
                self.action,
                self.user_id
            );
        }
    }
}

impl SecurityEvent {
    /// Create a new security event
    pub async fn create(
        pool: &DatabasePool,
        event_type: String,
        user_id: Option<Uuid>,
        user_email: Option<String>,
        severity: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
        description: String,
        metadata: Option<Value>,
    ) -> AppResult<SecurityEvent> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let new_event = NewSecurityEvent {
            event_type,
            user_id,
            user_email,
            severity,
            ip_address,
            user_agent,
            description,
            metadata,
        };

        let event = diesel::insert_into(security_events::table)
            .values(&new_event)
            .get_result::<SecurityEvent>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(event)
    }

    /// Get security events with filtering
    pub async fn get_events(
        pool: &DatabasePool,
        event_type: Option<String>,
        severity: Option<String>,
        resolved: Option<bool>,
        limit: i64,
    ) -> AppResult<Vec<SecurityEvent>> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let mut query = security_events::table.into_boxed();

        if let Some(event_type) = event_type {
            query = query.filter(security_events::event_type.eq(event_type));
        }

        if let Some(severity) = severity {
            query = query.filter(security_events::severity.eq(severity));
        }

        if let Some(resolved) = resolved {
            query = query.filter(security_events::resolved.eq(resolved));
        }

        let events = query
            .order(security_events::created_at.desc())
            .limit(limit)
            .get_results::<SecurityEvent>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(events)
    }

    /// Resolve a security event
    pub async fn resolve(
        pool: &DatabasePool,
        event_id: Uuid,
        resolved_by: Uuid,
    ) -> AppResult<SecurityEvent> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let update = SecurityEventUpdate {
            resolved: Some(true),
            resolved_by: Some(resolved_by),
            resolved_at: Some(Utc::now()),
        };

        let event = diesel::update(security_events::table.find(event_id))
            .set(&update)
            .get_result::<SecurityEvent>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(event)
    }

    /// Get total count of unresolved security events
    pub async fn get_unresolved_count(pool: &DatabasePool) -> AppResult<i64> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let count = security_events::table
            .filter(security_events::resolved.eq(false))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(count)
    }
}

/// `security_events.event_type` values the server writes.
pub mod security_event_type {
    /// A signed-in caller was refused for lacking a role (or the full staff set).
    pub const PERMISSION_DENIED: &str = "permission_denied";
    /// A request carried a valid token for a suspended account.
    pub const SUSPENDED_ACCOUNT: &str = "suspended_account";
}

impl NewSecurityEvent {
    /// A request to `route` from `client` refused because account `user_id` is suspended.
    pub fn suspended_account(user_id: Uuid, route: &str, client: &ClientInfo) -> Self {
        Self {
            event_type: security_event_type::SUSPENDED_ACCOUNT.into(),
            user_id: Some(user_id),
            user_email: None,
            severity: "medium".into(),
            ip_address: client.ip_address.clone(),
            user_agent: client.user_agent.clone(),
            description: format!("refused a {route} request from a suspended account"),
            metadata: Some(serde_json::json!({ "route": route })),
        }
    }

    /// Insert the event, logging rather than returning a failure: recording it must not
    /// change the reply to the request that raised it.
    pub async fn record(self, pool: &DatabasePool) {
        let written = async {
            let mut conn = pool.get().await.map_err(|e| e.to_string())?;
            diesel::insert_into(security_events::table)
                .values(&self)
                .execute(&mut conn)
                .await
                .map_err(|e| e.to_string())
        }
        .await;
        if let Err(why) = written {
            SECURITY_EVENT_WRITE_FAILURES.fetch_add(1, Ordering::Relaxed);
            tracing::error!(
                "could not write a {} security event: {why}",
                self.event_type
            );
        }
    }
}

/// System health metrics for admin dashboard
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    pub total_users: i64,
    pub active_users: i64,
    pub database_size_mb: f64,
    pub queue_items: i64,
    pub unresolved_security_events: i64,
    pub recent_audit_logs: i64,
}

impl SystemHealthMetrics {
    /// Get system health metrics
    pub async fn get_metrics(pool: &DatabasePool) -> AppResult<SystemHealthMetrics> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        // Get total users
        let total_users = crate::schema::users::table
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Get active users (logged in within last 30 days)
        let thirty_days_ago = Utc::now() - chrono::Duration::days(30);
        let active_users = crate::schema::users::table
            .filter(crate::schema::users::last_login_at.gt(thirty_days_ago))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Get queue items
        let queue_items = crate::schema::crawl_queue::table
            .filter(crate::schema::crawl_queue::status.eq("pending"))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Get unresolved security events
        let unresolved_security_events = SecurityEvent::get_unresolved_count(pool).await?;

        // Get recent audit logs (last 24 hours)
        let twenty_four_hours_ago = Utc::now() - chrono::Duration::hours(24);
        let recent_audit_logs = audit_logs::table
            .filter(audit_logs::created_at.gt(twenty_four_hours_ago))
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Database size estimation (placeholder - would need actual database size query)
        let database_size_mb = 0.0; // TODO: Implement actual database size calculation

        Ok(SystemHealthMetrics {
            total_users,
            active_users,
            database_size_mb,
            queue_items,
            unresolved_security_events,
            recent_audit_logs,
        })
    }
}
