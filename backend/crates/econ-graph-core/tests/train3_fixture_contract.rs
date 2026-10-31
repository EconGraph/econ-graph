//! Train 3's deterministic compatibility fixture, exercised on both database shapes.
use diesel::{connection::SimpleConnection, prelude::*, sql_types::Text};
use econ_graph_core::test_utils::ScratchDatabase;
use econ_graph_core::{run_migration_set, MigrationSet};
use std::process::Command;

const APP: &str = include_str!("fixtures/train3/app.sql");
const DATA: &str = include_str!("fixtures/train3/data.sql");
const RETAINED: &[&str] = &[
    "users",
    "chart_annotations",
    "annotation_comments",
    "chart_collaborators",
    "financial_annotations",
];

#[derive(QueryableByName)]
struct Value {
    #[diesel(sql_type = Text)]
    value: String,
}
fn value(conn: &mut PgConnection, sql: &str) -> String {
    diesel::sql_query(sql)
        .get_result::<Value>(conn)
        .unwrap()
        .value
}
fn retained(conn: &mut PgConnection) -> Vec<String> {
    RETAINED.iter().map(|table| value(conn, &format!("SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY id), '[]'::jsonb)::text AS value FROM {table} t WHERE id::text LIKE '31000000-%'"))).collect()
}
fn all_table_rows(conn: &mut PgConnection) -> std::collections::BTreeMap<String, String> {
    let tables = diesel::sql_query(
        "SELECT tablename AS value FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
    )
    .load::<Value>(conn)
    .unwrap();
    tables.into_iter().map(|row| {
        let quoted = row.value.replace('"', "\"\"");
        let rows = value(conn, &format!("SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text AS value FROM \"{quoted}\" t"));
        (row.value, rows)
    }).collect()
}
fn exists(conn: &mut PgConnection, table: &str) -> bool {
    value(
        conn,
        &format!("SELECT (to_regclass('public.{table}') IS NOT NULL)::text AS value"),
    ) == "true"
}
fn command_url(db: &ScratchDatabase) -> String {
    // The isolated runner executes matching-version Postgres tools inside its own container.
    // Only the host/port differ there; preserve this scratch database's generated name.
    let mut result = url::Url::parse(
        &std::env::var("TRAIN3_TOOL_DATABASE_URL").unwrap_or_else(|_| db.url.clone()),
    )
    .unwrap();
    result.set_path(url::Url::parse(&db.url).unwrap().path());
    result.to_string()
}
fn backup_roundtrip(db: &ScratchDatabase) {
    if std::env::var("TRAIN3_BACKUP_CHECK").as_deref() != Ok("1") {
        return;
    }
    let restored = ScratchDatabase::create("train3_restore");
    let output = Command::new(std::env::var("TRAIN3_PG_DUMP").unwrap_or_else(|_| "pg_dump".into()))
        .args([
            "--no-owner",
            "--no-privileges",
            "--dbname",
            &command_url(db),
        ])
        .output()
        .expect("pg_dump must be available when TRAIN3_BACKUP_CHECK=1");
    assert!(
        output.status.success(),
        "pg_dump: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = Command::new(std::env::var("TRAIN3_PSQL").unwrap_or_else(|_| "psql".into()))
        .args([
            "--no-psqlrc",
            "--set",
            "ON_ERROR_STOP=1",
            "--dbname",
            &command_url(&restored),
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("psql must be available");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&output.stdout)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "restore: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        all_table_rows(&mut db.connect()),
        all_table_rows(&mut restored.connect()),
        "a full database backup must retain every table and row"
    );
}
async fn check_shape(combined: bool) {
    let data = ScratchDatabase::create("train3_data");
    let app_separate = (!combined).then(|| ScratchDatabase::create("train3_app"));
    let app = app_separate.as_ref().unwrap_or(&data);
    run_migration_set(&data.url, MigrationSet::Data)
        .await
        .unwrap();
    assert!(
        !exists(&mut data.connect(), "users"),
        "crawler/data migration must not create app tables"
    );
    run_migration_set(&app.url, MigrationSet::App)
        .await
        .unwrap();
    if !combined {
        assert!(
            !exists(&mut app.connect(), "economic_series"),
            "app migrations must not create data tables"
        );
    }
    data.connect().batch_execute(DATA).unwrap();
    app.connect().batch_execute(APP).unwrap();
    let before = retained(&mut app.connect());
    assert!(before.iter().all(|rows| rows != "[]"));
    // Re-running the current baseline is deliberately a no-op; this is not a claim about a
    // future downstream migration. Existing release upgrade rehearsals remain separate.
    run_migration_set(&app.url, MigrationSet::App)
        .await
        .unwrap();
    run_migration_set(&data.url, MigrationSet::Data)
        .await
        .unwrap();
    assert_eq!(before, retained(&mut app.connect()));
    assert_eq!(value(&mut data.connect(), "SELECT value::text AS value FROM data_points WHERE id = '30000000-0000-4000-8000-000000000004'"), "123456789012345678901234567890.123456789012345678901234567890");
    assert_eq!(value(&mut data.connect(), "SELECT count(*)::text AS value FROM data_points WHERE series_id = '30000000-0000-4000-8000-000000000003' AND value IS NULL"), "1");
    backup_roundtrip(app);
    if !combined {
        backup_roundtrip(&data);
    }
    diesel::sql_query("DELETE FROM data_sources WHERE id = '30000000-0000-4000-8000-000000000001'")
        .execute(&mut data.connect())
        .unwrap();
    assert_eq!(
        before,
        retained(&mut app.connect()),
        "loss of a data target must not cascade into authored content"
    );
}
#[tokio::test]
async fn split_plane_fixture_preserves_content_and_exact_values() {
    check_shape(false).await;
}
#[tokio::test]
async fn combined_fixture_preserves_content_and_exact_values() {
    check_shape(true).await;
}
