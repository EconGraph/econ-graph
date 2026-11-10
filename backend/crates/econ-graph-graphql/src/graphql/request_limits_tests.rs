//! The per-request limits of [`crate::graphql::limits`] (ECO-888): the worst-case requests are
//! refused before they run, the site's own requests are not, and a request that runs too long
//! is cancelled.
//!
//! Each refusal test is paired with an accepted request one step smaller, so it can't pass
//! because everything is refused (ECO-329).

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use async_graphql::{Request, Response, Variables};
use serde_json::json;

use crate::graphql::limits::{
    rejection_counts, CostProbe, RequestTimeout, MAX_QUERY_COMPLEXITY, MAX_QUERY_DEPTH,
    MAX_QUERY_FIELDS, QUERY_LIMIT_CODE, REQUEST_TIMEOUT_CODE,
};
use crate::graphql::schema::create_schema_with_data;
use econ_graph_core::database::DatabasePool;

/// A pool that never connects: these queries are refused, or answered from the schema, before
/// any resolver touches the database. A request that does reach a resolver fails to get a
/// connection after one second.
fn unreachable_pool() -> DatabasePool {
    let manager = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
        diesel_async::AsyncPgConnection,
    >::new("postgres://nobody@127.0.0.1:1/none");
    DatabasePool::builder()
        .connection_timeout(Duration::from_secs(1))
        .build_unchecked(manager)
}

/// Run `request` on a schema whose databases are unreachable, with `data` in its context.
async fn run<T: Send + Sync + 'static>(request: impl Into<Request>, data: T) -> Response {
    let schema = create_schema_with_data(
        econ_graph_core::database::AppPool(unreachable_pool()),
        Some(econ_graph_core::database::DataPool(unreachable_pool())),
        data,
    );
    schema.execute(request).await
}

/// The `code` extension of each error.
fn codes(response: &Response) -> Vec<Option<String>> {
    response
        .errors
        .iter()
        .map(|e| {
            e.extensions
                .as_ref()
                .and_then(|x| x.get("code"))
                .map(|c| c.to_string().trim_matches('"').to_string())
        })
        .collect()
}

/// Whether `response` was refused by the depth, field or cost limit.
fn refused_by_a_limit(response: &Response) -> bool {
    codes(response)
        .iter()
        .any(|code| code.as_deref() == Some(QUERY_LIMIT_CODE))
}

/// The cost of `request` as the schema computed it, when it passed the limits; `None` when it
/// was refused.
async fn cost(request: impl Into<Request>) -> (Option<usize>, Response) {
    let probe = CostProbe::default();
    let response = run(request, probe.clone()).await;
    let cost = probe.0.load(Ordering::Relaxed);
    (
        (cost > 0 && !refused_by_a_limit(&response)).then_some(cost),
        response,
    )
}

/// A random series id for queries that never reach the database.
const SERIES: &str = "00000000-0000-4000-8000-000000000001";

#[tokio::test]
async fn shallow_query_is_answered() {
    let response = run("{ __typename }", ()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

/// The standard introspection query that GraphiQL, the playground and codegen tools send.
const INTROSPECTION_QUERY: &str = r#"
    query IntrospectionQuery {
      __schema {
        queryType { name } mutationType { name } subscriptionType { name }
        types { ...FullType }
        directives { name description locations args { ...InputValue } }
      }
    }
    fragment FullType on __Type {
      kind name description
      fields(includeDeprecated: true) {
        name description args { ...InputValue } type { ...TypeRef }
        isDeprecated deprecationReason
      }
      inputFields { ...InputValue }
      interfaces { ...TypeRef }
      enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason }
      possibleTypes { ...TypeRef }
    }
    fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue }
    fragment TypeRef on __Type {
      kind name
      ofType { kind name ofType { kind name ofType { kind name ofType { kind name
        ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }
    }
"#;

/// The limits leave room for the standard introspection query, so schema tooling works.
#[tokio::test]
async fn standard_introspection_query_is_answered() {
    let response = run(INTROSPECTION_QUERY, ()).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let fields = crate::graphql::limits::field_count(
        &async_graphql::parser::parse_query(INTROSPECTION_QUERY).unwrap(),
    );
    assert!(fields < MAX_QUERY_FIELDS / 2, "{fields} fields");
}

/// A query nested deeper than the depth limit is refused before it runs; one level less is not.
#[tokio::test]
async fn query_deeper_than_the_limit_is_refused() {
    // `levels` ofType selections inside four wrapping fields, around `name`: depth levels + 5.
    let nested = |levels: usize| {
        let inner = (0..levels).fold("name".to_string(), |inner, _| {
            format!("ofType {{ {inner} }}")
        });
        format!("{{ __schema {{ queryType {{ fields {{ type {{ {inner} }} }} }} }} }}")
    };
    let deep = run(nested(MAX_QUERY_DEPTH - 4), ()).await;
    assert!(refused_by_a_limit(&deep), "{:?}", deep.errors);
    assert!(
        deep.errors[0].message.contains("too deep"),
        "{:?}",
        deep.errors
    );
    let at_limit = run(nested(MAX_QUERY_DEPTH - 5), ()).await;
    assert!(at_limit.errors.is_empty(), "{:?}", at_limit.errors);
}

/// `n` aliased `__typename` selections: fields for the field limit, but free for cost.
fn typenames(n: usize) -> String {
    let fields: Vec<String> = (0..n).map(|i| format!("a{i}: __typename")).collect();
    format!("{{ {} }}", fields.join(" "))
}

/// A document with exactly [`MAX_QUERY_FIELDS`] selections runs; one more is refused before it
/// is validated or run, whatever the selections cost.
#[tokio::test]
async fn one_field_over_the_field_limit_is_refused() {
    let at_limit = run(typenames(MAX_QUERY_FIELDS), ()).await;
    assert!(at_limit.errors.is_empty(), "{:?}", at_limit.errors);

    let over = run(typenames(MAX_QUERY_FIELDS + 1), ()).await;
    assert_eq!(codes(&over), vec![Some(QUERY_LIMIT_CODE.to_string())]);
    assert!(
        over.errors[0].message.contains("too many fields"),
        "{:?}",
        over.errors
    );
}

/// Aliases spread through a fragment count once per spread.
#[tokio::test]
async fn fragments_count_toward_the_field_limit() {
    let per_fragment = MAX_QUERY_FIELDS / 4 + 1;
    let fields: Vec<String> = (0..per_fragment)
        .map(|i| format!("a{i}: __typename"))
        .collect();
    let query = format!(
        "{{ ...F q1: __schema {{ queryType {{ ...T }} }} q2: __schema {{ queryType {{ ...T }} }} \
         q3: __schema {{ queryType {{ ...T }} }} }} fragment F on Query {{ {0} }} \
         fragment T on __Type {{ {0} }}",
        fields.join(" ")
    );
    let response = run(query, ()).await;
    assert!(
        response.errors[0].message.contains("too many fields"),
        "{:?}",
        response.errors
    );
}

/// `seriesData` pages selecting only `totalCount`, one per entry of `units`, each reading that
/// many cost units of observations: each costs `1 + units * (1 + OBSERVATION_ROW_COST)`.
fn series_data_pages(units: &[usize]) -> String {
    let pages: Vec<String> = units
        .iter()
        .enumerate()
        .map(|(i, units)| {
            format!(
                r#"p{i}: seriesData(seriesId: "{SERIES}", first: {}) {{ totalCount }}"#,
                units * crate::graphql::limits::OBSERVATIONS_PER_COST_UNIT
            )
        })
        .collect();
    pages.join(" ")
}

/// A query costing exactly the limit passes it; one costing one more is refused. Pins the
/// observation cost formula: 10 pages of 3998 units in all cost 10 + 3998 * 5 = 20000.
#[tokio::test]
async fn query_at_the_cost_limit_is_accepted_and_one_more_is_refused() {
    assert_eq!(MAX_QUERY_COMPLEXITY, 20_000);
    let pages = series_data_pages(&[400, 400, 400, 400, 400, 400, 400, 400, 400, 398]);
    let (cost_at_limit, response) = cost(format!("{{ {pages} }}")).await;
    assert_eq!(
        cost_at_limit,
        Some(MAX_QUERY_COMPLEXITY),
        "{:?}",
        response.errors
    );

    // `seriesExists` is a scalar root field: cost 1.
    let over = format!(r#"{{ {pages} e: seriesExists(id: "{SERIES}") }}"#);
    let (cost_over, response) = cost(over).await;
    assert_eq!(cost_over, None);
    assert_eq!(codes(&response), vec![Some(QUERY_LIMIT_CODE.to_string())]);
    assert!(
        response.errors[0].message.contains("too complex"),
        "{:?}",
        response.errors
    );
}

/// The worst case ECO-888 describes: many aliases of a 100-series page, each with its full
/// `dataPoints`. Refused before it runs, as is a single such page, and the same through
/// `dataSources { series }` and `recentDataPoints`. The same pages without observations, and
/// one series' full `dataPoints`, are not refused.
#[tokio::test]
async fn worst_case_observation_requests_are_refused_before_they_run() {
    let aliased: Vec<String> = (0..50)
        .map(|i| {
            format!(
                r#"a{i}: seriesList(pagination: {{first: 100, after: "{}"}}) {{ nodes {{ dataPoints {{ date value }} }} }}"#,
                i * 100
            )
        })
        .collect();
    let refused = [
        format!("{{ {} }}", aliased.join(" ")),
        r#"{ seriesList(pagination: {first: 100}) { nodes { dataPoints { date value } } } }"#
            .to_string(),
        r#"{ seriesList(pagination: {first: 10}) { nodes { dataPoints { value } } } }"#.to_string(),
        r#"{ dataSources { series(first: 100) { nodes { dataPoints { value } } } } }"#.to_string(),
        r#"{ seriesList(pagination: {first: 100}) { nodes { recentDataPoints(limit: 10000) { date } } } }"#
            .to_string(),
        seven_pages_and("h: seriesData(seriesId: \"x\", first: 10000) { nodes { date value } }"),
    ];
    for query in refused {
        let started = Instant::now();
        let response = run(query.clone(), ()).await;
        assert_eq!(
            codes(&response),
            vec![Some(QUERY_LIMIT_CODE.to_string())],
            "{query}: {:?}",
            response.errors
        );
        assert!(response.errors[0].message.contains("too complex"));
        // Refused at validation: no resolver waited for the unreachable database.
        assert!(started.elapsed() < Duration::from_millis(900), "{query}");
    }

    let accepted = [
        r#"{ seriesList(pagination: {first: 100}) { nodes { id title } } }"#.to_string(),
        r#"{ seriesList(pagination: {first: 9}) { nodes { dataPoints { value } } } }"#.to_string(),
        r#"{ dataSources { series(first: 100) { nodes { id title } } } }"#.to_string(),
        format!(r#"{{ series(id: "{SERIES}") {{ dataPoints {{ date value }} }} }}"#),
        seven_pages_and(""),
    ];
    for query in accepted {
        let (cost, response) = cost(query.clone()).await;
        assert!(cost.is_some(), "{query}: {:?}", response.errors);
    }
}

/// Seven aliased 10,000-point `seriesData` pages of `{ date value }` (2,801 each), and `extra`.
fn seven_pages_and(extra: &str) -> String {
    let pages: Vec<String> = (0..7)
        .map(|i| {
            format!(r#"p{i}: seriesData(seriesId: "x", first: 10000) {{ nodes {{ date value }} }}"#)
        })
        .collect();
    format!("{{ {} {extra} }}", pages.join(" "))
}

/// Every operation of the data access contract (`docs/api/operations/train3-data.graphql`)
/// passes the limits: its consumers keep working.
#[tokio::test]
async fn the_data_access_contract_passes_the_limits() {
    let operations = include_str!("../../../../../docs/api/operations/train3-data.graphql");
    for (name, variables) in [
        (
            "Train3ObservationPage",
            json!({"seriesId": SERIES, "first": 10000}),
        ),
        (
            "Train3SeriesSubject",
            json!({"seriesId": SERIES, "datasetId": SERIES}),
        ),
    ] {
        let request = Request::new(operations)
            .operation_name(name)
            .variables(Variables::from_json(variables));
        let (cost, response) = cost(request).await;
        let cost = cost.unwrap_or_else(|| panic!("{name}: {:?}", response.errors));
        println!("{name} costs {cost}");
        assert!(cost <= MAX_QUERY_COMPLEXITY * 9 / 10, "{name} costs {cost}");
    }
}

/// A request still running at its timeout is cancelled and answered with a `REQUEST_TIMEOUT`
/// error, and counted. Here the resolver waits a second for a database connection; the
/// timeout is 100 ms.
#[tokio::test]
async fn a_request_past_its_timeout_is_cancelled() {
    let timeouts_before = timeouts();
    let started = Instant::now();
    let response = run(
        "{ dataSources { id } }",
        RequestTimeout(Duration::from_millis(100)),
    )
    .await;
    let elapsed = started.elapsed();
    assert_eq!(
        codes(&response),
        vec![Some(REQUEST_TIMEOUT_CODE.to_string())]
    );
    assert!(response.data == async_graphql::Value::Null);
    assert!(elapsed < Duration::from_millis(900), "took {elapsed:?}");
    assert!(timeouts() > timeouts_before);

    // With room to finish, the same request fails on the database instead, not the timeout.
    let response = run(
        "{ dataSources { id } }",
        RequestTimeout(Duration::from_secs(5)),
    )
    .await;
    assert!(!response.errors.is_empty());
    assert!(!codes(&response).contains(&Some(REQUEST_TIMEOUT_CODE.to_string())));
}

fn timeouts() -> u64 {
    rejection_counts()
        .into_iter()
        .find(|(reason, _)| *reason == "timeout")
        .unwrap()
        .1
}

/// Refusals are counted by reason.
#[tokio::test]
async fn refusals_are_counted_by_reason() {
    let count = |reason: &str| {
        rejection_counts()
            .into_iter()
            .find(|(r, _)| *r == reason)
            .unwrap()
            .1
    };
    let (fields, complexity) = (count("fields"), count("complexity"));
    run(typenames(MAX_QUERY_FIELDS + 1), ()).await;
    run(
        r#"{ seriesList(pagination: {first: 100}) { nodes { dataPoints { value } } } }"#,
        (),
    )
    .await;
    assert!(count("fields") > fields);
    assert!(count("complexity") > complexity);
    let reasons: Vec<&str> = rejection_counts().iter().map(|(r, _)| *r).collect();
    assert_eq!(
        reasons,
        [
            "complexity",
            "depth",
            "fields",
            "timeout",
            "statement_timeout"
        ]
    );
}

/// The text of the template literal that starts right after `marker` in `file` (a path under
/// the repository's `frontend/src`), so these tests send the site's queries as they are today.
fn frontend_query(file: &str, marker: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../frontend/src")
        .join(file);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found in {file}"))
        + marker.len();
    let end = start + source[start..].find('`').expect("closing backtick");
    let query = source[start..end].to_string();
    assert!(!query.contains("${"), "{marker}: interpolated query");
    query
}

/// Every query the site sends, at the largest page it asks for, passes the limits with room
/// to spare (at most 60% of the cost limit), so pages, charts and CSV exports keep working.
/// The queries are read from the frontend sources, so a change there is checked here.
#[tokio::test]
async fn the_sites_own_queries_pass_the_limits() {
    let cik = "0000320193";
    let cases = [
        (
            "utils/graphql.ts",
            "GET_SERIES_LIST: `",
            json!({"pagination": {"first": 100}}),
        ),
        (
            "utils/graphql.ts",
            "GET_SERIES_DETAIL: `",
            json!({"id": SERIES}),
        ),
        (
            "utils/graphql.ts",
            "GET_SERIES_DATA: `",
            json!({"seriesId": SERIES, "first": 10000, "transformation": "YEAR_OVER_YEAR"}),
        ),
        (
            "utils/graphql.ts",
            "SEARCH_SERIES: `",
            json!({"query": "gdp", "first": 100}),
        ),
        (
            "utils/graphql.ts",
            "GET_SEARCH_SUGGESTIONS: `",
            json!({"partialQuery": "gd", "limit": 10}),
        ),
        ("utils/graphql.ts", "GET_DATA_SOURCES: `", json!({})),
        (
            "hooks/useLatestObservation.ts",
            "GET_SERIES_LATEST_OBSERVATION = `",
            json!({"sourceName": "FRED", "externalId": "GDP"}),
        ),
        (
            "components/global/hooks/useWorldMapData.ts",
            "GET_MAP_DATASETS = `",
            json!({}),
        ),
        (
            "components/global/hooks/useWorldMapData.ts",
            "GET_MAP_CROSS_SECTION = `",
            json!({"datasetId": SERIES, "filter": [], "across": "area"}),
        ),
        (
            "components/global/hooks/useWorldMapData.ts",
            "GET_MAP_SERIES_META = `",
            json!({"id": SERIES}),
        ),
        (
            "graphql/companyResearch.ts",
            "FIND_COMPANIES = `",
            json!({"search": "apple", "first": 100}),
        ),
        (
            "graphql/companyResearch.ts",
            "GET_RESEARCH_COMPANY = `",
            json!({"cik": cik}),
        ),
        (
            "graphql/companyResearch.ts",
            "GET_COMPANY_STATEMENT = `",
            json!({"cik": cik, "kind": "INCOME_STATEMENT"}),
        ),
        (
            "graphql/companyResearch.ts",
            "GET_COMPANY_COMPARISONS = `",
            json!({"cik": cik, "periodEnd": "2024-12-31"}),
        ),
        (
            "graphql/companyResearch.ts",
            "GET_COMPANY_OTHER_FACTS = `",
            json!({"cik": cik, "periodEnd": "2024-12-31", "instantOnly": false}),
        ),
        (
            "graphql/companyResearch.ts",
            "GET_COMPANY_FACT_HISTORY = `",
            json!({"cik": cik}),
        ),
        ("graphql/companyNotes.ts", "query: `", json!({"cik": cik})),
    ];
    let runs = cases
        .into_iter()
        .map(|(file, marker, variables)| async move {
            let query = frontend_query(file, marker);
            let request = Request::new(query).variables(Variables::from_json(variables));
            let (cost, response) = cost(request).await;
            (file, marker, cost, response)
        });
    for (file, marker, cost, response) in futures::future::join_all(runs).await {
        let cost = cost.unwrap_or_else(|| panic!("{file} {marker}: {:?}", response.errors));
        println!("{file} {marker} costs {cost}");
        assert!(
            cost * 10 <= MAX_QUERY_COMPLEXITY * 6,
            "{file} {marker} costs {cost}, too close to the limit"
        );
    }
}
