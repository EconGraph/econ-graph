//! Catalogs that are off by default: their read role is defined in the realm file, but neither
//! signed-in users (through the `catalog-public` composite) nor anonymous visitors (through the
//! `econgraph-anonymous` client's scope mapping) receive it. Only users or groups an operator
//! grants the role can read the catalog. Every other catalog stays public by default.

use std::collections::BTreeSet;

use econ_graph_auth::Role;
use serde_json::Value;

const API_CLIENT: &str = "econ-graph-api";

/// Catalog read roles granted to nobody by default.
const RESTRICTED_BY_DEFAULT: &[Role] = &[Role::CatalogWorldTradeOrganizationRead];

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
fn world_trade_organization_catalog_is_restricted_by_default() {
    let realm = realm();
    let (defined, public, anonymous) = (
        defined(&realm),
        catalog_public(&realm),
        anonymous_scope(&realm),
    );
    let wto = Role::CatalogWorldTradeOrganizationRead.as_str();
    assert!(
        defined.contains(wto),
        "{wto} must stay defined so it can be granted"
    );
    assert!(!public.contains(wto), "{wto} must not be in catalog-public");
    assert!(
        !anonymous.contains(wto),
        "{wto} must not be mapped to anonymous visitors"
    );
}

#[test]
fn every_other_catalog_is_public_by_default() {
    let realm = realm();
    let (public, anonymous) = (catalog_public(&realm), anonymous_scope(&realm));
    for role in Role::all().iter().filter(|r| r.is_catalog()) {
        let name = role.as_str();
        if RESTRICTED_BY_DEFAULT.contains(role) {
            continue;
        }
        assert!(
            public.contains(name),
            "{name} is missing from catalog-public"
        );
        assert!(
            anonymous.contains(name),
            "{name} is missing from the anonymous scope"
        );
    }
}
