//! Explicit independent-service boundaries; the operation-routing proxy grants nothing.
use async_graphql::parser::types::{ExecutableDocument, OperationType, Selection, SelectionSet};
use async_graphql::Request;
use econ_graph_auth::OidcConfig;
use std::collections::HashSet;

pub const DATA_AUDIENCE: &str = "econ-graph-data";
pub const APP_AUDIENCE: &str = "econ-graph-api";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServicePlane {
    /// Existing deployments retain anonymous catalog behavior and both schema surfaces.
    #[default]
    Combined,
    /// Independent app service, with its own audience and no data database.
    App,
    /// Independent query-only data service; never a crawler or app administration endpoint.
    Data,
}

impl ServicePlane {
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim) {
            None | Some("") | Some("combined") => Ok(Self::Combined),
            Some("app") => Ok(Self::App),
            Some("data") => Ok(Self::Data),
            Some(_) => Err("GRAPHQL_SERVICE_PLANE must be combined, app or data".into()),
        }
    }

    pub fn validate(self, oidc: Option<&OidcConfig>, has_data: bool) -> Result<(), String> {
        if self == Self::Combined {
            return Ok(());
        }
        let config = oidc.ok_or("Independent GraphQL services require OIDC_ISSUER")?;
        let expected = if self == Self::Data {
            DATA_AUDIENCE
        } else {
            APP_AUDIENCE
        };
        if config.audience != expected {
            return Err(format!(
                "Independent {self:?} service requires OIDC_AUDIENCE={expected}"
            ));
        }
        if has_data != (self == Self::Data) {
            return Err(
                "Independent app requires no DATA_DATABASE_URL; independent data requires one"
                    .into(),
            );
        }
        Ok(())
    }
}

/// Check the actual selected operation and fragment-expanded root fields, not a supplied
/// operation name, alias, string prefix, or directive. Skipped forbidden roots still deny.
/// No resolver runs until the whole operation is accepted.
pub fn check_data_operation(request: &mut Request) -> Result<(), &'static str> {
    let name = request.operation_name.clone();
    let doc = request
        .parsed_query()
        .map_err(|_| "Invalid GraphQL document")?;
    let mut operations = doc.operations.iter().filter(|(candidate, _)| match &name {
        Some(name) => candidate.is_some_and(|candidate| candidate.as_str() == name),
        None => true,
    });
    let (_, operation) = operations.next().ok_or("Unknown GraphQL operation")?;
    if operations.next().is_some() {
        return Err("Choose exactly one GraphQL operation");
    }
    if operation.node.ty != OperationType::Query {
        return Err("The independent data service accepts read queries only");
    }
    check_roots(
        &operation.node.selection_set.node,
        doc,
        &mut HashSet::new(),
        0,
        &mut 1000,
    )
}

fn check_roots(
    selection: &SelectionSet,
    doc: &ExecutableDocument,
    visiting: &mut HashSet<String>,
    depth: usize,
    budget: &mut usize,
) -> Result<(), &'static str> {
    if depth > 32 {
        return Err("GraphQL root fragments are too deeply nested");
    }
    for item in &selection.items {
        *budget = budget
            .checked_sub(1)
            .ok_or("GraphQL root selection is too large")?;
        match &item.node {
            Selection::Field(field) => {
                if !matches!(
                    field.node.name.node.as_str(),
                    "series"
                        | "seriesExists"
                        | "seriesCatalog"
                        | "seriesByExternalId"
                        | "seriesList"
                        | "seriesData"
                        | "searchSeries"
                        | "searchSuggestions"
                        | "dataSources"
                        | "dataSource"
                        | "datasets"
                        | "dataset"
                        | "crossSection"
                        | "companies"
                        | "company"
                        | "companyFacts"
                        | "financialStatement"
                        | "companyRatios"
                        | "companyPeers"
                        | "companyResearchTargetExists"
                        | "__typename"
                        | "__schema"
                        | "__type"
                ) {
                    return Err("This field is not available on the independent data service");
                }
            }
            Selection::InlineFragment(fragment) => check_roots(
                &fragment.node.selection_set.node,
                doc,
                visiting,
                depth + 1,
                budget,
            )?,
            Selection::FragmentSpread(spread) => {
                let name = spread.node.fragment_name.node.as_str();
                if !visiting.insert(name.to_string()) {
                    return Err("Cyclic GraphQL fragment");
                }
                let fragment = doc.fragments.get(name).ok_or("Unknown GraphQL fragment")?;
                check_roots(
                    &fragment.node.selection_set.node,
                    doc,
                    visiting,
                    depth + 1,
                    budget,
                )?;
                visiting.remove(name);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_operation_boundary_cannot_be_bypassed_with_aliases_fragments_or_names() {
        for query in [
            "mutation { triggerCrawl { success } }",
            "{ harmless:crawlerStatus { isRunning } }",
            "query Series { ...Control } fragment Control on Query { crawlerStatus { isRunning } }",
            "{ ... on Query { users { totalCount } } }",
            "{ users @skip(if:true) {totalCount} }",
            "{ companyNotes(cik:\"42\") {id} }",
            "{ ... on Query { note:companyNotes(cik:\"42\") @skip(if:true) {id} } }",
            "subscription { __typename }",
            "query A {__typename} query B {__typename}",
            "{...A} fragment A on Query {...B} fragment B on Query {...A}",
            "{...Missing}",
        ] {
            assert!(
                check_data_operation(&mut Request::new(query)).is_err(),
                "{query}"
            );
        }
        for query in [
            "{ seriesList { nodes { id } } }",
            "{ companies {nodes {cik}} company(cik:\"42\") {cik} companyFacts(cik:\"42\") {nodes {value}} }",
            "{ ...Company } fragment Company on Query { statement:financialStatement(cik:\"42\",kind:BALANCE_SHEET) {lines {key}} companyResearchTargetExists(anchor:\"{}\") }",
            "{ companyRatios(cik:\"42\",periodEnd:\"2024-12-31\") {name} companyPeers(cik:\"42\",periodEnd:\"2024-12-31\") {company {cik}} }",
            "query Read { ...Data } fragment Data on Query { seriesData(seriesId:\"id\") {totalCount} }",
            "{... on Query { alias:datasets {id} }}",
        ] {
            assert!(check_data_operation(&mut Request::new(query)).is_ok(), "{query}");
        }
        let mut selected =
            Request::new("query Read {__typename} mutation Write {triggerCrawl{success}}")
                .operation_name("Read");
        assert!(check_data_operation(&mut selected).is_ok());
        selected.operation_name = Some("Write".into());
        assert!(check_data_operation(&mut selected).is_err());
    }

    #[test]
    fn independent_profiles_require_issuer_audience_and_their_own_plane() {
        let data = OidcConfig {
            issuer: "https://id.example/realms/econ-graph".into(),
            audience: DATA_AUDIENCE.into(),
            jwks_url: None,
        };
        let mut app = data.clone();
        app.audience = APP_AUDIENCE.into();
        assert!(ServicePlane::Data.validate(Some(&data), true).is_ok());
        assert!(ServicePlane::App.validate(Some(&app), false).is_ok());
        assert!(ServicePlane::Data.validate(Some(&app), true).is_err());
        assert!(ServicePlane::Data.validate(None, true).is_err());
        assert!(ServicePlane::Data.validate(Some(&data), false).is_err());
        assert!(ServicePlane::App.validate(Some(&app), true).is_err());
        assert!(ServicePlane::Combined.validate(None, true).is_ok());
        assert_eq!(ServicePlane::parse(None).unwrap(), ServicePlane::Combined);
        assert!(ServicePlane::parse(Some("typo")).is_err());
    }
}
