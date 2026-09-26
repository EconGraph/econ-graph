//! # Fine-grained role catalog
//!
//! The backend authorizes requests with fine-grained roles only. Each [`Role`] is one
//! capability, named `resource:action` (for example `annotation:create`,
//! `admin.users:suspend`). Plans, organization roles and staff levels never appear here: the
//! identity provider composes them out of these roles. A Keycloak protocol mapper on the
//! `econ-graph-api` client puts the flattened client roles in a top-level `roles` claim
//! (Keycloak's default `resource_access.<client>.roles` is not used); see
//! `docs/roadmap/auth-plans-permissions.md`.
//!
//! This enum is the source of truth for the catalog. The Keycloak realm defines the same set
//! as client roles; `econ-graph-roles --list` prints the catalog for the CI check that
//! compares the two (AUTH-4). It prints in declaration order, so compare the sets sorted.
//!
//! A role is in the catalog only once something checks it. Names from the earlier
//! `EconGraphPermission` draft (PR #149) such as `export:image`, `export:watermark_free` or
//! `analytics:advanced` are added, in the same naming style, by the change that first
//! enforces them.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Declares [`Role`] with its wire names, so the variant list, the strings and
/// [`Role::all`] cannot drift apart.
macro_rules! role_catalog {
    ($( $(#[$doc:meta])* $variant:ident => $name:literal, )+) => {
        /// A fine-grained role: one capability the backend checks, named `resource:action`.
        ///
        /// Serializes to and from its `resource:action` string. Parsing an unknown string fails.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Role {
            $( $(#[$doc])* $variant, )+
        }

        impl Role {
            /// Every role in the catalog, in declaration order.
            pub const ALL: &'static [Role] = &[ $( Role::$variant, )+ ];

            /// The role's wire name, as it appears in the token's `roles` claim.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Role::$variant => $name, )+
                }
            }
        }

        impl FromStr for Role {
            type Err = UnknownRole;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( $name => Ok(Role::$variant), )+
                    _ => Err(UnknownRole(s.to_string())),
                }
            }
        }
    };
}

role_catalog! {
    // Collaboration
    /// Create an annotation on a series.
    AnnotationCreate => "annotation:create",
    /// Comment on an annotation the caller can see.
    AnnotationComment => "annotation:comment",
    /// Share a chart the caller owns with another user.
    ChartShare => "chart:share",

    // API access
    /// Call the MCP endpoint (`/mcp`).
    ApiMcp => "api:mcp",

    // Staff: user administration
    /// List and read any user's account.
    AdminUsersRead => "admin.users:read",
    /// Create user accounts.
    AdminUsersCreate => "admin.users:create",
    /// Update any user's account.
    AdminUsersUpdate => "admin.users:update",
    /// Delete user accounts.
    AdminUsersDelete => "admin.users:delete",
    /// Suspend and reactivate user accounts.
    AdminUsersSuspend => "admin.users:suspend",
    /// List user sessions.
    AdminSessionsRead => "admin.sessions:read",
    /// End a user's sessions (force logout).
    AdminSessionsRevoke => "admin.sessions:revoke",

    // Staff: operations
    /// Trigger and manage crawls.
    AdminCrawlersManage => "admin.crawlers:manage",
    /// Read system health.
    AdminSystemRead => "admin.system:read",
    /// Read security events.
    AdminSecurityRead => "admin.security:read",
    /// Read the audit log.
    AdminAuditRead => "admin.audit:read",
}

impl Role {
    /// Every role in the catalog, in declaration order.
    pub fn all() -> &'static [Role] {
        Self::ALL
    }

    /// Whether this is a staff role (its resource is under `admin.`).
    pub fn is_staff(self) -> bool {
        self.as_str().starts_with("admin.")
    }
}

/// The catalog's wire names, one per line, for comparing against the Keycloak realm export.
pub fn role_list() -> String {
    Role::ALL.iter().map(|role| format!("{role}\n")).collect()
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Role {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Role {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// A role string that is not in the catalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown role: {0:?}")]
pub struct UnknownRole(pub String);

/// The authenticated caller of a request: who they are and which fine-grained roles they hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    /// The caller's user id (the token's `sub`).
    pub user_id: Uuid,
    /// The fine-grained roles the caller holds.
    pub roles: BTreeSet<Role>,
}

impl Principal {
    /// Build a principal from a user id and its roles.
    pub fn new(user_id: Uuid, roles: impl IntoIterator<Item = Role>) -> Self {
        Self {
            user_id,
            roles: roles.into_iter().collect(),
        }
    }

    /// Build a principal from a token's `roles` claim.
    ///
    /// Names outside the catalog are skipped, not rejected: a token can carry roles for other
    /// clients or Keycloak's own (`offline_access`, `default-roles-<realm>`). Parse the claim
    /// as strings and pass them here, rather than deserializing it as `Vec<Role>`, which fails
    /// on the first unknown name.
    pub fn from_claim(user_id: Uuid, claim: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let roles = claim
            .into_iter()
            .filter_map(|name| match name.as_ref().parse() {
                Ok(role) => Some(role),
                Err(UnknownRole(name)) => {
                    tracing::debug!("ignoring role {name:?} outside the catalog");
                    None
                }
            })
            .collect();
        Self { user_id, roles }
    }

    /// Whether the principal holds `role`.
    pub fn has_role(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }
}

/// Why [`authorize`] refused a request.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("missing role {0}")]
pub struct Forbidden(pub Role);

/// Allow the request only if `principal` holds `role`.
pub fn authorize(principal: &Principal, role: Role) -> Result<(), Forbidden> {
    if principal.has_role(role) {
        Ok(())
    } else {
        Err(Forbidden(role))
    }
}

/// A `users.role` value that the legacy mapping does not know.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown legacy role: {0:?}")]
pub struct UnknownLegacyRole(pub String);

/// Roles every signed-in legacy user gets, whatever their `users.role`.
const LEGACY_USER_ROLES: &[Role] = &[
    Role::AnnotationCreate,
    Role::AnnotationComment,
    Role::ChartShare,
    Role::ApiMcp,
];

/// Map a legacy `users.role` value onto fine-grained roles.
///
/// Temporary: it keeps today's behavior until Keycloak issues tokens with a `roles` claim,
/// and is removed with the `users.role` column. `admin` and `super_admin` get every staff
/// role (today both pass `require_admin`); `analyst`, `viewer` and `guest` get only the
/// signed-in user roles. Matching ignores ASCII case, as the old `UserRole` parser did.
pub fn roles_for_legacy(legacy_role: &str) -> Result<BTreeSet<Role>, UnknownLegacyRole> {
    let staff = match legacy_role.to_ascii_lowercase().as_str() {
        "super_admin" | "superadmin" | "admin" => true,
        "analyst" | "viewer" | "guest" => false,
        _ => return Err(UnknownLegacyRole(legacy_role.to_string())),
    };
    let mut roles: BTreeSet<Role> = LEGACY_USER_ROLES.iter().copied().collect();
    if staff {
        roles.extend(Role::ALL.iter().copied().filter(|role| role.is_staff()));
    }
    Ok(roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn staff_roles() -> BTreeSet<Role> {
        [
            Role::AdminUsersRead,
            Role::AdminUsersCreate,
            Role::AdminUsersUpdate,
            Role::AdminUsersDelete,
            Role::AdminUsersSuspend,
            Role::AdminSessionsRead,
            Role::AdminSessionsRevoke,
            Role::AdminCrawlersManage,
            Role::AdminSystemRead,
            Role::AdminSecurityRead,
            Role::AdminAuditRead,
        ]
        .into_iter()
        .collect()
    }

    fn user_roles() -> BTreeSet<Role> {
        [
            Role::AnnotationCreate,
            Role::AnnotationComment,
            Role::ChartShare,
            Role::ApiMcp,
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn every_role_round_trips_through_its_name() {
        for &role in Role::all() {
            assert_eq!(role.as_str().parse::<Role>(), Ok(role));
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, format!("\"{}\"", role.as_str()));
            assert_eq!(serde_json::from_str::<Role>(&json).unwrap(), role);
        }
    }

    #[test]
    fn names_are_unique_and_resource_action_shaped() {
        let names: BTreeSet<&str> = Role::all().iter().map(|r| r.as_str()).collect();
        assert_eq!(names.len(), Role::all().len(), "duplicate role name");
        for name in names {
            let (resource, action) = name.split_once(':').expect("resource:action");
            let ok = |s: &str| {
                !s.is_empty()
                    && s.chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c == '.')
            };
            assert!(
                ok(resource) && ok(action) && !action.contains('.'),
                "{name}"
            );
        }
    }

    #[test]
    fn unknown_role_is_rejected() {
        for bad in ["", "admin", "chart:delete", "Chart:Share", "chart:share "] {
            assert_eq!(bad.parse::<Role>(), Err(UnknownRole(bad.to_string())));
        }
        assert!(serde_json::from_str::<Role>("\"series:nuke\"").is_err());
        assert!(serde_json::from_str::<Vec<Role>>(r#"["api:mcp","nope:nope"]"#).is_err());
    }

    #[test]
    fn role_list_has_one_name_per_line() {
        let list = role_list();
        let lines: Vec<&str> = list.lines().collect();
        assert_eq!(lines.len(), Role::all().len());
        assert!(lines.contains(&"admin.users:suspend"));
        assert!(list.ends_with('\n'));
    }

    #[test]
    fn legacy_staff_roles_get_user_and_staff_roles() {
        let expected: BTreeSet<Role> = user_roles().union(&staff_roles()).copied().collect();
        for legacy in ["admin", "super_admin", "superadmin", "ADMIN"] {
            assert_eq!(roles_for_legacy(legacy).unwrap(), expected, "{legacy}");
        }
    }

    #[test]
    fn legacy_non_staff_roles_get_only_user_roles() {
        for legacy in ["analyst", "viewer", "guest", "Viewer"] {
            assert_eq!(roles_for_legacy(legacy).unwrap(), user_roles(), "{legacy}");
        }
    }

    #[test]
    fn staff_set_is_every_admin_role() {
        let from_catalog: BTreeSet<Role> = Role::all()
            .iter()
            .copied()
            .filter(|r| r.is_staff())
            .collect();
        assert_eq!(from_catalog, staff_roles());
    }

    #[test]
    fn unknown_legacy_role_is_rejected() {
        for bad in ["", "root", "read_only", "support"] {
            assert_eq!(
                roles_for_legacy(bad),
                Err(UnknownLegacyRole(bad.to_string()))
            );
        }
    }

    #[test]
    fn authorize_allows_held_role_and_denies_missing_one() {
        let principal = Principal::new(Uuid::nil(), [Role::AnnotationCreate]);
        assert_eq!(authorize(&principal, Role::AnnotationCreate), Ok(()));
        assert_eq!(
            authorize(&principal, Role::AdminUsersRead),
            Err(Forbidden(Role::AdminUsersRead))
        );
    }

    #[test]
    fn principal_from_claim_skips_names_outside_the_catalog() {
        let principal = Principal::from_claim(
            Uuid::nil(),
            [
                "offline_access",
                "api:mcp",
                "default-roles-econ-graph",
                "admin.audit:read",
            ],
        );
        assert_eq!(
            principal.roles,
            [Role::ApiMcp, Role::AdminAuditRead].into_iter().collect()
        );
    }

    #[test]
    fn authorize_denies_everything_without_roles() {
        let principal = Principal::new(Uuid::nil(), []);
        for &role in Role::all() {
            assert_eq!(authorize(&principal, role), Err(Forbidden(role)));
        }
    }

    #[test]
    fn legacy_viewer_cannot_use_staff_roles() {
        let principal = Principal::new(Uuid::nil(), roles_for_legacy("viewer").unwrap());
        for &role in Role::all().iter().filter(|r| r.is_staff()) {
            assert!(authorize(&principal, role).is_err(), "{role}");
        }
        let admin = Principal::new(Uuid::nil(), roles_for_legacy("admin").unwrap());
        for &role in Role::all() {
            assert!(authorize(&admin, role).is_ok(), "{role}");
        }
    }
}
