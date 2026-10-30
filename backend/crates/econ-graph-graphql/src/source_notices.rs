// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! # Source notices
//!
//! The citation and notice each data source's terms require wherever its data are shown or
//! exported, keyed by the source's code ([`SourceId`]). This table is the only copy of the text:
//! GraphQL serves it as `DataSourceType.notice`, and the web app (series page, CSV export, About
//! page) and the MCP server read it from there. A source without a notice gets `null`; clients
//! then show only the source name.
//!
//! The wording is quoted from each provider's terms; see `docs/data-sources/source-notices.md`.

use async_graphql::SimpleObject;
use econ_graph_crawler::persist::data_source_template;
use econ_graph_crawler::SourceId;

/// The attribution a data source's terms require.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
pub struct SourceNotice {
    /// The citation to show with the data, such as "Source: FHFA HPI®"; null when the terms
    /// prescribe none.
    pub citation: Option<String>,
    /// The statement the terms require, such as a no-endorsement notice.
    pub notice: String,
    /// The provider's terms of use.
    pub terms_url: String,
    /// Whether the terms require the date the data were retrieved from the source to be shown
    /// alongside the data (BLS).
    pub retrieval_date_required: bool,
}

/// Sources the `catalog-public` grant in the Keycloak realm makes public although they have no
/// notice. A test requires the public sources without a notice to be exactly these, so making
/// another source public without one fails, as does leaving a source here once it has a notice
/// or is no longer public.
pub const PUBLIC_WITHOUT_NOTICE: &[SourceId] = &[
    SourceId::Imf,
    SourceId::Ecb,
    SourceId::Oecd,
    SourceId::Boe,
    SourceId::Boj,
    SourceId::Boc,
    SourceId::Rba,
    SourceId::Snb,
    SourceId::UnStats,
    SourceId::Ilo,
];

/// The notice for `source`, if it has one.
pub fn notice(source: SourceId) -> Option<SourceNotice> {
    let (citation, notice, terms_url, retrieval_date_required) = match source {
        SourceId::Bls => (
            Some("Source: U.S. Bureau of Labor Statistics"),
            "BLS.gov cannot vouch for the data or analyses derived from these data after the \
             data have been retrieved from BLS.gov.",
            "https://www.bls.gov/developers/termsOfService.htm",
            true,
        ),
        SourceId::Bea => (
            Some("Source: U.S. Bureau of Economic Analysis"),
            "This product uses the Bureau of Economic Analysis (BEA) Data API but is not \
             endorsed or certified by BEA.",
            "https://apps.bea.gov/API/_pdf/bea_api_tos.pdf",
            false,
        ),
        SourceId::Census => (
            Some("Source: U.S. Census Bureau"),
            "This product uses the Census Bureau Data API but is not endorsed or certified by \
             the Census Bureau.",
            "https://www.census.gov/data/developers/about/terms-of-service.html",
            false,
        ),
        SourceId::Fhfa => (
            Some("Source: FHFA HPI®"),
            "This product uses FHFA Data but is neither endorsed nor certified by FHFA.",
            "https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service",
            false,
        ),
        SourceId::WorldBank => (
            Some("The World Bank: World Development Indicators"),
            "World Bank data are licensed under CC BY 4.0. Anyone who reuses them must \
             attribute The World Bank and its data providers in the format \"The World Bank: \
             Dataset name: Data source (if known)\", as the World Bank's dataset terms of use \
             require.",
            "https://www.worldbank.org/ext/en/legal/terms-conditions/datasets",
            false,
        ),
        SourceId::Fred => (
            None,
            "This product uses the FRED® API but is not endorsed or certified by the Federal \
             Reserve Bank of St. Louis.",
            "https://fred.stlouisfed.org/docs/api/terms_of_use.html",
            false,
        ),
        SourceId::Imf
        | SourceId::Ecb
        | SourceId::Oecd
        | SourceId::Boe
        | SourceId::Boj
        | SourceId::Boc
        | SourceId::Rba
        | SourceId::Snb
        | SourceId::UnStats
        | SourceId::Ilo
        | SourceId::Wto
        | SourceId::Sec => return None,
    };
    Some(SourceNotice {
        citation: citation.map(str::to_string),
        notice: notice.to_string(),
        terms_url: terms_url.to_string(),
        retrieval_date_required,
    })
}

/// The source whose `data_sources.name` is `name`. Names come from the crawler's templates,
/// which create the rows, so this is the one place a display name maps to a code.
pub fn source_for_name(name: &str) -> Option<SourceId> {
    SourceId::ALL
        .into_iter()
        .find(|&s| data_source_template(s).name == name)
}

/// The notice for the source named `name`, if it has one.
pub fn notice_for_name(name: &str) -> Option<SourceNotice> {
    source_for_name(name).and_then(notice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_auth::Role;

    /// The providers' required sentences, verbatim, for every reviewed source and FRED.
    #[test]
    fn reviewed_sources_have_their_notices() {
        let get = |s| notice(s).unwrap_or_else(|| panic!("{s} has a notice"));
        let bls = get(SourceId::Bls);
        assert_eq!(
            bls.notice,
            "BLS.gov cannot vouch for the data or analyses derived from these data after the \
             data have been retrieved from BLS.gov."
        );
        assert!(bls.retrieval_date_required);
        let bea = get(SourceId::Bea);
        assert_eq!(
            bea.notice,
            "This product uses the Bureau of Economic Analysis (BEA) Data API but is not \
             endorsed or certified by BEA."
        );
        assert_eq!(
            bea.citation.as_deref(),
            Some("Source: U.S. Bureau of Economic Analysis")
        );
        assert_eq!(
            get(SourceId::Census).notice,
            "This product uses the Census Bureau Data API but is not endorsed or certified by \
             the Census Bureau."
        );
        let fhfa = get(SourceId::Fhfa);
        assert_eq!(
            fhfa.notice,
            "This product uses FHFA Data but is neither endorsed nor certified by FHFA."
        );
        assert_eq!(fhfa.citation.as_deref(), Some("Source: FHFA HPI®"));
        let world_bank = get(SourceId::WorldBank);
        assert_eq!(
            world_bank.citation.as_deref(),
            Some("The World Bank: World Development Indicators")
        );
        assert_eq!(
            world_bank.terms_url,
            "https://www.worldbank.org/ext/en/legal/terms-conditions/datasets"
        );
        assert_eq!(
            get(SourceId::Fred).notice,
            "This product uses the FRED® API but is not endorsed or certified by the Federal \
             Reserve Bank of St. Louis."
        );
        for s in SourceId::ALL {
            if let Some(n) = notice(s) {
                assert!(
                    !n.notice.contains("  "),
                    "{s}: doubled space in {:?}",
                    n.notice
                );
                assert!(n.terms_url.starts_with("https://"), "{s}");
            }
        }
    }

    /// Every data source name maps to its code, and unknown names map to nothing.
    #[test]
    fn names_map_to_codes() {
        for s in SourceId::ALL {
            assert_eq!(source_for_name(&data_source_template(s).name), Some(s));
        }
        assert_eq!(
            notice_for_name("Bureau of Labor Statistics (BLS)"),
            notice(SourceId::Bls)
        );
        assert_eq!(
            notice_for_name("U.S. Census Bureau"),
            notice(SourceId::Census)
        );
        assert_eq!(source_for_name("bureau of labor statistics (bls)"), None);
        assert_eq!(notice_for_name("A source added later"), None);
        assert_eq!(notice_for_name(""), None);
    }

    /// The sources the default public grant (`catalog-public` in the Keycloak realm) lets anyone
    /// read without a notice are exactly `PUBLIC_WITHOUT_NOTICE`: a source can't become public
    /// without one unless it is added there.
    #[test]
    fn public_sources_without_a_notice_are_exactly_the_allowed_ones() {
        let realm: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../config/keycloak/econ-graph-realm.json"
        ))
        .unwrap();
        let public: Vec<&str> = realm["roles"]["realm"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == "catalog-public")
            .expect("catalog-public role")["composites"]["client"]["econ-graph-api"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        assert!(public.contains(&"catalog.bls:read"), "{public:?}");
        let mut without_notice = Vec::new();
        for role in &public {
            let source = SourceId::ALL
                .into_iter()
                .find(|&s| {
                    Role::for_catalog(&data_source_template(s).name)
                        .is_some_and(|r| r.as_str() == *role)
                })
                .unwrap_or_else(|| panic!("{role} grants no known source"));
            if notice(source).is_none() {
                without_notice.push(source);
            }
        }
        let mut allowed = PUBLIC_WITHOUT_NOTICE.to_vec();
        allowed.sort_by_key(|s| s.as_str());
        without_notice.sort_by_key(|s| s.as_str());
        assert_eq!(
            without_notice, allowed,
            "public sources without a notice must be exactly PUBLIC_WITHOUT_NOTICE"
        );
        // The reviewed sources are public and have notices.
        for s in [
            SourceId::Bls,
            SourceId::Bea,
            SourceId::Census,
            SourceId::Fhfa,
            SourceId::WorldBank,
        ] {
            let role = Role::for_catalog(&data_source_template(s).name).unwrap();
            assert!(public.contains(&role.as_str()), "{s} is public");
            assert!(notice(s).is_some(), "{s} has a notice");
        }
    }
}
