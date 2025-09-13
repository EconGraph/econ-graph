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
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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

    /// Get audit logs with filtering and pagination
    pub async fn get_logs(
        pool: &DatabasePool,
        user_id: Option<Uuid>,
        action: Option<String>,
        resource_type: Option<String>,
        limit: i64,
        offset: i64,
    ) -> AppResult<Vec<AuditLog>> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let mut query = audit_logs::table.into_boxed();

        if let Some(user_id) = user_id {
            query = query.filter(audit_logs::user_id.eq(user_id));
        }

        if let Some(action) = action {
            query = query.filter(audit_logs::action.eq(action));
        }

        if let Some(resource_type) = resource_type {
            query = query.filter(audit_logs::resource_type.eq(resource_type));
        }

        let logs = query
            .order(audit_logs::created_at.desc())
            .limit(limit)
            .offset(offset)
            .get_results::<AuditLog>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(logs)
    }

    /// Get total count of audit logs matching filters
    pub async fn get_count(
        pool: &DatabasePool,
        user_id: Option<Uuid>,
        action: Option<String>,
        resource_type: Option<String>,
    ) -> AppResult<i64> {
        let mut conn = pool.get().await.map_err(|e| {
            AppError::DatabaseError(format!("Failed to get database connection: {}", e))
        })?;

        let mut query = audit_logs::table.into_boxed();

        if let Some(user_id) = user_id {
            query = query.filter(audit_logs::user_id.eq(user_id));
        }

        if let Some(action) = action {
            query = query.filter(audit_logs::action.eq(action));
        }

        if let Some(resource_type) = resource_type {
            query = query.filter(audit_logs::resource_type.eq(resource_type));
        }

        let count = query
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        Ok(count)
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

/// System health metrics for admin dashboard
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    pub total_users: i64,
    pub active_users: i64,
    pub total_sessions: i64,
    pub active_sessions: i64,
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

        // Get total sessions
        let total_sessions = crate::schema::user_sessions::table
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Get active sessions (not expired)
        let active_sessions = crate::schema::user_sessions::table
            .filter(crate::schema::user_sessions::expires_at.gt(Utc::now()))
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
            total_sessions,
            active_sessions,
            database_size_mb,
            queue_items,
            unresolved_security_events,
            recent_audit_logs,
        })
    }
}
