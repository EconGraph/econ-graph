//! Catalogs that are off by default: their read role is defined in the realm file, but neither
//! signed-in users (through the `catalog-public` composite) nor anonymous visitors (through the
//! `econgraph-anonymous` client's scope mapping) receive it. Only users or groups an operator
//! grants the role can read the catalog. The catalogs that are public by default are granted to
//! signed-in users and anonymous visitors alike.

use std::collections::BTreeSet;

use econ_graph_auth::Role;
use serde_json::Value;

const API_CLIENT: &str = "econ-graph-api";

/// Catalog read roles granted to nobody by default.
const RESTRICTED_BY_DEFAULT: &[Role] = &[
    Role::CatalogWorldTradeOrganizationRead,
    Role::CatalogBankOfCanadaRead,
];

fn realm() -> Value {
    serde_json::from_str(include_str!(
        "../../../../config/keycloak/econ-graph-realm.json"
    ))
    .expect("the realm file is JSON")
}

fn names(values: &Value) -> BTreeSet<String> {
    values
        .as_array()
        .expect("an array of role names")
        .iter()
        .map(|v| v.as_str().expect("a role name").to_string())
        .collect()
}

/// The client roles the realm file defines on `econ-graph-api`.
fn defined(realm: &Value) -> BTreeSet<String> {
    realm["roles"]["client"][API_CLIENT]
        .as_array()
        .expect("econ-graph-api client roles")
        .iter()
        .map(|r| r["name"].as_str().expect("a role name").to_string())
        .collect()
}

/// The `econ-graph-api` roles in the `catalog-public` realm composite.
fn catalog_public(realm: &Value) -> BTreeSet<String> {
    let role = realm["roles"]["realm"]
        .as_array()
        .expect("realm roles")
        .iter()
        .find(|r| r["name"] == "catalog-public")
        .expect("a catalog-public role");
    names(&role["composites"]["client"][API_CLIENT])
}

/// The `econ-graph-api` roles the anonymous reader's client scope maps.
fn anonymous_scope(realm: &Value) -> BTreeSet<String> {
    let entry = realm["clientScopeMappings"][API_CLIENT]
        .as_array()
        .expect("econ-graph-api client scope mappings")
        .iter()
        .find(|e| e["client"] == "econgraph-anonymous")
        .expect("a scope mapping for econgraph-anonymous");
    names(&entry["roles"])
}

#[test]
fn new_catalogs_are_restricted_by_default() {
    let realm = realm();
    let (defined, public, anonymous) = (
        defined(&realm),
        catalog_public(&realm),
        anonymous_scope(&realm),
    );
    for role in RESTRICTED_BY_DEFAULT {
        let role = role.as_str();
        assert!(
            defined.contains(role),
            "{role} must stay defined so it can be granted"
        );
        assert!(
            !public.contains(role),
            "{role} must not be in catalog-public"
        );
        assert!(
            !anonymous.contains(role),
            "{role} must not be mapped to anonymous visitors"
        );
    }
}

/// Restricting catalogs leaves the public defaults alone: the realm file's catalog-public set and
/// the anonymous scope hold the same catalog reads, none restricted, and the restriction did not
/// take others with it (BEA and BLS stay public).
#[test]
fn restricting_a_catalog_leaves_the_public_defaults_alone() {
    let realm = realm();
    let (public, anonymous) = (catalog_public(&realm), anonymous_scope(&realm));
    assert_eq!(public, anonymous, "signed-in and anonymous defaults differ");
    for name in &public {
        let role: Role = name
            .parse()
            .unwrap_or_else(|_| panic!("{name} is not a role"));
        assert!(role.is_catalog(), "{name} is not a catalog read");
        assert!(
            !RESTRICTED_BY_DEFAULT.contains(&role),
            "{name} is restricted"
        );
    }
    for role in [Role::CatalogBeaRead, Role::CatalogBlsRead] {
        assert!(
            public.contains(role.as_str()),
            "{} is not public",
            role.as_str()
        );
    }
}
