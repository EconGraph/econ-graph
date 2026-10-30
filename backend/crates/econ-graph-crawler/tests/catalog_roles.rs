// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Every data source the crawler can write is readable through a catalog role, and every
//! catalog role is granted by default.
//!
//! The API serves a source's series only to a caller holding the role
//! `Role::for_catalog(data_sources.name)` returns, and that function matches on the exact name
//! in each `DataSource` template. A source whose name has no match is readable by nobody; a
//! catalog role missing from the realm file's `catalog-public` composite is readable by no
//! signed-in or anonymous reader unless an operator grants it; and one missing from the
//! `econgraph-anonymous` client's scope mapping never reaches the anonymous reader's token even
//! though `catalog-public` grants it. All three fail silently, so they are checked here.

use std::collections::BTreeSet;

use econ_graph_auth::Role;
use econ_graph_crawler::persist::data_source_template;
use econ_graph_crawler::source::SourceId;

/// Sources with no catalog role, and why. SEC filings are served by their own API, not the
/// series catalog.
const SOURCES_WITHOUT_A_CATALOG_ROLE: &[SourceId] = &[SourceId::Sec];

/// Catalog roles deliberately left out of the default `catalog-public` grant. None today.
const CATALOG_ROLES_NOT_PUBLIC: &[Role] = &[];

/// Roles in `catalog-public` deliberately kept out of the anonymous reader's scope mapping, so
/// only signed-in users get them by default. None today.
const PUBLIC_ROLES_NOT_ANONYMOUS: &[Role] = &[];

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

#[test]
fn every_public_catalog_role_is_in_the_anonymous_scope_mapping() {
    let realm = realm();
    let public: Vec<Role> = catalog_public(&realm)
        .iter()
        .map(|name| name.parse().expect("catalog-public holds catalog roles"))
        .collect();
    check_holds(
        &anonymous_scope(&realm),
        &public,
        PUBLIC_ROLES_NOT_ANONYMOUS,
        "the econgraph-anonymous scope mapping",
    );
}

#[test]
fn every_catalog_role_is_in_catalog_public() {
    let catalog: Vec<Role> = Role::ALL
        .iter()
        .copied()
        .filter(|role| role.is_catalog())
        .collect();
    check_holds(
        &catalog_public(&realm()),
        &catalog,
        CATALOG_ROLES_NOT_PUBLIC,
        "catalog-public",
    );
}
