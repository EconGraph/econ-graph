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
