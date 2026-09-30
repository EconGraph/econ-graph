//! # GraphQL Context Management
//!
//! Authentication and authorization context for GraphQL resolvers.
//!
//! Each request carries an optional [`Principal`]: the signed-in user and the fine-grained
//! [`Role`]s they hold. Resolvers ask for the one role they need with
//! [`GraphQLContext::require_role`]. The roles come from the access token's `roles` claim (see
//! [`econ_graph_auth::bearer::authenticate`]); `users.role` is not read.

use crate::imports::*;
use econ_graph_auth::{Caller, Principal, Role};
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
    /// A context for `caller`, or for an anonymous request when `None`.
    pub fn new(caller: Option<Caller>) -> Self {
        let (user, principal) = match caller {
            Some(Caller { user, principal }) => {
                debug_assert_eq!(user.id, principal.user_id, "row and token disagree");
                (Some(user), Some(principal))
            }
            None => (None, None),
        };
        Self {
            user,
            principal,
            client_ip: None,
            request_timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// A context for an anonymous request.
    pub fn anonymous() -> Self {
        Self::new(None)
    }

    /// A context for `user` holding exactly `roles`.
    pub fn signed_in(user: User, roles: impl IntoIterator<Item = Role>) -> Self {
        let principal = Principal::new(user.id, roles);
        Self::new(Some(Caller { user, principal }))
    }

    /// Test helper: a context for `user`, holding every role when `staff` is set, otherwise
    /// only the non-staff roles.
    #[cfg(test)]
    pub(crate) fn for_test_user(user: Option<User>, staff: bool) -> Self {
        let Some(user) = user else {
            return Self::anonymous();
        };
        let roles: Vec<Role> = Role::all()
            .iter()
            .copied()
            .filter(|role| staff || !role.is_staff())
            .collect();
        Self::signed_in(user, roles)
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

    /// Check if user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.principal.is_some()
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

/// Helper function to require a fine-grained role from GraphQL context
pub fn require_role<'a>(ctx: &'a Context<'a>, role: Role) -> Result<&'a Principal> {
    let context = ctx.data::<Arc<GraphQLContext>>()?;
    context.require_role(role)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(label: &str) -> User {
        let now = chrono::Utc::now();
        User {
            id: uuid::Uuid::new_v4(),
            email: format!("{label}@example.com"),
            name: label.to_string(),
            avatar_url: None,
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
    fn anonymous_caller_must_authenticate() {
        let ctx = GraphQLContext::anonymous();
        assert!(!ctx.is_authenticated());
        let err = message(ctx.require_role(Role::AnnotationCreate).unwrap_err());
        assert_eq!(err, "Authentication required");
    }

    #[test]
    fn require_role_checks_the_token_roles_not_users_role() {
        // The label says admin, but the token grants only annotation:create.
        let ctx = GraphQLContext::signed_in(user("admin"), [Role::AnnotationCreate]);
        let principal = ctx.require_role(Role::AnnotationCreate).unwrap();
        assert_eq!(principal.user_id, ctx.user.as_ref().unwrap().id);
        let err = message(ctx.require_role(Role::AdminUsersDelete).unwrap_err());
        assert_eq!(err, "Insufficient permissions");

        let staff = GraphQLContext::signed_in(user("viewer"), Role::all().iter().copied());
        for &role in Role::all() {
            assert!(staff.require_role(role).is_ok(), "{role}");
        }
    }

    #[test]
    fn a_token_without_catalog_roles_grants_nothing() {
        let ctx = GraphQLContext::signed_in(user("admin"), []);
        assert!(ctx.is_authenticated());
        for &role in Role::all() {
            assert!(ctx.require_role(role).is_err(), "{role}");
        }
    }
}
