use super::*;
use bigdecimal::BigDecimal;
use chrono::TimeDelta;
use diesel::sql_types::{BigInt, Text};
use econ_graph_crawler::CrawlError;
use std::io::{Cursor, Write};
use std::str::FromStr;
const FACTS: &[u8] = include_bytes!("../../test_data/companyfacts/company.json");
const SUBS: &[u8] = include_bytes!("../../test_data/companyfacts/submissions.json");
const HISTORY: &[u8] = include_bytes!("../../test_data/companyfacts/history.json");
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(
            *name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn stage(bytes: &[u8]) -> NamedTempFile {
    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(bytes).unwrap();
    tmp
}
#[test]
fn exact_values_restatements_and_duplicate_metadata() {
    let parsed = parse_companyfacts(FACTS).unwrap();
    assert_eq!(parsed.cik, "0000320193");
    assert_eq!(parsed.facts.len(), 4);
    let original = parsed
        .facts
        .iter()
        .find(|f| f.subject.concept == "Revenues" && f.form == "10-Q")
        .unwrap();
    assert_eq!(
        original.value,
        BigDecimal::from_str("119575000000.12345678901234567890").unwrap()
    );
    assert_eq!(original.fiscal_year, Some(2024));
    assert_eq!(original.frame.as_deref(), Some("CY2023Q4"));
    assert!(parsed
        .facts
        .iter()
        .any(|f| f.value == BigDecimal::from_str("900719925474099312345").unwrap()));
    assert!(parsed.facts.iter().any(|f| f.subject.unit == "USD/shares"));
}
#[test]
fn fiscal_year_null_is_optional_but_nonnull_values_must_be_integers() {
    let parsed = parse_companyfacts(FACTS).unwrap();
    let asset = parsed
        .facts
        .iter()
        .find(|f| f.subject.concept == "Assets")
        .unwrap();
    assert_eq!(asset.fiscal_year, None);
    let raw = std::str::from_utf8(FACTS).unwrap();
    let absent = parse_companyfacts(raw.replace(",\"fy\":null", "").as_bytes()).unwrap();
    let absent_asset = absent
        .facts
        .iter()
        .find(|f| f.subject.concept == "Assets")
        .unwrap();
    assert_eq!(asset, absent_asset);
    for value in ["\"2024\"", "2024.5", "true", "2147483648", "{}"] {
        let invalid = raw.replace("\"fy\":null", &format!("\"fy\":{value}"));
        assert!(
            parse_companyfacts(invalid.as_bytes()).is_err(),
            "accepted {value}"
        );
    }
}

#[test]
fn conflicting_rows_within_one_accession_are_quarantined_not_loaded() {
    let text = std::str::from_utf8(FACTS).unwrap();
    let parsed = parse_companyfacts(
        text.replacen("119575000000.12345678901234567890", "1", 1)
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(parsed.duplicates.len(), 1);
    let (first, second) = &parsed.duplicates[0];
    assert_eq!(first.id, second.id);
    assert_eq!(first.value, BigDecimal::from(1));
    assert!(parsed.facts.iter().all(|f| f.id != first.id));
    assert_eq!(parsed.facts.len(), 3);
    assert!(parse_companyfacts(FACTS).unwrap().duplicates.is_empty());
}
#[test]
fn rejects_bad_periods_and_nonnumeric_values() {
    let text = std::str::from_utf8(FACTS).unwrap();
    for changed in [
        text.replace("\"start\":\"2023-10-01\"", "\"start\":\"2025-10-01\""),
        text.replace("\"val\":2.18", "\"val\":\"2.18\""),
    ] {
        assert!(parse_companyfacts(changed.as_bytes()).is_err());
    }
}
#[test]
fn submissions_history_and_misaligned_columns() {
    let parsed = parse_submissions(SUBS, "0000320193").unwrap();
    assert_eq!(parsed.filings.len(), 2);
    assert_eq!(parsed.historical_files.len(), 1);
    assert!(parse_submissions(SUBS, "0000000001").is_err());
    let bad = std::str::from_utf8(SUBS)
        .unwrap()
        .replace("\"isXBRL\":[1,1]", "\"isXBRL\":[1]");
    assert!(parse_submissions(bad.as_bytes(), "0000320193").is_err());
}
#[test]
fn zip_integrity_and_bounds() {
    let bytes = zip(&[("CIK0000320193.json", FACTS)]);
    let tmp = stage(&bytes);
    let mut archive = BulkArchive::open_verified(tmp.path(), ArchiveLimits::default()).unwrap();
    assert_eq!(archive.read("CIK0000320193.json").unwrap(), FACTS);
    assert!(BulkArchive::open_verified(
        stage(&bytes[..bytes.len() / 2]).path(),
        ArchiveLimits::default()
    )
    .is_err());
    assert!(BulkArchive::open_verified(
        tmp.path(),
        ArchiveLimits {
            max_entry_bytes: 10,
            ..ArchiveLimits::default()
        }
    )
    .is_err());
    assert!(BulkArchive::open_verified(
        stage(&zip(&[("../CIK0000320193.json", FACTS)])).path(),
        ArchiveLimits::default()
    )
    .is_err());
    let mut corrupted = bytes.clone();
    let offset = corrupted.windows(8).position(|w| w == b"entityNa").unwrap();
    corrupted[offset] = b'X';
    assert!(
        BulkArchive::open_verified(stage(&corrupted).path(), ArchiveLimits::default()).is_err()
    );
}
#[test]
fn upstream_archive_receipt_is_required_but_staleness_is_not_fatal() {
    let old = Utc::now() - TimeDelta::hours(25);
    let parsed = source_timestamp(Some(&old.to_rfc2822())).unwrap();
    assert_eq!(parsed.timestamp(), old.timestamp());
    assert!(source_timestamp(None).is_err());
    assert!(source_timestamp(Some("yesterday")).is_err());
}
#[test]
fn bulk_downloads_retry_transient_failures_twice() {
    let transient = CrawlError::Transient("connection reset".into());
    assert_eq!(retry_delay(&transient, 1), Some(Duration::from_secs(30)));
    assert_eq!(retry_delay(&transient, 2), Some(Duration::from_secs(60)));
    assert_eq!(retry_delay(&transient, 3), None);
    let throttled = CrawlError::RateLimited {
        retry_after: Some(Duration::from_secs(3600)),
    };
    assert_eq!(retry_delay(&throttled, 1), Some(Duration::from_secs(600)));
    assert_eq!(retry_delay(&CrawlError::NotFound("gone".into()), 1), None);
}
// The database tests below need DATABASE_URL with permission to create a disposable database;
// each one migrates its own scratch database and never touches the caller's.
async fn scratch_database() -> (
    econ_graph_core::test_utils::ScratchDatabase,
    AsyncPgConnection,
) {
    std::env::var("DATABASE_URL").expect("DATABASE_URL is required for the ingestion regression");
    // The guard drops the scratch database on success or panic, after the connection
    // returned with it has been released.
    let database = tokio::task::spawn_blocking(|| {
        econ_graph_core::test_utils::ScratchDatabase::create("companyfacts")
    })
    .await
    .unwrap();
    econ_graph_core::database::run_migration_set(
        &database.url,
        econ_graph_core::database::MigrationSet::Data,
    )
    .await
    .unwrap();
    let conn = AsyncPgConnection::establish(&database.url).await.unwrap();
    (database, conn)
}
fn staged(facts: &NamedTempFile, subs: &NamedTempFile) -> IngestConfig {
    IngestConfig {
        staging_dir: std::env::temp_dir(),
        companyfacts_zip: Some(facts.path().to_owned()),
        submissions_zip: Some(subs.path().to_owned()),
        user_agent: "test@econgraph.com".into(),
        limits: ArchiveLimits::default(),
    }
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct Value {
    #[diesel(sql_type=Text)]
    value: String,
}
async fn count(conn: &mut AsyncPgConnection, sql: &str) -> i64 {
    diesel::sql_query(sql)
        .get_result::<Count>(conn)
        .await
        .unwrap()
        .count
}
async fn text(conn: &mut AsyncPgConnection, sql: &str) -> String {
    diesel::sql_query(sql)
        .get_result::<Value>(conn)
        .await
        .unwrap()
        .value
}
/// Every stored row version of company_facts and company_fact_filings: unchanged after a run
/// only if that run wrote no row there (an UPDATE, even to the same values, changes xmin).
async fn row_versions(conn: &mut AsyncPgConnection) -> String {
    text(
        conn,
        "SELECT coalesce((SELECT string_agg(id::text||':'||xmin::text, ',' ORDER BY id) FROM company_facts),'')
             ||'|'||coalesce((SELECT string_agg(id::text||':'||xmin::text, ',' ORDER BY id) FROM company_fact_filings),'') AS value",
    )
    .await
}
async fn newest_run(url: &str) -> store::LoadRun {
    let pool = econ_graph_core::database::DataPool(
        econ_graph_core::database::create_pool(url).await.unwrap(),
    );
    store::load_evidence(&pool).await.unwrap().remove(0)
}
/// A second issuer (CIK 0000789019), sorted after Apple, with one fact and one filing, or two
/// of each. Its company fields are longer than the `companies` columns.
fn later_company(extra_fact: bool) -> (Vec<u8>, Vec<u8>) {
    let mut rows = vec![serde_json::json!({"end":"2024-06-30","val":512163000000u64,
        "accn":"0000789019-24-000010","fy":2024,"fp":"FY","form":"10-K","filed":"2024-07-30"})];
    if extra_fact {
        rows.push(serde_json::json!({"end":"2025-06-30","val":619003000000u64,
            "accn":"0000789019-25-000010","fy":2025,"fp":"FY","form":"10-K","filed":"2025-07-30"}));
    }
    let facts = serde_json::json!({"cik":789019,"entityName":"Later Co","facts":{"us-gaap":{
        "Assets":{"label":"Assets","description":"Sum of the carrying amounts of all assets.","units":{"USD":rows}}}}});
    let accessions: Vec<_> = if extra_fact {
        vec!["0000789019-24-000010", "0000789019-25-000010"]
    } else {
        vec!["0000789019-24-000010"]
    };
    let n = accessions.len();
    let filed = &["2024-07-30", "2025-07-30"][..n];
    let reported = &["2024-06-30", "2025-06-30"][..n];
    let subs = serde_json::json!({"cik":"0000789019",
        "name": format!("  {}  ", "L".repeat(300)),
        "tickers":["TOOLONGTICKER"],"sic":"735712","sicDescription":"S".repeat(300),
        "entityType":"E".repeat(80),
        "filings":{"recent":{"accessionNumber":accessions,
            "filingDate":filed,"reportDate":reported,
            "form":vec!["10-K";n],"primaryDocument":vec!["msft-10k.htm";n],"isXBRL":vec![1;n]}}});
    (
        serde_json::to_vec(&facts).unwrap(),
        serde_json::to_vec(&subs).unwrap(),
    )
}
fn archives(
    apple_facts: &[u8],
    apple_subs: &[u8],
    later: &(Vec<u8>, Vec<u8>),
) -> (NamedTempFile, NamedTempFile) {
    (
        stage(&zip(&[
            ("CIK0000320193.json", apple_facts),
            ("CIK0000789019.json", &later.0),
        ])),
        stage(&zip(&[
            ("CIK0000320193.json", apple_subs),
            ("CIK0000320193-submissions-001.json", HISTORY),
            ("CIK0000789019.json", &later.1),
        ])),
    )
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with permission to create a disposable database"]
async fn database_repeat_load_failed_run_and_recovery() {
    let (database, mut conn) = scratch_database().await;
    let url = database.url.clone();
    let facts = stage(&zip(&[("CIK0000320193.json", FACTS)]));
    let subs = stage(&zip(&[
        ("CIK0000320193.json", SUBS),
        ("CIK0000320193-submissions-001.json", HISTORY),
    ]));
    let config = staged(&facts, &subs);
    let first = ingest(&url, config.clone()).await.unwrap();
    assert_eq!(
        (
            first.companies_count,
            first.facts_count,
            first.filings_count,
            first.fact_rows_written,
            first.filing_rows_written
        ),
        (1, 4, 3, 4, 3)
    );
    assert_eq!(
        count(&mut conn, "SELECT count(*) AS count FROM company_facts").await,
        4
    );
    // Labels live once per concept, not on every fact.
    assert_eq!(
        text(
            &mut conn,
            "SELECT description AS value FROM company_fact_concepts WHERE concept='Revenues'"
        )
        .await,
        "Revenue from contracts with customers."
    );
    let versions = row_versions(&mut conn).await;

    // ECO-890 acceptance: a second run over an unchanged archive writes zero rows.
    let second = ingest(&url, config.clone()).await.unwrap();
    assert_eq!(first.companyfacts_sha256, second.companyfacts_sha256);
    assert_eq!(
        (
            second.companies_count,
            second.facts_count,
            second.filings_count,
            second.fact_rows_written,
            second.filing_rows_written,
            second.companies_skipped,
            second.conflicts_count
        ),
        (1, 4, 3, 0, 0, 1, 0)
    );
    assert_eq!(row_versions(&mut conn).await, versions);
    let run = newest_run(&url).await;
    assert_eq!(
        (
            run.status.as_str(),
            run.fact_rows_written,
            run.companies_skipped
        ),
        ("succeeded", 0, 1)
    );

    // A changed archive entry with the same facts (only a frame annotation differs) is
    // compared row by row and still writes nothing.
    let reframed = std::str::from_utf8(FACTS)
        .unwrap()
        .replace("CY2023Q4", "CY2023Q4I");
    let reframed = stage(&zip(&[("CIK0000320193.json", reframed.as_bytes())]));
    let third = ingest(&url, staged(&reframed, &subs)).await.unwrap();
    assert_eq!(
        (
            third.companies_skipped,
            third.fact_rows_written,
            third.filing_rows_written,
            third.facts_count
        ),
        (0, 0, 0, 4)
    );
    assert_eq!(row_versions(&mut conn).await, versions);
    let value = text(
        &mut conn,
        "SELECT value::text AS value FROM company_facts WHERE concept='Assets'",
    )
    .await;
    assert_eq!(value, "900719925474099312345");

    let pool = econ_graph_core::database::DataPool(
        econ_graph_core::database::create_pool(&url).await.unwrap(),
    );
    let recovered = store::load_evidence(&pool).await.unwrap();
    assert!(store::check_freshness(&recovered, Utc::now()).is_err());
    assert_eq!(recovered[0].source_mode, "staged");
    let mut downloaded = recovered[0].clone();
    downloaded.source_mode = "downloaded".into();
    downloaded.companyfacts_last_modified = Some(Utc::now() - TimeDelta::hours(1));
    downloaded.submissions_last_modified = Some(Utc::now() - TimeDelta::hours(1));
    store::check_freshness(&[downloaded.clone()], Utc::now()).unwrap();
    downloaded.companyfacts_last_modified = Some(Utc::now() - TimeDelta::hours(25));
    assert!(store::check_freshness(&[downloaded], Utc::now()).is_err());
    assert!(store::check_freshness(&recovered, Utc::now() + TimeDelta::hours(25)).is_err());
    // A custom-concept-only XBRL issuer has submissions but no companyfacts entry.
    let make_issuer = |cik: &str, xbrl: bool| {
        let mut root: serde_json::Value = serde_json::from_slice(SUBS).unwrap();
        root["cik"] = serde_json::json!(cik);
        root["name"] = serde_json::json!(format!("Issuer {cik}"));
        root["filings"]["files"] = serde_json::json!([]);
        root["filings"]["recent"] = serde_json::json!({
            "accessionNumber":[format!("{cik}-24-000001")],"filingDate":["2024-02-02"],
            "reportDate":["2023-12-31"],"form":["10-K"],"primaryDocument":["report.htm"],
            "isXBRL":[if xbrl {1}else{0}]});
        serde_json::to_vec(&root).unwrap()
    };
    let custom = make_issuer("0009999902", true);
    let non_xbrl = make_issuer("0009999903", false);
    let extended_subs = stage(&zip(&[
        ("CIK0000320193.json", SUBS),
        ("CIK0000320193-submissions-001.json", HISTORY),
        ("CIK0009999902.json", &custom),
        ("CIK0009999903.json", &non_xbrl),
    ]));
    let extended = staged(&facts, &extended_subs);
    let loaded = ingest(&url, extended.clone()).await.unwrap();
    assert_eq!(
        (
            loaded.companies_count,
            loaded.facts_count,
            loaded.filings_count
        ),
        (2, 4, 4)
    );
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS count FROM company_fact_filings WHERE cik='0009999902' AND is_xbrl"
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS count FROM company_facts WHERE cik='0009999902'"
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS count FROM companies WHERE cik='0009999903'"
        )
        .await,
        0
    );
    // An archive with no usable facts cannot claim a successful nightly snapshot.
    let empty_facts = stage(&zip(&[(
        "CIK0000320193.json",
        br#"{"cik":320193,"entityName":"Apple Inc.","facts":{}}"#,
    )]));
    let no_facts = IngestConfig {
        companyfacts_zip: Some(empty_facts.path().to_owned()),
        ..extended.clone()
    };
    assert!(ingest(&url, no_facts).await.is_err());
    let failed = store::load_evidence(&pool).await.unwrap();
    assert_eq!(failed[0].status, "failed");
    assert_eq!(failed[0].facts_count, 0);
    assert!(store::check_freshness(&failed, Utc::now()).is_err());
    // Digests recorded by the failed run do not let the next run skip Apple; nothing was
    // deleted by the empty archive.
    let after_failure = ingest(&url, extended).await.unwrap();
    assert_eq!(
        (after_failure.facts_count, after_failure.fact_rows_written),
        (4, 0)
    );
    assert_eq!(after_failure.companies_skipped, 1);
    assert_eq!(
        count(&mut conn, "SELECT count(*) AS count FROM company_facts").await,
        4
    );
    drop(conn);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with permission to create a disposable database"]
async fn database_changed_fact_is_quarantined_and_later_companies_load() {
    let (database, mut conn) = scratch_database().await;
    let url = database.url.clone();
    let (facts, subs) = archives(FACTS, SUBS, &later_company(false));
    let first = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(
        (
            first.companies_count,
            first.facts_count,
            first.conflicts_count
        ),
        (2, 5, 0)
    );
    // Company fields are clipped to the column widths, as the legacy writer does.
    assert_eq!(
        text(&mut conn, "SELECT length(name)||'/'||ticker||'/'||sic_code||'/'||length(sic_description)||'/'||length(entity_type) AS value FROM companies WHERE cik='0000789019'").await,
        "255/TOOLONGTIC/7357/255/50"
    );

    // SEC corrects one stored Apple fact and one filing's form; the later company (sorted
    // after Apple) has a new filing with a new fact.
    let changed = std::str::from_utf8(FACTS)
        .unwrap()
        .replace("900719925474099312345", "900719925474099312346");
    let changed_subs = std::str::from_utf8(SUBS).unwrap().replace(
        "\"form\":[\"10-Q\",\"10-Q/A\"]",
        "\"form\":[\"10-Q\",\"10-K/A\"]",
    );
    let (facts, subs) = archives(
        changed.as_bytes(),
        changed_subs.as_bytes(),
        &later_company(true),
    );
    let second = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(second.status, "succeeded_with_conflicts");
    assert_eq!(second.conflicts_count, 2);
    assert_eq!(
        (second.fact_rows_written, second.filing_rows_written),
        (1, 1)
    );
    // The stored fact and filing are unchanged.
    assert_eq!(
        text(&mut conn, "SELECT value::text AS value FROM company_facts WHERE cik='0000320193' AND concept='Assets'").await,
        "900719925474099312345"
    );
    assert_eq!(
        text(
            &mut conn,
            "SELECT form AS value FROM company_fact_filings WHERE accession='0000320193-24-000007'"
        )
        .await,
        "10-Q/A"
    );
    // The later company loaded.
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS count FROM company_facts WHERE cik='0000789019'"
        )
        .await,
        2
    );
    // The conflicts are recorded with both values.
    assert_eq!(
        text(&mut conn, "SELECT stored->>'value'||'>'||(proposed->>'value') AS value FROM company_fact_conflicts WHERE kind='fact'").await,
        "900719925474099312345>900719925474099312346"
    );
    assert_eq!(
        text(&mut conn, "SELECT stored->>'form'||'>'||(proposed->>'form') AS value FROM company_fact_conflicts WHERE kind='filing'").await,
        "10-Q/A>10-K/A"
    );
    // ECO-890 acceptance: the run record states the conflict count.
    let run = newest_run(&url).await;
    assert_eq!(
        (run.status.as_str(), run.conflicts_count),
        ("succeeded_with_conflicts", 2)
    );
    assert_eq!(run.id, second.id);

    // The same archive again: the open conflicts are seen again (not duplicated), Apple is not
    // skipped while they are open, and nothing is written.
    let versions = row_versions(&mut conn).await;
    let third = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!((third.conflicts_count, third.companies_skipped), (2, 1));
    assert_eq!((third.fact_rows_written, third.filing_rows_written), (0, 0));
    assert_eq!(row_versions(&mut conn).await, versions);
    assert_eq!(
        count(
            &mut conn,
            "SELECT sum(seen_count)::bigint AS count FROM company_fact_conflicts"
        )
        .await,
        4
    );

    // Two source rows for one revision key that disagree are quarantined too; the run goes on.
    let contradictory = changed.replacen("119575000000.12345678901234567890", "1", 1);
    let (facts, subs) = archives(
        contradictory.as_bytes(),
        changed_subs.as_bytes(),
        &later_company(true),
    );
    let fourth = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(fourth.conflicts_count, 3);
    assert_eq!(
        count(&mut conn, "SELECT count(*) AS count FROM company_fact_conflicts WHERE kind='fact_source_duplicate' AND status='open'").await,
        1
    );
    drop(conn);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with permission to create a disposable database"]
async fn database_operator_accepts_correction_as_new_revision() {
    let (database, mut conn) = scratch_database().await;
    let url = database.url.clone();
    let (facts, subs) = archives(FACTS, SUBS, &later_company(false));
    ingest(&url, staged(&facts, &subs)).await.unwrap();
    let changed = std::str::from_utf8(FACTS)
        .unwrap()
        .replace("900719925474099312345", "900719925474099312346")
        .replace("\"val\":2.18", "\"val\":2.19");
    let (facts, subs) = archives(changed.as_bytes(), SUBS, &later_company(false));
    let loaded = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(loaded.conflicts_count, 2);
    let open = store::list_conflicts(&mut conn, "open").await.unwrap();
    assert_eq!(open.len(), 2);
    let assets = open
        .iter()
        .find(|c| c.proposed["value"] == "900719925474099312346")
        .unwrap();
    let eps = open.iter().find(|c| c.proposed["value"] == "2.19").unwrap();
    assert!(store::accept_conflict(&mut conn, assets.id, " ", None)
        .await
        .is_err());

    let revision = store::accept_conflict(
        &mut conn,
        assets.id,
        "ops@econgraph.com",
        Some("SEC corrected the 10-Q"),
    )
    .await
    .unwrap();
    store::reject_conflict(
        &mut conn,
        eps.id,
        "ops@econgraph.com",
        Some("rounding only"),
    )
    .await
    .unwrap();
    // The correction is revision 2; revision 1, the stored fact, is unchanged.
    assert_eq!(
        text(&mut conn, &format!("SELECT r.revision||':'||r.value::text||':'||f.value::text||':'||r.accepted_by AS value FROM company_fact_revisions r JOIN company_facts f ON f.id=r.fact_id WHERE r.id='{revision}'")).await,
        "2:900719925474099312346:900719925474099312345:ops@econgraph.com"
    );
    assert!(
        store::accept_conflict(&mut conn, assets.id, "ops@econgraph.com", None)
            .await
            .is_err()
    );
    assert_eq!(
        store::list_conflicts(&mut conn, "open")
            .await
            .unwrap()
            .len(),
        0
    );

    // The next load compares with revision 2 and the rejection, so it has no conflicts and
    // records Apple's digest; the load after it skips Apple.
    let next = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(
        (next.status, next.conflicts_count, next.fact_rows_written),
        ("succeeded", 0, 0)
    );
    let after = ingest(&url, staged(&facts, &subs)).await.unwrap();
    assert_eq!(after.companies_skipped, 2);

    // A further correction is compared with revision 2 and accepted as revision 3.
    let again = changed.replace("900719925474099312346", "900719925474099312347");
    let (facts, subs) = archives(again.as_bytes(), SUBS, &later_company(false));
    assert_eq!(
        ingest(&url, staged(&facts, &subs))
            .await
            .unwrap()
            .conflicts_count,
        1
    );
    let open = store::list_conflicts(&mut conn, "open").await.unwrap();
    assert_eq!(open[0].stored["revision"], 2);
    store::accept_conflict(&mut conn, open[0].id, "ops@econgraph.com", None)
        .await
        .unwrap();
    assert_eq!(
        count(
            &mut conn,
            "SELECT max(revision)::bigint AS count FROM company_fact_revisions"
        )
        .await,
        3
    );
    drop(conn);
}

#[tokio::test]
#[ignore = "requires DATABASE_URL with permission to create a disposable database"]
async fn database_archive_staleness_is_recorded_not_fatal() {
    let (_database, mut conn) = scratch_database().await;
    let counts = store::RunCounts {
        companies: 1,
        facts: 4,
        filings: 3,
        ..store::RunCounts::default()
    };
    let hashes = (Some("a".repeat(64)), Some("b".repeat(64)));
    for (age, stale) in [(TimeDelta::hours(1), false), (TimeDelta::hours(30), true)] {
        let id = store::start_run(&mut conn, false).await.unwrap();
        let modified = Some(Utc::now() - age);
        let recorded = store::finish_run(
            &mut conn,
            id,
            true,
            &counts,
            None,
            hashes.clone(),
            (modified, modified),
        )
        .await
        .unwrap();
        assert_eq!(recorded, stale);
        assert_eq!(
            text(&mut conn, &format!("SELECT status||':'||archive_stale AS value FROM company_fact_load_runs WHERE id='{id}'")).await,
            format!("succeeded:{stale}")
        );
    }
    // A succeeded run cannot claim a fresh archive it did not have.
    assert!(diesel::sql_query(
        "UPDATE company_fact_load_runs SET archive_stale=false WHERE archive_stale"
    )
    .execute(&mut conn)
    .await
    .is_err());
    drop(conn);
}
#[test]
fn active_load_grace_requires_a_fresh_verified_success() {
    let now = Utc::now();
    let completed = now - TimeDelta::hours(20);
    let successful = store::LoadRun {
        id: uuid::Uuid::new_v4(),
        started_at: completed - TimeDelta::hours(1),
        completed_at: Some(completed),
        status: "succeeded".into(),
        source_mode: "downloaded".into(),
        companies_count: 1,
        facts_count: 4,
        filings_count: 3,
        error: None,
        companyfacts_sha256: Some("a".repeat(64)),
        submissions_sha256: Some("b".repeat(64)),
        companyfacts_last_modified: Some(completed - TimeDelta::hours(6)),
        submissions_last_modified: Some(completed - TimeDelta::hours(6)),
        conflicts_count: 0,
        fact_rows_written: 4,
        filing_rows_written: 3,
        companies_skipped: 0,
        archive_stale: false,
    };
    for counts in [(0, 4, 3), (1, 0, 3), (1, 4, 0)] {
        let mut empty = successful.clone();
        empty.companies_count = counts.0;
        empty.facts_count = counts.1;
        empty.filings_count = counts.2;
        assert!(store::check_freshness(&[empty], now).is_err());
    }
    // Publication is now 26h old but was fresh when this verified snapshot completed.
    store::check_freshness(std::slice::from_ref(&successful), now).unwrap();
    // Quarantined conflicts do not stop a load from being a fresh snapshot.
    let mut with_conflicts = successful.clone();
    with_conflicts.status = store::SUCCEEDED_WITH_CONFLICTS.into();
    with_conflicts.conflicts_count = 2;
    store::check_freshness(&[with_conflicts], now).unwrap();
    let mut stale_archive = successful.clone();
    stale_archive.companyfacts_last_modified = Some(completed - TimeDelta::hours(30));
    stale_archive.archive_stale = true;
    assert!(store::check_freshness(&[stale_archive], now).is_err());
    let mut running = successful.clone();
    running.id = uuid::Uuid::new_v4();
    running.status = "running".into();
    running.started_at = now - TimeDelta::hours(1);
    running.completed_at = None;
    store::check_freshness(&[running.clone(), successful.clone()], now).unwrap();
    assert!(store::check_freshness(&[running.clone()], now).is_err());
    running.started_at = now - TimeDelta::hours(9);
    assert!(store::check_freshness(&[running, successful.clone()], now).is_err());
    let mut staged = successful.clone();
    staged.source_mode = "staged".into();
    staged.started_at = now;
    staged.completed_at = Some(now);
    staged.companyfacts_last_modified = None;
    staged.submissions_last_modified = None;
    assert!(store::check_freshness(&[staged.clone()], now).is_err());
    store::check_freshness(&[staged.clone(), successful.clone()], now).unwrap();
    let mut stale = successful;
    stale.completed_at = Some(now - TimeDelta::hours(25));
    assert!(store::check_freshness(&[staged, stale], now).is_err());
}
