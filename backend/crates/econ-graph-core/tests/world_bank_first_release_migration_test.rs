// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Migration 2026-11-01-000100_world_bank_first_release on a populated database: afterwards
//! every World Bank (series, date) has one original release, its earliest real vintage;
//! synthetic legacy rows, other sources, the row count and every value are unchanged, and
//! `originalOnly` reads return the first release while `asOf` and latest reads are unchanged.
//! The checks that guard it (a revision twin, a missing World Bank source) fail it with
//! nothing changed.

use chrono::NaiveDate;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Bool, Date, Text};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use econ_graph_core::models::{exclude_synthetic_legacy_rows, revision_filter};
use econ_graph_core::schema::data_points;
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/data");

const FIRST_RELEASE: &str = "20261101000100";

const WORLD_BANK: &str = "World Bank Open Data";
const FRED: &str = "Federal Reserve Economic Data (FRED)";

async fn start_postgres() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "first_release")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@localhost:{port}/first_release");
    (container, url)
}

fn connect(url: &str) -> PgConnection {
    // The server may restart once after initdb; retry briefly.
    for _ in 0..30 {
        if let Ok(conn) = PgConnection::establish(url) {
            return conn;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    PgConnection::establish(url).expect("connect to postgres")
}

/// Runs pending migrations up to, but not including, `version`.
fn migrate_to_before(conn: &mut PgConnection, version: &str) {
    loop {
        let pending = conn.pending_migrations(MIGRATIONS).expect("pending");
        let Some(next) = pending.first() else {
            panic!("migration {version} is not pending");
        };
        if next.name().version().to_string() == version {
            return;
        }
        conn.run_migration(next.as_ref()).expect("run migration");
    }
}

/// Runs `version`, the next pending migration.
fn run_first_release(conn: &mut PgConnection) -> diesel::migration::Result<()> {
    let pending = conn.pending_migrations(MIGRATIONS).expect("pending");
    let next = pending.first().expect("a pending migration");
    assert_eq!(next.name().version().to_string(), FIRST_RELEASE);
    conn.run_migration(next.as_ref()).map(|_| ())
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A series of the seeded source `source`, with a dataset of its own.
fn create_series(conn: &mut PgConnection, source: &str, external_id: &str) {
    diesel::sql_query(
        "WITH src AS (SELECT id FROM data_sources WHERE name = $1), \
              ds AS (INSERT INTO datasets (source_id, code, name) \
                     SELECT id, $2, $2 FROM src RETURNING id, source_id) \
         INSERT INTO economic_series (source_id, external_id, title, frequency, dataset_id) \
         SELECT source_id, $2, $2, 'Annual', id FROM ds",
    )
    .bind::<Text, _>(source)
    .bind::<Text, _>(external_id)
    .execute(conn)
    .expect("create series");
}

/// Stores one row of `series`: `(date, revision_date, is_original_release, value)`.
fn point(conn: &mut PgConnection, series: &str, row: (&str, &str, bool, &str)) {
    let (date, revision_date, original, value) = row;
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, revision_date, is_original_release, value) \
         SELECT id, $2, $3, $4, $5::numeric FROM economic_series WHERE external_id = $1",
    )
    .bind::<Text, _>(series)
    .bind::<Date, _>(d(date))
    .bind::<Date, _>(d(revision_date))
    .bind::<Bool, _>(original)
    .bind::<Text, _>(value)
    .execute(conn)
    .expect("store point");
}

const WB_SERIES: &str = "wdi/TEST.USA";
const FRED_SERIES: &str = "TESTFRED";

/// World Bank rows as legacy crawls and compare-and-skip left them, and a FRED series with two
/// original releases of one date (which is not this migration's business).
fn populate(conn: &mut PgConnection) {
    create_series(conn, WORLD_BANK, WB_SERIES);
    create_series(conn, FRED, FRED_SERIES);
    for row in [
        // Three legacy releases of 2000, and a synthetic legacy row (revision_date = date).
        ("2000-01-01", "2020-07-01", true, "1.0"),
        ("2000-01-01", "2021-07-01", true, "1.1"),
        ("2000-01-01", "2022-07-01", true, "1.2"),
        ("2000-01-01", "2000-01-01", true, "0.9"),
        // 2001: one original and a compare-and-skip revision.
        ("2001-01-01", "2020-07-01", true, "2.0"),
        ("2001-01-01", "2023-07-01", false, "2.1"),
        // 2002: first published in 2021, the same value released again in 2022.
        ("2002-01-01", "2021-07-01", true, "3.0"),
        ("2002-01-01", "2022-07-01", true, "3.0"),
    ] {
        point(conn, WB_SERIES, row);
    }
    point(conn, FRED_SERIES, ("2000-01-01", "2020-07-01", true, "5.0"));
    point(conn, FRED_SERIES, ("2000-01-01", "2021-07-01", true, "5.1"));
}

#[derive(QueryableByName, Debug, PartialEq)]
struct Row {
    #[diesel(sql_type = Date)]
    date: NaiveDate,
    #[diesel(sql_type = Date)]
    revision_date: NaiveDate,
    #[diesel(sql_type = Bool)]
    is_original_release: bool,
    #[diesel(sql_type = Text)]
    value: String,
}

/// Every row of `series`, in key order.
fn rows(conn: &mut PgConnection, series: &str) -> Vec<Row> {
    diesel::sql_query(
        "SELECT dp.date, dp.revision_date, dp.is_original_release, dp.value::text AS value \
         FROM data_points dp JOIN economic_series es ON es.id = dp.series_id \
         WHERE es.external_id = $1 ORDER BY dp.date, dp.revision_date, dp.is_original_release",
    )
    .bind::<Text, _>(series)
    .load(conn)
    .expect("rows")
}

fn row_count(conn: &mut PgConnection) -> i64 {
    #[derive(QueryableByName)]
    struct N {
        #[diesel(sql_type = BigInt)]
        n: i64,
    }
    diesel::sql_query("SELECT COUNT(*) AS n FROM data_points")
        .get_result::<N>(conn)
        .expect("count")
        .n
}

/// `(date, revision_date, value)` the series service's read returns for these options: the
/// same `exclude_synthetic_legacy_rows` and `revision_filter` filters it applies.
fn read(
    conn: &mut PgConnection,
    original_only: bool,
    as_of: Option<NaiveDate>,
    latest: bool,
) -> Vec<(NaiveDate, NaiveDate, String)> {
    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: uuid::Uuid,
    }
    let series = diesel::sql_query("SELECT id FROM economic_series WHERE external_id = $1")
        .bind::<Text, _>(WB_SERIES)
        .get_result::<Id>(conn)
        .expect("series id")
        .id;
    let mut query = data_points::table
        .filter(data_points::series_id.eq(series))
        .into_boxed();
    let as_of_or_latest = as_of.is_some() || latest;
    if original_only || as_of_or_latest {
        query = query.filter(exclude_synthetic_legacy_rows());
    }
    if original_only {
        query = query.filter(data_points::is_original_release.eq(true));
    }
    if as_of_or_latest {
        query = query.filter(revision_filter(as_of, original_only));
    }
    query
        .order_by((data_points::date, data_points::revision_date))
        .select((
            data_points::date,
            data_points::revision_date,
            data_points::value,
        ))
        .load::<(NaiveDate, NaiveDate, Option<bigdecimal::BigDecimal>)>(conn)
        .expect("read")
        .into_iter()
        .map(|(date, revision, value)| (date, revision, value.unwrap().to_string()))
        .collect()
}

fn p(date: &str, revision: &str, value: &str) -> (NaiveDate, NaiveDate, String) {
    (d(date), d(revision), value.to_string())
}

#[tokio::test]
async fn first_release_keeps_one_original_per_world_bank_date() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    migrate_to_before(&mut conn, FIRST_RELEASE);
    populate(&mut conn);

    let count_before = row_count(&mut conn);
    let fred_before = rows(&mut conn, FRED_SERIES);
    let as_of = Some(d("2022-12-31"));
    let as_of_before = read(&mut conn, false, as_of, false);
    let latest_before = read(&mut conn, false, None, true);
    // Before: originalOnly returns every legacy release, and its asOf the newest one.
    assert_eq!(
        read(&mut conn, true, None, false),
        [
            p("2000-01-01", "2020-07-01", "1.0"),
            p("2000-01-01", "2021-07-01", "1.1"),
            p("2000-01-01", "2022-07-01", "1.2"),
            p("2001-01-01", "2020-07-01", "2.0"),
            p("2002-01-01", "2021-07-01", "3.0"),
            p("2002-01-01", "2022-07-01", "3.0"),
        ]
    );
    assert_eq!(
        read(&mut conn, true, as_of, false)[0],
        p("2000-01-01", "2022-07-01", "1.2")
    );

    run_first_release(&mut conn).expect("migration");

    // One original per date, the earliest real vintage; the synthetic row is untouched.
    let original = |date: &str, revision: &str, is_original_release: bool, value: &str| Row {
        date: d(date),
        revision_date: d(revision),
        is_original_release,
        value: value.to_string(),
    };
    assert_eq!(
        rows(&mut conn, WB_SERIES),
        [
            original("2000-01-01", "2000-01-01", true, "0.9"),
            original("2000-01-01", "2020-07-01", true, "1.0"),
            original("2000-01-01", "2021-07-01", false, "1.1"),
            original("2000-01-01", "2022-07-01", false, "1.2"),
            original("2001-01-01", "2020-07-01", true, "2.0"),
            original("2001-01-01", "2023-07-01", false, "2.1"),
            original("2002-01-01", "2021-07-01", true, "3.0"),
            original("2002-01-01", "2022-07-01", false, "3.0"),
        ]
    );
    // Nothing deleted, no other source touched.
    assert_eq!(row_count(&mut conn), count_before);
    assert_eq!(rows(&mut conn, FRED_SERIES), fred_before);

    // originalOnly now returns the first release, with or without asOf.
    let first = [
        p("2000-01-01", "2020-07-01", "1.0"),
        p("2001-01-01", "2020-07-01", "2.0"),
        p("2002-01-01", "2021-07-01", "3.0"),
    ];
    assert_eq!(read(&mut conn, true, None, false), first);
    assert_eq!(read(&mut conn, true, as_of, false), first);
    // asOf and latest reads see the same values as before.
    assert_eq!(read(&mut conn, false, as_of, false), as_of_before);
    assert_eq!(read(&mut conn, false, None, true), latest_before);
}

#[tokio::test]
async fn first_release_refuses_a_revision_twin_and_changes_nothing() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    migrate_to_before(&mut conn, FIRST_RELEASE);
    populate(&mut conn);
    // A revision at the same vintage as the 2022 legacy release of 2000.
    point(
        &mut conn,
        WB_SERIES,
        ("2000-01-01", "2022-07-01", false, "1.25"),
    );
    let before = rows(&mut conn, WB_SERIES);

    let err = run_first_release(&mut conn).expect_err("a twin fails the migration");
    assert!(err.to_string().contains("already have a revision"), "{err}");
    assert_eq!(rows(&mut conn, WB_SERIES), before);
    assert_eq!(
        conn.pending_migrations(MIGRATIONS).expect("pending")[0]
            .name()
            .version()
            .to_string(),
        FIRST_RELEASE
    );
}

#[tokio::test]
async fn first_release_refuses_without_the_world_bank_source() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    migrate_to_before(&mut conn, FIRST_RELEASE);
    populate(&mut conn);
    diesel::sql_query("UPDATE data_sources SET name = 'World Bank (renamed)' WHERE name = $1")
        .bind::<Text, _>(WORLD_BANK)
        .execute(&mut conn)
        .expect("rename");
    let before = rows(&mut conn, WB_SERIES);

    let err = run_first_release(&mut conn).expect_err("no World Bank source fails it");
    assert!(err.to_string().contains("found 0"), "{err}");
    assert_eq!(rows(&mut conn, WB_SERIES), before);
}

/// A World Bank write in progress (its transaction holds a series' economic_series row lock,
/// as persist_series does) makes the migration wait at most lock_timeout and fail with
/// nothing changed, so no fetch can compare against a label the migration is changing.
#[tokio::test]
async fn first_release_waits_for_a_world_bank_write_and_fails_cleanly() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    migrate_to_before(&mut conn, FIRST_RELEASE);
    populate(&mut conn);
    let before = rows(&mut conn, WB_SERIES);

    let mut writer = connect(&url);
    writer.batch_execute("BEGIN").expect("begin");
    diesel::sql_query("UPDATE economic_series SET title = title WHERE external_id = $1")
        .bind::<Text, _>(WB_SERIES)
        .execute(&mut writer)
        .expect("lock the series row");

    let started = std::time::Instant::now();
    let err = run_first_release(&mut conn).expect_err("a locked series fails the migration");
    assert!(err.to_string().contains("lock timeout"), "{err}");
    assert!(started.elapsed() >= std::time::Duration::from_secs(9));
    writer.batch_execute("ROLLBACK").expect("rollback");
    assert_eq!(rows(&mut conn, WB_SERIES), before);

    // Once the write is done, the migration runs.
    run_first_release(&mut conn).expect("migration");
}
