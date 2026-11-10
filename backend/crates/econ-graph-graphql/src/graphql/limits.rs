//! # What one GraphQL request may cost
//!
//! Four limits bound the work a single request can make the API do, before or while it runs
//! (ECO-888). `docs/security/GRAPHQL_SECURITY_GUIDE.md` states them for API users.
//!
//! - **Depth** ([`MAX_QUERY_DEPTH`]): how deeply selections nest.
//! - **Fields** ([`MAX_QUERY_FIELDS`]): how many field selections the document holds once its
//!   fragments are expanded, aliases included. This bounds how many resolvers one request can
//!   start, however cheap each one is, so a request can't alias one field hundreds of times.
//! - **Cost** ([`MAX_QUERY_COMPLEXITY`]): async-graphql's complexity, computed before the
//!   request runs from the arguments it was sent. A field costs 1 plus its selections, except
//!   that a list multiplies its selections by the rows it can return:
//!   - a list of series, companies or users costs its page size times its selections
//!     ([`list_cost`]);
//!   - a list of observations (or company facts) costs one unit per
//!     [`OBSERVATIONS_PER_COST_UNIT`] rows, times its selections plus
//!     [`OBSERVATION_ROW_COST`], since every row is read from the database whole however few
//!     of its fields are selected ([`observation_cost`]);
//!   - a root that runs an expensive query whatever it returns (search, statements, peers)
//!     adds a fixed [`EXPENSIVE_QUERY_COST`].
//!
//!   An unpaged list costs its worst case: `EconomicSeries.dataPoints` costs as if it returned
//!   [`MAX_OBSERVATIONS_PER_PAGE`] rows.
//! - **Time** ([`REQUEST_TIMEOUT`]): a request still running after this long is cancelled
//!   and answered with a `REQUEST_TIMEOUT` error. The API's database connections also carry a
//!   statement timeout (`econ_graph_core::database::API_STATEMENT_TIMEOUT`), so a cancelled
//!   request's query stops on the server too.
//!
//! Every rejection is counted by reason ([`rejection_counts`]); the backend exports the counts
//! as `econgraph_graphql_rejected_requests_total{reason}`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_graphql::extensions::{
    Extension, ExtensionContext, ExtensionFactory, NextExecute, NextParseQuery, NextPrepareRequest,
    NextRequest, NextValidation,
};
use async_graphql::parser::types::{
    DocumentOperations, ExecutableDocument, Selection, SelectionSet,
};
use async_graphql::{
    ErrorExtensionValues, Name, Request, Response, ServerError, ServerResult, ValidationResult,
    Variables,
};

/// Deepest query the API runs. The standard introspection query that GraphiQL, the
/// playground and codegen tools send nests 13 levels, so 15 leaves it room while still
/// refusing pathologically nested queries before they run.
pub const MAX_QUERY_DEPTH: usize = 15;

/// Most field selections one request may hold, counted after expanding fragments, with every
/// alias counted. The standard introspection query holds about 160; the site's largest query
/// about 60.
pub const MAX_QUERY_FIELDS: usize = 500;

/// Largest cost (see the module docs) the API runs.
///
/// The site's heaviest request, one 10,000-point page of `seriesData`, costs about 5,600; a
/// 100-series page of `seriesList` or `searchSeries` about 3,500. The heaviest operation of
/// the data access contract (`docs/api/operations/train3-data.graphql`), which reads one
/// series' history five ways through unpaged `dataPoints`, about 17,100. The limit admits
/// those while holding one request to about 100,000 observations, however they are asked for.
pub const MAX_QUERY_COMPLEXITY: usize = 20_000;

/// Observation rows that make one unit of cost.
pub const OBSERVATIONS_PER_COST_UNIT: usize = 25;

/// What reading one observation row costs on top of its selected fields, in fields.
pub const OBSERVATION_ROW_COST: usize = 4;

/// Most observations one page (`seriesData`, `EconomicSeries.dataPoints`,
/// `EconomicSeries.recentDataPoints`) returns. Matches the data service's page cap.
pub const MAX_OBSERVATIONS_PER_PAGE: usize = 10_000;

/// Most series one page of `DataSource.series` (or `seriesList`) returns.
pub const MAX_SERIES_PER_PAGE: usize = 100;

/// Fixed cost of a root that runs an expensive query whatever it returns: full-text search,
/// a company statement or ratios (every fact of the company), cross sections. At most about
/// 40 of them fit in one request.
pub const EXPENSIVE_QUERY_COST: usize = 500;

/// Cost of each peer `companyPeers` compares: its facts are read whole.
pub const COMPANY_PEER_COST: usize = 200;

/// Cost of a field that makes its own database query for each object it is selected on
/// (`EconomicSeries.source`, `EconomicSeries.dataPointCount`, `DataSource.seriesCount`), on top
/// of its selections.
pub const PER_OBJECT_QUERY_COST: usize = 5;

/// Rows the unpaged `dataSources` is costed at; the catalog has about a dozen sources.
pub const UNPAGED_SOURCE_ROWS: usize = 25;

/// Rows the unpaged `datasets` is costed at; sources publish a handful of datasets each.
pub const UNPAGED_DATASET_ROWS: usize = 100;

/// Most results one page of `searchSeries` returns (the search service's cap).
pub const MAX_SEARCH_RESULTS: usize = 1000;

/// Lines a company statement is costed at, at least, besides its page of unclassified facts.
pub const STATEMENT_LINE_ROWS: usize = 100;

/// Rows `crossSection` is costed at: one per area or dimension value, a few hundred at most.
pub const CROSS_SECTION_ROWS: usize = 250;

/// How long a request may run before it is cancelled. The edge gives up after 30 seconds
/// (`k8s/manifests/gateway/routes/httproutes.yaml`), so the API answers first with an error
/// the client can read.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);

/// The `code` extension of a request cancelled at [`REQUEST_TIMEOUT`].
pub const REQUEST_TIMEOUT_CODE: &str = "REQUEST_TIMEOUT";

/// The `code` extension of a request refused by the depth, field or cost limit.
pub const QUERY_LIMIT_CODE: &str = "QUERY_LIMIT_EXCEEDED";

/// A schema's request timeout when it should not be [`REQUEST_TIMEOUT`] (tests use a short
/// one). Put it in the schema's data.
#[derive(Clone, Copy, Debug)]
pub struct RequestTimeout(pub Duration);

/// Rows a page argument asks for: `first` (or `default` when omitted), at most `max`. A
/// negative value counts as none; the resolver rejects it.
pub fn page_rows(first: Option<i32>, default: usize, max: usize) -> usize {
    first.map_or(default, |first| {
        usize::try_from(first).unwrap_or(0).min(max)
    })
}

/// Cost of a list returning up to `rows` objects, each costing `child_complexity`.
pub fn list_cost(rows: usize, child_complexity: usize) -> usize {
    1usize.saturating_add(rows.saturating_mul(child_complexity))
}

/// Cost of a list returning up to `rows` observations, each costing `child_complexity`.
pub fn observation_cost(rows: usize, child_complexity: usize) -> usize {
    let units = rows.div_ceil(OBSERVATIONS_PER_COST_UNIT);
    1usize
        .saturating_add(units.saturating_mul(child_complexity.saturating_add(OBSERVATION_ROW_COST)))
}

/// Cost of `financialStatement`: it reads every fact of the company
/// ([`EXPENSIVE_QUERY_COST`]), then returns its lines and a page of unclassified facts (100 by
/// default, at most 250; 0 skips them), costed as observations.
pub fn statement_cost(unclassified_first: Option<i32>, child_complexity: usize) -> usize {
    let rows = page_rows(unclassified_first, 100, 250).max(STATEMENT_LINE_ROWS);
    EXPENSIVE_QUERY_COST.saturating_add(observation_cost(rows, child_complexity))
}

/// Why a request was refused or cut short, as the rejection metric labels it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// Over [`MAX_QUERY_COMPLEXITY`].
    Complexity,
    /// Over [`MAX_QUERY_DEPTH`].
    Depth,
    /// Over [`MAX_QUERY_FIELDS`].
    Fields,
    /// Still running at [`REQUEST_TIMEOUT`].
    Timeout,
    /// A database statement hit the API's statement timeout.
    StatementTimeout,
}

impl Rejection {
    /// Every reason, in the order [`rejection_counts`] reports them.
    pub const ALL: [Rejection; 5] = [
        Rejection::Complexity,
        Rejection::Depth,
        Rejection::Fields,
        Rejection::Timeout,
        Rejection::StatementTimeout,
    ];

    /// The metric label.
    pub fn as_str(self) -> &'static str {
        match self {
            Rejection::Complexity => "complexity",
            Rejection::Depth => "depth",
            Rejection::Fields => "fields",
            Rejection::Timeout => "timeout",
            Rejection::StatementTimeout => "statement_timeout",
        }
    }

    fn counter(self) -> &'static AtomicU64 {
        static COUNTS: [AtomicU64; 5] = [
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
            AtomicU64::new(0),
        ];
        &COUNTS[self as usize]
    }

    fn record(self) {
        self.counter().fetch_add(1, Ordering::Relaxed);
    }
}

/// Requests refused or cut short since the process started, by reason.
pub fn rejection_counts() -> [(&'static str, u64); 5] {
    Rejection::ALL.map(|reason| (reason.as_str(), reason.counter().load(Ordering::Relaxed)))
}

/// A client-facing error for a request refused by a limit, with `code`.
fn limit_error(message: impl Into<String>, code: &str) -> ServerError {
    let mut error = ServerError::new(message, None);
    let mut extensions = ErrorExtensionValues::default();
    extensions.set("code", code);
    error.extensions = Some(extensions);
    error
}

/// A client-facing resolver error for a request that asked for more rows than one field
/// returns at once, with the [`QUERY_LIMIT_CODE`].
pub fn too_many_rows(message: impl Into<String>) -> async_graphql::Error {
    use async_graphql::ErrorExtensions;
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", QUERY_LIMIT_CODE);
    })
}

/// Field selections in `doc`'s largest operation, fragments expanded and aliases counted.
/// Saturates rather than overflowing; a fragment cycle (which validation refuses) counts once.
pub fn field_count(doc: &ExecutableDocument) -> usize {
    struct Counter<'a> {
        doc: &'a ExecutableDocument,
        fragments: HashMap<&'a Name, usize>,
        expanding: HashSet<&'a Name>,
    }

    impl<'a> Counter<'a> {
        fn set(&mut self, set: &'a SelectionSet) -> usize {
            set.items.iter().fold(0usize, |total, item| {
                let count = match &item.node {
                    Selection::Field(field) => {
                        1usize.saturating_add(self.set(&field.node.selection_set.node))
                    }
                    Selection::InlineFragment(inline) => self.set(&inline.node.selection_set.node),
                    Selection::FragmentSpread(spread) => {
                        self.fragment(&spread.node.fragment_name.node)
                    }
                };
                total.saturating_add(count)
            })
        }

        fn fragment(&mut self, name: &'a Name) -> usize {
            if let Some(&count) = self.fragments.get(name) {
                return count;
            }
            let Some(definition) = self.doc.fragments.get(name) else {
                return 0;
            };
            if !self.expanding.insert(name) {
                return 0;
            }
            let count = self.set(&definition.node.selection_set.node);
            self.expanding.remove(name);
            self.fragments.insert(name, count);
            count
        }
    }

    let mut counter = Counter {
        doc,
        fragments: HashMap::new(),
        expanding: HashSet::new(),
    };
    doc.operations
        .iter()
        .map(|(_, operation)| counter.set(&operation.node.selection_set.node))
        .max()
        .unwrap_or(0)
}

/// Schema extension that cancels a request at its timeout ([`REQUEST_TIMEOUT`], or the
/// schema's [`RequestTimeout`]). Register it first, so it bounds every other extension too.
pub struct RequestDeadline;

impl ExtensionFactory for RequestDeadline {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(RequestDeadline)
    }
}

#[async_trait::async_trait]
impl Extension for RequestDeadline {
    async fn request(&self, ctx: &ExtensionContext<'_>, next: NextRequest<'_>) -> Response {
        let timeout = ctx
            .data_opt::<RequestTimeout>()
            .map_or(REQUEST_TIMEOUT, |timeout| timeout.0);
        match tokio::time::timeout(timeout, next.run(ctx)).await {
            Ok(response) => response,
            Err(_) => {
                Rejection::Timeout.record();
                tracing::warn!(
                    timeout_ms = timeout.as_millis() as u64,
                    "GraphQL request cancelled at its timeout"
                );
                Response::from_errors(vec![limit_error(
                    format!(
                        "The request was cancelled after {} seconds. Ask for less: a smaller \
                         page, fewer series, or a narrower date range.",
                        timeout.as_secs_f64()
                    ),
                    REQUEST_TIMEOUT_CODE,
                )])
            }
        }
    }
}

/// Schema extension that refuses a document over [`MAX_QUERY_FIELDS`], gives depth and cost
/// refusals the [`QUERY_LIMIT_CODE`], and counts every refusal by reason. Register it last, so
/// it sees resolver errors before [`crate::graphql::errors::SanitizeErrors`] rewrites them.
///
/// A document holding several operations is cut down to the one the request names (and the
/// fragments it uses) before it is validated, so a request is costed by the operation it runs,
/// not by every operation in the document it came from.
pub struct QueryLimits;

impl ExtensionFactory for QueryLimits {
    fn create(&self) -> Arc<dyn Extension> {
        Arc::new(QueryLimitsExtension::default())
    }
}

/// One request's [`QueryLimits`]: the operation it names, kept from `prepare_request` for
/// `parse_query`.
#[derive(Default)]
struct QueryLimitsExtension {
    operation_name: std::sync::Mutex<Option<String>>,
}

/// Keep only the operation `name` of a multi-operation `doc`, and the fragments it spreads,
/// directly or through other fragments. A document without that operation is left alone, so
/// async-graphql reports it as usual.
pub fn keep_operation(doc: &mut ExecutableDocument, name: &str) {
    let DocumentOperations::Multiple(operations) = &mut doc.operations else {
        return;
    };
    if !operations.keys().any(|key| key.as_str() == name) {
        return;
    }
    operations.retain(|key, _| key.as_str() == name);

    fn spreads<'a>(set: &'a SelectionSet, out: &mut Vec<&'a Name>) {
        for item in &set.items {
            match &item.node {
                Selection::Field(field) => spreads(&field.node.selection_set.node, out),
                Selection::InlineFragment(inline) => spreads(&inline.node.selection_set.node, out),
                Selection::FragmentSpread(spread) => out.push(&spread.node.fragment_name.node),
            }
        }
    }
    let mut pending = Vec::new();
    for operation in operations.values() {
        spreads(&operation.node.selection_set.node, &mut pending);
    }
    let mut used: HashSet<Name> = HashSet::new();
    while let Some(name) = pending.pop() {
        if used.insert(name.clone())
            && let Some(fragment) = doc.fragments.get(name)
        {
            spreads(&fragment.node.selection_set.node, &mut pending);
        }
    }
    doc.fragments.retain(|name, _| used.contains(name));
}

/// The message async-graphql refuses an over-cost query with.
const TOO_COMPLEX: &str = "Query is too complex.";
/// The message async-graphql refuses an over-deep query with.
const TOO_DEEP: &str = "Query is nested too deep.";
/// What PostgreSQL says when a statement hits `statement_timeout`.
const STATEMENT_TIMEOUT_MESSAGE: &str = "canceling statement due to statement timeout";

#[async_trait::async_trait]
impl Extension for QueryLimitsExtension {
    async fn prepare_request(
        &self,
        ctx: &ExtensionContext<'_>,
        request: Request,
        next: NextPrepareRequest<'_>,
    ) -> ServerResult<Request> {
        let request = next.run(ctx, request).await?;
        *self
            .operation_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = request.operation_name.clone();
        Ok(request)
    }

    async fn parse_query(
        &self,
        ctx: &ExtensionContext<'_>,
        query: &str,
        variables: &Variables,
        next: NextParseQuery<'_>,
    ) -> ServerResult<ExecutableDocument> {
        let mut doc = next.run(ctx, query, variables).await?;
        let operation_name = self
            .operation_name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(name) = operation_name {
            keep_operation(&mut doc, &name);
        }
        let fields = field_count(&doc);
        if fields > MAX_QUERY_FIELDS {
            Rejection::Fields.record();
            return Err(limit_error(
                format!(
                    "Query has too many fields: {fields} selections (aliases and expanded \
                     fragments included), more than the limit of {MAX_QUERY_FIELDS}. Split \
                     it into several requests."
                ),
                QUERY_LIMIT_CODE,
            ));
        }
        Ok(doc)
    }

    async fn validation(
        &self,
        ctx: &ExtensionContext<'_>,
        next: NextValidation<'_>,
    ) -> Result<ValidationResult, Vec<ServerError>> {
        let result = next.run(ctx).await;
        match result {
            Ok(result) => {
                if let Some(probe) = ctx.data_opt::<CostProbe>() {
                    probe.0.store(result.complexity, Ordering::Relaxed);
                }
                Ok(result)
            }
            Err(mut errors) => {
                for error in &mut errors {
                    let (reason, explanation) = match error.message.as_str() {
                        TOO_COMPLEX => (
                            Rejection::Complexity,
                            format!(
                                "Query is too complex: its cost is over the limit of \
                                 {MAX_QUERY_COMPLEXITY}. Lists cost their page size times \
                                 their selections; ask for smaller pages, fewer aliases, or \
                                 page observations with seriesData."
                            ),
                        ),
                        TOO_DEEP => (
                            Rejection::Depth,
                            format!(
                                "Query is nested too deep: more than {MAX_QUERY_DEPTH} levels."
                            ),
                        ),
                        _ => continue,
                    };
                    reason.record();
                    *error = limit_error(explanation, QUERY_LIMIT_CODE);
                }
                Err(errors)
            }
        }
    }

    async fn execute(
        &self,
        ctx: &ExtensionContext<'_>,
        operation_name: Option<&str>,
        next: NextExecute<'_>,
    ) -> Response {
        let response = next.run(ctx, operation_name).await;
        if response
            .errors
            .iter()
            .any(|error| error.message.contains(STATEMENT_TIMEOUT_MESSAGE))
        {
            Rejection::StatementTimeout.record();
        }
        response
    }
}

/// Where a schema stores each accepted request's cost, when its data holds one. For tests that
/// check what a query costs.
#[derive(Clone, Default)]
pub struct CostProbe(pub Arc<std::sync::atomic::AtomicUsize>);

#[cfg(test)]
mod tests {
    use super::*;

    fn count(query: &str) -> usize {
        field_count(&async_graphql::parser::parse_query(query).unwrap())
    }

    #[test]
    fn fields_are_counted_with_aliases_and_nesting() {
        assert_eq!(count("{ a { b c } d: a { b } }"), 5);
    }

    #[test]
    fn fragments_are_expanded_where_spread() {
        assert_eq!(
            count("{ a { ...F } b { ...F ... on T { e } } } fragment F on T { c d }"),
            7
        );
    }

    /// Doubling a fragment at each level makes an exponential document; it is counted (and
    /// refused) without expanding it.
    #[test]
    fn exponential_fragments_saturate_without_blowing_up() {
        let mut query = String::from("{ ...F0 } fragment F64 on Query { x }");
        for level in 0..64 {
            let next = level + 1;
            query.push_str(&format!(
                " fragment F{level} on Query {{ a{level}: f {{ ...F{next} }} b{level}: f {{ ...F{next} }} }}"
            ));
        }
        assert!(count(&query) > MAX_QUERY_FIELDS);
    }

    #[test]
    fn a_fragment_cycle_counts_once() {
        assert_eq!(
            count("{ ...A } fragment A on Query { x ...B } fragment B on Query { y ...A }"),
            2
        );
    }

    #[test]
    fn keep_operation_keeps_the_named_operation_and_its_fragments() {
        let mut doc = async_graphql::parser::parse_query(
            "query A { ...F } query B { ...G } fragment F on Q { ...H } \
             fragment G on Q { y } fragment H on Q { x }",
        )
        .unwrap();
        keep_operation(&mut doc, "A");
        let names: Vec<_> = doc
            .operations
            .iter()
            .map(|(n, _)| n.unwrap().to_string())
            .collect();
        assert_eq!(names, ["A"]);
        let mut fragments: Vec<_> = doc.fragments.keys().map(|n| n.to_string()).collect();
        fragments.sort();
        assert_eq!(fragments, ["F", "H"]);

        // An unknown name leaves the document alone.
        let mut doc = async_graphql::parser::parse_query("query A { a } query B { b }").unwrap();
        keep_operation(&mut doc, "C");
        assert_eq!(doc.operations.iter().count(), 2);
    }

    #[test]
    fn the_largest_operation_counts() {
        assert_eq!(count("query A { a } query B { a b c }"), 3);
    }

    #[test]
    fn observation_cost_scales_with_rows_and_fields() {
        assert_eq!(observation_cost(0, 3), 1);
        assert_eq!(observation_cost(1, 3), 1 + 7);
        assert_eq!(observation_cost(10_000, 10), 1 + 400 * 14);
        assert_eq!(observation_cost(usize::MAX, usize::MAX), usize::MAX);
    }

    #[test]
    fn page_rows_defaults_caps_and_ignores_negatives() {
        assert_eq!(page_rows(None, 50, 100), 50);
        assert_eq!(page_rows(Some(500), 50, 100), 100);
        assert_eq!(page_rows(Some(-1), 50, 100), 0);
    }
}
