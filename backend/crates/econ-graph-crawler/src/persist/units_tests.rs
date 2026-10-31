//! DB-backed tests for the units columns [`persist_series`] writes: `units_source`, `units_raw`
//! and `units_parsed`. Need `DATABASE_URL`; skipped when it is unset.

use super::*;
use crate::adapter::NewSeriesMetadataLite;
use crate::dataset::{parse_dataset_file, DatasetCatalog};
use crate::persist::stable_id_tests::{database_url, FreshDb};
use crate::units::{Scale, StructuredUnit, UnitsContext};

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

fn series(area: &str, metadata: Option<NewSeriesMetadataLite>) -> FetchedSeries {
    FetchedSeries {
        metadata,
        points: Vec::new(),
        dataset: SeriesDataset::new("wdi", [("indicator", "NY.GDP.MKTP.CD"), ("area", area)]),
        validators: None,
    }
}

fn meta(units: Option<&str>) -> NewSeriesMetadataLite {
    NewSeriesMetadataLite {
        title: "t".into(),
        units: units.map(str::to_string),
        ..NewSeriesMetadataLite::default()
    }
}

type UnitsRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<serde_json::Value>,
);

async fn units_row(pool: &DatabasePool, external_id: &str) -> UnitsRow {
    use econ_graph_core::schema::economic_series::dsl;
    let mut conn = pool.get().await.unwrap();
    dsl::economic_series
        .filter(dsl::external_id.eq(external_id))
        .select((
            dsl::units,
            dsl::units_source,
            dsl::units_raw,
            dsl::units_parsed,
        ))
        .first(&mut conn)
        .await
        .unwrap()
}

#[tokio::test]
async fn persist_series_records_units_provenance() {
    let Some(admin_url) = database_url() else {
        return;
    };
    let db = FreshDb::create(&admin_url, "econgraph_units_provenance").await;
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(
            SourceId::WorldBank,
            &["wdi"],
            parse_dataset_file(WDI).unwrap(),
        )
        .unwrap();
    sync_datasets(&db.pool, &catalog).await.unwrap();
    let wb = SourceId::WorldBank;

    // Units from a structured field: stored as given, parsed.
    persist_series(
        &db.pool,
        wb,
        "a",
        &series("USA", Some(meta(Some("current US$")))),
    )
    .await
    .unwrap();
    assert_eq!(
        units_row(&db.pool, "a").await,
        (
            Some("current US$".into()),
            Some("structured".into()),
            Some("current US$".into()),
            Some(serde_json::json!({"kind": "currency", "currency": "USD", "scale": "units"})),
        )
    );

    // The adapter's currency names a bare "Dollars" in the parsed unit; the text is unchanged.
    let fred_like = NewSeriesMetadataLite {
        units_context: UnitsContext::US_DOLLARS,
        ..meta(Some("Billions of Dollars"))
    };
    persist_series(&db.pool, wb, "f", &series("GBR", Some(fred_like)))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "f").await,
        (
            Some("Billions of Dollars".into()),
            Some("structured".into()),
            Some("Billions of Dollars".into()),
            Some(serde_json::json!({"kind": "currency", "currency": "USD", "scale": "billions"})),
        )
    );

    // Unrecognised text is still stored, with no parsed unit.
    persist_series(
        &db.pool,
        wb,
        "b",
        &series("CAN", Some(meta(Some("Mt CO2e")))),
    )
    .await
    .unwrap();
    assert_eq!(
        units_row(&db.pool, "b").await,
        (
            Some("Mt CO2e".into()),
            Some("structured".into()),
            Some("Mt CO2e".into()),
            None
        )
    );

    // Reworded text with structured units that disagree with it: the text is stored as the
    // adapter gave it, units_raw keeps the source's wording, and the structured scale wins in
    // the parsed unit.
    let reworded = NewSeriesMetadataLite {
        units_raw: Some("Millions of current dollars".into()),
        structured_unit: Some(Box::new(StructuredUnit {
            scale: Some(Scale::Units),
            ..StructuredUnit::default()
        })),
        ..meta(Some("Thousands of current dollars"))
    };
    persist_series(&db.pool, wb, "c", &series("MEX", Some(reworded)))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "c").await,
        (
            Some("Thousands of current dollars".into()),
            Some("structured".into()),
            Some("Millions of current dollars".into()),
            Some(serde_json::json!({"kind": "currency", "scale": "units"})),
        )
    );

    // No units: unknown.
    persist_series(&db.pool, wb, "d", &series("FRA", Some(meta(None))))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "d").await,
        (None, Some("unknown".into()), None, None)
    );

    // Only the text the units would come from, and it gave none. Stored units are kept, with
    // their own source, text and parsed unit.
    let unparsed = || NewSeriesMetadataLite {
        units_raw: Some("Month-end, levels".into()),
        units_source: UnitsSource::ParsedFromText,
        ..meta(None)
    };
    let before = units_row(&db.pool, "a").await;
    persist_series(&db.pool, wb, "a", &series("USA", Some(unparsed())))
        .await
        .unwrap();
    assert_eq!(units_row(&db.pool, "a").await, before);
    assert_eq!(before.0.as_deref(), Some("current US$"));
    // A series with no units anywhere is unknown, and keeps the text.
    persist_series(&db.pool, wb, "d", &series("FRA", Some(unparsed())))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "d").await,
        (
            None,
            Some("unknown".into()),
            Some("Month-end, levels".into()),
            None
        )
    );

    // A fetch without metadata keeps every units column.
    persist_series(&db.pool, wb, "c", &series("MEX", None))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "c").await.2.as_deref(),
        Some("Millions of current dollars")
    );

    // Units from discovery's catalog row are structured, with no parsed unit stored.
    persist_discovered(
        &db.pool,
        wb,
        &[DiscoveredSeries {
            external_id: "e".into(),
            title: "e".into(),
            description: None,
            units: Some("persons".into()),
            frequency: None,
            data_url: None,
            dataset: series("DEU", None).dataset,
        }],
    )
    .await
    .unwrap();
    persist_series(&db.pool, wb, "e", &series("DEU", Some(meta(None))))
        .await
        .unwrap();
    persist_series(&db.pool, wb, "e", &series("DEU", None))
        .await
        .unwrap();
    assert_eq!(
        units_row(&db.pool, "e").await,
        (
            Some("persons".into()),
            Some("structured".into()),
            Some("persons".into()),
            None
        )
    );

    db.drop().await;
}
