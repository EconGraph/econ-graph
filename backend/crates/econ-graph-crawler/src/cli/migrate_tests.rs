//! DB-backed tests for `crawler migrate`. They need `DATABASE_URL` and are skipped when it is
//! unset. Each test creates its own empty database next to `DATABASE_URL`'s (same server, a name
//! unique to the test) and drops it at the end, so they never touch the shared test database.

use clap::Parser;
use diesel::connection::SimpleConnection;
use diesel::{Connection, PgConnection, RunQueryDsl};
use econ_graph_core::MigrationSet;

use super::Cli;

/// The data baseline's version, recorded by every data database.
const DATA_BASELINE_VERSION: &str = "20261021000100";
/// The app baseline's version, which `crawler migrate` must never apply or record.
const APP_BASELINE_VERSION: &str = "20261021000200";

/// An empty database of its own, dropped when the test ends.
struct ScratchDatabase {
    admin_url: String,
    name: String,
    url: String,
}

impl ScratchDatabase {
    /// `None` (the test should skip) when `DATABASE_URL` is unset.
    fn create(tag: &str) -> Option<Self> {
        let Ok(admin_url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set; skipping DB-backed crawler migrate test");
            return None;
        };
        let name = format!("eg_migrate_{tag}_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::establish(&admin_url).expect("connect to DATABASE_URL");
        diesel::sql_query(format!("CREATE DATABASE {name}"))
            .execute(&mut admin)
            .expect("create a scratch database");
        let mut url = url::Url::parse(&admin_url).expect("DATABASE_URL should be a URL");
        url.set_path(&format!("/{name}"));
        Some(Self {
            admin_url,
            name,
            url: url.to_string(),
        })
    }

    fn connect(&self) -> PgConnection {
        PgConnection::establish(&self.url).expect("connect to the scratch database")
    }

    /// Runs `crawler --database-url <this database> migrate [args]` and returns its output.
    async fn migrate(&self, args: &[&str]) -> anyhow::Result<String> {
        let mut full = vec!["crawler", "--database-url", self.url.as_str(), "migrate"];
        full.extend_from_slice(args);
        Cli::try_parse_from(full).unwrap().execute().await
    }
}

impl Drop for ScratchDatabase {
    fn drop(&mut self) {
        if let Ok(mut conn) = PgConnection::establish(&self.admin_url) {
            let _ = diesel::sql_query(format!(
                "DROP DATABASE IF EXISTS {} WITH (FORCE)",
                self.name
            ))
            .execute(&mut conn);
        }
    }
}

#[derive(diesel::QueryableByName)]
struct TextRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    value: String,
}

fn text_rows(conn: &mut PgConnection, query: &str) -> Vec<String> {
    diesel::sql_query(query)
        .load::<TextRow>(conn)
        .expect("query should run")
        .into_iter()
        .map(|r| r.value)
        .collect()
}

/// Every version in `__diesel_schema_migrations`, sorted; `None` when the table doesn't exist.
fn recorded_versions(conn: &mut PgConnection) -> Option<Vec<String>> {
    let exists = text_rows(
        conn,
        "SELECT (to_regclass('public.__diesel_schema_migrations') IS NOT NULL)::text AS value",
    );
    (exists == ["true"]).then(|| {
        text_rows(
            conn,
            "SELECT version AS value FROM __diesel_schema_migrations ORDER BY version",
        )
    })
}

fn has_table(conn: &mut PgConnection, table: &str) -> bool {
    text_rows(
        conn,
        &format!("SELECT (to_regclass('public.{table}') IS NOT NULL)::text AS value"),
    ) == ["true"]
}

fn lines(out: &str) -> Vec<String> {
    out.lines().map(str::to_string).collect()
}

/// `migrate` applies every data migration, prints each version, and records exactly those; a
/// second run applies and prints nothing, and both succeed.
#[tokio::test]
async fn migrate_applies_the_data_set_once_and_prints_each_version() {
    let Some(db) = ScratchDatabase::create("apply") else {
        return;
    };

    let out = db.migrate(&[]).await.expect("migrate should succeed");
    let applied = lines(&out);
    assert!(applied.contains(&"00000000000000".to_string()), "{out}");
    assert!(
        applied.contains(&DATA_BASELINE_VERSION.to_string()),
        "{out}"
    );
    let mut sorted = applied.clone();
    sorted.sort();
    assert_eq!(applied, sorted, "versions print in the order they ran");

    let mut conn = db.connect();
    assert_eq!(recorded_versions(&mut conn), Some(applied));
    assert!(has_table(&mut conn, "data_sources"));

    let out = db.migrate(&[]).await.expect("a second run should succeed");
    assert_eq!(out, "", "a current data set applies nothing");
}

/// `--dry-run` lists the pending versions and leaves `__diesel_schema_migrations` as it was:
/// absent on an empty database, and unchanged on one that holds the app set.
#[tokio::test]
async fn migrate_dry_run_lists_pending_versions_and_changes_nothing() {
    let Some(db) = ScratchDatabase::create("dry_run") else {
        return;
    };
    let mut conn = db.connect();

    let pending = db
        .migrate(&["--dry-run"])
        .await
        .expect("dry run should succeed");
    assert!(lines(&pending).contains(&DATA_BASELINE_VERSION.to_string()));
    assert_eq!(
        recorded_versions(&mut conn),
        None,
        "a dry run must not even create the migrations table"
    );

    econ_graph_core::run_migration_set(&db.url, MigrationSet::App)
        .await
        .expect("the app set should apply");
    let before = recorded_versions(&mut conn);
    let out = db
        .migrate(&["--dry-run"])
        .await
        .expect("dry run should succeed");
    assert_eq!(
        lines(&out),
        lines(&pending)
            .into_iter()
            .filter(|v| v != "00000000000000")
            .collect::<Vec<_>>(),
        "the shared initial setup is recorded by the app set; the rest of the data set is pending"
    );
    assert_eq!(recorded_versions(&mut conn), before);
    assert!(!has_table(&mut conn, "data_sources"));

    // A real run applies exactly what the dry run listed; afterwards nothing is pending.
    assert_eq!(db.migrate(&[]).await.expect("migrate"), out);
    assert_eq!(
        db.migrate(&["--dry-run"]).await.expect("dry run"),
        "",
        "nothing is pending once the data set is current"
    );
}

/// On one database that holds both sets, `migrate` never applies or records an app migration,
/// even where one is pending: here a database built by the single v4.0.0 baseline, which has the
/// app tables but not the app baseline's version (applying the app set would record it).
#[tokio::test]
async fn migrate_never_applies_or_records_an_app_migration() {
    let Some(db) = ScratchDatabase::create("never_app") else {
        return;
    };
    let mut conn = db.connect();

    // A new database: only data tables appear, only data versions are recorded.
    db.migrate(&[]).await.expect("migrate should succeed");
    assert!(!has_table(&mut conn, "users"));
    let data_versions = recorded_versions(&mut conn).expect("migrations table");
    assert!(!data_versions.contains(&APP_BASELINE_VERSION.to_string()));

    // Add the app tables without recording the app baseline, as the single baseline left them.
    conn.batch_execute(include_str!(
        "../../../../migrations/app/2026-10-21-000200_v4_0_app_baseline/up.sql"
    ))
    .expect("the app baseline's SQL should run");
    assert_eq!(db.migrate(&["--dry-run"]).await.expect("dry run"), "");
    assert_eq!(db.migrate(&[]).await.expect("migrate"), "");
    assert_eq!(
        recorded_versions(&mut conn),
        Some(data_versions),
        "the app baseline must not be recorded by crawler migrate"
    );
}
