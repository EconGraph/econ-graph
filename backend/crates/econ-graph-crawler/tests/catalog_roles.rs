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
//! signed-in or anonymous reader unless an operator grants it. Both fail silently, so they are
//! checked here.

use std::collections::BTreeSet;

use econ_graph_auth::Role;
use econ_graph_crawler::persist::data_source_template;
use econ_graph_crawler::source::SourceId;

/// Sources with no catalog role, and why. SEC filings are served by their own API, not the
/// series catalog.
const SOURCES_WITHOUT_A_CATALOG_ROLE: &[SourceId] = &[SourceId::Sec];

/// Catalog roles deliberately left out of the default `catalog-public` grant. None today.
const CATALOG_ROLES_NOT_PUBLIC: &[Role] = &[];

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

#[test]
fn every_catalog_role_is_in_catalog_public() {
    let realm: serde_json::Value = serde_json::from_str(REALM).expect("realm file is JSON");
    let public = realm["roles"]["realm"]
        .as_array()
        .expect("realm roles")
        .iter()
        .find(|role| role["name"] == "catalog-public")
        .expect("catalog-public is defined");
    let granted: BTreeSet<&str> = public["composites"]["client"]["econ-graph-api"]
        .as_array()
        .expect("catalog-public has econ-graph-api composites")
        .iter()
        .map(|name| name.as_str().expect("role name"))
        .collect();
    for role in Role::ALL.iter().filter(|role| role.is_catalog()) {
        let listed = CATALOG_ROLES_NOT_PUBLIC.contains(role);
        assert_eq!(
            granted.contains(role.as_str()),
            !listed,
            "{role}: in catalog-public must be {}",
            !listed
        );
    }
}
