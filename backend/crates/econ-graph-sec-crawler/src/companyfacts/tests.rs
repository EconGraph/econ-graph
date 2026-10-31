use super::*;
use bigdecimal::BigDecimal;
use chrono::TimeDelta;
use diesel::sql_types::{BigInt, Text};
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
fn rejects_conflicts_bad_periods_and_nonnumeric_values() {
    let text = std::str::from_utf8(FACTS).unwrap();
    for changed in [
        text.replacen("119575000000.12345678901234567890", "1", 1),
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
fn upstream_archive_freshness() {
    let now = Utc::now();
    assert!(validate_source_timestamp(Some(&now.to_rfc2822()), now).is_ok());
    assert!(validate_source_timestamp(None, now).is_err());
    assert!(
        validate_source_timestamp(Some(&(now - TimeDelta::hours(25)).to_rfc2822()), now).is_err()
    );
    assert!(
        validate_source_timestamp(Some(&(now + TimeDelta::hours(1)).to_rfc2822()), now).is_err()
    );
}
#[tokio::test]
async fn database_repeat_load_failed_run_and_recovery() {
    let Ok(url) = std::env::var("TRAIN5_INGEST_TEST_DATABASE_URL") else {
        return;
    };
    econ_graph_core::database::run_migration_set(
        &url,
        econ_graph_core::database::MigrationSet::Data,
    )
    .await
    .unwrap();
    let mut conn = AsyncPgConnection::establish(&url).await.unwrap();
    diesel::sql_query("TRUNCATE company_fact_load_runs,company_facts,company_fact_filings")
        .execute(&mut conn)
        .await
        .unwrap();
    let facts = stage(&zip(&[("CIK0000320193.json", FACTS)]));
    let subs = stage(&zip(&[
        ("CIK0000320193.json", SUBS),
        ("CIK0000320193-submissions-001.json", HISTORY),
    ]));
    let config = IngestConfig {
        staging_dir: std::env::temp_dir(),
        companyfacts_zip: Some(facts.path().to_owned()),
        submissions_zip: Some(subs.path().to_owned()),
        user_agent: "test@econgraph.com".into(),
        limits: ArchiveLimits::default(),
    };
    let first = ingest(&url, config.clone()).await.unwrap();
    let second = ingest(&url, config.clone()).await.unwrap();
    assert_eq!(
        (
            first.companies_count,
            first.facts_count,
            first.filings_count
        ),
        (1, 4, 3)
    );
    assert_eq!(first.companyfacts_sha256, second.companyfacts_sha256);
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type=BigInt)]
        count: i64,
    }
    let count: Count = diesel::sql_query("SELECT count(*) AS count FROM company_facts")
        .get_result(&mut conn)
        .await
        .unwrap();
    assert_eq!(count.count, 4);
    #[derive(QueryableByName)]
    struct Value {
        #[diesel(sql_type=Text)]
        value: String,
    }
    let value: Value =
        diesel::sql_query("SELECT value::text AS value FROM company_facts WHERE concept='Assets'")
            .get_result(&mut conn)
            .await
            .unwrap();
    assert_eq!(value.value, "900719925474099312345");
    let changed = std::str::from_utf8(FACTS)
        .unwrap()
        .replace("900719925474099312345", "900719925474099312346");
    let bad = stage(&zip(&[("CIK0000320193.json", changed.as_bytes())]));
    let mut broken = config.clone();
    broken.companyfacts_zip = Some(bad.path().to_owned());
    assert!(ingest(&url, broken).await.is_err());
    let pool = econ_graph_core::database::DataPool(
        econ_graph_core::database::create_pool(&url).await.unwrap(),
    );
    let failed = store::load_evidence(&pool).await.unwrap();
    assert_eq!(failed[0].status, "failed");
    assert!(store::check_freshness(&failed, Utc::now()).is_err());
    let value: Value =
        diesel::sql_query("SELECT value::text AS value FROM company_facts WHERE concept='Assets'")
            .get_result(&mut conn)
            .await
            .unwrap();
    assert_eq!(value.value, "900719925474099312345");
    ingest(&url, config).await.unwrap();
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
    let extended = IngestConfig {
        submissions_zip: Some(extended_subs.path().to_owned()),
        companyfacts_zip: Some(facts.path().to_owned()),
        staging_dir: std::env::temp_dir(),
        user_agent: "test@econgraph.com".into(),
        limits: ArchiveLimits::default(),
    };
    let loaded = ingest(&url, extended.clone()).await.unwrap();
    assert_eq!(
        (
            loaded.companies_count,
            loaded.facts_count,
            loaded.filings_count
        ),
        (2, 4, 4)
    );
    let count: Count = diesel::sql_query(
        "SELECT count(*) AS count FROM company_fact_filings WHERE cik='0009999902' AND is_xbrl",
    )
    .get_result(&mut conn)
    .await
    .unwrap();
    assert_eq!(count.count, 1);
    let count: Count =
        diesel::sql_query("SELECT count(*) AS count FROM company_facts WHERE cik='0009999902'")
            .get_result(&mut conn)
            .await
            .unwrap();
    assert_eq!(count.count, 0);
    let count: Count =
        diesel::sql_query("SELECT count(*) AS count FROM companies WHERE cik='0009999903'")
            .get_result(&mut conn)
            .await
            .unwrap();
    assert_eq!(count.count, 0);
    // An archive with no usable facts cannot claim a successful nightly snapshot.
    let empty_facts = stage(&zip(&[(
        "CIK0000320193.json",
        br#"{"cik":320193,"entityName":"Apple Inc.","facts":{}}"#,
    )]));
    let no_facts = IngestConfig {
        companyfacts_zip: Some(empty_facts.path().to_owned()),
        ..extended
    };
    assert!(ingest(&url, no_facts).await.is_err());
    let failed = store::load_evidence(&pool).await.unwrap();
    assert_eq!(failed[0].status, "failed");
    assert_eq!(failed[0].facts_count, 0);
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
