/**
 * REQUIREMENT: Enhanced GraphQL context for Keycloak JWT authorization
 * PURPOSE: Use JWT token claims for authorization instead of database lookups
 * This enables fine-grained permissions from JWT tokens without additional API calls
 */
use crate::graphql::context::GraphQLContext;
use async_graphql::{Error as GraphQLError, Result};
use econ_graph_auth::auth::permissions::EconGraphPermission;
use econ_graph_core::auth_models::User;
use std::collections::HashSet;
use tracing::{debug, info, warn};

/// Enhanced GraphQL context that uses JWT token claims for authorization
#[derive(Debug, Clone)]
pub struct KeycloakGraphQLContext {
    pub user: Option<User>,
    pub jwt_roles: HashSet<String>,
    pub jwt_permissions: HashSet<String>,
    pub subscription_tier: Option<String>,
    pub usage_limits: Option<UsageLimits>,
    pub organization: Option<String>,
    pub client_ip: Option<String>,
    pub request_timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: String,
}

#[derive(Debug, Clone)]
pub struct UsageLimits {
    pub charts_per_month: Option<u32>,
    pub data_requests_per_day: Option<u32>,
}

impl KeycloakGraphQLContext {
    /// Create new context from JWT token claims
    pub fn from_jwt_claims(
        user: Option<User>,
        jwt_roles: Vec<String>,
        jwt_permissions: Vec<String>,
        subscription_tier: Option<String>,
        usage_limits: Option<UsageLimits>,
        organization: Option<String>,
    ) -> Self {
        Self {
            user,
            jwt_roles: jwt_roles.into_iter().collect(),
            jwt_permissions: jwt_permissions.into_iter().collect(),
            subscription_tier,
            usage_limits,
            organization,
            client_ip: None,
            request_timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create context with client information
    pub fn with_client_info(
        user: Option<User>,
        jwt_roles: Vec<String>,
        jwt_permissions: Vec<String>,
        subscription_tier: Option<String>,
        usage_limits: Option<UsageLimits>,
        organization: Option<String>,
        client_ip: Option<String>,
    ) -> Self {
        Self {
            user,
            jwt_roles: jwt_roles.into_iter().collect(),
            jwt_permissions: jwt_permissions.into_iter().collect(),
            subscription_tier,
            usage_limits,
            organization,
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

    /// Check if user has a specific JWT permission
    pub fn has_jwt_permission(&self, permission: &str) -> bool {
        self.jwt_permissions.contains(permission)
    }

    /// Check if user has a specific JWT role
    pub fn has_jwt_role(&self, role: &str) -> bool {
        self.jwt_roles.contains(role)
    }

    /// Check if user has subscription tier access
    pub fn has_subscription_tier(&self, required_tier: &str) -> bool {
        if let Some(user_tier) = &self.subscription_tier {
            match (user_tier.as_str(), required_tier) {
                ("enterprise", _) => true, // Enterprise has access to everything
                ("pro", "free" | "pro") => true,
                ("free", "free") => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Check usage limits
    pub fn check_usage_limit(&self, limit_type: &str, current_usage: u32) -> Result<()> {
        if let Some(limits) = &self.usage_limits {
            match limit_type {
                "charts_per_month" => {
                    if let Some(max_charts) = limits.charts_per_month {
                        if current_usage >= max_charts {
                            return Err(GraphQLError::new("Monthly chart limit exceeded"));
                        }
                    }
                }
                "data_requests_per_day" => {
                    if let Some(max_requests) = limits.data_requests_per_day {
                        if current_usage >= max_requests {
                            return Err(GraphQLError::new("Daily data request limit exceeded"));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Convert JWT permission to internal Permission enum
    fn jwt_permission_to_internal(&self, jwt_permission: &str) -> Option<EconGraphPermission> {
        match jwt_permission {
            "data:read" => Some(EconGraphPermission::DataRead),
            "data:write" => Some(EconGraphPermission::DataWrite),
            "graph:create" => Some(EconGraphPermission::GraphCreate),
            "graph:share" => Some(EconGraphPermission::GraphShare),
            "graph:advanced" => Some(EconGraphPermission::GraphAdvanced),
            "export:data" => Some(EconGraphPermission::ExportData),
            "api:access" => Some(EconGraphPermission::ApiAccess),
            "workspace:create" => Some(EconGraphPermission::WorkspaceCreate),
            "customize:data-sources" => Some(EconGraphPermission::CustomizeDataSources),
            "admin:users" => Some(EconGraphPermission::AdminUsers),
            "admin:system" => Some(EconGraphPermission::AdminSystem),
            _ => None,
        }
    }

    /// Check if user has a specific permission (using JWT claims)
    pub fn has_permission(&self, permission: &EconGraphPermission) -> bool {
        // Check if JWT has specific permission
        for jwt_permission in &self.jwt_permissions {
            if let Some(internal_permission) = self.jwt_permission_to_internal(jwt_permission) {
                if std::mem::discriminant(&internal_permission)
                    == std::mem::discriminant(permission)
                {
                    return true;
                }
            }
        }

        // Fallback to role-based permissions
        self.has_role_based_permission(permission)
    }

    /// Check role-based permissions (fallback)
    fn has_role_based_permission(&self, permission: &EconGraphPermission) -> bool {
        if self.has_jwt_role("admin") {
            return true; // Admin has all permissions
        }

        if self.has_jwt_role("analyst") {
            match permission {
                EconGraphPermission::DataRead
                | EconGraphPermission::GraphCreate
                | EconGraphPermission::GraphShare => true,
                _ => false,
            }
        } else if self.has_jwt_role("viewer") {
            match permission {
                EconGraphPermission::DataRead => true,
                _ => false,
            }
        } else {
            false
        }
    }

    /// Require a specific permission
    pub fn require_permission(&self, permission: &EconGraphPermission) -> Result<&User> {
        let user = self.current_user()?;

        if !self.has_permission(permission) {
            warn!(
                "Permission denied for user {} (JWT roles: {:?}, JWT permissions: {:?}): {}",
                user.id,
                self.jwt_roles,
                self.jwt_permissions,
                format!("{:?}", permission)
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!(
            "Permission granted for user {} (JWT roles: {:?}, JWT permissions: {:?}): {}",
            user.id,
            self.jwt_roles,
            self.jwt_permissions,
            format!("{:?}", permission)
        );

        Ok(user)
    }

    /// Check subscription-based feature access
    pub fn require_subscription_tier(&self, required_tier: &str) -> Result<&User> {
        let user = self.current_user()?;

        if !self.has_subscription_tier(required_tier) {
            warn!(
                "Subscription tier insufficient for user {} (current: {:?}, required: {}): {}",
                user.id,
                self.subscription_tier,
                required_tier,
                "Feature requires higher subscription tier"
            );
            return Err(GraphQLError::new(format!(
                "Feature requires {} subscription",
                required_tier
            )));
        }

        debug!(
            "Subscription tier check passed for user {} (tier: {:?})",
            user.id, self.subscription_tier
        );

        Ok(user)
    }

    /// Check organization-based access
    pub fn require_organization_access(&self, target_organization: &str) -> Result<&User> {
        let user = self.current_user()?;

        // Admin can access any organization
        if self.has_jwt_role("admin") {
            return Ok(user);
        }

        // Users can only access their own organization
        if let Some(user_org) = &self.organization {
            if user_org == target_organization {
                return Ok(user);
            }
        }

        warn!(
            "Organization access denied for user {} (user org: {:?}, target org: {})",
            user.id, self.organization, target_organization
        );

        Err(GraphQLError::new("Access denied for this organization"))
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
        let jwt_roles = format!("{:?}", self.jwt_roles);
        let jwt_permissions = format!("{:?}", self.jwt_permissions);
        let client_ip = self.client_ip.as_deref().unwrap_or("unknown");

        if allowed {
            info!(
                "JWT Authorization granted - User: {} (JWT roles: {}, JWT permissions: {}), Action: {}, Resource: {}, IP: {}, Request: {}, Reason: {}",
                user_id,
                jwt_roles,
                jwt_permissions,
                action,
                resource,
                client_ip,
                self.request_id,
                reason.unwrap_or("JWT permission check passed")
            );
        } else {
            warn!(
                "JWT Authorization denied - User: {} (JWT roles: {}, JWT permissions: {}), Action: {}, Resource: {}, IP: {}, Request: {}, Reason: {}",
                user_id,
                jwt_roles,
                jwt_permissions,
                action,
                resource,
                client_ip,
                self.request_id,
                reason.unwrap_or("JWT permission check failed")
            );
        }
    }
}

impl From<KeycloakGraphQLContext> for GraphQLContext {
    fn from(keycloak_ctx: KeycloakGraphQLContext) -> Self {
        // Convert JWT permissions to internal permissions
        let mut permissions = std::collections::HashSet::new();
        for jwt_permission in &keycloak_ctx.jwt_permissions {
            if let Some(internal_permission) =
                keycloak_ctx.jwt_permission_to_internal(jwt_permission)
            {
                permissions.insert(internal_permission);
            }
        }

        // Add role-based permissions
        for jwt_role in &keycloak_ctx.jwt_roles {
            match jwt_role.as_str() {
                "admin" => {
                    permissions.insert(EconGraphPermission::AdminSystem);
                    permissions.insert(EconGraphPermission::AdminUsers);
                    permissions.insert(EconGraphPermission::AdminSecurity);
                }
                "analyst" => {
                    permissions.insert(EconGraphPermission::DataRead);
                    permissions.insert(EconGraphPermission::GraphCreate);
                    permissions.insert(EconGraphPermission::GraphShare);
                }
                "viewer" => {
                    permissions.insert(EconGraphPermission::DataRead);
                }
                _ => {}
            }
        }

        // Convert to the old Permission enum for compatibility
        let mut old_permissions = std::collections::HashSet::new();
        for perm in permissions {
            match perm {
                EconGraphPermission::DataRead => {
                    old_permissions.insert(crate::graphql::context::Permission::ReadEconomicData);
                }
                EconGraphPermission::GraphCreate => {
                    old_permissions.insert(crate::graphql::context::Permission::CreateCharts);
                }
                EconGraphPermission::GraphShare => {
                    old_permissions.insert(crate::graphql::context::Permission::ShareCharts);
                }
                EconGraphPermission::AdminUsers => {
                    old_permissions.insert(crate::graphql::context::Permission::UpdateUsers);
                }
                EconGraphPermission::AdminSystem => {
                    old_permissions.insert(crate::graphql::context::Permission::UpdateSystemConfig);
                }
                EconGraphPermission::AdminSecurity => {
                    old_permissions.insert(crate::graphql::context::Permission::ManageSecurity);
                }
                _ => {} // Skip permissions that don't have equivalents
            }
        }

        GraphQLContext {
            user: None,      // TODO: Map Keycloak user to core User type
            user_role: None, // We don't need this anymore
            permissions: old_permissions,
            client_ip: keycloak_ctx.client_ip,
            request_timestamp: keycloak_ctx.request_timestamp,
            request_id: keycloak_ctx.request_id,
        }
    }
}
