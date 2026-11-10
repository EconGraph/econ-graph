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
    Role::CatalogSecRead,
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

/// No realm role grants SEC reads on a fresh realm, directly or through a composite. An operator
/// grants them once company research is accepted.
#[test]
fn no_realm_role_grants_sec_reads_by_default() {
    let realm = realm();
    let sec = Role::CatalogSecRead.as_str();
    for role in realm["roles"]["realm"].as_array().expect("realm roles") {
        let granted = role["composites"]["client"][API_CLIENT]
            .as_array()
            .into_iter()
            .flatten()
            .any(|name| name == sec);
        assert!(!granted, "{} must not grant {sec}", role["name"]);
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

/// The realm-file attribute that marks a role granted to nobody. preserve-catalog-grants.py,
/// catalog-access.py and the dev-readonly overlay read the same attribute.
const NEVER_GRANTED: &str = "econgraph.never-granted";

/// The `econ-graph-api` roles the realm file marks never granted. Fails closed, as the deploy
/// check does: the attribute with any value but `["true"]` is an error, not an unmarked role.
fn marked_never_granted(realm: &Value) -> Result<BTreeSet<String>, String> {
    let mut marked = BTreeSet::new();
    for role in realm["roles"]["client"][API_CLIENT]
        .as_array()
        .expect("econ-graph-api client roles")
    {
        let name = role["name"].as_str().expect("a role name");
        match role["attributes"].get(NEVER_GRANTED) {
            None => {}
            Some(value) if *value == serde_json::json!(["true"]) => {
                marked.insert(name.to_string());
            }
            Some(value) => {
                return Err(format!(
                    "{name} has {NEVER_GRANTED} = {value}; it must be [\"true\"], or be removed"
                ))
            }
        }
    }
    Ok(marked)
}

fn marked_roles(realm: &Value) -> BTreeSet<String> {
    marked_never_granted(realm).unwrap_or_else(|problem| panic!("{problem}"))
}

/// A hand edit of the marker to anything but `["true"]` fails rather than unprotecting the role.
#[test]
fn a_malformed_never_granted_marker_fails() {
    let mut realm = realm();
    let massive = Role::CatalogMassiveMarketDataRead.as_str();
    realm["roles"]["client"][API_CLIENT]
        .as_array_mut()
        .expect("econ-graph-api client roles")
        .iter_mut()
        .find(|r| r["name"] == massive)
        .expect("the Massive role")["attributes"][NEVER_GRANTED] = serde_json::json!(["True"]);
    let problem = marked_never_granted(&realm).expect_err("a malformed marker is refused");
    assert!(problem.contains(massive), "{problem}");
}

/// The realm file's marker and `Role::is_never_granted` agree, so neither can be dropped
/// unnoticed, and Massive's market data is on both.
#[test]
fn never_granted_roles_match_the_realm_file() {
    let marked = marked_roles(&realm());
    let listed: BTreeSet<String> = Role::ALL
        .iter()
        .filter(|role| role.is_never_granted())
        .map(|role| role.as_str().to_string())
        .collect();
    assert_eq!(
        marked, listed,
        "realm file markers and Role::is_never_granted differ"
    );
    assert!(Role::CatalogMassiveMarketDataRead.is_never_granted());
    for role in Role::ALL.iter().filter(|role| role.is_never_granted()) {
        assert!(
            role.is_catalog(),
            "{role} is never granted but not a catalog read"
        );
    }
}

/// Every grant in the realm file of a MARKED role: composites (realm roles, the default role,
/// `admin` and `super_admin` among them, and client roles), users, groups and client or client
/// scope mappings.
fn grants_of(realm: &Value, marked: &BTreeSet<String>) -> Vec<String> {
    let marked_in = |v: &Value| -> Vec<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|n| marked.contains(*n))
            .map(str::to_string)
            .collect()
    };
    let mut found = Vec::new();
    let mut roles: Vec<&Value> = realm["roles"]["realm"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    if let Some(clients) = realm["roles"]["client"].as_object() {
        roles.extend(clients.values().filter_map(Value::as_array).flatten());
    }
    for role in roles {
        for name in marked_in(&role["composites"]["client"][API_CLIENT]) {
            found.push(format!("composite {} grants {name}", role["name"]));
        }
    }
    for user in realm["users"].as_array().into_iter().flatten() {
        for name in marked_in(&user["clientRoles"][API_CLIENT]) {
            found.push(format!("user {} grants {name}", user["username"]));
        }
    }
    let mut groups: Vec<&Value> = realm["groups"].as_array().into_iter().flatten().collect();
    while let Some(group) = groups.pop() {
        for name in marked_in(&group["clientRoles"][API_CLIENT]) {
            found.push(format!("group {} grants {name}", group["name"]));
        }
        groups.extend(group["subGroups"].as_array().into_iter().flatten());
    }
    for entry in realm["clientScopeMappings"][API_CLIENT]
        .as_array()
        .into_iter()
        .flatten()
    {
        for name in marked_in(&entry["roles"]) {
            found.push(format!("scope mapping {entry} admits {name}"));
        }
    }
    found
}

/// No composite, user, group or scope mapping in the realm file grants a never-granted role, and
/// each is still defined, so it can be granted on purpose later. The dev-readonly client's
/// automatic catalog scope leaves them out (scripts/data-access/test_independent_access.py).
#[test]
fn nothing_in_the_realm_file_grants_a_never_granted_role() {
    let realm = realm();
    let marked = marked_roles(&realm);
    assert!(marked.contains(Role::CatalogMassiveMarketDataRead.as_str()));
    assert!(marked.is_subset(&defined(&realm)));
    assert_eq!(grants_of(&realm, &marked), Vec::<String>::new());
}

/// The check above catches the grant the deploy check is tested with: the role added to `admin`.
#[test]
fn a_never_granted_role_on_admin_is_caught() {
    let mut realm = realm();
    let marked = marked_roles(&realm);
    let massive = Role::CatalogMassiveMarketDataRead.as_str();
    realm["roles"]["realm"]
        .as_array_mut()
        .expect("realm roles")
        .iter_mut()
        .find(|r| r["name"] == "admin")
        .expect("an admin role")["composites"]["client"][API_CLIENT]
        .as_array_mut()
        .expect("admin's econ-graph-api composites")
        .push(massive.into());
    assert_eq!(
        grants_of(&realm, &marked),
        vec![format!("composite \"admin\" grants {massive}")]
    );
}
