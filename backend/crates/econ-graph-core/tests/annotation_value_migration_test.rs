// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Annotation coordinates survive the unconstrained NUMERIC migration exactly.

use diesel::migration::MigrationSource;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::sql_types::{Integer, Nullable, Text};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/app");
const NUMERIC_FIX: &str = "20261026000100";

async fn database() -> (ContainerAsync<GenericImage>, PgConnection) {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "annotation_numeric")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@localhost:{port}/annotation_numeric");
    for _ in 0..30 {
        if let Ok(conn) = PgConnection::establish(&url) {
            return (container, conn);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    panic!("connect to postgres");
}

#[derive(QueryableByName)]
struct ColumnType {
    #[diesel(sql_type = Integer)]
    typmod: i32,
}

fn typmod(conn: &mut PgConnection) -> i32 {
    diesel::sql_query(
        "SELECT atttypmod AS typmod FROM pg_attribute \
         WHERE attrelid = 'chart_annotations'::regclass AND attname = 'annotation_value'",
    )
    .get_result::<ColumnType>(conn)
    .expect("annotation column type")
    .typmod
}

fn insert(conn: &mut PgConnection, value: Option<&str>) {
    diesel::sql_query(
        "WITH u AS (INSERT INTO users (email, name) \
                    VALUES ('numeric@example.com', 'Numeric') \
                    ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name RETURNING id) \
         INSERT INTO chart_annotations (user_id, annotation_date, annotation_value, title) \
         SELECT id, '2024-01-01', CAST($1 AS NUMERIC), 'Exact coordinate' FROM u",
    )
    .bind::<Nullable<Text>, _>(value)
    .execute(conn)
    .expect("insert annotation");
}

#[derive(QueryableByName)]
struct Value {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

fn values(conn: &mut PgConnection) -> Vec<Option<String>> {
    diesel::sql_query("SELECT annotation_value::text AS value FROM chart_annotations ORDER BY id")
        .load::<Value>(conn)
        .expect("read annotations")
        .into_iter()
        .map(|row| row.value)
        .collect()
}

fn revert_numeric(conn: &mut PgConnection) -> Result<(), String> {
    let migrations = <EmbeddedMigrations as MigrationSource<Pg>>::migrations(&MIGRATIONS)
        .expect("embedded migrations");
    let migration = migrations
        .iter()
        .find(|m| m.name().version().to_string() == NUMERIC_FIX)
        .expect("numeric migration");
    let reverted = conn
        .revert_migration(migration.as_ref())
        .map_err(|e| e.to_string())?;
    assert_eq!(reverted.to_string(), NUMERIC_FIX);
    Ok(())
}

#[tokio::test]
async fn annotation_numeric_migration_up_down_up_preserves_existing_values() {
    let (_container, mut conn) = database().await;
    // Apply only earlier migrations, then populate the old column before widening it.
    for migration in conn
        .pending_migrations(MIGRATIONS)
        .expect("pending migrations")
    {
        if migration.name().version().to_string().as_str() < NUMERIC_FIX {
            conn.run_migration(&migration).expect("earlier migration");
        }
    }
    let legacy_typmod = typmod(&mut conn);
    assert_ne!(legacy_typmod, -1);
    insert(&mut conn, Some("99999999999999.999999"));
    insert(&mut conn, None);
    let before = values(&mut conn);

    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");
    assert_eq!(typmod(&mut conn), -1);
    assert_eq!(values(&mut conn), before);
    revert_numeric(&mut conn).expect("migrate down");
    assert_eq!(typmod(&mut conn), legacy_typmod);
    assert_eq!(values(&mut conn), before);
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrate up again");
    assert_eq!(typmod(&mut conn), -1);
    assert_eq!(values(&mut conn), before);
}

#[tokio::test]
async fn annotation_numeric_keeps_large_values_and_fractional_precision() {
    let (_container, mut conn) = database().await;
    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");
    for value in [
        "190358563845627.40625",
        "-190358563845627.40625",
        "0.123456789012345678901234567890",
        "9999999999999999999999999999999999999999",
        "1.50",
    ] {
        diesel::sql_query("DELETE FROM chart_annotations")
            .execute(&mut conn)
            .unwrap();
        insert(&mut conn, Some(value));
        assert_eq!(values(&mut conn), vec![Some(value.to_string())]);
    }
}

#[tokio::test]
async fn annotation_numeric_rollback_refuses_rounding_and_overflow() {
    let (_container, mut conn) = database().await;
    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");
    for value in [
        "190358563845627.40625",
        "-190358563845627.40625",
        "0.1234567",
        "-0.1234567",
        "99999999999999.9999995",
        "NaN",
        "Infinity",
        "-Infinity",
    ] {
        diesel::sql_query("DELETE FROM chart_annotations")
            .execute(&mut conn)
            .unwrap();
        insert(&mut conn, Some(value));
        let before = values(&mut conn);
        let error = revert_numeric(&mut conn).expect_err("rollback must refuse data loss");
        assert!(error.contains("overflow or lose precision"), "{error}");
        assert_eq!(typmod(&mut conn), -1);
        assert_eq!(values(&mut conn), before);
        assert!(conn
            .applied_migrations()
            .unwrap()
            .iter()
            .any(|v| v.to_string() == NUMERIC_FIX));
    }
}
