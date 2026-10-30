//! `check-selection` per source, against recorded or documented-shape catalog fixtures served
//! by a mock upstream. Every checker's [`CrawlCtx`] has [`offline_pool`], so a check that
//! touched a database would fail.

use std::collections::HashMap;

use super::*;
use crate::catalog_selection::BeaSelection;
use crate::testkit::{test_ctx_with, MockSource, Reply, Route};

const FRED_RELEASE_53: &str = include_str!("../../tests/fixtures/fred/release_series_53.json");
const BEA_NIPA_TITLES: &str = include_str!("../../tests/fixtures/bea/nipa_table_titles.json");
const BEA_REGIONAL_TITLES: &str =
    include_str!("../../tests/fixtures/bea/regional_table_titles.json");
const BEA_NIPA: &[(&str, &str, &str)] = &[
    (
        "T10101",
        "A",
        include_str!("../../tests/fixtures/bea/nipa_t10101_a.json"),
    ),
    (
        "T10101",
        "Q",
        include_str!("../../tests/fixtures/bea/nipa_t10101_q.json"),
    ),
    (
        "T10105",
        "A",
        include_str!("../../tests/fixtures/bea/nipa_t10105_a.json"),
    ),
    (
        "T10105",
        "Q",
        include_str!("../../tests/fixtures/bea/nipa_t10105_q.json"),
    ),
    (
        "T10106",
        "A",
        include_str!("../../tests/fixtures/bea/nipa_t10106_a.json"),
    ),
    (
        "T10106",
        "Q",
        include_str!("../../tests/fixtures/bea/nipa_t10106_q.json"),
    ),
];
const BEA_REGIONAL_LINES: &str =
    include_str!("../../tests/fixtures/bea/regional_sagdp2_linecodes.json");
const BEA_REGIONAL_GEOS: &str =
    include_str!("../../tests/fixtures/bea/regional_sagdp2_geofips.json");
const BLS_CU: &str = include_str!("../../tests/fixtures/bls/series/cu.series");
const BLS_LN: &str = include_str!("../../tests/fixtures/bls/series/ln.series");
const WB_CURRENT: &str =
    include_str!("../../tests/fixtures/world_bank/indicator_ny_gdp_pcap_cd.json");
const WB_ARCHIVED: &str =
    include_str!("../../tests/fixtures/world_bank/indicator_archived_ag_agr_trac_no.json");
const WB_UNKNOWN: &str = include_str!("../../tests/fixtures/world_bank/indicator_unknown.json");
const CENSUS_VARIABLES: &str = include_str!("../../tests/fixtures/census/variables.json");
const CENSUS_GEOGRAPHY: &str = include_str!("../../tests/fixtures/census/geography.json");
const CENSUS_STATES: &str = include_str!("../../tests/fixtures/census/state.txt");
const FHFA_MASTER: &str = include_str!("../../tests/fixtures/fhfa/hpi_master.csv");

fn date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Every adapter pointed at `mock`, with the testkit's keys and fast policies (overridden by
/// `policies`) and a pool that cannot connect.
fn checker_with(mock: &MockSource, policies: HashMap<SourceId, SourcePolicy>) -> Checker {
    let mut ctx = test_ctx_with(policies);
    ctx.pool = offline_pool();
    let base = mock.base_url();
    Checker {
        ctx,
        // The BLS fixtures' latest periods are in 2024.
        today: date("2025-03-01"),
        fred: FredAdapter::new(&base),
        bea: BeaAdapter::new(&base),
        bls: BlsAdapter::new(&base).with_download_url(&base),
        world_bank: WorldBankAdapter::new(&base),
        census: CensusAdapter::new(&base).with_states_url(mock.url("/geo/state.txt")),
        fhfa: FhfaAdapter::new(&base),
        fred_rules: None,
        bls_series: None,
        wdi_indicators: None,
    }
}

fn checker(mock: &MockSource) -> Checker {
    checker_with(mock, HashMap::new())
}

async fn check_one(checker: &Checker, source: SourceId) -> SourceCheck {
    let report = checker.check(&[source]).await;
    assert_eq!(report.sources.len(), 1);
    report.sources.into_iter().next().unwrap()
}

/// `(kind, entry)` of each problem, in order.
fn problems(check: &SourceCheck) -> Vec<(FindingKind, &str)> {
    check
        .problems
        .iter()
        .map(|f| (f.kind, f.entry.as_str()))
        .collect()
}

fn report_of(check: SourceCheck) -> SelectionReport {
    SelectionReport {
        sources: vec![check],
    }
}

// --- FRED --------------------------------------------------------------------------------

fn fred_rules(text: &str) -> FredCatalogRules {
    FredCatalogRules::parse(text).unwrap()
}

fn fred_series(id: &str, title: &str, notes: &str) -> Reply {
    Reply::json(serde_json::json!({"seriess": [{
        "id": id, "title": title, "frequency": "Monthly", "units": "Percent", "notes": notes
    }]}))
}

async fn mount_fred(mock: &MockSource) {
    mock.mount(
        &Route::get("/release/series").query("release_id", "53"),
        Reply::json_str(FRED_RELEASE_53),
    )
    .await;
    mock.mount(
        &Route::get("/series").query("series_id", "UNRATE"),
        fred_series("UNRATE", "Unemployment Rate", "Labor force data"),
    )
    .await;
    mock.mount(
        &Route::get("/series").query("series_id", "OLDRATE"),
        fred_series("OLDRATE", "Old Rate (DISCONTINUED)", "Replaced."),
    )
    .await;
    mock.mount(
        &Route::get("/series").query("series_id", "PRIVATE"),
        fred_series(
            "PRIVATE",
            "Licensed Index",
            "Copyright, 2026, Example Co. Reprinted with permission.",
        ),
    )
    .await;
    mock.mount(
        &Route::get("/series").query("series_id", "GONE"),
        Reply::json_str(include_str!(
            "../../tests/fixtures/fred/error_series_does_not_exist.json"
        ))
        .with_status(400),
    )
    .await;
}

const FRED_RULE: &str = "[[rules]]\nrelease_id=53\ntitle_contains='gross domestic product'\n\
                         frequencies=['Quarterly']\n";

#[tokio::test]
async fn fred_clean_selection_reports_counts_and_refresh_requests() {
    let mock = MockSource::start().await;
    mount_fred(&mock).await;
    let mut checker = checker(&mock);
    checker.fred_rules = Some(fred_rules(&format!(
        "max_series=10\nmax_catalog_series=100\ninclude=['UNRATE']\n{FRED_RULE}max_matches=3\n"
    )));
    let check = check_one(&checker, SourceId::Fred).await;
    assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    assert_eq!(check.selected, Some(4));
    assert_eq!(check.rules.len(), 1);
    assert_eq!(check.rules[0].matched, 3);
    assert_eq!(check.rules[0].max_matches, Some(3));
    assert!(check.rules[0].rule.starts_with("release_id=53"));
    assert_eq!(
        check.bounds,
        [
            Bound {
                what: "catalog rows inspected".into(),
                count: 4,
                limit: 100
            },
            Bound {
                what: "selected series".into(),
                count: 4,
                limit: 10
            },
        ]
    );
    // GDPOLD is rule-selected and discontinued: a notice, not a problem.
    assert_eq!(check.notices.len(), 1);
    assert_eq!(check.notices[0].kind, FindingKind::Discontinued);
    assert_eq!(check.notices[0].entry, "GDPOLD");
    assert_eq!(check.refresh.as_ref().unwrap().requests, 12);
    assert_eq!(report_of(check).exit_code(), EXIT_CLEAN);
}

#[tokio::test]
async fn fred_reports_missing_discontinued_restricted_and_over_broad() {
    let mock = MockSource::start().await;
    mount_fred(&mock).await;
    let mut checker = checker(&mock);
    checker.fred_rules = Some(fred_rules(&format!(
        "max_series=10\nmax_catalog_series=100\n\
         include=['GONE','OLDRATE','PRIVATE','UNRATE']\n{FRED_RULE}max_matches=2\n"
    )));
    let check = check_one(&checker, SourceId::Fred).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(
        problems(&check),
        [
            (FindingKind::OverBroad, check.rules[0].rule.as_str()),
            (FindingKind::Missing, "GONE"),
            (FindingKind::Discontinued, "OLDRATE"),
            (FindingKind::Restricted, "PRIVATE"),
        ]
    );
    assert_eq!(report_of(check).exit_code(), EXIT_PROBLEMS);
}

#[tokio::test]
async fn fred_known_retired_probe_is_a_notice() {
    let mock = MockSource::start().await;
    mount_fred(&mock).await;
    let mut checker = checker(&mock);
    checker.fred_rules = Some(fred_rules(&format!(
        "max_series=10\nmax_catalog_series=100\ninclude=['GONE']\n\
         expected_missing=['GONE']\n{FRED_RULE}"
    )));
    let check = check_one(&checker, SourceId::Fred).await;
    assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    assert!(check
        .notices
        .iter()
        .any(|n| n.kind == FindingKind::ExpectedMissing && n.entry == "GONE"));
}

#[tokio::test]
async fn fred_over_cap_selection_is_reported_without_fetching_overrides() {
    let mock = MockSource::start().await;
    mount_fred(&mock).await;
    let mut checker = checker(&mock);
    checker.fred_rules = Some(fred_rules(&format!(
        "max_series=2\nmax_catalog_series=100\ninclude=['UNRATE']\n{FRED_RULE}"
    )));
    let check = check_one(&checker, SourceId::Fred).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(problems(&check), [(FindingKind::OverCap, "selection")]);
    assert!(check.problems[0]
        .detail
        .contains("selected series 3 exceeds 2"));
    assert!(!mock
        .received_requests()
        .await
        .iter()
        .any(|r| r.url.path() == "/series"));
}

// --- BEA ---------------------------------------------------------------------------------

fn bea_rules(text: &str) -> BeaSelection {
    BeaSelection::parse(text).unwrap()
}

async fn mount_bea(mock: &MockSource) {
    for (dataset, body) in [("NIPA", BEA_NIPA_TITLES), ("Regional", BEA_REGIONAL_TITLES)] {
        mock.mount(
            &Route::get("/")
                .query("method", "GetParameterValues")
                .query("DatasetName", dataset)
                .query("ParameterName", "TableName"),
            Reply::json_str(body),
        )
        .await;
    }
    for (table, frequency, body) in BEA_NIPA {
        mock.mount(
            &Route::get("/")
                .query("method", "GetData")
                .query("DatasetName", "NIPA")
                .query("TableName", *table)
                .query("Frequency", *frequency),
            Reply::json_str(*body),
        )
        .await;
    }
    // BEA lists T30100 but serves no rows for it.
    mock.mount(
        &Route::get("/")
            .query("method", "GetData")
            .query("TableName", "T30100"),
        Reply::json(serde_json::json!({"BEAAPI": {"Results": {"Data": []}}})),
    )
    .await;
    for (target, body) in [
        ("LineCode", BEA_REGIONAL_LINES),
        ("GeoFips", BEA_REGIONAL_GEOS),
    ] {
        mock.mount(
            &Route::get("/")
                .query("method", "GetParameterValuesFiltered")
                .query("TableName", "SAGDP2")
                .query("TargetParameter", target),
            Reply::json_str(body),
        )
        .await;
    }
}

fn bea_checker(mock: &MockSource, rules: &str) -> Checker {
    let mut checker = checker(mock);
    checker.bea = BeaAdapter::new(mock.base_url()).with_selection(bea_rules(rules));
    checker
}

const BEA_CLEAN: &str = "max_tables=4\nmax_series=5000\n\
    [[rule]]\ndataset='NIPA'\nprefix='T10101'\nfrequencies=['A','Q']\nmax_matches=1\n\
    [[rule]]\ndataset='Regional'\nprefix='SAGDP2'\nfrequencies=['A']\n";

#[tokio::test]
async fn bea_clean_selection_reports_tables_series_and_refresh_requests() {
    let mock = MockSource::start().await;
    mount_bea(&mock).await;
    let check = check_one(&bea_checker(&mock, BEA_CLEAN), SourceId::Bea).await;
    assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    assert_eq!(
        check.rules.iter().map(|r| r.matched).collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(check.bounds[0].what, "selected tables");
    assert_eq!(check.bounds[0].count, 2);
    let selected = check.selected.unwrap();
    assert!(selected > 2, "{check:#?}");
    assert_eq!(check.bounds[1].count, selected);
    let refresh = check.refresh.as_ref().unwrap();
    // NIPA: one GetData per table and frequency; Regional: two requests per line.
    assert!(refresh.requests >= 4, "{refresh:?}");
    assert_eq!(report_of(check).exit_code(), EXIT_CLEAN);
}

#[tokio::test]
async fn bea_reports_a_rule_matching_nothing_and_a_table_without_rows() {
    let mock = MockSource::start().await;
    mount_bea(&mock).await;
    let rules = "max_tables=4\nmax_series=5000\n\
        [[rule]]\ndataset='NIPA'\nprefix='T99999'\nfrequencies=['A']\n\
        [[rule]]\ndataset='NIPA'\nprefix='T30100'\nfrequencies=['A']\n";
    let check = check_one(&bea_checker(&mock, rules), SourceId::Bea).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(
        problems(&check),
        [
            (FindingKind::Missing, "NIPA T99999* frequencies=A"),
            (FindingKind::Discontinued, "T30100"),
        ]
    );
}

#[tokio::test]
async fn bea_reports_an_over_broad_rule() {
    let mock = MockSource::start().await;
    mount_bea(&mock).await;
    let rules = "max_tables=8\nmax_series=5000\n\
        [[rule]]\ndataset='NIPA'\nprefix='T101'\nfrequencies=['A']\nmax_matches=1\n";
    let check = check_one(&bea_checker(&mock, rules), SourceId::Bea).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(
        problems(&check),
        [(FindingKind::OverBroad, "NIPA T101* frequencies=A")]
    );
    assert_eq!(check.rules[0].matched, 3);
}

#[tokio::test]
async fn bea_reports_an_over_cap_selection_before_requesting_tables() {
    let mock = MockSource::start().await;
    mount_bea(&mock).await;
    let rules = "max_tables=2\nmax_series=5000\n\
        [[rule]]\ndataset='NIPA'\nprefix='T101'\nfrequencies=['A']\n";
    let check = check_one(&bea_checker(&mock, rules), SourceId::Bea).await;
    assert_eq!(
        problems(&check),
        [(FindingKind::OverCap, "selected tables")]
    );
    assert!(!mock.received_requests().await.iter().any(|r| r
        .url
        .query_pairs()
        .any(|(k, v)| k == "method" && v == "GetData")));

    // Too many series is over cap too.
    let rules = "max_tables=8\nmax_series=3\n\
        [[rule]]\ndataset='NIPA'\nprefix='T10101'\nfrequencies=['A','Q']\n";
    let check = check_one(&bea_checker(&mock, rules), SourceId::Bea).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert!(check
        .problems
        .iter()
        .all(|p| p.kind == FindingKind::OverCap));
}

// --- BLS ---------------------------------------------------------------------------------

async fn mount_bls(mock: &MockSource) {
    // An extra cu.series row whose latest period is years old.
    let cu = format!(
        "{BLS_CU}CUUR0000SEOLD\t0000\tSEOLD\tU\tR\tS\t1982-84=100\tA retired item\t\t1990\tM01\t2019\tM12\n"
    );
    mock.mount(&Route::get("/cu/cu.series"), Reply::text(cu))
        .await;
    mock.mount(&Route::get("/ln/ln.series"), Reply::text(BLS_LN))
        .await;
}

#[tokio::test]
async fn bls_clean_list_reports_the_refresh_against_the_daily_quota() {
    let mock = MockSource::start().await;
    mount_bls(&mock).await;
    let mut checker = checker(&mock);
    checker.bls_series = Some(vec![
        "CUUR0000SA0".into(),
        "CUSR0000SA0".into(),
        "LNS14000000".into(),
    ]);
    let check = check_one(&checker, SourceId::Bls).await;
    assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    assert_eq!(check.curated, Some(3));
    assert_eq!(check.selected, Some(3));
    let refresh = check.refresh.as_ref().unwrap();
    // The test policy batches one series per request; BLS's keyless quota is 25.
    assert_eq!(refresh.requests, 3);
    assert_eq!(refresh.daily_quota, Some(25));
    // Only the surveys with curated ids are downloaded.
    assert_eq!(mock.received_requests().await.len(), 2);
}

#[tokio::test]
async fn bls_reports_missing_discontinued_and_over_quota() {
    let mock = MockSource::start().await;
    mount_bls(&mock).await;
    let mut checker = checker_with(
        &mock,
        HashMap::from([(
            SourceId::Bls,
            SourcePolicy {
                daily_quota: Some(1),
                ..crate::testkit::fast_policy(SourceId::Bls)
            },
        )]),
    );
    checker.bls_series = Some(vec![
        "CUUR0000SA0".into(),
        "CUUR0000SAGONE".into(),
        "CUUR0000SEOLD".into(),
        "LNS14000000".into(),
        "ZZ0000000000".into(),
    ]);
    let check = check_one(&checker, SourceId::Bls).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(
        problems(&check),
        [
            (FindingKind::Missing, "CUUR0000SAGONE"),
            (FindingKind::Discontinued, "CUUR0000SEOLD"),
            (FindingKind::Missing, "ZZ0000000000"),
            (FindingKind::OverQuota, "refresh"),
        ]
    );
    assert_eq!(check.selected, Some(2));
}

// --- World Bank ------------------------------------------------------------------------

async fn mount_world_bank(mock: &MockSource) {
    for (id, body) in [
        ("NY.GDP.PCAP.CD", WB_CURRENT),
        ("AG.AGR.TRAC.NO", WB_ARCHIVED),
        ("NOT.A.REAL.ID", WB_UNKNOWN),
    ] {
        mock.mount(
            &Route::get(format!("/indicator/{id}")).query("format", "json"),
            Reply::json_str(body),
        )
        .await;
    }
}

#[tokio::test]
async fn world_bank_clean_list_and_missing_or_archived_indicators() {
    let mock = MockSource::start().await;
    mount_world_bank(&mock).await;
    let mut checker = checker(&mock);
    checker.wdi_indicators = Some(vec!["NY.GDP.PCAP.CD".into()]);
    let check = check_one(&checker, SourceId::WorldBank).await;
    assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    assert_eq!(check.selected, Some(1));
    assert_eq!(check.refresh.as_ref().unwrap().requests, 1);

    checker.wdi_indicators = Some(vec![
        "NY.GDP.PCAP.CD".into(),
        "AG.AGR.TRAC.NO".into(),
        "NOT.A.REAL.ID".into(),
    ]);
    let check = check_one(&checker, SourceId::WorldBank).await;
    assert_eq!(check.status, CheckStatus::Problems, "{check:#?}");
    assert_eq!(
        problems(&check),
        [
            (FindingKind::Discontinued, "AG.AGR.TRAC.NO"),
            (FindingKind::Missing, "NOT.A.REAL.ID"),
        ]
    );
    assert!(check.problems[0].detail.contains("WDI Database Archives"));
}

// --- Census and FHFA ---------------------------------------------------------------------

async fn mount_census(mock: &MockSource) {
    mock.mount(&Route::get("/geo/state.txt"), Reply::text(CENSUS_STATES))
        .await;
    mock.mount(
        &Route::get("/timeseries/bds/variables.json"),
        Reply::json_str(CENSUS_VARIABLES),
    )
    .await;
    mock.mount(
        &Route::get("/timeseries/bds/geography.json"),
        Reply::json_str(CENSUS_GEOGRAPHY),
    )
    .await;
}

#[tokio::test]
async fn census_and_fhfa_report_their_discovered_scope() {
    let mock = MockSource::start().await;
    mount_census(&mock).await;
    mock.mount(
        &Route::get(crate::sources::fhfa::MASTER_CSV_PATH),
        Reply::text(FHFA_MASTER),
    )
    .await;
    // FHFA's own policy, as the worker's fetcher has it: every series in one batch.
    let fhfa_policy = SourcePolicy {
        max_batch: crate::sources::fhfa::MAX_BATCH,
        ..crate::testkit::fast_policy(SourceId::Fhfa)
    };
    let checker = checker_with(&mock, HashMap::from([(SourceId::Fhfa, fhfa_policy)]));
    let report = checker.check(&[SourceId::Census, SourceId::Fhfa]).await;
    for check in &report.sources {
        assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
        assert!(check.selected.unwrap() > 0, "{check:#?}");
        assert!(check.refresh.is_some());
    }
    let census = &report.sources[0];
    assert_eq!(census.rules.len(), 1);
    assert!(census.rules[0].matched > 0);
    // Every FHFA series shares one batch: one download refreshes them all.
    assert_eq!(report.sources[1].refresh.as_ref().unwrap().requests, 1);
    assert_eq!(report.exit_code(), EXIT_CLEAN);
}

#[tokio::test]
async fn a_catalog_that_fails_to_download_fails_the_check() {
    let mock = MockSource::start().await;
    mock.mount(
        &Route::get(crate::sources::fhfa::MASTER_CSV_PATH),
        Reply::status(500),
    )
    .await;
    let check = check_one(&checker(&mock), SourceId::Fhfa).await;
    assert_eq!(check.status, CheckStatus::Failed, "{check:#?}");
    assert!(check.error.is_some());
    assert_eq!(report_of(check).exit_code(), EXIT_FAILED);
}

// --- Keys, exit codes, output, no database -----------------------------------------------

#[tokio::test]
async fn a_source_without_its_key_is_skipped_not_failed() {
    let mock = MockSource::start().await;
    let mut checker = checker(&mock);
    checker.ctx.keys = ApiKeys::default();
    let report = checker
        .check(&[SourceId::Fred, SourceId::Bea, SourceId::Census])
        .await;
    for check in &report.sources {
        assert_eq!(check.status, CheckStatus::Skipped, "{check:#?}");
    }
    assert_eq!(
        report.sources[0].skipped.as_deref(),
        Some("no API key (FRED_API_KEY is not set)")
    );
    assert_eq!(report.exit_code(), EXIT_SKIPPED);
    assert!(mock.received_requests().await.is_empty());
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["sources"][0]["status"], "skipped");
    assert!(json["sources"][0].get("error").is_none());
    assert!(report.to_text().starts_with("FRED: skipped: no API key"));

    // A problem elsewhere outranks a skip; a failure outranks both.
    let mut problem = SourceCheck::new(SourceId::Bls);
    problem.problem(FindingKind::Missing, "X", "gone");
    let mut mixed = report.clone();
    mixed.sources.push(problem.finish(Ok(())));
    assert_eq!(mixed.exit_code(), EXIT_PROBLEMS);
    mixed
        .sources
        .push(SourceCheck::new(SourceId::Fhfa).finish(Err(CrawlError::Transient("down".into()))));
    assert_eq!(mixed.exit_code(), EXIT_FAILED);
}

#[test]
fn text_output_lists_rules_bounds_refresh_and_findings() {
    let mut check = SourceCheck::new(SourceId::Fred);
    check.selected = Some(4);
    check.rule("release_id=53".into(), 3, Some(2), "series");
    check.bound("selected series", 4, 10);
    check.refresh(RefreshEstimate::new(
        12,
        &SourcePolicy::default_for(SourceId::Fred),
    ));
    check.notice(FindingKind::Discontinued, "GDPOLD", "selected by a rule");
    let text = report_of(check.finish(Ok(()))).to_text();
    assert_eq!(
        text,
        "FRED: 1 problem(s)\n\
         \x20 rule release_id=53: 3 matched (max_matches 2)\n\
         \x20 selected series: 4 of 10\n\
         \x20 refresh: about 12 requests, 0.1 min at 120/min, no daily quota\n\
         \x20 problem over_broad: release_id=53: matches 3 series, more than its max_matches of 2\n\
         \x20 notice discontinued: GDPOLD: selected by a rule\n"
    );
}

/// The whole check, every source, against fixtures: it finishes with a pool that can't connect
/// (any read or write would fail a source) and never opens a connection.
#[tokio::test]
async fn check_selection_needs_no_database_and_writes_nothing() {
    let mock = MockSource::start().await;
    mount_fred(&mock).await;
    mount_bea(&mock).await;
    mount_bls(&mock).await;
    mount_world_bank(&mock).await;
    mount_census(&mock).await;
    mock.mount(
        &Route::get(crate::sources::fhfa::MASTER_CSV_PATH),
        Reply::text(FHFA_MASTER),
    )
    .await;
    let mut checker = bea_checker(&mock, BEA_CLEAN);
    checker.fred_rules = Some(fred_rules(&format!(
        "max_series=10\nmax_catalog_series=100\ninclude=['UNRATE']\n{FRED_RULE}"
    )));
    checker.bls_series = Some(vec!["CUUR0000SA0".into(), "LNS14000000".into()]);
    checker.wdi_indicators = Some(vec!["NY.GDP.PCAP.CD".into()]);
    let report = checker.check(&CHECKED_SOURCES).await;
    assert_eq!(report.sources.len(), CHECKED_SOURCES.len());
    for check in &report.sources {
        assert_eq!(check.status, CheckStatus::Ok, "{check:#?}");
    }
    assert_eq!(report.exit_code(), EXIT_CLEAN);
    assert_eq!(checker.ctx.pool.state().connections, 0);
    assert!(checker.ctx.pool.get().await.is_err());
}

/// A failed request's error lands in the report, never the API key in its URL.
#[tokio::test]
async fn a_failed_keyed_request_never_puts_the_key_in_the_report() {
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/"), Reply::status(500)).await;
    mock.mount(&Route::get("/release/series"), Reply::status(500))
        .await;
    let checker = bea_checker(&mock, BEA_CLEAN);
    let report = checker.check(&[SourceId::Fred, SourceId::Bea]).await;
    for check in &report.sources {
        assert_eq!(check.status, CheckStatus::Failed, "{check:#?}");
    }
    let text = format!(
        "{}{}",
        report.to_text(),
        serde_json::to_string(&report).unwrap()
    );
    assert!(
        !text.contains(crate::testkit::TEST_API_KEY),
        "the key leaked: {text}"
    );
}
