//! DB-backed tests for [`reference_file_etag`], [`set_reference_file_etag`] and
//! [`merge_dataset_dimension_codes`]. Need `DATABASE_URL`; skipped when it is unset (see
//! [`stable_id_tests::database_url`]).

use econ_graph_core::models::Code;

use super::*;
use crate::dataset::{parse_dataset_file, DatasetCatalog};
use crate::persist::stable_id_tests::{database_url, FreshDb};

const WDI: &str = r#"
[[dataset]]
code = "wdi"
name = "World Development Indicators"

[[dataset.dimensions]]
name = "indicator"
label = "Indicator"

[[dataset.dimensions]]
name = "area"
label = "Country or area"
codes = [{ code = "USA", label = "United States" }]
"#;

async fn catalog(pool: &DatabasePool) {
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &["wdi"],
            parse_dataset_file(WDI).unwrap(),
        )
        .unwrap();
    sync_datasets(pool, &catalog).await.unwrap();
}

async fn dimension_codes(pool: &DatabasePool, name: &str) -> Option<Vec<Code>> {
    use econ_graph_core::schema::datasets::dsl;
    let mut conn = pool.get().await.unwrap();
    let dims: DatasetComponents = dsl::datasets
        .filter(dsl::code.eq("wdi"))
        .select(dsl::dimensions)
        .first(&mut conn)
        .await
        .unwrap();
    dims.0
        .into_iter()
        .find(|d| d.name == name)
        .and_then(|d| d.codes)
}

#[tokio::test]
async fn merge_adds_new_codes_and_updates_existing_without_dropping_others() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_reference_merge").await;
    catalog(&db.pool).await;

    // "USA" (from the file) should keep its own label unless this merge renames it; a merge
    // can add "FRA" and later rename it without touching "USA".
    merge_dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        "wdi",
        "area",
        &[("FRA".to_string(), "France".to_string())],
    )
    .await
    .unwrap();
    let codes = dimension_codes(&db.pool, "area").await.unwrap();
    assert_eq!(
        codes,
        vec![
            Code::new("FRA", "France"),
            Code::new("USA", "United States")
        ]
    );

    merge_dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        "wdi",
        "area",
        &[("FRA".to_string(), "France (updated)".to_string())],
    )
    .await
    .unwrap();
    let codes = dimension_codes(&db.pool, "area").await.unwrap();
    assert_eq!(
        codes,
        vec![
            Code::new("FRA", "France (updated)"),
            Code::new("USA", "United States")
        ]
    );

    // A dimension with no codes yet (none in the file) gets a fresh list.
    merge_dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        "wdi",
        "indicator",
        &[("NY.GDP.PCAP.CD".to_string(), "GDP per capita".to_string())],
    )
    .await
    .unwrap();
    assert_eq!(
        dimension_codes(&db.pool, "indicator").await,
        Some(vec![Code::new("NY.GDP.PCAP.CD", "GDP per capita")])
    );

    // Unknown dataset or dimension: no-op, not an error, and reported as such.
    assert!(
        !merge_dataset_dimension_codes(&db.pool, SourceId::WorldBank, "nope", "area", &[])
            .await
            .unwrap()
    );
    assert!(
        !merge_dataset_dimension_codes(&db.pool, SourceId::WorldBank, "wdi", "nope", &[])
            .await
            .unwrap()
    );

    db.drop().await;
}

#[tokio::test]
async fn merged_codes_survive_a_resync_the_file_doesnt_mention() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_reference_resync").await;
    catalog(&db.pool).await;

    merge_dataset_dimension_codes(
        &db.pool,
        SourceId::WorldBank,
        "wdi",
        "area",
        &[("FRA".to_string(), "France".to_string())],
    )
    .await
    .unwrap();

    // Re-running sync_datasets from the same file (as happens at every worker startup) must not
    // drop "FRA": it isn't in the file, but it was merged in since.
    catalog(&db.pool).await;
    let codes = dimension_codes(&db.pool, "area").await.unwrap();
    assert_eq!(
        codes,
        vec![
            Code::new("FRA", "France"),
            Code::new("USA", "United States")
        ]
    );

    db.drop().await;
}

#[tokio::test]
async fn reference_file_etag_roundtrips_and_defaults_to_none() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_reference_etag").await;
    catalog(&db.pool).await;

    let url = "https://download.bls.gov/pub/time.series/cu/cu.item";
    assert_eq!(
        reference_file_etag(&db.pool, SourceId::Bls, url)
            .await
            .unwrap(),
        None
    );

    set_reference_file_etag(&db.pool, SourceId::Bls, url, Some("\"abc123\""))
        .await
        .unwrap();
    assert_eq!(
        reference_file_etag(&db.pool, SourceId::Bls, url)
            .await
            .unwrap(),
        Some("\"abc123\"".to_string())
    );

    // A later fetch with no ETag clears the cached one (the source stopped sending one).
    set_reference_file_etag(&db.pool, SourceId::Bls, url, None)
        .await
        .unwrap();
    assert_eq!(
        reference_file_etag(&db.pool, SourceId::Bls, url)
            .await
            .unwrap(),
        None
    );

    db.drop().await;
}
