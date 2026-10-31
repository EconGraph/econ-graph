use chrono::{TimeZone, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use econ_graph_core::schema::{companies, financial_statements};
use econ_graph_core::test_utils::ScratchDatabase;
use econ_graph_core::{create_pool, run_migration_set, MigrationSet};
use econ_graph_sec_crawler::storage::{XbrlStorage, XbrlStorageConfig};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn repeated_ingest_preserves_legacy_id_and_refuses_silent_retargeting() {
    let database = ScratchDatabase::create("filing_identity");
    run_migration_set(&database.url, MigrationSet::Data)
        .await
        .unwrap();
    let pool = create_pool(&database.url).await.unwrap();
    let company = Uuid::from_u128(100);
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let mut conn = pool.get().await.unwrap();
    diesel::insert_into(companies::table)
        .values((
            companies::id.eq(company),
            companies::cik.eq("0000320193"),
            companies::name.eq("Identity fixture"),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
    let first = storage
        .store_xbrl_file(
            "0000320193-24-000001",
            b"original",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None,
        )
        .await
        .unwrap();
    // Simulate an existing legacy row; ingestion must reuse this ID rather than derive one.
    let legacy = Uuid::from_u128(101);
    let mut conn = pool.get().await.unwrap();
    diesel::update(financial_statements::table.find(first.id))
        .set(financial_statements::id.eq(legacy))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let repeated = storage
        .store_xbrl_file(
            "0000320193-24-000001",
            b"original",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None,
        )
        .await
        .unwrap();
    assert_eq!(repeated.id, legacy);
    let changed = storage
        .store_xbrl_file(
            "0000320193-24-000001",
            b"changed",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None,
        )
        .await
        .unwrap_err();
    assert!(changed.to_string().contains("filing content changed"));
    assert_eq!(
        storage
            .retrieve_xbrl_file("0000320193-24-000001")
            .await
            .unwrap(),
        b"original"
    );
    let amendment = storage
        .store_xbrl_file(
            "0000320193-24-000002",
            b"changed",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K/A"),
            None,
        )
        .await
        .unwrap();
    assert_ne!(amendment.id, legacy);
    let (left, right) = tokio::join!(
        storage.store_xbrl_file(
            "0000320193-24-000003",
            b"concurrent",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None
        ),
        storage.store_xbrl_file(
            "0000320193-24-000003",
            b"concurrent",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None
        ),
    );
    assert_eq!(left.unwrap().id, right.unwrap().id);
    let mut conn = pool.get().await.unwrap();
    assert_eq!(
        financial_statements::table
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .unwrap(),
        3
    );
    drop(conn);
    drop(storage);
    drop(pool);
}

async fn fixture(
    name: &str,
) -> (
    ScratchDatabase,
    econ_graph_core::database::DatabasePool,
    XbrlStorage,
    Uuid,
    chrono::DateTime<Utc>,
    Uuid,
) {
    let database = ScratchDatabase::create(name);
    run_migration_set(&database.url, MigrationSet::Data)
        .await
        .unwrap();
    let pool = create_pool(&database.url).await.unwrap();
    let company = Uuid::new_v4();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let mut conn = pool.get().await.unwrap();
    diesel::insert_into(companies::table)
        .values((
            companies::id.eq(company),
            companies::cik.eq("0000320193"),
            companies::name.eq("Completion fixture"),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let storage = XbrlStorage::new(pool.clone(), XbrlStorageConfig::default());
    let filing = storage
        .store_xbrl_file(
            "0000320193-24-000001",
            b"original",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None,
        )
        .await
        .unwrap()
        .id;
    (database, pool, storage, company, now, filing)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn metadata_only_completion_is_atomic_preserves_identity_and_rejects_partial_rows() {
    let (_database, pool, storage, company, now, filing) = fixture("filing_completion").await;
    let mut conn = pool.get().await.unwrap();
    diesel::update(financial_statements::table.find(filing))
        .set((
            financial_statements::xbrl_file_content.eq(None::<Vec<u8>>),
            financial_statements::xbrl_file_hash.eq(None::<String>),
            financial_statements::xbrl_file_size_bytes.eq(None::<i64>),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let bad_metadata = storage
        .store_xbrl_file(
            "0000320193-24-000001",
            b"original",
            company,
            now,
            now,
            2025,
            None,
            Some("10-K"),
            None,
        )
        .await
        .unwrap_err();
    assert!(bad_metadata.to_string().contains("filing metadata changed"));
    let (left, right) = tokio::join!(
        storage.store_xbrl_file(
            "0000320193-24-000001",
            b"left",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None
        ),
        storage.store_xbrl_file(
            "0000320193-24-000001",
            b"right",
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None
        )
    );
    assert_ne!(
        left.is_ok(),
        right.is_ok(),
        "exactly one competing payload must win"
    );
    let winner = if let Ok(row) = left {
        row
    } else {
        right.unwrap()
    };
    assert_eq!(winner.id, filing);
    let bytes = storage
        .retrieve_xbrl_file("0000320193-24-000001")
        .await
        .unwrap();
    assert!(bytes == b"left" || bytes == b"right");
    let mut conn = pool.get().await.unwrap();
    // A hash without content is inconsistent, rather than a metadata-only row to overwrite.
    diesel::update(financial_statements::table.find(filing))
        .set(financial_statements::xbrl_file_content.eq(None::<Vec<u8>>))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    assert!(storage
        .store_xbrl_file(
            "0000320193-24-000001",
            &bytes,
            company,
            now,
            now,
            2024,
            None,
            Some("10-K"),
            None
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("filing content changed"));
}

#[cfg(feature = "xbrl-parser")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_dts_storage_reuses_null_safe_identity_and_completes_unresolved_references() {
    use econ_graph_core::schema::{
        xbrl_instance_dts_references as refs, xbrl_taxonomy_schemas as schemas,
    };
    use econ_graph_sec_crawler::models::DtsReference;
    let (_database, pool, storage, _, _, filing) = fixture("filing_dts_race").await;
    let reference = DtsReference {
        reference_type: "schemaRef".into(),
        reference_href: "https://example.com/us-gaap.xsd".into(),
        reference_role: None,
        reference_arcrole: None,
    };
    let unresolved = Uuid::new_v4();
    let mut conn = pool.get().await.unwrap();
    diesel::insert_into(refs::table)
        .values((
            refs::id.eq(unresolved),
            refs::statement_id.eq(filing),
            refs::reference_type.eq(&reference.reference_type),
            refs::reference_href.eq(&reference.reference_href),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);
    let results = futures::future::join_all((0..12).map(|_| {
        storage.store_taxonomy_component(
            &reference,
            b"schema",
            "https://example.com/us-gaap.xsd",
            &filing,
        )
    }))
    .await;
    for result in results {
        result.unwrap();
    }
    // Nullable role and arcrole are identity fields; distinct references must survive.
    for (role, arcrole) in [
        (Some("role"), None),
        (None, Some("arc")),
        (Some("role"), Some("arc")),
    ] {
        let distinct = DtsReference {
            reference_role: role.map(str::to_owned),
            reference_arcrole: arcrole.map(str::to_owned),
            ..reference.clone()
        };
        storage
            .store_taxonomy_component(
                &distinct,
                b"schema",
                "https://example.com/us-gaap.xsd",
                &filing,
            )
            .await
            .unwrap();
    }
    let mut conn = pool.get().await.unwrap();
    assert_eq!(
        refs::table
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        schemas::table
            .count()
            .get_result::<i64>(&mut conn)
            .await
            .unwrap(),
        4,
        "duplicate requests must not leave orphan schema rows"
    );
    let (resolved, schema_id, error) = refs::table
        .find(unresolved)
        .select((
            refs::is_resolved,
            refs::resolved_schema_id,
            refs::resolution_error,
        ))
        .first::<(bool, Option<Uuid>, Option<String>)>(&mut conn)
        .await
        .unwrap();
    assert!(resolved && schema_id.is_some() && error.is_none());
}
