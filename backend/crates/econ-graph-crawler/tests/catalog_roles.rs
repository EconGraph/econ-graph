// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Every data source the crawler can write is readable through a catalog role, and the catalogs
//! the realm file grants by default are exactly those not listed here as restricted.
//!
//! The API serves a source's series only to a caller holding the role
//! `Role::for_catalog(data_sources.name)` returns, and that function matches on the exact name
//! in each `DataSource` template. A source whose name has no match is readable by nobody; a
//! catalog role missing from the realm file's `catalog-public` composite is readable by no
//! signed-in or anonymous reader unless an operator grants it; and one missing from the
//! `econgraph-anonymous` client's scope mapping never reaches the anonymous reader's token even
//! though `catalog-public` grants it. All three fail silently, so they are checked here, and a
//! role left out of either on purpose must be listed below.

use std::collections::BTreeSet;

use econ_graph_auth::Role;
use econ_graph_crawler::persist::data_source_template;
use econ_graph_crawler::source::SourceId;

/// Every source, including SEC company research, has an explicit catalog role.
const SOURCES_WITHOUT_A_CATALOG_ROLE: &[SourceId] = &[];

/// Catalog roles deliberately left out of the default `catalog-public` grant. The public set is
/// read from the realm file; every other catalog role must be listed here, and nothing listed
/// here may be public.
const CATALOG_ROLES_NOT_PUBLIC: &[Role] = &[
    // Admins only.
    Role::CatalogFredRead,
    // Restricted by default; granted explicitly.
    Role::CatalogWorldTradeOrganizationRead,
    Role::CatalogBankOfCanadaRead,
    // Not reviewed for public access; granted to nobody by default.
    Role::CatalogImfRead,
    Role::CatalogEcbRead,
    Role::CatalogOecdRead,
    Role::CatalogBoeRead,
    Role::CatalogBojRead,
    Role::CatalogRbaRead,
    Role::CatalogSnbRead,
    Role::CatalogUnstatsRead,
    Role::CatalogIloRead,
    // Company research requires sign-in; no default anonymous grant.
    Role::CatalogSecRead,
];

/// Catalog roles deliberately left out of the anonymous reader's scope mapping. Anonymous
/// visitors get exactly the public catalogs, so this is the same list.
const CATALOG_ROLES_NOT_ANONYMOUS: &[Role] = CATALOG_ROLES_NOT_PUBLIC;

const REALM: &str = include_str!("../../../../config/keycloak/econ-graph-realm.json");

#[test]
fn every_source_template_maps_to_a_catalog_role() {
    let mut seen = BTreeSet::new();
    for source in SourceId::ALL {
        let name = data_source_template(source).name;
        let role = Role::for_catalog(&name);
        if SOURCES_WITHOUT_A_CATALOG_ROLE.contains(&source) {
            assert_eq!(
                role, None,
                "{source} ({name}) is listed as having no catalog role"
            );
            continue;
        }
        let role = role.unwrap_or_else(|| {
            panic!(
                "{source}: data_sources name {name:?} has no Role::for_catalog match, so its \
                 series are readable by nobody"
            )
        });
        assert!(
            role.is_catalog(),
            "{source}: {role} is not a catalog read role"
        );
        assert!(
            seen.insert(role),
            "{source}: {role} is shared with another source"
        );
    }
}

fn realm() -> serde_json::Value {
    serde_json::from_str(REALM).expect("realm file is JSON")
}

fn names(list: &serde_json::Value, what: &str) -> BTreeSet<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("{what} is a list"))
        .iter()
        .map(|name| name.as_str().expect("role name").to_string())
        .collect()
}

/// The `econ-graph-api` roles the realm file's `catalog-public` composite grants.
fn catalog_public(realm: &serde_json::Value) -> BTreeSet<String> {
    let public = realm["roles"]["realm"]
        .as_array()
        .expect("realm roles")
        .iter()
        .find(|role| role["name"] == "catalog-public")
        .expect("catalog-public is defined");
    names(
        &public["composites"]["client"]["econ-graph-api"],
        "catalog-public's econ-graph-api composites",
    )
}

/// The `econ-graph-api` roles the realm file maps into the `econgraph-anonymous` client's scope.
fn anonymous_scope(realm: &serde_json::Value) -> BTreeSet<String> {
    let entry = realm["clientScopeMappings"]["econ-graph-api"]
        .as_array()
        .expect("econ-graph-api client scope mappings")
        .iter()
        .find(|entry| entry["client"] == "econgraph-anonymous")
        .expect("econgraph-anonymous has an econ-graph-api scope mapping");
    names(&entry["roles"], "the econgraph-anonymous scope mapping")
}

/// Fails, naming each role, unless `held` holds exactly the roles in `want` other than
/// `except`, and none of `except`.
fn check_holds(held: &BTreeSet<String>, want: &[Role], except: &[Role], what: &str) {
    for role in want {
        let excepted = except.contains(role);
        assert_eq!(
            held.contains(role.as_str()),
            !excepted,
            "{role}: in {what} must be {}",
            !excepted
        );
    }
}

fn catalog_roles() -> Vec<Role> {
    Role::ALL
        .iter()
        .copied()
        .filter(|role| role.is_catalog())
        .collect()
}

/// The `econ-graph-api` client roles the realm file defines.
fn defined_roles(realm: &serde_json::Value) -> BTreeSet<String> {
    realm["roles"]["client"]["econ-graph-api"]
        .as_array()
        .expect("econ-graph-api client roles")
        .iter()
        .map(|role| role["name"].as_str().expect("role name").to_string())
        .collect()
}

#[test]
fn every_catalog_role_but_the_listed_ones_is_in_the_anonymous_scope_mapping() {
    check_holds(
        &anonymous_scope(&realm()),
        &catalog_roles(),
        CATALOG_ROLES_NOT_ANONYMOUS,
        "the econgraph-anonymous scope mapping",
    );
}

#[test]
fn every_catalog_role_but_the_listed_ones_is_in_catalog_public() {
    check_holds(
        &catalog_public(&realm()),
        &catalog_roles(),
        CATALOG_ROLES_NOT_PUBLIC,
        "catalog-public",
    );
}

/// The Bank of Canada catalog is defined but restricted by default: an operator grants it to a
/// user, a group or `catalog-public`.
#[test]
fn bank_of_canada_is_defined_but_not_granted_by_default() {
    let realm = realm();
    let role = Role::CatalogBankOfCanadaRead.as_str();
    assert!(
        defined_roles(&realm).contains(role),
        "{role} is not defined"
    );
    assert!(
        !catalog_public(&realm).contains(role),
        "{role} is in catalog-public"
    );
    assert!(
        !anonymous_scope(&realm).contains(role),
        "{role} is in the econgraph-anonymous scope mapping"
    );
}
