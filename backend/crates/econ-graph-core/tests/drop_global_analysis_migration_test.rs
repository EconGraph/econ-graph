// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Migration 2026-11-10-000100_drop_global_analysis_tables (ECO-898): on the unchanged v4.0
//! baseline seeds it drops the eight legacy global-analysis tables; on anything else (an
//! edited seed, an extra or duplicated row, a row in a table the baseline left empty) it
//! refuses, and all eight tables are still there with their rows.

use diesel::prelude::*;
use diesel::sql_types::BigInt;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/data");

const DROP_GLOBAL_ANALYSIS: &str = "20261110000100";

const TABLES: [&str; 8] = [
    "countries",
    "global_economic_indicators",
    "global_indicator_data",
    "country_correlations",
    "trade_relationships",
    "global_economic_events",
    "event_country_impacts",
    "leading_indicators",
];

/// The text every refusal's error starts with.
const REFUSAL: &str = "Not dropping the global-analysis tables";

async fn start_postgres() -> (ContainerAsync<GenericImage>, u16) {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    (container, port)
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

/// A new database on the container, migrated to just before the drop migration.
fn database_before_drop(port: u16, name: &str) -> PgConnection {
    let mut admin = connect(&format!(
        "postgres://postgres:postgres@localhost:{port}/postgres"
    ));
    diesel::sql_query(format!("CREATE DATABASE {name}"))
        .execute(&mut admin)
        .expect("create database");
    let mut conn = connect(&format!(
        "postgres://postgres:postgres@localhost:{port}/{name}"
    ));
    loop {
        let pending = conn.pending_migrations(MIGRATIONS).expect("pending");
        let next = pending
            .first()
            .unwrap_or_else(|| panic!("migration {DROP_GLOBAL_ANALYSIS} is not pending"));
        if next.name().version().to_string() == DROP_GLOBAL_ANALYSIS {
            return conn;
        }
        conn.run_migration(next.as_ref()).expect("run migration");
    }
}

/// Runs the drop migration, which must be the next pending one.
fn run_drop(conn: &mut PgConnection) -> Result<(), String> {
    let pending = conn.pending_migrations(MIGRATIONS).expect("pending");
    let next = pending.first().expect("the drop migration is pending");
    assert_eq!(next.name().version().to_string(), DROP_GLOBAL_ANALYSIS);
    conn.run_migration(next.as_ref())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    n: i64,
}

fn count(conn: &mut PgConnection, query: &str) -> i64 {
    diesel::sql_query(query)
        .get_result::<Count>(conn)
        .expect(query)
        .n
}

/// How many of the eight tables exist.
fn tables_present(conn: &mut PgConnection) -> i64 {
    let names = TABLES.map(|t| format!("'{t}'")).join(", ");
    count(
        conn,
        &format!(
            "SELECT count(*) AS n FROM pg_tables \
             WHERE schemaname = 'public' AND tablename IN ({names})"
        ),
    )
}

fn exec(conn: &mut PgConnection, statement: &str) {
    diesel::sql_query(statement).execute(conn).expect(statement);
}

/// The migration refuses with a message naming `table`, and leaves every table, the rows it
/// counted, and the migration itself (still pending) as they were.
fn assert_refused(conn: &mut PgConnection, table: &str, countries: i64, events: i64) {
    let error = run_drop(conn).expect_err("the drop migration must refuse");
    assert!(error.contains(REFUSAL), "unexpected error: {error}");
    assert!(error.contains(table), "the refusal names {table}: {error}");
    assert_eq!(tables_present(conn), 8, "all eight tables are still there");
    assert_eq!(
        count(conn, "SELECT count(*) AS n FROM countries"),
        countries
    );
    assert_eq!(
        count(conn, "SELECT count(*) AS n FROM global_economic_events"),
        events
    );
    let pending = conn.pending_migrations(MIGRATIONS).expect("pending");
    assert_eq!(
        pending.first().map(|m| m.name().version().to_string()),
        Some(DROP_GLOBAL_ANALYSIS.to_string()),
        "the refused migration is still pending"
    );
}

#[tokio::test]
async fn drop_global_analysis_tables_only_on_unchanged_seeds() {
    let (_container, port) = start_postgres().await;

    // The unchanged baseline: 20 seeded countries and 5 seeded events, nothing else. It drops
    // all eight tables.
    let mut conn = database_before_drop(port, "baseline_seeds");
    assert_eq!(tables_present(&mut conn), 8);
    assert_eq!(count(&mut conn, "SELECT count(*) AS n FROM countries"), 20);
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS n FROM global_economic_events"
        ),
        5
    );
    run_drop(&mut conn).expect("the unchanged seeds are dropped");
    assert_eq!(tables_present(&mut conn), 0);

    // An edited seed country.
    let mut conn = database_before_drop(port, "edited_country");
    exec(
        &mut conn,
        "UPDATE countries SET population = 331000000 WHERE iso_code = 'USA'",
    );
    assert_refused(&mut conn, "countries", 20, 5);

    // A countries row the baseline did not seed.
    let mut conn = database_before_drop(port, "extra_country");
    exec(
        &mut conn,
        "INSERT INTO countries (iso_code, iso_code_2, name, region) \
         VALUES ('NZL', 'NZ', 'New Zealand', 'Oceania')",
    );
    assert_refused(&mut conn, "countries", 21, 5);

    // A duplicate of a seeded event, identical in every compared column.
    let mut conn = database_before_drop(port, "duplicate_event");
    exec(
        &mut conn,
        "INSERT INTO global_economic_events \
             (name, description, event_type, severity, start_date, end_date, \
              primary_country_id, affected_regions, economic_impact_score) \
         SELECT name, description, event_type, severity, start_date, end_date, \
                primary_country_id, affected_regions, economic_impact_score \
         FROM global_economic_events WHERE name = 'Brexit'",
    );
    assert_refused(&mut conn, "global_economic_events", 20, 6);

    // One row in a table the baseline left empty.
    let mut conn = database_before_drop(port, "child_row");
    exec(
        &mut conn,
        "INSERT INTO global_economic_indicators \
             (country_id, indicator_code, indicator_name, category, frequency) \
         SELECT id, 'NY.GDP.MKTP.CD', 'GDP (current US$)', 'GDP', 'Annual' \
         FROM countries WHERE iso_code = 'USA'",
    );
    assert_refused(&mut conn, "global_economic_indicators", 20, 5);
    assert_eq!(
        count(
            &mut conn,
            "SELECT count(*) AS n FROM global_economic_indicators"
        ),
        1
    );
}
