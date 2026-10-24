// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Migration 2026-10-24-000100_audit_logs_keep_on_user_delete: applies on top of the
//! v4.0 baseline and survives an up, down, up cycle on an empty database.

use diesel::prelude::*;
use diesel::sql_types::{Bool, Text};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use econ_graph_core::test_utils::POSTGRES_IMAGE_TAG;
use testcontainers::core::WaitFor;
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../../migrations/");

const BASELINE: &str = "20261021000100";
const AUDIT_FIX: &str = "20261024000100";

#[derive(QueryableByName, Debug, PartialEq)]
struct FkState {
    #[diesel(sql_type = Text)]
    definition: String,
    #[diesel(sql_type = Bool)]
    nullable: bool,
}

/// The audit_logs.user_id foreign key definition and whether the column is nullable.
fn fk_state(conn: &mut PgConnection) -> FkState {
    diesel::sql_query(
        "SELECT pg_get_constraintdef(c.oid) AS definition, \
                NOT a.attnotnull AS nullable \
         FROM pg_constraint c \
         JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attname = 'user_id' \
         WHERE c.conrelid = 'audit_logs'::regclass AND c.conname = 'audit_logs_user_id_fkey'",
    )
    .get_result(conn)
    .expect("audit_logs_user_id_fkey exists")
}

fn applied(conn: &mut PgConnection) -> Vec<String> {
    conn.applied_migrations()
        .expect("applied migrations")
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

fn assert_fixed(conn: &mut PgConnection) {
    assert_eq!(
        fk_state(conn),
        FkState {
            definition: "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL".into(),
            nullable: true,
        }
    );
}

fn assert_baseline(conn: &mut PgConnection) {
    assert_eq!(
        fk_state(conn),
        FkState {
            definition: "FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE".into(),
            nullable: false,
        }
    );
}

async fn start_postgres() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("postgres", POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
        .with_env_var("POSTGRES_DB", "audit_fix")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .start()
        .await
        .expect("start postgres");
    let port = container.get_host_port_ipv4(5432).await.expect("port");
    let url = format!("postgres://postgres:postgres@localhost:{port}/audit_fix");
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

#[tokio::test]
async fn audit_fix_migration_up_down_up() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);

    // Up: applies after the baseline, in order.
    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");
    let versions = applied(&mut conn);
    let baseline_pos = versions.iter().position(|v| v == BASELINE);
    let fix_pos = versions.iter().position(|v| v == AUDIT_FIX);
    assert!(baseline_pos.is_some() && fix_pos.is_some(), "{versions:?}");
    assert_fixed(&mut conn);

    // Down: back to the baseline definition.
    let reverted = conn
        .revert_last_migration(MIGRATIONS)
        .expect("migrate down");
    assert_eq!(reverted.to_string(), AUDIT_FIX);
    assert_baseline(&mut conn);

    // Up again.
    conn.run_pending_migrations(MIGRATIONS)
        .expect("migrate up again");
    assert_fixed(&mut conn);
}

/// The documented down.sql limitation: once a user's audit rows hold NULL, rolling back
/// fails (rather than deleting audit history) and leaves the schema as it was.
#[tokio::test]
async fn audit_fix_down_refuses_to_drop_orphaned_history() {
    let (_container, url) = start_postgres().await;
    let mut conn = connect(&url);
    conn.run_pending_migrations(MIGRATIONS).expect("migrate up");

    diesel::sql_query(
        "WITH u AS (INSERT INTO users (email, name) VALUES ('gone@example.com', 'Gone User') \
                    RETURNING id) \
         INSERT INTO audit_logs (user_id, user_name, action, resource_type) \
         SELECT id, 'Gone User', 'login', 'user' FROM u",
    )
    .execute(&mut conn)
    .expect("insert user and audit row");
    diesel::sql_query("DELETE FROM users WHERE email = 'gone@example.com'")
        .execute(&mut conn)
        .expect("delete user");

    assert!(
        conn.revert_last_migration(MIGRATIONS).is_err(),
        "down must fail while NULL user_id rows exist"
    );
    assert_fixed(&mut conn);
    assert!(applied(&mut conn).iter().any(|v| v == AUDIT_FIX));

    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    let kept: Count = diesel::sql_query(
        "SELECT count(*) AS n FROM audit_logs WHERE user_id IS NULL AND user_name = 'Gone User'",
    )
    .get_result(&mut conn)
    .expect("count");
    assert_eq!(kept.n, 1);
}
