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
use uuid::Uuid;

/// Audit log entry model for tracking admin actions
#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Selectable, Identifiable)]
#[diesel(table_name = audit_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuditLog {
    pub id: Uuid,
    /// User who performed the action; `None` once that user has been deleted
    /// (`ON DELETE SET NULL`), in which case `user_name` still names them.
    pub user_id: Option<Uuid>,
    /// Name of the user at the time of the action, kept after the user is deleted.
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
    /// Whether the user who performed this action has since been deleted.
    pub fn user_deleted(&self) -> bool {
        self.user_id.is_none()
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::user::NewUser;
    use crate::schema::users;

    static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

    /// Pool on `DATABASE_URL` with the embedded migrations applied (once per test binary).
    async fn test_pool() -> DatabasePool {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must point at a Postgres database for admin model tests");
        MIGRATED
            .get_or_init(|| async {
                crate::database::run_migrations(&url)
                    .await
                    .expect("migrations failed");
            })
            .await;
        crate::database::create_pool(&url)
            .await
            .expect("Failed to connect to test database")
    }

    /// Deleting a user keeps their audit rows: `user_id` becomes NULL and `user_name`
    /// still names them (migration 2026-10-24-000100_audit_logs_keep_on_user_delete).
    #[tokio::test]
    async fn audit_logs_survive_user_deletion() {
        let pool = &test_pool().await;
        let mut conn = pool.get().await.expect("connection");

        let email = format!("audit-delete-{}@example.com", Uuid::new_v4());
        let user_id: Uuid = diesel::insert_into(users::table)
            .values(&NewUser {
                email,
                name: "Audit Deletion Tester".to_string(),
                avatar_url: None,
                organization: None,
                theme: "light".to_string(),
                default_chart_type: "line".to_string(),
                notifications_enabled: true,
                collaboration_enabled: true,
                email_verified: false,
            })
            .returning(users::id)
            .get_result(&mut conn)
            .await
            .expect("insert user");

        let mut log_ids = Vec::new();
        for action in ["login", "update_series"] {
            let log = AuditLog::create(
                pool,
                user_id,
                "Audit Deletion Tester".to_string(),
                action.to_string(),
                "user".to_string(),
                Some(user_id.to_string()),
                None,
                None,
                None,
            )
            .await
            .expect("create audit log");
            assert_eq!(log.user_id, Some(user_id));
            assert!(!log.user_deleted());
            log_ids.push(log.id);
        }

        let deleted = diesel::delete(users::table.filter(users::id.eq(user_id)))
            .execute(&mut conn)
            .await
            .expect("delete user");
        assert_eq!(deleted, 1);

        let rows: Vec<AuditLog> = audit_logs::table
            .filter(audit_logs::id.eq_any(&log_ids))
            .select(AuditLog::as_select())
            .load(&mut conn)
            .await
            .expect("load audit logs");
        assert_eq!(rows.len(), 2, "audit rows must survive the user's deletion");
        for row in rows {
            assert_eq!(row.user_id, None);
            assert!(row.user_deleted());
            assert_eq!(row.user_name, "Audit Deletion Tester");
        }
    }
}
