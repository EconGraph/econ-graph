// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Migration 2026-10-25-000100_observation_values_unconstrained_numeric on a populated
//! database: up keeps every stored value and then stores values DECIMAL(20,6) could not; down
//! refuses while such values exist, and otherwise restores DECIMAL(20,6) with the values intact.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Nullable, Numeric, Text};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/");

const NUMERIC_FIX: &str = "20261025000100";

/// Points written before the migration, each a value DECIMAL(20,6) holds.
const POPULATED_POINTS: i64 = 5000;

/// Values DECIMAL(20,6) overflows or rounds, as text: a World Bank aggregate near 1.9e14
/// (NY.GDP.MKTP.PP.CD for WLD is of this size), one past the old 14 integer digits with
/// fractional digits, and one with more than 6 fractional digits.
const WIDE_VALUES: [&str; 3] = [
    "189543211876543.2109375",
    "123456789012345678.123456789",
    "0.000123456789012345",
];

async fn start_postgres() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "numeric_fix")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@localhost:{port}/numeric_fix");
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

#[derive(QueryableByName)]
struct ColumnType {
    #[diesel(sql_type = Text)]
    t: String,
}

fn column_type(conn: &mut PgConnection, table: &str, column: &str) -> String {
    diesel::sql_query(format!(
        "SELECT format_type(atttypid, atttypmod) AS t FROM pg_attribute \
         WHERE attrelid = '{table}'::regclass AND attname = '{column}'"
    ))
    .get_result::<ColumnType>(conn)
    .expect("column type")
    .t
}

fn relfilenode(conn: &mut PgConnection, relation: &str) -> i64 {
    #[derive(QueryableByName)]
    struct Node {
        #[diesel(sql_type = BigInt)]
        n: i64,
    }
    diesel::sql_query(format!(
        "SELECT pg_relation_filenode('{relation}')::bigint AS n"
    ))
    .get_result::<Node>(conn)
    .expect("filenode")
    .n
}

/// A series to hang points on (uses the baseline's seeded FRED data source).
fn create_series(conn: &mut PgConnection) {
    diesel::sql_query(
        "WITH src AS (SELECT id FROM data_sources WHERE name LIKE '%FRED%' LIMIT 1), \
              ds AS (INSERT INTO datasets (source_id, code, name) \
                     SELECT id, 'numeric_fix', 'numeric fix' FROM src RETURNING id, source_id) \
         INSERT INTO economic_series (source_id, external_id, title, frequency, dataset_id) \
         SELECT source_id, 'numeric_fix', 'numeric fix', 'Monthly', id FROM ds",
    )
    .execute(conn)
    .expect("create series");
}

/// Writes [`POPULATED_POINTS`] points with values DECIMAL(20,6) holds exactly.
fn populate(conn: &mut PgConnection) {
    diesel::sql_query(format!(
        "INSERT INTO data_points (series_id, date, value, revision_date) \
         SELECT s.id, DATE '1900-01-01' + g, (g * 1234.567891)::numeric(20,6) - 99999999.5, \
                DATE '2026-01-01' \
         FROM economic_series s, generate_series(1, {POPULATED_POINTS}) g \
         WHERE s.external_id = 'numeric_fix'"
    ))
    .execute(conn)
    .expect("populate");
}

/// A fingerprint of every populated value, in date order.
fn checksum(conn: &mut PgConnection) -> String {
    diesel::sql_query(
        "SELECT md5(string_agg(value::text, ',' ORDER BY date)) AS t FROM data_points \
         WHERE date < DATE '1900-01-01' + 100000",
    )
    .get_result::<ColumnType>(conn)
    .expect("checksum")
    .t
}

/// Stores `WIDE_VALUES` on dates after the populated ones and reads them back as text.
fn store_wide_values(conn: &mut PgConnection) -> Vec<String> {
    for (i, v) in WIDE_VALUES.iter().enumerate() {
        diesel::sql_query(
            "INSERT INTO data_points (series_id, date, value, revision_date) \
             SELECT id, DATE '2400-01-01' + $1::int, $2, DATE '2026-01-01' \
             FROM economic_series WHERE external_id = 'numeric_fix'",
        )
        .bind::<diesel::sql_types::Integer, _>(i as i32)
        .bind::<Nullable<Numeric>, _>(Some(BigDecimal::from_str(v).unwrap()))
        .execute(conn)
        .expect("store a wide value");
    }
    read_wide_values(conn)
}

/// The values [`store_wide_values`] stored, as text.
fn read_wide_values(conn: &mut PgConnection) -> Vec<String> {
    #[derive(QueryableByName)]
    struct V {
        #[diesel(sql_type = Nullable<Numeric>)]
        value: Option<BigDecimal>,
    }
    diesel::sql_query("SELECT value FROM data_points WHERE date >= DATE '2400-01-01' ORDER BY date")
        .load::<V>(conn)
        .expect("read wide values")
        .into_iter()
        .map(|v| v.value.unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn numeric_fix_up_down_on_a_populated_database() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    migrate_to_before(&mut conn, NUMERIC_FIX);
    assert_eq!(
        column_type(&mut conn, "data_points", "value"),
        "numeric(20,6)"
    );
    create_series(&mut conn);
    populate(&mut conn);
    let before = checksum(&mut conn);
    let heap = relfilenode(&mut conn, "data_points");
    let index = relfilenode(&mut conn, "idx_data_points_latest_revision");

    // Up: unconstrained NUMERIC, without rewriting the table or rebuilding its indexes, and
    // every stored value unchanged.
    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");
    assert!(conn
        .applied_migrations()
        .unwrap()
        .iter()
        .any(|v| v.to_string() == NUMERIC_FIX));
    assert_eq!(column_type(&mut conn, "data_points", "value"), "numeric");
    assert_eq!(
        column_type(&mut conn, "global_indicator_data", "value"),
        "numeric"
    );
    assert_eq!(relfilenode(&mut conn, "data_points"), heap, "no rewrite");
    assert_eq!(
        relfilenode(&mut conn, "idx_data_points_latest_revision"),
        index,
        "no index rebuild"
    );
    assert_eq!(checksum(&mut conn), before);
    diesel::sql_query(
        "INSERT INTO crawl_queue (source, series_id, kind) VALUES ('FRED', 'GDP', 'refetch_series')",
    )
    .execute(&mut conn)
    .expect("refetch_series is an allowed kind");

    // Values the old type overflowed or rounded are stored and read back exactly.
    assert_eq!(store_wide_values(&mut conn), WIDE_VALUES);

    // Down refuses while a value doesn't fit DECIMAL(20,6) (or a refetch_series row exists),
    // and leaves everything as it was.
    assert!(conn.revert_last_migration(MIGRATIONS).is_err());
    assert_eq!(column_type(&mut conn, "data_points", "value"), "numeric");
    assert_eq!(read_wide_values(&mut conn), WIDE_VALUES);
    diesel::sql_query("DELETE FROM crawl_queue WHERE kind = 'refetch_series'")
        .execute(&mut conn)
        .unwrap();
    assert!(
        conn.revert_last_migration(MIGRATIONS).is_err(),
        "the wide values alone still block the rollback"
    );

    // With only values that fit, down restores DECIMAL(20,6) and keeps them.
    diesel::sql_query("DELETE FROM data_points WHERE date >= DATE '2400-01-01'")
        .execute(&mut conn)
        .unwrap();
    let reverted = conn
        .revert_last_migration(MIGRATIONS)
        .expect("migrate down");
    assert_eq!(reverted.to_string(), NUMERIC_FIX);
    assert_eq!(
        column_type(&mut conn, "data_points", "value"),
        "numeric(20,6)"
    );
    assert_eq!(
        column_type(&mut conn, "global_indicator_data", "value"),
        "numeric(20,6)"
    );
    assert_eq!(checksum(&mut conn), before);
    assert!(diesel::sql_query(
        "INSERT INTO crawl_queue (source, series_id, kind) VALUES ('FRED', 'GDP', 'refetch_series')",
    )
    .execute(&mut conn)
    .is_err());

    // Up again.
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrate up again");
    assert_eq!(column_type(&mut conn, "data_points", "value"), "numeric");
    assert_eq!(checksum(&mut conn), before);
}
