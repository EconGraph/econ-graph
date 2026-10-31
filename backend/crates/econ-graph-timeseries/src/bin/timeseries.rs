use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use clap::{Parser, Subcommand};
use diesel::{
    prelude::*,
    sql_types::{Bool, Text, Uuid as SqlUuid},
};
use diesel_async::{RunQueryDsl, SimpleAsyncConnection};
use econ_graph_core::{
    database::{create_pool, run_migration_set, MigrationSet},
    models::Observation,
};
use econ_graph_timeseries::{encoding, SqlCatalog, StorageConfig};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Plan bounded compaction for every catalog table; --apply requires exact recovery target.
    Maintenance {
        #[arg(long)]
        max_rows: u64,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        expire_before: Option<DateTime<Utc>>,
        #[arg(long, default_value_t = 1)]
        retain_last: usize,
    },
    /// Write genuine Iceberg/Parquet via the same library used by the crawl commit path.
    /// Requires an explicitly disposable database and warehouse; never deletes objects.
    Smoke {
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        #[arg(long)]
        migrate: bool,
        /// Required disposable database identity: actual DB must be eg_smoke_<UUID without dashes>.
        #[arg(long)]
        disposable_target: Uuid,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::Maintenance {
            max_rows,
            apply,
            expire_before,
            retain_last,
        } => maintenance(max_rows, apply, expire_before, retain_last).await,
        Command::Smoke {
            output,
            migrate,
            disposable_target,
        } => smoke(output, migrate, disposable_target).await,
    }
}
async fn smoke(
    output: Option<std::path::PathBuf>,
    migrate: bool,
    disposable_target: Uuid,
) -> Result<()> {
    let selection = econ_graph_core::observation_storage::ObservationStorage::from_env()?;
    anyhow::ensure!(
        selection.target_id == Some(disposable_target),
        "--disposable-target must match explicit TIMESERIES_TARGET_ID"
    );
    // No generic DATABASE_URL fallback: a normal application environment is never sufficient
    // to authorize this fixture writer or its optional data migrations.
    let url =
        std::env::var("DATA_DATABASE_URL").context("smoke requires explicit DATA_DATABASE_URL")?;
    let expected_database = format!("eg_smoke_{}", disposable_target.simple());
    let parsed =
        url::Url::parse(&url).context("smoke DATA_DATABASE_URL must be a PostgreSQL URL")?;
    anyhow::ensure!(
        matches!(parsed.scheme(), "postgres" | "postgresql")
            && parsed.path() == format!("/{expected_database}"),
        "smoke requires its exact eg_smoke_<disposable UUID> database"
    );
    anyhow::ensure!(
        !parsed
            .query_pairs()
            .any(|(key, _)| matches!(key.as_ref(), "dbname" | "database")),
        "smoke URL must not override its database identity in query parameters"
    );
    let mut storage = StorageConfig::from_env()?;
    // The database identity also scopes every object. Even a mistakenly broad warehouse
    // configuration cannot overwrite another target's metadata/data files.
    if storage.warehouse.contains("://") {
        let mut warehouse =
            url::Url::parse(&storage.warehouse).context("invalid smoke warehouse URL")?;
        warehouse.set_path(&format!(
            "{}/eg_smoke_{}",
            warehouse.path().trim_end_matches('/'),
            disposable_target.simple()
        ));
        storage.warehouse = warehouse.to_string();
    } else {
        storage.warehouse = format!(
            "{}/eg_smoke_{}",
            storage.warehouse.trim_end_matches('/'),
            disposable_target.simple()
        );
    }
    let catalog = SqlCatalog::new(create_pool(&url).await?, storage);
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    let series = Uuid::new_v4();
    let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let rows:Vec<Observation>=[("2024-01-01","2024-01-01",Some("999"),true),("2024-01-01","2024-02-01",Some("123456789012345678901234567890123456789012345678901.123456789123456789"),true),("2024-01-01","2024-03-01",None,false),("2024-02-01","2024-02-01",Some("0"),true),("2024-03-01","2024-03-01",Some("0.0000000000000000000000000000000000000000000000000000000000000001"),true)]
        .into_iter().enumerate().map(|(i,(date,revision,value,original))|serde_json::from_value(json!({"id":Uuid::new_v5(&series,&i.to_be_bytes()),"series_id":series,"date":date,"revision_date":revision,"value":value,"is_original_release":original,"created_at":timestamp,"updated_at":timestamp,"attributes":{}}))).collect::<std::result::Result<_,_>>()?;
    let mut conn = catalog.pool.get().await?;
    let (published, prepared) = conn
        .build_transaction()
        .read_committed()
        .run::<_, anyhow::Error, _>(async |conn| {
            #[derive(QueryableByName)]
            struct Database {
                #[diesel(sql_type = Text)]
                name: String,
            }
            let actual = diesel::sql_query("SELECT current_database() AS name")
                .get_result::<Database>(conn)
                .await?;
            anyhow::ensure!(actual.name == expected_database,
                "connected database does not match disposable smoke target");
            conn.batch_execute("SET LOCAL search_path = public, pg_catalog; SELECT pg_advisory_xact_lock_shared(1162303316)").await?;
            #[derive(QueryableByName)]
            struct Exists {
                #[diesel(sql_type = Bool)]
                present: bool,
            }
            let state_exists = diesel::sql_query("SELECT to_regclass('public.timeseries_recovery_state') IS NOT NULL AS present")
                .get_result::<Exists>(conn).await?.present;
            if state_exists {
                #[derive(QueryableByName)]
                struct Target {
                    #[diesel(sql_type = SqlUuid)]
                    target_id: Uuid,
                    #[diesel(sql_type = Text)]
                    target_label: String,
                }
                let state = diesel::sql_query("SELECT target_id,target_label FROM timeseries_recovery_state WHERE singleton")
                    .get_result::<Target>(conn).await.optional()?;
                if let Some(state) = state {
                    anyhow::ensure!(state.target_id == disposable_target
                        && state.target_label == format!("disposable-timeseries-smoke:{disposable_target}"),
                        "existing recovery target is not this disposable smoke target");
                    // This precedes even pending migration checks. --migrate cannot
                    // change a paused or mismatched durable target.
                    selection.assert_write_allowed_conn(conn).await?;
                }
                if migrate {
                    let pending = econ_graph_core::database::pending_migration_versions(
                        &url, MigrationSet::Data).await?;
                    anyhow::ensure!(pending.is_empty(),
                        "smoke refuses migrations on an initialized target; migrate a fresh disposable database");
                }
            } else {
                let occupied = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname NOT IN ('pg_catalog','information_schema') AND n.nspname NOT LIKE 'pg_toast%' AND c.relkind IN ('r','p','v','m','f')) AS present")
                    .get_result::<Exists>(conn).await?.present;
                anyhow::ensure!(!occupied, "smoke refuses an unrecognized nonempty database");
                anyhow::ensure!(migrate, "fresh disposable smoke target requires explicit --migrate");
                // Hold the shared recovery lock across bootstrap on the separate
                // migration connection; this empty database has no user relation locks.
                run_migration_set(&url, MigrationSet::Data).await?;
            }
            let initialized = diesel::sql_query("INSERT INTO timeseries_recovery_state(target_id,target_label,backend) VALUES($1,$2,$3) ON CONFLICT(singleton) DO NOTHING")
                .bind::<SqlUuid, _>(disposable_target)
                .bind::<Text, _>(format!("disposable-timeseries-smoke:{disposable_target}"))
                .bind::<Text, _>(selection.backend.as_str())
                .execute(conn).await?;
            if initialized == 1 {
                diesel::sql_query("INSERT INTO timeseries_recovery_transitions(target_id,epoch,action,evidence) VALUES($1,0,'initialize',$2::jsonb)")
                    .bind::<SqlUuid, _>(disposable_target)
                    .bind::<Text, _>(json!({"kind":"disposable-smoke","database":expected_database}).to_string())
                    .execute(conn).await?;
            }
            selection.assert_write_allowed_conn(conn).await?;
            conn.batch_execute(&format!("INSERT INTO data_sources(id,name,description,base_url) VALUES('{source}','Iceberg smoke {source}','disposable test','https://example.test'); INSERT INTO datasets(id,source_id,code,name) VALUES('{dataset}','{source}','smoke','Iceberg smoke'); INSERT INTO economic_series(id,source_id,external_id,title,frequency,dataset_id) VALUES('{series}','{source}','smoke','Iceberg smoke','monthly','{dataset}')")).await?;
            let prepared = catalog.prepare_append_conn(conn, Uuid::new_v4(), dataset, &rows).await?;
            let published = SqlCatalog::commit_prepared_conn(conn, &prepared).await?;
            Ok((published, prepared))
        })
        .await?;
    drop(conn);
    let read = catalog.read_rows(&prepared.next).await?;
    anyhow::ensure!(
        encoding::canonical_rows(&rows)? == encoding::canonical_rows(&read)?,
        "Iceberg roundtrip changed observation content"
    );
    let canonical_columns = [
        "id",
        "series_id",
        "date",
        "revision_date",
        "value",
        "is_original_release",
        "attributes",
    ];
    let mut lines = vec![];
    for row in &read {
        let attrs = serde_json::to_value(&row.attributes)?;
        let values = vec![
            Some(row.id.to_string()),
            Some(row.series_id.to_string()),
            Some(row.date.to_string()),
            Some(row.revision_date.to_string()),
            row.value.as_ref().map(ToString::to_string),
            Some(row.is_original_release.to_string()),
            (!attrs.is_null()).then(|| attrs.to_string()),
        ];
        lines.push(serde_json::to_string(&values)?);
    }
    lines.sort();
    let bytes = format!("{}\n", lines.join("\n"));
    let digest: String = Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let report = json!({"status":"passed","disposable_target":disposable_target,"requires_conditional_create":false,"metadata_location":prepared.next.metadata_location,"rows":read.len(),"sha256":digest,"canonical_columns":canonical_columns,"sql_catalog_commit":published,"dataset_id":dataset,"commit_id":prepared.next.commit_id});
    let output = output.or_else(|| std::env::var_os("SMOKE_OUTPUT_JSON").map(Into::into));
    if let Some(path) = output {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}

async fn maintenance(
    max_rows: u64,
    apply: bool,
    expire_before: Option<DateTime<Utc>>,
    retain_last: usize,
) -> Result<()> {
    anyhow::ensure!(
        max_rows > 0,
        "maintenance needs an explicit positive row budget"
    );
    let url = std::env::var("DATA_DATABASE_URL").context("DATA_DATABASE_URL is required")?;
    let catalog = SqlCatalog::new(create_pool(&url).await?, StorageConfig::from_env()?);
    let selection = econ_graph_core::observation_storage::ObservationStorage::from_env()?;
    if apply {
        anyhow::ensure!(
            selection.target_id.is_some(),
            "maintenance publication requires explicit TIMESERIES_TARGET_ID"
        );
    }
    let mut reports = vec![];
    for pointer in catalog.checkpoint().await?.tables {
        let plan = catalog.maintenance_plan(pointer.dataset_id).await?;
        let mut compacted = false;
        let mut expired = false;
        if apply && plan.compact_recommended {
            let prepared = catalog
                .prepare_compaction(Uuid::new_v4(), pointer.dataset_id, max_rows)
                .await?;
            let mut conn = catalog.pool.get().await?;
            compacted = conn
                .build_transaction()
                .run::<_, anyhow::Error, _>(async |conn| {
                    selection.assert_write_allowed_conn(conn).await?;
                    SqlCatalog::commit_prepared_conn(conn, &prepared).await
                })
                .await?;
        }
        if apply && let Some(cutoff) = expire_before {
            let prepared = catalog
                .prepare_snapshot_expiry(
                    Uuid::new_v4(),
                    pointer.dataset_id,
                    cutoff.timestamp_millis(),
                    retain_last,
                )
                .await?;
            let mut conn = catalog.pool.get().await?;
            expired = conn
                .build_transaction()
                .run::<_, anyhow::Error, _>(async |conn| {
                    selection.assert_write_allowed_conn(conn).await?;
                    SqlCatalog::commit_prepared_conn(conn, &prepared).await
                })
                .await?;
        }
        reports.push(json!({"plan":plan,"dry_run":!apply,"compacted":compacted,"snapshot_metadata_expired":expired,"objects_deleted":0,"current":catalog.pointer(pointer.dataset_id).await?}));
    }
    println!(
        "{}",
        serde_json::to_string(&json!({"format":1,"status":"passed","tables":reports}))?
    );
    Ok(())
}
