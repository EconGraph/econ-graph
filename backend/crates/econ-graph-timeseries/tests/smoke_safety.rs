//! The smoke executable is an explicit disposable fixture writer, never a migration shortcut
//! for a normal application database. All database cases use an owned UUID-named scratch DB.
use anyhow::{Context, Result};
use diesel::{connection::SimpleConnection, prelude::*, sql_types::BigInt};
use std::process::{Command, Output};
use uuid::Uuid;

#[test]
fn smoke_requires_explicit_disposable_identity_before_reading_environment() {
    let out = Command::new(env!("CARGO_BIN_EXE_timeseries"))
        .args(["smoke", "--migrate"])
        .env_remove("DATA_DATABASE_URL")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--disposable-target"));
}
struct Disposable {
    target: Uuid,
    name: String,
    url: String,
    admin: String,
    warehouse: tempfile::TempDir,
}
impl Disposable {
    fn new(admin: &str) -> Result<Self> {
        let target = Uuid::new_v4();
        let name = format!("eg_smoke_{}", target.simple());
        let mut conn = PgConnection::establish(admin)?;
        conn.batch_execute(&format!("CREATE DATABASE {name}"))?;
        let mut url = url::Url::parse(admin)?;
        url.set_path(&format!("/{name}"));
        url.set_query(None);
        Ok(Self {
            target,
            name,
            url: url.to_string(),
            admin: admin.into(),
            warehouse: tempfile::tempdir()?,
        })
    }
    fn connect(&self) -> Result<PgConnection> {
        Ok(PgConnection::establish(&self.url)?)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_timeseries"));
        command
            .arg("smoke")
            .arg("--disposable-target")
            .arg(self.target.to_string())
            .env("DATA_DATABASE_URL", &self.url)
            .env("TIMESERIES_TARGET_ID", self.target.to_string())
            .env("TIMESERIES_BACKEND", "postgres")
            .env("TIMESERIES_ICEBERG_MIRROR", "false")
            .env("ICEBERG_WAREHOUSE", self.warehouse.path())
            .env_remove("DATABASE_URL")
            .env_remove("S3_ENDPOINT")
            .env_remove("SMOKE_OUTPUT_JSON");
        command
    }
    fn count(&self, sql: &str) -> Result<i64> {
        #[derive(QueryableByName)]
        struct Count {
            #[diesel(sql_type=BigInt)]
            n: i64,
        }
        Ok(diesel::sql_query(sql)
            .get_result::<Count>(&mut self.connect()?)?
            .n)
    }
    fn public_tables(&self) -> Result<i64> {
        self.count("SELECT count(*) AS n FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND c.relkind IN ('r','p','v','m','f')")
    }
}
impl Drop for Disposable {
    fn drop(&mut self) {
        if let Ok(mut conn) = PgConnection::establish(&self.admin) {
            let _ = conn.batch_execute(&format!("DROP DATABASE {} WITH (FORCE)", self.name));
        }
    }
}
fn failed(output: Output, expected: &str) {
    assert!(
        !output.status.success(),
        "unexpected smoke success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "missing error {expected:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL DATA_DATABASE_URL admin; creates and drops only owned eg_smoke_UUID DB"]
async fn smoke_refuses_unbound_paused_or_mismatched_targets_and_rolls_back_metadata() -> Result<()>
{
    let admin = std::env::var("DATA_DATABASE_URL")
        .context("DATA_DATABASE_URL for explicit disposable smoke test")?;
    let db = Disposable::new(&admin)?;
    assert_eq!(db.public_tables()?, 0);
    failed(
        db.command()
            .arg("--migrate")
            .env_remove("DATA_DATABASE_URL")
            .env("DATABASE_URL", &db.url)
            .output()?,
        "explicit DATA_DATABASE_URL",
    );
    assert_eq!(
        db.public_tables()?,
        0,
        "DATABASE_URL fallback must not run migrations"
    );
    failed(
        db.command()
            .arg("--migrate")
            .env("TIMESERIES_TARGET_ID", Uuid::new_v4().to_string())
            .output()?,
        "must match explicit TIMESERIES_TARGET_ID",
    );
    assert_eq!(db.public_tables()?, 0);
    let mut wrong = url::Url::parse(&db.url)?;
    wrong.set_path("/postgres");
    failed(
        db.command()
            .arg("--migrate")
            .env("DATA_DATABASE_URL", wrong.to_string())
            .output()?,
        "exact eg_smoke_",
    );
    assert_eq!(db.public_tables()?, 0);
    failed(db.command().output()?, "explicit --migrate");
    assert_eq!(db.public_tables()?, 0);
    let mut overridden = url::Url::parse(&db.url)?;
    overridden.set_query(Some("dbname=postgres"));
    failed(
        db.command()
            .arg("--migrate")
            .env("DATA_DATABASE_URL", overridden.to_string())
            .output()?,
        "must not override its database identity",
    );
    assert_eq!(db.public_tables()?, 0);
    db.connect()?.batch_execute(
        "CREATE SCHEMA unrelated; CREATE TABLE unrelated.owned_marker(id integer)",
    )?;
    failed(
        db.command().arg("--migrate").output()?,
        "unrecognized nonempty database",
    );
    assert_eq!(
        db.public_tables()?,
        0,
        "unrecognized schema must not trigger migrations"
    );
    db.connect()?
        .batch_execute("DROP SCHEMA unrelated CASCADE")?;
    let file_warehouse = url::Url::from_directory_path(db.warehouse.path()).unwrap();
    let output = db
        .command()
        .arg("--migrate")
        .env("ICEBERG_WAREHOUSE", file_warehouse.to_string())
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "fresh explicit disposable smoke failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["status"], "passed");
    assert_eq!(report["rows"], 5);
    assert_eq!(report["disposable_target"], db.target.to_string());
    assert!(report["metadata_location"]
        .as_str()
        .unwrap()
        .contains(&format!("eg_smoke_{}", db.target.simple())));
    assert_eq!(
        db.count("SELECT count(*) AS n FROM data_sources WHERE name LIKE 'Iceberg smoke %'")?,
        1
    );
    assert_eq!(db.count("SELECT count(*) AS n FROM iceberg_catalog")?, 1);
    // Leave a pending migration marker to prove the pause failure takes precedence over
    // migration work. Re-running that ALTER first would report a duplicate column instead.
    let mut conn = db.connect()?;
    conn.batch_execute("DELETE FROM __diesel_schema_migrations WHERE version='20261031010000'")?;
    conn.batch_execute(&format!(
        "UPDATE timeseries_recovery_state SET writes_paused=true,pause_token='{}'",
        Uuid::new_v4()
    ))?;
    failed(db.command().arg("--migrate").output()?, "paused");
    assert_eq!(
        db.count(
            "SELECT count(*) AS n FROM __diesel_schema_migrations WHERE version='20261031010000'"
        )?,
        0
    );
    assert_eq!(
        db.count("SELECT count(*) AS n FROM data_sources WHERE name LIKE 'Iceberg smoke %'")?,
        1
    );
    conn.batch_execute("UPDATE timeseries_recovery_state SET writes_paused=false,pause_token=NULL; INSERT INTO __diesel_schema_migrations(version) VALUES('20261031010000')")?;
    // A same-name database with another durable target is still rejected before migrations.
    conn.batch_execute(&format!("DELETE FROM timeseries_recovery_transitions; UPDATE timeseries_recovery_state SET target_id='{}'",Uuid::new_v4()))?;
    failed(
        db.command().arg("--migrate").output()?,
        "not this disposable smoke target",
    );
    assert_eq!(
        db.count("SELECT count(*) AS n FROM data_sources WHERE name LIKE 'Iceberg smoke %'")?,
        1
    );
    conn.batch_execute(&format!(
        "UPDATE timeseries_recovery_state SET target_id='{}',target_label='production'",
        db.target
    ))?;
    failed(
        db.command().arg("--migrate").output()?,
        "not this disposable smoke target",
    );
    assert_eq!(db.count("SELECT count(*) AS n FROM iceberg_catalog")?, 1);
    conn.batch_execute(&format!(
        "UPDATE timeseries_recovery_state SET target_label='disposable-timeseries-smoke:{}'",
        db.target
    ))?;
    // Any failure after metadata and object staging must roll back metadata and pointers
    // together. Only unreachable UUID-scoped objects may remain.
    conn.batch_execute("CREATE FUNCTION smoke_reject_receipt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.generation>0 THEN RAISE EXCEPTION 'injected smoke receipt failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER smoke_reject_receipt BEFORE INSERT ON iceberg_commits FOR EACH ROW EXECUTE FUNCTION smoke_reject_receipt()")?;
    failed(db.command().output()?, "injected smoke receipt failure");
    assert_eq!(
        db.count("SELECT count(*) AS n FROM data_sources WHERE name LIKE 'Iceberg smoke %'")?,
        1
    );
    assert_eq!(
        db.count("SELECT count(*) AS n FROM datasets WHERE code='smoke'")?,
        1
    );
    assert_eq!(
        db.count("SELECT count(*) AS n FROM economic_series WHERE external_id='smoke'")?,
        1
    );
    assert_eq!(db.count("SELECT count(*) AS n FROM iceberg_catalog")?, 1);
    Ok(())
}
