//! DB-backed tests for the validator store: [`stored_fetch_state`], the validators
//! [`persist_series`] writes, [`url_validators`] and [`set_url_validators_conn`]. Need
//! `DATABASE_URL`; skipped when it is unset.

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
"#;

fn series(validators: Option<Validators>) -> FetchedSeries {
    FetchedSeries {
        metadata: None,
        points: Vec::new(),
        dataset: SeriesDataset::new("wdi", [("indicator", "NY.GDP.PCAP.CD"), ("area", "USA")]),
        validators,
    }
}

#[tokio::test]
async fn series_validators_round_trip_and_none_clears_them() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_series_validators").await;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &["wdi"],
            parse_dataset_file(WDI).unwrap(),
        )
        .unwrap();
    sync_datasets(&db.pool, &catalog).await.unwrap();
    let id = "wdi/NY.GDP.PCAP.CD.USA".to_string();
    let ids = [id.clone(), "never-fetched".to_string()];

    assert!(stored_fetch_state(&db.pool, SourceId::WorldBank, &ids)
        .await
        .unwrap()
        .is_empty());

    let v = Validators {
        etag: Some("\"e\"".into()),
        last_modified: Some("Tue, 01 Sep 2026 00:00:00 GMT".into()),
        content_sha256: Some("ff".into()),
        version: Some("2026-09-01".into()),
    };
    persist_series(&db.pool, SourceId::WorldBank, &id, &series(Some(v.clone())))
        .await
        .unwrap();
    let state = stored_fetch_state(&db.pool, SourceId::WorldBank, &ids)
        .await
        .unwrap();
    assert_eq!(state.len(), 1, "an id with no series row is absent");
    assert_eq!(
        state[&id],
        StoredFetchState {
            dataset: series(None).dataset,
            validators: Some(v),
        }
    );

    persist_series(&db.pool, SourceId::WorldBank, &id, &series(None))
        .await
        .unwrap();
    let state = stored_fetch_state(&db.pool, SourceId::WorldBank, &ids)
        .await
        .unwrap();
    assert_eq!(state[&id].validators, None);

    db.drop().await;
}

#[tokio::test]
async fn url_validators_round_trip_and_share_the_reference_cache() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_url_validators").await;
    let url = "https://example.test/catalog.csv";
    assert_eq!(
        url_validators(&db.pool, SourceId::Fhfa, url).await.unwrap(),
        None
    );

    let v = Validators {
        etag: Some("\"e\"".into()),
        last_modified: None,
        content_sha256: Some("ff".into()),
        version: None,
    };
    set_url_validators(&db.pool, SourceId::Fhfa, url, &v)
        .await
        .unwrap();
    assert_eq!(
        url_validators(&db.pool, SourceId::Fhfa, url).await.unwrap(),
        Some(v)
    );
    // The same row backs reference_file_etag.
    assert_eq!(
        reference_file_etag(&db.pool, SourceId::Fhfa, url)
            .await
            .unwrap()
            .as_deref(),
        Some("\"e\"")
    );
    // Another source's row for the same URL is separate.
    assert_eq!(
        url_validators(&db.pool, SourceId::Fred, url).await.unwrap(),
        None
    );

    db.drop().await;
}
