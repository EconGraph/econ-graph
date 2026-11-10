//! `stock_price_daily` tests: the helper's series and observations, and an Iceberg round trip.
//!
//! The Iceberg test needs `DATABASE_URL` (a server it can create a scratch database on) and is
//! skipped when it is unset. It runs its body in a child process so the storage environment
//! (`TIMESERIES_BACKEND=iceberg` and the rest) never touches other tests, as the worker's Iceberg
//! test does.

use std::str::FromStr;
use std::time::Duration;

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, TimeZone, Utc};

use super::*;
use crate::dataset::DatasetCatalog;

fn d(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

/// Three share classes. GOOGL and GOOG share a CIK, which is why the series key is the FIGI.
fn symbols() -> Vec<SymbolRow> {
    [
        ("BBG001S5N8V8", "0000320193", "AAPL", "Apple Inc."),
        (
            "BBG009S39JY5",
            "0001652044",
            "GOOGL",
            "Alphabet Inc. Class A",
        ),
        (
            "BBG009S3NB21",
            "0001652044",
            "GOOG",
            "Alphabet Inc. Class C",
        ),
    ]
    .into_iter()
    .map(|(figi, cik, ticker, name)| SymbolRow {
        share_class_figi: figi.into(),
        cik10: cik.into(),
        ticker: ticker.into(),
        exchange: "XNAS".into(),
        currency: "USD".into(),
        name: name.into(),
        valid_from: d("2000-01-01"),
        valid_to: None,
    })
    .collect()
}

/// A bar whose values differ by ticker and day, with up to four decimal places and an
/// 11-digit volume. GOOG has no VWAP on 2024-01-09.
fn bar(t: usize, row: &SymbolRow, date: NaiveDate) -> DailyBar {
    let day = date.format("%d").to_string().parse::<u32>().unwrap();
    let base = 100 + 50 * t as u32 + day;
    DailyBar {
        ticker: row.ticker.clone(),
        date,
        open: dec(&format!("{base}.1000")),
        high: dec(&format!("{}.25", base + 2)),
        low: dec(&format!("{}.0001", base - 1)),
        close: dec(&format!("{base}.{day:02}")),
        volume: dec(&format!("{}", 12_345_678_901u64 + u64::from(day))),
        vwap: (row.ticker != "GOOG" || day != 9).then(|| dec(&format!("{base}.4567"))),
    }
}

#[test]
fn external_ids_round_trip() {
    for field in PriceField::ALL {
        let id = external_id("BBG001S5N8V8", field);
        assert_eq!(id, format!("BBG001S5N8V8:{}", field.as_str()));
        assert_eq!(parse_external_id(&id), Some(("BBG001S5N8V8", field)));
        assert_eq!(PriceField::parse(field.as_str()), Some(field));
    }
    for bad in [
        "BBG001S5N8V8",
        "BBG001S5N8V8:adj_close",
        ":close",
        "BBG001S5N8V8/close",
    ] {
        assert_eq!(parse_external_id(bad), None, "{bad}");
    }
}

/// The shipped definition has the contract's dimensions, and every helper series matches it.
#[test]
fn series_match_the_shipped_definition() {
    let defs = crate::reference::datasets(SourceId::Massive)
        .unwrap()
        .to_vec();
    let mut catalog = DatasetCatalog::empty();
    catalog
        .insert(SourceId::Massive, &[DATASET_CODE], defs)
        .unwrap();
    let def = catalog.get(SourceId::Massive, DATASET_CODE).unwrap();
    assert_eq!(
        def.dimension_names().collect::<Vec<_>>(),
        ["ticker", "exchange", "currency", "field"]
    );
    let description = def.description.as_deref().unwrap();
    assert!(description.contains("unadjusted"), "{description}");
    assert!(description.contains(&format!("Licence class: {LICENCE_CLASS}")));
    let codes: Vec<&str> = def.dimensions[3]
        .codes
        .as_ref()
        .unwrap()
        .iter()
        .map(|c| c.code.as_str())
        .collect();
    assert_eq!(codes, PriceField::ALL.map(PriceField::as_str));
    assert_eq!(
        def.attributes
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>(),
        ["cik10"]
    );

    let row = &symbols()[0];
    for s in discovered_series(row) {
        catalog
            .check(SourceId::Massive, &s.external_id, &s.dataset)
            .unwrap();
    }
    let bars = [bar(0, row, d("2024-01-08"))];
    for (id, fetched) in fetched_series(row, &bars, d("2024-01-08")).unwrap() {
        catalog
            .check(SourceId::Massive, &id, &fetched.dataset)
            .unwrap();
    }
}

#[test]
fn series_metadata_states_units_cik_and_licence() {
    let row = &symbols()[1];
    let close = series_metadata(row, PriceField::Close);
    assert_eq!(close.title, "Alphabet Inc. Class A (GOOGL): daily close");
    assert_eq!(close.units.as_deref(), Some("USD"));
    assert_eq!(close.frequency.as_deref(), Some(FREQUENCY));
    let description = close.description.unwrap();
    for needle in [
        "unadjusted",
        "BBG009S39JY5",
        "SEC CIK 0001652044",
        "Massive, a market data vendor",
        "Licence class: internal",
    ] {
        assert!(description.contains(needle), "{needle}: {description}");
    }
    let volume = series_metadata(row, PriceField::Volume);
    assert_eq!(volume.units.as_deref(), Some(VOLUME_UNITS));
    // The units texts are recognised and agree with the structured units.
    for field in PriceField::ALL {
        let meta = series_metadata(row, field);
        assert!(
            !matches!(
                crate::units::check_text(
                    meta.units.as_deref().unwrap(),
                    meta.structured_unit.as_deref(),
                    &meta.units_context
                ),
                crate::units::TextCheck::Unrecognised
            ),
            "{field}"
        );
    }
}

#[test]
fn fetched_series_has_one_series_per_field() {
    let rows = symbols();
    let goog = &rows[2];
    let bars = [bar(2, goog, d("2024-01-08")), bar(2, goog, d("2024-01-09"))];
    let series = fetched_series(goog, &bars, d("2024-01-10")).unwrap();
    assert_eq!(
        series.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        PriceField::ALL.map(|f| external_id("BBG009S3NB21", f))
    );
    for (id, fetched) in &series {
        let (_, field) = parse_external_id(id).unwrap();
        assert_eq!(fetched.dataset, series_dataset(goog, field));
        assert_eq!(fetched.dataset.dimensions.0["field"], field.as_str());
        assert_eq!(fetched.dataset.dimensions.0["ticker"], "GOOG");
        // No VWAP on the 9th.
        let expected = if field == PriceField::Vwap { 1 } else { 2 };
        assert_eq!(fetched.points.len(), expected, "{id}");
        for (p, b) in fetched.points.iter().zip(&bars) {
            assert_eq!(p.date, b.date);
            assert_eq!(p.value.as_ref(), b.value(field));
            assert_eq!(p.revision_date, d("2024-01-10"));
            assert_eq!(
                p.attributes,
                Some(serde_json::json!({"cik10": "0001652044"}))
            );
        }
    }
}

#[test]
fn bars_must_be_the_rows_ticker_and_distinct_days() {
    let rows = symbols();
    let other = bar(0, &rows[0], d("2024-01-08"));
    let err = fetched_series(&rows[1], &[other], d("2024-01-08")).unwrap_err();
    assert!(err.to_string().contains("AAPL"), "{err}");
    let twice = [
        bar(1, &rows[1], d("2024-01-08")),
        bar(1, &rows[1], d("2024-01-08")),
    ];
    let err = observations(&rows[1], &twice, d("2024-01-08"), true, Utc::now()).unwrap_err();
    assert!(err.to_string().contains("two bars"), "{err}");
}

#[test]
fn observations_have_stable_ids() {
    let row = &symbols()[0];
    let bars = [bar(0, row, d("2024-01-08")), bar(0, row, d("2024-01-09"))];
    let at = Utc.with_ymd_and_hms(2024, 1, 9, 22, 0, 0).unwrap()
        + chrono::Duration::nanoseconds(123_456_789);
    let first = observations(row, &bars, d("2024-01-09"), true, at).unwrap();
    let again = observations(row, &bars, d("2024-01-09"), true, at).unwrap();
    assert_eq!(first, again);
    assert_eq!(first.len(), 12);
    let ids: BTreeSet<Uuid> = first.iter().map(|o| o.id).collect();
    assert_eq!(ids.len(), 12, "ids are distinct");
    for o in &first {
        assert_eq!(
            o.created_at.timestamp_subsec_nanos() % 1000,
            0,
            "microseconds"
        );
        assert_eq!(
            o.id,
            observation_id(o.series_id, o.date, o.revision_date, o.is_original_release)
        );
        let field = PriceField::ALL
            .into_iter()
            .find(|f| series_id("BBG001S5N8V8", *f) == o.series_id)
            .unwrap();
        assert_eq!(
            o.series_id,
            stable_series_id(SourceId::Massive, &external_id("BBG001S5N8V8", field))
        );
    }
    // A later vintage or a revision is a different observation.
    let s = series_id("BBG001S5N8V8", PriceField::Close);
    let base = observation_id(s, d("2024-01-08"), d("2024-01-08"), true);
    assert_ne!(
        base,
        observation_id(s, d("2024-01-08"), d("2024-01-09"), true)
    );
    assert_ne!(
        base,
        observation_id(s, d("2024-01-08"), d("2024-01-08"), false)
    );
}

/// The namespace is derived from a fixed URL. If this fails, every backfilled id changes.
#[test]
fn observation_namespace_is_pinned() {
    assert_eq!(
        OBSERVATION_ID_NAMESPACE,
        Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            b"https://econgraph.com/ns/massive/observation"
        )
    );
}

const PROOF_ENV: &str = "MASSIVE_ICEBERG_PROOF";

/// One week of bars for three share classes, published through series persistence with the
/// Iceberg backend, read back exactly; a vendor correction keeps both revisions; and the
/// maintenance plan and a budgeted compaction pass for the dataset.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stock_price_daily_round_trips_through_iceberg() {
    use diesel::connection::SimpleConnection;
    use econ_graph_core::test_utils::ScratchDatabase;
    if std::env::var(PROOF_ENV).is_ok() {
        iceberg_proof().await;
        return;
    }
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set; skipping the stock_price_daily Iceberg test");
        return;
    }
    let db = ScratchDatabase::create("massive_iceberg");
    econ_graph_core::run_migrations(&db.url).await.unwrap();
    let warehouse = tempfile::tempdir().unwrap();
    let target = Uuid::new_v4();
    // Only this disposable fixture sets the control row directly (see the worker's Iceberg test).
    db.connect()
        .batch_execute(&format!(
            "INSERT INTO timeseries_recovery_state(target_id,target_label) \
             VALUES ('{target}','disposable massive proof'); \
             UPDATE timeseries_recovery_state SET backend='iceberg', epoch=1"
        ))
        .unwrap();
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "sources::massive::dataset::tests::stock_price_daily_round_trips_through_iceberg",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PROOF_ENV, "1")
        .env("DATABASE_URL", &db.url)
        .env("DATA_DATABASE_URL", &db.url)
        .env("TIMESERIES_BACKEND", "iceberg")
        .env("TIMESERIES_TARGET_ID", target.to_string())
        .env("TIMESERIES_ICEBERG_MIRROR", "true")
        .env("ICEBERG_WAREHOUSE", warehouse.path())
        .kill_on_drop(true);
    let status = tokio::time::timeout(Duration::from_secs(300), command.status())
        .await
        .expect("bounded stock_price_daily proof timed out")
        .expect("spawn stock_price_daily proof");
    assert!(status.success(), "stock_price_daily Iceberg proof failed");
}

/// What a read of one series should return: `(date, revision_date, value, original)`.
type Expected = Vec<(NaiveDate, NaiveDate, String, bool)>;

async fn read(
    store: &econ_graph_timeseries::IcebergStore,
    series: Uuid,
    revisions: econ_graph_core::models::RevisionMode,
) -> Expected {
    use econ_graph_core::models::{ObservationRequest, TimeSeriesStore};
    let page = store
        .read_page(ObservationRequest {
            series_id: series,
            start_date: None,
            end_date: None,
            revisions,
            limit: None,
            offset: None,
            context: None,
        })
        .await
        .unwrap();
    page_rows(&page.points)
}

fn page_rows(points: &[Observation]) -> Expected {
    points
        .iter()
        .map(|o| {
            assert!(o.attributes.as_ref().unwrap()["cik10"].is_string());
            (
                o.date,
                o.revision_date,
                o.value.as_ref().unwrap().to_string(),
                o.is_original_release,
            )
        })
        .collect()
}

async fn publish(pool: &DatabasePool, row: &SymbolRow, bars: &[DailyBar], revision: NaiveDate) {
    for (id, fetched) in fetched_series(row, bars, revision).unwrap() {
        crate::persist::persist_series(pool, SourceId::Massive, &id, &fetched)
            .await
            .unwrap();
    }
}

async fn iceberg_proof() {
    use econ_graph_core::models::RevisionMode;
    use econ_graph_timeseries::{IcebergStore, SqlCatalog, StorageConfig};

    let pool = econ_graph_core::create_pool(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut datasets = DatasetCatalog::empty();
    datasets
        .insert(
            SourceId::Massive,
            &[DATASET_CODE],
            crate::reference::datasets(SourceId::Massive)
                .unwrap()
                .to_vec(),
        )
        .unwrap();
    crate::persist::sync_datasets(&pool, &datasets)
        .await
        .unwrap();

    // One trading week, published day by day as the scheduled crawl would.
    let rows = symbols();
    let week: Vec<NaiveDate> = (8..=12)
        .map(|day| NaiveDate::from_ymd_opt(2024, 1, day).unwrap())
        .collect();
    for &day in &week {
        for (t, row) in rows.iter().enumerate() {
            publish(&pool, row, &[bar(t, row, day)], day).await;
        }
    }

    let catalog = SqlCatalog::new(pool.clone(), StorageConfig::from_env().unwrap());
    let store = IcebergStore::new(catalog.clone());
    let dataset_id = {
        let mut conn = pool.get().await.unwrap();
        dataset_id_conn(&mut conn).await.unwrap()
    };
    let expected = |t: usize, row: &SymbolRow, field: PriceField| -> Expected {
        week.iter()
            .filter_map(|&day| {
                let value = bar(t, row, day).value(field)?.to_string();
                Some((day, day, value, true))
            })
            .collect()
    };
    for (t, row) in rows.iter().enumerate() {
        for field in PriceField::ALL {
            let id = series_id(&row.share_class_figi, field);
            let want = expected(t, row, field);
            assert_eq!(
                want.len(),
                if (t, field) == (2, PriceField::Vwap) {
                    4
                } else {
                    5
                }
            );
            for mode in [
                RevisionMode::All,
                RevisionMode::Latest,
                RevisionMode::Original,
            ] {
                assert_eq!(
                    read(&store, id, mode).await,
                    want,
                    "{} {field} {mode:?}",
                    row.ticker
                );
            }
        }
    }
    // The CIK is on every observation, and GOOG and GOOGL share it.
    let page = catalog
        .read_series_at(
            &catalog.pointer(dataset_id).await.unwrap().unwrap(),
            &[series_id("BBG009S3NB21", PriceField::Close)],
            None,
            None,
            100,
        )
        .await
        .unwrap();
    assert!(page
        .iter()
        .all(|o| o.attributes == Some(serde_json::json!({"cik10": "0001652044"}))));

    // Massive corrects AAPL's close for Wednesday; the re-fetch on Friday sees it.
    let before = catalog.pointer(dataset_id).await.unwrap().unwrap();
    let wednesday = d("2024-01-10");
    let friday = d("2024-01-12");
    let mut corrected = bar(0, &rows[0], wednesday);
    let original_close = corrected.close.to_string();
    corrected.close = dec("160.1099");
    publish(&pool, &rows[0], &[corrected], friday).await;

    let close = series_id("BBG001S5N8V8", PriceField::Close);
    let both: Expected = read(&store, close, RevisionMode::All)
        .await
        .into_iter()
        .filter(|r| r.0 == wednesday)
        .collect();
    assert_eq!(
        both,
        [
            (wednesday, wednesday, original_close.clone(), true),
            (wednesday, friday, "160.1099".to_string(), false),
        ],
        "both revisions are kept"
    );
    let on = |rows: Expected| rows.into_iter().find(|r| r.0 == wednesday).unwrap().2;
    assert_eq!(
        on(read(&store, close, RevisionMode::Latest).await),
        "160.1099"
    );
    assert_eq!(
        on(read(&store, close, RevisionMode::AsOf(d("2024-01-11"))).await),
        original_close
    );
    assert_eq!(
        on(read(&store, close, RevisionMode::AsOf(friday)).await),
        "160.1099"
    );
    assert_eq!(
        on(read(&store, close, RevisionMode::Original).await),
        original_close
    );
    // The snapshot from before the correction still reads the original.
    let pinned = store
        .read_page_at(
            &before,
            econ_graph_core::models::ObservationRequest {
                series_id: close,
                start_date: Some(wednesday),
                end_date: Some(wednesday),
                revisions: RevisionMode::All,
                limit: None,
                offset: None,
                context: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        page_rows(&pinned.points),
        [(wednesday, wednesday, original_close.clone(), true)]
    );
    // The unchanged fields of the re-fetched bar are not stored again.
    for field in PriceField::ALL
        .into_iter()
        .filter(|f| *f != PriceField::Close)
    {
        let id = series_id("BBG001S5N8V8", field);
        assert_eq!(
            read(&store, id, RevisionMode::All).await,
            expected(0, &rows[0], field),
            "{field}"
        );
    }

    // 3 share classes x 6 fields x 5 days, less GOOG's missing VWAP, plus the correction.
    let logical = 3 * 6 * 5 - 1 + 1;
    // The maintenance dry run's plan, which `timeseries maintenance --max-rows` reports.
    let plan = catalog.maintenance_plan(dataset_id).await.unwrap();
    assert_eq!(plan.logical_rows, logical);
    assert_eq!(plan.series, 18);
    assert!(
        plan.physical_rows > plan.logical_rows && plan.compact_recommended,
        "each publication re-mirrors its series: {plan:?}"
    );
    // A budget below the dataset is refused; one that covers it compacts, keeping every row.
    let err = catalog
        .prepare_compaction(Uuid::new_v4(), dataset_id, logical - 1)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("row budget"), "{err}");
    let prepared = catalog
        .prepare_compaction(Uuid::new_v4(), dataset_id, logical)
        .await
        .unwrap();
    assert_eq!(prepared.rows as u64, logical);
    let before_compaction = read(&store, close, RevisionMode::All).await;
    {
        use diesel_async::AsyncConnection;
        let mut conn = pool.get().await.unwrap();
        let selection =
            econ_graph_core::observation_storage::ObservationStorage::from_env().unwrap();
        let committed = conn
            .transaction::<_, anyhow::Error, _>(async |conn| {
                selection.assert_write_allowed_conn(conn).await?;
                SqlCatalog::commit_prepared_conn(conn, &prepared).await
            })
            .await
            .unwrap();
        assert!(committed);
    }
    let plan = catalog.maintenance_plan(dataset_id).await.unwrap();
    assert_eq!((plan.physical_rows, plan.logical_rows), (logical, logical));
    assert_eq!(
        read(&store, close, RevisionMode::All).await,
        before_compaction
    );
}
