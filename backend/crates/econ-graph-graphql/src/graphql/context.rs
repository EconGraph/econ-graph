//! # GraphQL Context Management
//!
//! Authentication and authorization context for GraphQL resolvers.
//!
//! Each request carries an optional [`Principal`]: the signed-in user and the fine-grained
//! [`Role`]s they hold. Resolvers ask for the one role they need with
//! [`GraphQLContext::require_role`]. Until Keycloak issues tokens with a `roles` claim, the roles
//! come from the user's legacy `users.role` column through [`roles_for_legacy`].

use crate::imports::*;
use econ_graph_auth::{roles_for_legacy, Principal, Role};
use tracing::{debug, warn};

/// GraphQL context containing the authenticated user and their fine-grained roles
#[derive(Clone)]
pub struct GraphQLContext {
    pub user: Option<User>,
    /// The signed-in caller and their roles; `None` for anonymous requests
    pub principal: Option<Principal>,
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
        Self::new_with_client_info(user, None)
    }

    /// Create a new GraphQL context with client information
    pub fn new_with_client_info(user: Option<User>, client_ip: Option<String>) -> Self {
        let principal = user.as_ref().map(principal_for_legacy_user);

        Self {
            user,
            principal,
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

    /// The signed-in caller and their roles
    pub fn principal(&self) -> Result<&Principal> {
        self.principal
            .as_ref()
            .ok_or_else(|| GraphQLError::new("Authentication required"))
    }

    /// Require the signed-in caller to hold `role`.
    ///
    /// Looks only at the [`Principal`], so it works for a caller known only from a token.
    /// Resolvers that need the caller's database row call [`Self::current_user`] as well.
    pub fn require_role(&self, role: Role) -> Result<&Principal> {
        let principal = self.principal()?;

        if econ_graph_auth::authorize(principal, role).is_err() {
            warn!(
                request_id = %self.request_id,
                client_ip = self.client_ip.as_deref().unwrap_or("unknown"),
                "Role {} denied for user {}",
                role,
                principal.user_id
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!("Role {} granted for user {}", role, principal.user_id);
        Ok(principal)
    }

    /// Require every staff role.
    ///
    /// Kept only until each resolver asks for its specific role with [`Self::require_role`].
    /// Legacy `admin` and `super_admin` users hold every staff role, so this passes exactly
    /// for them, as before. Requiring all of them, not any, keeps a narrow staff composite
    /// (say, audit readers) out of resolvers still guarded this way.
    pub fn require_admin(&self) -> Result<&User> {
        let user = self.current_user()?;
        let is_admin = self.principal.as_ref().is_some_and(|principal| {
            Role::all()
                .iter()
                .filter(|role| role.is_staff())
                .all(|&role| principal.has_role(role))
        });

        if !is_admin {
            warn!(
                request_id = %self.request_id,
                client_ip = self.client_ip.as_deref().unwrap_or("unknown"),
                "Staff roles required, denied for user {}",
                user.id
            );
            return Err(GraphQLError::new("Insufficient permissions"));
        }

        debug!("Staff roles granted for user {}", user.id);
        Ok(user)
    }

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.principal.is_some()
    }
}

/// Build a principal from a user's legacy `users.role`; an unknown value gets no roles.
fn principal_for_legacy_user(user: &User) -> Principal {
    let roles = roles_for_legacy(&user.role).unwrap_or_else(|err| {
        warn!("User {} has no roles: {}", user.id, err);
        Default::default()
    });
    Principal {
        user_id: user.id,
        roles,
    }
}

/// Helper function to get the current user from GraphQL context
pub fn current_user<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.current_user()
}

/// The signed-in caller's id, or `None` for an anonymous request.
///
/// Unlike `current_user(ctx).ok()`, a request with no `GraphQLContext` at all is an
/// error rather than silently treated as anonymous.
pub fn current_user_id_opt(ctx: &Context<'_>) -> Result<Option<uuid::Uuid>> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    Ok(context.user.as_ref().map(|user| user.id))
}

/// Helper function to require admin role from GraphQL context
pub fn require_admin<'a>(ctx: &'a Context<'a>) -> Result<&'a User> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_admin()
}

/// Helper function to require a fine-grained role from GraphQL context
pub fn require_role<'a>(ctx: &'a Context<'a>, role: Role) -> Result<&'a Principal> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_role(role)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: &str) -> User {
        let now = chrono::Utc::now();
        User {
            id: uuid::Uuid::new_v4(),
            email: format!("{role}@example.com"),
            name: role.to_string(),
            avatar_url: None,
            provider: "email".to_string(),
            provider_id: None,
            password_hash: None,
            role: role.to_string(),
            organization: None,
            theme: "light".to_string(),
            default_chart_type: "line".to_string(),
            notifications_enabled: true,
            collaboration_enabled: true,
            is_active: true,
            email_verified: true,
            created_at: now,
            updated_at: now,
            last_login_at: None,
        }
    }

    fn message(err: GraphQLError) -> String {
        err.message
    }

    #[test]
    fn require_admin_passes_exactly_for_legacy_admins() {
        for role in ["admin", "super_admin", "superadmin", "Admin", "SUPER_ADMIN"] {
            let ctx = GraphQLContext::new(Some(user(role)));
            assert!(ctx.require_admin().is_ok(), "{role}");
        }
        for role in ["analyst", "viewer", "guest", "root", ""] {
            let ctx = GraphQLContext::new(Some(user(role)));
            let err = message(ctx.require_admin().unwrap_err());
            assert_eq!(err, "Insufficient permissions", "{role}");
        }
    }

    #[test]
    fn require_admin_rejects_a_partial_staff_set() {
        let mut ctx = GraphQLContext::new(Some(user("viewer")));
        ctx.principal = Some(Principal::new(
            ctx.user.as_ref().unwrap().id,
            [Role::AdminAuditRead],
        ));
        assert!(ctx.require_admin().is_err());
    }

    #[test]
    fn anonymous_caller_must_authenticate() {
        let ctx = GraphQLContext::new(None);
        let err = message(ctx.require_admin().unwrap_err());
        assert_eq!(err, "Authentication required");
        let err = message(ctx.require_role(Role::AnnotationCreate).unwrap_err());
        assert_eq!(err, "Authentication required");
    }

    #[test]
    fn require_role_checks_the_principal() {
        let viewer = GraphQLContext::new(Some(user("viewer")));
        let principal = viewer.require_role(Role::AnnotationCreate).unwrap();
        assert_eq!(principal.user_id, viewer.user.as_ref().unwrap().id);
        let err = message(viewer.require_role(Role::AdminUsersDelete).unwrap_err());
        assert_eq!(err, "Insufficient permissions");

        let admin = GraphQLContext::new(Some(user("admin")));
        for &role in Role::all() {
            assert!(admin.require_role(role).is_ok(), "{role}");
        }
    }

    #[test]
    fn unknown_legacy_role_gets_no_roles() {
        let ctx = GraphQLContext::new(Some(user("root")));
        assert!(ctx.principal.as_ref().unwrap().roles.is_empty());
        for &role in Role::all() {
            assert!(ctx.require_role(role).is_err(), "{role}");
        }
    }
}
