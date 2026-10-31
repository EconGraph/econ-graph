use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use clap::{Parser, Subcommand};
use diesel_async::SimpleAsyncConnection;
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
    /// Write genuine Iceberg/Parquet via the same library used by the crawl commit path.
    /// Requires an explicitly disposable database and warehouse; never deletes objects.
    Smoke {
        #[arg(long)]
        output: Option<std::path::PathBuf>,
        #[arg(long)]
        migrate: bool,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    match Args::parse().command {
        Command::Smoke { output, migrate } => smoke(output, migrate).await,
    }
}
async fn smoke(output: Option<std::path::PathBuf>, migrate: bool) -> Result<()> {
    let url = std::env::var("DATA_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .context("DATA_DATABASE_URL is required")?;
    if migrate {
        run_migration_set(&url, MigrationSet::Data).await?;
    }
    let catalog = SqlCatalog::new(create_pool(&url).await?, StorageConfig::from_env()?);
    let source = Uuid::new_v4();
    let dataset = Uuid::new_v4();
    let series = Uuid::new_v4();
    let mut conn = catalog.pool.get().await?;
    conn.batch_execute(&format!("INSERT INTO data_sources(id,name,description,base_url) VALUES('{source}','Iceberg smoke {source}','disposable test','https://example.test'); INSERT INTO datasets(id,source_id,code,name) VALUES('{dataset}','{source}','smoke','Iceberg smoke'); INSERT INTO economic_series(id,source_id,external_id,title,frequency,dataset_id) VALUES('{series}','{source}','smoke','Iceberg smoke','monthly','{dataset}')")).await?;
    drop(conn);
    let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let rows:Vec<Observation>=[("2024-01-01","2024-01-01",Some("999"),true),("2024-01-01","2024-02-01",Some("123456789012345678901234567890123456789012345678901.123456789123456789"),true),("2024-01-01","2024-03-01",None,false),("2024-02-01","2024-02-01",Some("0"),true),("2024-03-01","2024-03-01",Some("0.0000000000000000000000000000000000000000000000000000000000000001"),true)]
        .into_iter().enumerate().map(|(i,(date,revision,value,original))|serde_json::from_value(json!({"id":Uuid::new_v5(&series,&i.to_be_bytes()),"series_id":series,"date":date,"revision_date":revision,"value":value,"is_original_release":original,"created_at":timestamp,"updated_at":timestamp,"attributes":{}}))).collect::<std::result::Result<_,_>>()?;
    let prepared = catalog
        .prepare_append(Uuid::new_v4(), dataset, &rows)
        .await?;
    let mut conn = catalog.pool.get().await?;
    let published = conn
        .build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            SqlCatalog::commit_prepared_conn(conn, &prepared).await
        })
        .await?;
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
    let report = json!({"status":"passed","requires_conditional_create":false,"metadata_location":prepared.next.metadata_location,"rows":read.len(),"sha256":digest,"canonical_columns":canonical_columns,"sql_catalog_commit":published,"dataset_id":dataset,"commit_id":prepared.next.commit_id});
    let output = output.or_else(|| std::env::var_os("SMOKE_OUTPUT_JSON").map(Into::into));
    if let Some(path) = output {
        std::fs::write(path, serde_json::to_vec_pretty(&report)?)?;
    }
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
