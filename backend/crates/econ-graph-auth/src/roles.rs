//! # Fine-grained role catalog
//!
//! The backend authorizes requests with fine-grained roles only. Each [`Role`] is one
//! capability, named `resource:action` (for example `annotation:create`,
//! `admin.users:suspend`). Plans, organization roles and staff levels never appear here: the
//! identity provider composes them out of these roles. A Keycloak protocol mapper on the
//! `econ-graph-api` client puts the flattened client roles in a top-level `roles` claim
//! (Keycloak's default `resource_access.<client>.roles` is not used); see the auth, plans and
//! permissions design record:
//! <https://linear.app/econgraph/document/auth-plans-and-permissions-roadmap-design-record-from-pr-176-eae73ef9ee81>.
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
    // Catalog reads (including development-only sources).
    /// Read the Federal Reserve Economic Data (FRED) catalog.
    CatalogFredRead => "catalog.fred:read",
    /// Read the Bureau of Labor Statistics (BLS) catalog.
    CatalogBlsRead => "catalog.bls:read",
    /// Read the U.S. Census Bureau catalog.
    CatalogCensusRead => "catalog.census:read",
    /// Read the World Bank Open Data catalog.
    CatalogWorldBankRead => "catalog.world_bank:read",
    /// Read the Bureau of Economic Analysis (BEA) catalog.
    CatalogBeaRead => "catalog.bea:read",
    /// Read the International Monetary Fund (IMF) catalog.
    CatalogImfRead => "catalog.imf:read",
    /// Read the Federal Housing Finance Agency (FHFA) catalog.
    CatalogFhfaRead => "catalog.fhfa:read",
    /// Read the European Central Bank (ECB) catalog.
    CatalogEcbRead => "catalog.ecb:read",
    /// Read the OECD (Organisation for Economic Co-operation and Development) catalog.
    CatalogOecdRead => "catalog.oecd:read",
    /// Read the Bank of England (BoE) catalog.
    CatalogBoeRead => "catalog.boe:read",
    /// Read the World Trade Organization (WTO) catalog.
    CatalogWorldTradeOrganizationRead => "catalog.world_trade_organization:read",
    /// Read the Bank of Japan (BoJ) catalog.
    CatalogBojRead => "catalog.boj:read",
    /// Read the Reserve Bank of Australia (RBA) catalog.
    CatalogRbaRead => "catalog.rba:read",
    /// Read the Bank of Canada (BoC) catalog.
    CatalogBocRead => "catalog.boc:read",
    /// Read the Swiss National Bank (SNB) catalog.
    CatalogSnbRead => "catalog.snb:read",
    /// Read the UN Statistics Division catalog.
    CatalogUnstatsRead => "catalog.unstats:read",
    /// Read the International Labour Organization (ILO) catalog.
    CatalogIloRead => "catalog.ilo:read",

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

    /// Whether this capability can be granted to anonymous data readers.
    pub fn is_catalog(self) -> bool {
        self.as_str().starts_with("catalog.") && self.as_str().ends_with(":read")
    }

    /// Catalog read capability for a canonical data-source name; unknown sources deny access.
    pub fn for_catalog(name: &str) -> Option<Self> {
        match name {
            "Federal Reserve Economic Data (FRED)" => Some(Self::CatalogFredRead),
            "Bureau of Labor Statistics (BLS)" => Some(Self::CatalogBlsRead),
            "U.S. Census Bureau" => Some(Self::CatalogCensusRead),
            "World Bank Open Data" => Some(Self::CatalogWorldBankRead),
            "Bureau of Economic Analysis (BEA)" => Some(Self::CatalogBeaRead),
            "International Monetary Fund (IMF)" => Some(Self::CatalogImfRead),
            "Federal Housing Finance Agency (FHFA)" => Some(Self::CatalogFhfaRead),
            "European Central Bank (ECB)" => Some(Self::CatalogEcbRead),
            "OECD (Organisation for Economic Co-operation and Development)" => {
                Some(Self::CatalogOecdRead)
            }
            "Bank of England (BoE)" => Some(Self::CatalogBoeRead),
            "World Trade Organization (WTO)" => Some(Self::CatalogWorldTradeOrganizationRead),
            "Bank of Japan (BoJ)" => Some(Self::CatalogBojRead),
            "Reserve Bank of Australia (RBA)" => Some(Self::CatalogRbaRead),
            "Bank of Canada (BoC)" => Some(Self::CatalogBocRead),
            "Swiss National Bank (SNB)" => Some(Self::CatalogSnbRead),
            "UN Statistics Division" => Some(Self::CatalogUnstatsRead),
            "International Labour Organization (ILO)" => Some(Self::CatalogIloRead),
            _ => None,
        }
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

    /// The WTO's catalog role spells the organization out; the acronym form is not a role.
    #[test]
    fn world_trade_organization_catalog_role_is_spelled_out() {
        let role = Role::for_catalog("World Trade Organization (WTO)").unwrap();
        assert_eq!(role, Role::CatalogWorldTradeOrganizationRead);
        assert_eq!(role.as_str(), "catalog.world_trade_organization:read");
        assert!(role.is_catalog());
        assert!("catalog.wto:read".parse::<Role>().is_err());
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
    fn staff_set_is_every_admin_role() {
        let from_catalog: BTreeSet<Role> = Role::all()
            .iter()
            .copied()
            .filter(|r| r.is_staff())
            .collect();
        assert_eq!(from_catalog, staff_roles());
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
}
