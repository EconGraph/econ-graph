//! DB-backed backfill tests on the Iceberg backend. Each runs its phase in a child process, so
//! the storage environment (`TIMESERIES_BACKEND=iceberg`, the warehouse) is the child's own and
//! never leaks into other crawler tests. Skipped when `DATABASE_URL` is unset.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel::sql_types::{BigInt, Nullable, Uuid as SqlUuid};
use diesel::QueryableByName;
use diesel_async::RunQueryDsl;
use econ_graph_core::DatabasePool;
use econ_graph_timeseries::{SqlCatalog, StorageConfig};
use uuid::Uuid;

use super::estimate::{StorageModel, MEASURED};
use super::*;
use crate::dataset::DatasetCatalog;
use crate::persist;

const PHASE_VAR: &str = "MASSIVE_BACKFILL_TEST_PHASE";
const TICKERS: [&str; 10] = [
    "AAA", "BBB", "CCC", "DDD", "EEE", "FFF", "GGG", "HHH", "III", "JJJ",
];
/// The window under test: one year of trading days.
const END: &str = "2025-12-31";

fn d(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

fn figi(ticker: &str) -> String {
    format!("BBG00TEST{ticker}")
}

/// Runs `phase` in a child process against its own scratch database and warehouse, with the
/// observation backend set to Iceberg.
async fn run_phase(test: &str, phase: &str) {
    use diesel::connection::SimpleConnection;
    use econ_graph_core::test_utils::ScratchDatabase;
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("DATABASE_URL not set; skipping DB-backed Massive backfill test");
        return;
    }
    let db = ScratchDatabase::create("massive_backfill");
    econ_graph_core::run_migrations(&db.url).await.unwrap();
    let warehouse = tempfile::tempdir().unwrap();
    let target = Uuid::new_v4();
    db.connect()
        .batch_execute(&format!(
            "INSERT INTO timeseries_recovery_state(target_id,target_label,backend,epoch) \
             VALUES ('{target}','disposable Massive backfill test','iceberg',1)"
        ))
        .unwrap();
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("sources::massive::backfill::tests::{test}"),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(PHASE_VAR, phase)
        .env("DATABASE_URL", &db.url)
        .env("DATA_DATABASE_URL", &db.url)
        .env("TIMESERIES_BACKEND", "iceberg")
        .env("TIMESERIES_TARGET_ID", target.to_string())
        .env("ICEBERG_WAREHOUSE", warehouse.path())
        .kill_on_drop(true);
    let status = tokio::time::timeout(Duration::from_secs(600), command.status())
        .await
        .expect("backfill test phase timed out")
        .expect("spawn backfill test phase");
    assert!(status.success(), "backfill test phase {phase} failed");
}

/// Deterministic fake bars: a random walk per ticker, one bar per trading day. Optionally fails
/// every request after the first `fail_after`.
struct FakeBars {
    calls: AtomicUsize,
    fail_after: Option<usize>,
}

impl FakeBars {
    fn new(fail_after: Option<usize>) -> Self {
        FakeBars {
            calls: AtomicUsize::new(0),
            fail_after,
        }
    }

    fn calls(&self) -> usize {
        AtomicUsize::load(&self.calls, Ordering::SeqCst)
    }

    fn call(&self) -> Result<(), ClientError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        match self.fail_after {
            Some(limit) if n > limit => Err(ClientError::Parse("injected crash".into())),
            _ => Ok(()),
        }
    }

    fn bar(ticker: &str, date: NaiveDate) -> DailyBar {
        // A cheap deterministic hash of ticker and date drives the walk.
        let seed = ticker
            .bytes()
            .fold(u64::from(date.num_days_from_ce().unsigned_abs()), |h, b| {
                h.wrapping_mul(1_099_511_628_211).wrapping_add(u64::from(b))
            });
        let cents = 10_000 + i64::try_from(seed % 5_000).unwrap();
        let price = |offset: i64| BigDecimal::new((cents + offset).into(), 2);
        DailyBar {
            ticker: ticker.to_string(),
            date,
            open: price(0),
            high: price(150),
            low: price(-120),
            close: price(37),
            volume: BigDecimal::from(1_000_000 + (seed % 900_000)),
            vwap: Some(BigDecimal::new((cents * 100 + 1_234).into(), 4)),
        }
    }
}

#[async_trait]
impl DailyBars for FakeBars {
    async fn ticker_range(
        &self,
        ticker: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DailyBar>, ClientError> {
        self.call()?;
        Ok(calendar::trading_days(from, to)
            .into_iter()
            .map(|day| Self::bar(ticker, day))
            .collect())
    }

    async fn grouped(&self, date: NaiveDate) -> Result<Vec<DailyBar>, ClientError> {
        self.call()?;
        if !calendar::is_trading_day(date) {
            return Ok(Vec::new());
        }
        // Grouped daily lists every ticker the vendor has, not only the symbol table's.
        let mut bars: Vec<_> = TICKERS.iter().map(|t| Self::bar(t, date)).collect();
        bars.push(Self::bar("NOTINDEX", date));
        Ok(bars)
    }
}

fn symbols(dir: &Path) -> SymbolTable {
    let path = dir.join("symbols.csv");
    let mut csv =
        String::from("share_class_figi,cik10,ticker,exchange,currency,name,valid_from,valid_to\n");
    for (i, t) in TICKERS.iter().enumerate() {
        csv.push_str(&format!(
            "{},{:010},{t},XNYS,USD,{t} Inc,2000-01-03,\n",
            figi(t),
            i + 1
        ));
    }
    std::fs::write(&path, csv).unwrap();
    SymbolTable::load(&path).unwrap()
}

fn options(batch_rows: Option<usize>, mode: Option<FetchMode>) -> BackfillOptions {
    BackfillOptions {
        years: 1,
        end: Some(d(END)),
        mode,
        batch_rows,
        today: d("2026-01-05"),
    }
}

async fn setup() -> (DatabasePool, SqlCatalog) {
    let pool = econ_graph_core::create_pool(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut datasets = DatasetCatalog::empty();
    datasets
        .insert(
            SourceId::Massive,
            &[dataset::DATASET_CODE],
            crate::reference::datasets(SourceId::Massive)
                .unwrap()
                .to_vec(),
        )
        .unwrap();
    persist::sync_datasets(&pool, &datasets).await.unwrap();
    let catalog = SqlCatalog::new(pool.clone(), StorageConfig::from_env().unwrap());
    (pool, catalog)
}

async fn count(pool: &DatabasePool, sql: &str) -> i64 {
    #[derive(QueryableByName)]
    struct N {
        #[diesel(sql_type = BigInt)]
        n: i64,
    }
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(sql)
        .get_result::<N>(&mut conn)
        .await
        .unwrap()
        .n
}

async fn generation(pool: &DatabasePool) -> Option<i64> {
    #[derive(QueryableByName)]
    struct G {
        #[diesel(sql_type = Nullable<BigInt>)]
        g: Option<i64>,
    }
    let mut conn = pool.get().await.unwrap();
    let dataset = dataset::dataset_id_conn(&mut conn).await.unwrap();
    diesel::sql_query("SELECT MAX(generation) AS g FROM iceberg_catalog WHERE dataset_id = $1")
        .bind::<SqlUuid, _>(dataset)
        .get_result::<G>(&mut conn)
        .await
        .unwrap()
        .g
}

fn failures() -> u64 {
    CRAWLER_SOURCE_HEALTH_METRICS
        .backfill_runs_total
        .with_label_values(&["MASSIVE", "failure"])
        .get()
}

/// Logical rows per series, and physical rows, from the table's current snapshot.
async fn stored(pool: &DatabasePool, catalog: &SqlCatalog) -> (BTreeMap<Uuid, usize>, u64) {
    let mut conn = pool.get().await.unwrap();
    let dataset = dataset::dataset_id_conn(&mut conn).await.unwrap();
    drop(conn);
    let pointer = catalog.pointer(dataset).await.unwrap().unwrap();
    let rows = catalog.read_rows(&pointer).await.unwrap();
    let mut keys = HashSet::new();
    let mut per_series = BTreeMap::new();
    for row in &rows {
        assert!(
            keys.insert((row.series_id, row.date, row.revision_date)),
            "duplicate observation {} {}",
            row.series_id,
            row.date
        );
        *per_series.entry(row.series_id).or_default() += 1;
    }
    let plan = catalog.maintenance_plan(dataset).await.unwrap();
    assert_eq!(plan.logical_rows as usize, rows.len());
    (per_series, plan.physical_rows)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_year_for_ten_tickers_survives_a_crash_and_a_rerun() {
    match std::env::var(PHASE_VAR).as_deref() {
        Ok("crash_and_rerun") => crash_and_rerun().await,
        Ok(other) => panic!("unknown phase {other}"),
        Err(_) => {
            run_phase(
                "one_year_for_ten_tickers_survives_a_crash_and_a_rerun",
                "crash_and_rerun",
            )
            .await;
        }
    }
}

async fn crash_and_rerun() {
    let (pool, catalog) = setup().await;
    let dir = tempfile::tempdir().unwrap();
    let symbols = symbols(dir.path());
    let start = window_start(d(END), 1);
    let days = calendar::trading_days(start, d(END)).len();
    let per_ticker = FIELDS * days;
    // Three tickers per commit.
    let batch = Some(3 * per_ticker);

    // 1. Crash on the sixth request (after the plan probe and four tickers): one commit of three
    //    tickers is durable, the fourth ticker's rows were never published.
    let before = failures();
    let crashing = FakeBars::new(Some(5));
    let err = run(&pool, &catalog, &crashing, &symbols, &options(batch, None))
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("injected crash"), "{err:#}");
    assert_eq!(failures(), before + 1);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) AS n FROM massive_backfill_units").await,
        3
    );
    let (per_series, physical) = stored(&pool, &catalog).await;
    assert_eq!(per_series.len(), 3 * FIELDS);
    assert_eq!(physical as usize, 3 * per_ticker);
    let generation_after_crash = generation(&pool).await;

    // 2. Crash inside the commit transaction, after the Iceberg pointer moved: the unit insert
    //    fails, so the pointer and the unit rows roll back together.
    let mut conn = pool.get().await.unwrap();
    diesel::sql_query(
        "CREATE FUNCTION t_reject_unit() RETURNS trigger LANGUAGE plpgsql AS \
         $$ BEGIN RAISE EXCEPTION 'injected commit crash'; END $$",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    diesel::sql_query(
        "CREATE TRIGGER t_reject_unit BEFORE INSERT ON massive_backfill_units \
         FOR EACH ROW EXECUTE FUNCTION t_reject_unit()",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    let err = run(
        &pool,
        &catalog,
        &FakeBars::new(None),
        &symbols,
        &options(batch, None),
    )
    .await
    .unwrap_err();
    assert!(
        format!("{err:#}").contains("injected commit crash"),
        "{err:#}"
    );
    assert_eq!(generation(&pool).await, generation_after_crash);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) AS n FROM massive_backfill_units").await,
        3
    );
    diesel::sql_query("DROP TRIGGER t_reject_unit ON massive_backfill_units")
        .execute(&mut conn)
        .await
        .unwrap();
    drop(conn);

    // 3. The rerun resumes the open run, skips the three loaded tickers and loads the rest.
    let client = FakeBars::new(None);
    let report = run(&pool, &catalog, &client, &symbols, &options(batch, None))
        .await
        .unwrap();
    assert!(report.resumed);
    assert_eq!(report.window.mode, FetchMode::TickerRange);
    assert_eq!(
        (report.units_total, report.units_skipped, report.units_done),
        (10, 3, 7)
    );
    // No plan probe on resume: one request per remaining ticker.
    assert_eq!(client.calls(), 7);
    assert_eq!(report.rows_written, 7 * per_ticker);
    assert_eq!(report.commits, 3);
    assert_eq!(
        CRAWLER_SOURCE_HEALTH_METRICS
            .backfill_days_remaining
            .with_label_values(&["MASSIVE"])
            .get(),
        0
    );

    // Every series has exactly one row per trading day, and nothing was written twice.
    let (per_series, physical) = stored(&pool, &catalog).await;
    assert_eq!(per_series.len(), TICKERS.len() * FIELDS);
    for t in TICKERS {
        for field in PriceField::ALL {
            assert_eq!(
                per_series.get(&dataset::series_id(&figi(t), field)),
                Some(&days),
                "{t} {field:?}"
            );
        }
    }
    assert_eq!(physical as usize, TICKERS.len() * per_ticker);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) AS n FROM massive_backfill_runs WHERE completed_at IS NOT NULL"
        )
        .await,
        1
    );
    // Every unit's commit is a published Iceberg commit (the purge's starting point).
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) AS n FROM massive_backfill_units u \
             JOIN iceberg_commits c ON c.commit_id = u.commit_id"
        )
        .await,
        10
    );

    // 4. Running again loads nothing: the finished run covers the window.
    let client = FakeBars::new(None);
    let report = run(&pool, &catalog, &client, &symbols, &options(batch, None))
        .await
        .unwrap();
    assert_eq!((report.units_total, report.rows_written), (0, 0));
    assert_eq!(client.calls(), 0);
    assert_eq!(
        stored(&pool, &catalog).await.1 as usize,
        TICKERS.len() * per_ticker
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grouped_daily_loads_only_listed_tickers_in_bounded_commits() {
    match std::env::var(PHASE_VAR).as_deref() {
        Ok("grouped") => grouped().await,
        Ok(other) => panic!("unknown phase {other}"),
        Err(_) => {
            run_phase(
                "grouped_daily_loads_only_listed_tickers_in_bounded_commits",
                "grouped",
            )
            .await;
        }
    }
}

async fn grouped() {
    let (pool, catalog) = setup().await;
    let dir = tempfile::tempdir().unwrap();
    let symbols = symbols(dir.path());
    let start = window_start(d(END), 1);
    let days = calendar::trading_days(start, d(END)).len();
    let per_day = TICKERS.len() * FIELDS;
    let batch = 21 * per_day;
    let client = FakeBars::new(None);
    let report = run(
        &pool,
        &catalog,
        &client,
        &symbols,
        &options(Some(batch), Some(FetchMode::GroupedDaily)),
    )
    .await
    .unwrap();
    assert_eq!(report.units_total, days);
    assert_eq!(report.commits, days.div_ceil(21));
    // The plan probe, then one request per trading day.
    assert_eq!(client.calls(), days + 1);
    let (per_series, physical) = stored(&pool, &catalog).await;
    assert_eq!(per_series.len(), per_day);
    assert!(per_series.values().all(|&n| n == days));
    assert_eq!(physical as usize, days * per_day);
}

/// Bytes of the warehouse's files, by kind.
#[derive(Debug, Default)]
struct WarehouseBytes {
    parquet: u64,
    parquet_files: u64,
    avro: u64,
    metadata_json: Vec<u64>,
}

fn warehouse_bytes(dir: &Path, out: &mut WarehouseBytes) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let len = entry.metadata().unwrap().len();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() {
            warehouse_bytes(&path, out);
        } else if name.ends_with(".parquet") {
            out.parquet += len;
            out.parquet_files += 1;
        } else if name.ends_with(".avro") {
            out.avro += len;
        } else if name.ends_with(".metadata.json") {
            out.metadata_json.push(len);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn measured_storage_model_matches_the_writer() {
    match std::env::var(PHASE_VAR).as_deref() {
        Ok(phase @ ("measure_ticker" | "measure_grouped")) => measure(phase).await,
        Ok(other) => panic!("unknown phase {other}"),
        Err(_) => {
            // Each mode writes its own warehouse; the phases print their byte counts and the
            // model is fitted from both.
            run_phase(
                "measured_storage_model_matches_the_writer",
                "measure_ticker",
            )
            .await;
            run_phase(
                "measured_storage_model_matches_the_writer",
                "measure_grouped",
            )
            .await;
        }
    }
}

/// Writes one year for ten tickers through the real writer in one mode, then checks the
/// [`MEASURED`] model predicts what landed in the warehouse.
async fn measure(phase: &str) {
    let (pool, catalog) = setup().await;
    let dir = tempfile::tempdir().unwrap();
    let symbols = symbols(dir.path());
    let start = window_start(d(END), 1);
    let days = calendar::trading_days(start, d(END)).len();
    let (mode, batch) = if phase == "measure_ticker" {
        (FetchMode::TickerRange, 2 * FIELDS * days)
    } else {
        (FetchMode::GroupedDaily, 21 * TICKERS.len() * FIELDS)
    };
    let report = run(
        &pool,
        &catalog,
        &FakeBars::new(None),
        &symbols,
        &options(Some(batch), Some(mode)),
    )
    .await
    .unwrap();
    let mut bytes = WarehouseBytes::default();
    warehouse_bytes(
        Path::new(&std::env::var("ICEBERG_WAREHOUSE").unwrap()),
        &mut bytes,
    );
    bytes.metadata_json.sort_unstable();
    let rows = report.rows_written as f64;
    let commits = report.commits as f64;
    let files = bytes.parquet_files as f64;
    let metadata: u64 = bytes.metadata_json.iter().sum();
    eprintln!(
        "storage measurement {phase}: rows={rows} commits={commits} files={files} \
         parquet={} avro={} metadata_json={:?}",
        bytes.parquet, bytes.avro, bytes.metadata_json
    );
    // Predict each kind of bytes from the model and compare.
    let m: StorageModel = MEASURED;
    let parquet = rows * m.data_bytes_per_row + files * m.data_file_overhead_bytes;
    let avro = commits * m.commit_overhead_bytes + files * m.manifest_bytes_per_file;
    // The empty table's metadata file, then one per commit with a growing snapshot list.
    let json = m.metadata_base_bytes
        + commits * m.metadata_base_bytes
        + commits * (commits + 1.0) / 2.0 * m.metadata_bytes_per_snapshot;
    let within = |what: &str, predicted: f64, actual: u64| {
        let actual = actual as f64;
        let ratio = predicted / actual;
        assert!(
            (0.75..=1.33).contains(&ratio),
            "{phase} {what}: model predicts {predicted:.0} bytes, the writer stored {actual:.0} \
             (ratio {ratio:.2}); re-measure MEASURED from the line above"
        );
    };
    within("parquet", parquet, bytes.parquet);
    within("manifests", avro, bytes.avro);
    within("metadata files", json, metadata);
}
