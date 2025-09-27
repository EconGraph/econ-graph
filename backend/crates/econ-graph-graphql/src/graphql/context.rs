//! # GraphQL Context Management
//!
//! This module provides authentication and authorization context for GraphQL resolvers.
//! It ensures proper security and role-based access control throughout the API.
//!
//! # Design Principles
//!
//! 1. **Security First**: All context operations prioritize security and validation
//! 2. **Role-Based Access**: Comprehensive role-based access control implementation
//! 3. **Audit Trail**: All authorization decisions are logged for security
//! 4. **Performance**: Context operations are optimized for minimal overhead
//! 5. **Principle of Least Privilege**: Users only get minimum required permissions
//! 6. **Defense in Depth**: Multiple layers of authorization checks
//!
//! # Quality Standards
//!
//! - All context operations must be secure and validated
//! - Authorization checks must be comprehensive and consistent
//! - Error messages must not leak sensitive information
//! - All context operations must have comprehensive documentation
//! - All authorization decisions must be auditable
//! - Permission checks must be granular and specific

use crate::imports::*;
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// User permissions for fine-grained access control
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

/// User roles with associated permissions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    SuperAdmin,
    Admin,
    Analyst,
    Viewer,
    Guest,
}

impl UserRole {
    /// Get permissions for a role
    pub fn get_permissions(&self) -> HashSet<Permission> {
        match self {
            UserRole::SuperAdmin => {
                // Super admin has all permissions
                vec![
                    Permission::ReadEconomicData,
                    Permission::WriteEconomicData,
                    Permission::DeleteEconomicData,
                    Permission::ReadUsers,
                    Permission::CreateUsers,
                    Permission::UpdateUsers,
                    Permission::DeleteUsers,
                    Permission::ReadSystemConfig,
                    Permission::UpdateSystemConfig,
                    Permission::ManageCrawlers,
                    Permission::ViewLogs,
                    Permission::ManageSecurity,
                    Permission::CreateCharts,
                    Permission::UpdateCharts,
                    Permission::DeleteCharts,
                    Permission::ShareCharts,
                    Permission::CreateAnnotations,
                    Permission::UpdateAnnotations,
                    Permission::DeleteAnnotations,
                    Permission::AccessGraphQL,
                    Permission::AccessREST,
                    Permission::AccessMCP,
                    Permission::ViewMetrics,
                    Permission::ViewHealth,
                    Permission::ViewSecurityEvents,
                ]
                .into_iter()
                .collect()
            }
            UserRole::Admin => {
                let mut permissions = HashSet::new();
                permissions.insert(Permission::ReadEconomicData);
                permissions.insert(Permission::WriteEconomicData);
                permissions.insert(Permission::DeleteEconomicData);
                permissions.insert(Permission::ReadUsers);
                permissions.insert(Permission::CreateUsers);
                permissions.insert(Permission::UpdateUsers);
                permissions.insert(Permission::ReadSystemConfig);
                permissions.insert(Permission::UpdateSystemConfig);
                permissions.insert(Permission::ManageCrawlers);
                permissions.insert(Permission::ViewLogs);
                permissions.insert(Permission::CreateCharts);
                permissions.insert(Permission::UpdateCharts);
                permissions.insert(Permission::DeleteCharts);
                permissions.insert(Permission::ShareCharts);
                permissions.insert(Permission::CreateAnnotations);
                permissions.insert(Permission::UpdateAnnotations);
                permissions.insert(Permission::DeleteAnnotations);
                permissions.insert(Permission::AccessGraphQL);
                permissions.insert(Permission::AccessREST);
                permissions.insert(Permission::AccessMCP);
                permissions.insert(Permission::ViewMetrics);
                permissions.insert(Permission::ViewHealth);
                permissions.insert(Permission::ViewSecurityEvents);
                permissions
            }
            UserRole::Analyst => {
                let mut permissions = HashSet::new();
                permissions.insert(Permission::ReadEconomicData);
                permissions.insert(Permission::CreateCharts);
                permissions.insert(Permission::UpdateCharts);
                permissions.insert(Permission::ShareCharts);
                permissions.insert(Permission::CreateAnnotations);
                permissions.insert(Permission::UpdateAnnotations);
                permissions.insert(Permission::AccessGraphQL);
                permissions.insert(Permission::ViewMetrics);
                permissions
            }
            UserRole::Viewer => {
                let mut permissions = HashSet::new();
                permissions.insert(Permission::ReadEconomicData);
                permissions.insert(Permission::AccessGraphQL);
                permissions
            }
            UserRole::Guest => {
                let mut permissions = HashSet::new();
                permissions.insert(Permission::ReadEconomicData);
                permissions
            }
        }
    }

    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.get_permissions().contains(permission)
    }

    /// Get role from string
    pub fn from_role_string(role_str: &str) -> Option<Self> {
        match role_str.to_lowercase().as_str() {
            "super_admin" | "superadmin" => Some(UserRole::SuperAdmin),
            "admin" => Some(UserRole::Admin),
            "analyst" => Some(UserRole::Analyst),
            "viewer" => Some(UserRole::Viewer),
            "guest" => Some(UserRole::Guest),
            _ => None,
        }
    }

    /// Convert role to string
    pub fn to_string(&self) -> String {
        match self {
            UserRole::SuperAdmin => "super_admin".to_string(),
            UserRole::Admin => "admin".to_string(),
            UserRole::Analyst => "analyst".to_string(),
            UserRole::Viewer => "viewer".to_string(),
            UserRole::Guest => "guest".to_string(),
        }
    }
}

/// GraphQL context containing the authenticated user and enhanced security
#[derive(Clone)]
pub struct GraphQLContext {
    pub user: Option<User>,
    /// User's role with permissions
    pub user_role: Option<UserRole>,
    /// User's specific permissions (for fine-grained control)
    pub permissions: HashSet<Permission>,
    /// Client IP address for security logging
    pub client_ip: Option<String>,
    /// Request timestamp for audit trail
    pub request_timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID for tracking
    pub request_id: String,
}

impl GraphQLContext {
    /// Create a new GraphQL context
    pub fn new(user: Option<User>) -> Self {
        let user_role = user
            .as_ref()
            .and_then(|u| UserRole::from_role_string(&u.role));
        let permissions = user_role
            .as_ref()
            .map(|role| role.get_permissions())
            .unwrap_or_default();

        Self {
            user,
            user_role,
            permissions,
            client_ip: None,
            request_timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create a new GraphQL context with client information
    pub fn new_with_client_info(user: Option<User>, client_ip: Option<String>) -> Self {
        let user_role = user
            .as_ref()
            .and_then(|u| UserRole::from_role_string(&u.role));
        let permissions = user_role
            .as_ref()
            .map(|role| role.get_permissions())
            .unwrap_or_default();

        Self {
            user,
            user_role,
            permissions,
            client_ip,
            request_timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Get the current authenticated user
    pub fn current_user(&self) -> Result<&User> {
        self.user
            .as_ref()
            .ok_or_else(|| GraphQLError::new("Authentication required"))
    }

    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions.contains(permission)
    }

    /// Require a specific permission
    pub fn require_permission(&self, permission: &Permission) -> Result<&User> {
        let user = self.current_user()?;

        if !self.has_permission(permission) {
            warn!(
                "Permission denied for user {} (role: {}): {}",
                user.id,
                user.role,
                format!("{:?}", permission)
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!(
            "Permission granted for user {} (role: {}): {}",
            user.id,
            user.role,
            format!("{:?}", permission)
        );

        Ok(user)
    }

    /// Check if user has any of the specified permissions
    pub fn has_any_permission(&self, permissions: &[Permission]) -> bool {
        permissions.iter().any(|p| self.has_permission(p))
    }

    /// Require any of the specified permissions
    pub fn require_any_permission(&self, permissions: &[Permission]) -> Result<&User> {
        let user = self.current_user()?;

        if !self.has_any_permission(permissions) {
            warn!(
                "Permission denied for user {} (role: {}): any of {:?}",
                user.id, user.role, permissions
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!(
            "Permission granted for user {} (role: {}): any of {:?}",
            user.id, user.role, permissions
        );

        Ok(user)
    }

    /// Check if user has all of the specified permissions
    pub fn has_all_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(p))
    }

    /// Require all of the specified permissions
    pub fn require_all_permissions(&self, permissions: &[Permission]) -> Result<&User> {
        let user = self.current_user()?;

        if !self.has_all_permissions(permissions) {
            warn!(
                "Permission denied for user {} (role: {}): all of {:?}",
                user.id, user.role, permissions
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!(
            "Permission granted for user {} (role: {}): all of {:?}",
            user.id, user.role, permissions
        );

        Ok(user)
    }

    /// Check if the current user has admin role
    pub fn require_admin(&self) -> Result<&User> {
        self.require_any_permission(&[
            Permission::ReadSystemConfig,
            Permission::UpdateSystemConfig,
            Permission::ManageCrawlers,
            Permission::ViewLogs,
            Permission::ManageSecurity,
        ])
    }

    /// Check if the current user has super admin role
    pub fn require_super_admin(&self) -> Result<&User> {
        self.require_permission(&Permission::ManageSecurity)
    }

    /// Check if the current user can manage the specified user
    pub fn can_manage_user(&self, target_user_id: uuid::Uuid) -> Result<&User> {
        let user = self.current_user()?;

        // Super admins can manage anyone
        if self.has_permission(&Permission::ManageSecurity) {
            return Ok(user);
        }

        // Admins can manage users
        if self.has_permission(&Permission::UpdateUsers) {
            return Ok(user);
        }

        // Users can only manage themselves
        if user.id == target_user_id {
            return Ok(user);
        }

        Err(GraphQLError::new(
            "Insufficient permissions to manage this user",
        ))
    }

    /// Check if user can read economic data
    pub fn can_read_economic_data(&self) -> Result<&User> {
        self.require_permission(&Permission::ReadEconomicData)
    }

    /// Check if user can write economic data
    pub fn can_write_economic_data(&self) -> Result<&User> {
        self.require_permission(&Permission::WriteEconomicData)
    }

    /// Check if user can delete economic data
    pub fn can_delete_economic_data(&self) -> Result<&User> {
        self.require_permission(&Permission::DeleteEconomicData)
    }

    /// Check if user can create charts
    pub fn can_create_charts(&self) -> Result<&User> {
        self.require_permission(&Permission::CreateCharts)
    }

    /// Check if user can update charts
    pub fn can_update_charts(&self) -> Result<&User> {
        self.require_permission(&Permission::UpdateCharts)
    }

    /// Check if user can delete charts
    pub fn can_delete_charts(&self) -> Result<&User> {
        self.require_permission(&Permission::DeleteCharts)
    }

    /// Check if user can create annotations
    pub fn can_create_annotations(&self) -> Result<&User> {
        self.require_permission(&Permission::CreateAnnotations)
    }

    /// Check if user can update annotations
    pub fn can_update_annotations(&self) -> Result<&User> {
        self.require_permission(&Permission::UpdateAnnotations)
    }

    /// Check if user can delete annotations
    pub fn can_delete_annotations(&self) -> Result<&User> {
        self.require_permission(&Permission::DeleteAnnotations)
    }

    /// Check if user can manage crawlers
    pub fn can_manage_crawlers(&self) -> Result<&User> {
        self.require_permission(&Permission::ManageCrawlers)
    }

    /// Check if user can view logs
    pub fn can_view_logs(&self) -> Result<&User> {
        self.require_permission(&Permission::ViewLogs)
    }

    /// Check if user can view metrics
    pub fn can_view_metrics(&self) -> Result<&User> {
        self.require_permission(&Permission::ViewMetrics)
    }

    /// Check if user can view security events
    pub fn can_view_security_events(&self) -> Result<&User> {
        self.require_permission(&Permission::ViewSecurityEvents)
    }

    /// Get user's role
    pub fn get_user_role(&self) -> Option<&UserRole> {
        self.user_role.as_ref()
    }

    /// Get user's permissions
    pub fn get_permissions(&self) -> &HashSet<Permission> {
        &self.permissions
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.user.is_some()
    }

    /// Get client IP address
    pub fn get_client_ip(&self) -> Option<&String> {
        self.client_ip.as_ref()
    }

    /// Get request timestamp
    pub fn get_request_timestamp(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.request_timestamp
    }

    /// Get request ID
    pub fn get_request_id(&self) -> &String {
        &self.request_id
    }

    /// Log authorization decision for audit trail
    pub fn log_authorization_decision(
        &self,
        action: &str,
        resource: &str,
        allowed: bool,
        reason: Option<&str>,
    ) {
        let user_id = self
            .user
            .as_ref()
            .map(|u| u.id.to_string())
            .unwrap_or("anonymous".to_string());
        let role = self
            .user
            .as_ref()
            .map(|u| u.role.clone())
            .unwrap_or("none".to_string());
        let client_ip = self.client_ip.as_deref().unwrap_or("unknown");

        if allowed {
            info!(
                "Authorization granted - User: {} ({}), Action: {}, Resource: {}, IP: {}, Request: {}, Reason: {}",
                user_id,
                role,
                action,
                resource,
                client_ip,
                self.request_id,
                reason.unwrap_or("Permission check passed")
            );
        } else {
            warn!(
                "Authorization denied - User: {} ({}), Action: {}, Resource: {}, IP: {}, Request: {}, Reason: {}",
                user_id,
                role,
                action,
                resource,
                client_ip,
                self.request_id,
                reason.unwrap_or("Permission check failed")
            );
        }
    }
}

/// Helper function to get the current user from GraphQL context
pub fn current_user<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.current_user()
}

/// Helper function to require admin role from GraphQL context
pub fn require_admin<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_admin()
}

/// Helper function to require super admin role from GraphQL context
pub fn require_super_admin<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_super_admin()
}

/// Helper function to check if user can manage another user
pub fn can_manage_user<'a>(ctx: &'a Context<'a>, target_user_id: uuid::Uuid) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_manage_user(target_user_id)
}

/// Helper function to require a specific permission
pub fn require_permission<'a>(ctx: &'a Context<'a>, permission: &Permission) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_permission(permission)
}

/// Helper function to require any of the specified permissions
pub fn require_any_permission<'a>(
    ctx: &'a Context<'a>,
    permissions: &[Permission],
) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_any_permission(permissions)
}

/// Helper function to require all of the specified permissions
pub fn require_all_permissions<'a>(
    ctx: &'a Context<'a>,
    permissions: &[Permission],
) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_all_permissions(permissions)
}

/// Helper function to check if user can read economic data
pub fn can_read_economic_data<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_read_economic_data()
}

/// Helper function to check if user can write economic data
pub fn can_write_economic_data<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_write_economic_data()
}

/// Helper function to check if user can create charts
pub fn can_create_charts<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_create_charts()
}

/// Helper function to check if user can create annotations
pub fn can_create_annotations<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_create_annotations()
}

/// Helper function to check if user can manage crawlers
pub fn can_manage_crawlers<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_manage_crawlers()
}

/// Helper function to check if user can view metrics
pub fn can_view_metrics<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_view_metrics()
}

/// Helper function to check if user can view security events
pub fn can_view_security_events<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.can_view_security_events()
}
