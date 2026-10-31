//! Real storage bridge for the reversible recovery toolbox. Nothing selects a backend merely
//! by preparing/copying objects; imports require an exact durable paused target and token.
use anyhow::{ensure, Context, Result};
use bigdecimal::BigDecimal;
use clap::{Parser, Subcommand};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Jsonb, Nullable, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncPgConnection, RunQueryDsl, SimpleAsyncConnection};
use econ_graph_core::{
    models::{DataPoint, Observation},
    schema::{data_points, datasets, economic_series},
};
use econ_graph_timeseries::{encoding, CatalogCheckpoint, SqlCatalog, StorageConfig};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::OpenOptions,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use uuid::Uuid;

const LOCK: i64 = 1162303316;
#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    ExportIceberg {
        #[arg(long)]
        expect_target: Uuid,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        sql_snapshot: Option<String>,
    },
    Bootstrap {
        #[arg(long)]
        expect_target: Uuid,
        #[arg(long)]
        pause_token: Uuid,
        #[arg(long)]
        epoch: i64,
    },
    ApplyTransition {
        #[arg(long)]
        expect_target: Uuid,
    },
    ImportBatch {
        #[arg(long)]
        expect_target: Uuid,
    },
    /// Copy only the catalog-reachable closure, including all retained metadata/history.
    CaptureObjects {
        #[arg(long)]
        expect_target: Uuid,
        #[arg(long)]
        output_root: PathBuf,
        #[arg(long)]
        sql_snapshot: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, QueryableByName)]
struct Fence {
    #[diesel(sql_type=SqlUuid)]
    target_id: Uuid,
    #[diesel(sql_type=Text)]
    backend: String,
    #[diesel(sql_type=BigInt)]
    epoch: i64,
    #[diesel(sql_type=Bool)]
    writes_paused: bool,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pause_token: Option<Uuid>,
}
#[derive(Deserialize)]
struct ImportRequest {
    format: u32,
    batch_id: Uuid,
    target_id: Uuid,
    source_checkpoint: String,
    dataset_id: Uuid,
    row_count: usize,
    content_sha256: String,
    observations: Vec<Value>,
    fence: Fence,
}
#[derive(QueryableByName)]
struct Metadata {
    #[diesel(sql_type=Jsonb)]
    value: Value,
}
async fn guard(
    conn: &mut AsyncPgConnection,
    expected: Uuid,
    exclusive: bool,
    imported_snapshot: bool,
) -> Result<Fence> {
    if !imported_snapshot {
        let name = if exclusive {
            "pg_advisory_xact_lock"
        } else {
            "pg_advisory_xact_lock_shared"
        };
        conn.batch_execute(&format!("SELECT {name}({LOCK})"))
            .await?;
    }
    let state = diesel::sql_query("SELECT target_id,backend,epoch,writes_paused,pause_token FROM timeseries_recovery_state WHERE singleton")
        .get_result::<Fence>(conn).await?;
    ensure!(state.target_id == expected, "recovery target mismatch");
    Ok(state)
}
async fn import_snapshot(conn: &mut AsyncPgConnection, value: Option<&str>) -> Result<()> {
    if let Some(snapshot) = value {
        ensure!(
            !snapshot.is_empty() && snapshot.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "invalid exported SQL snapshot identifier"
        );
        conn.batch_execute(&format!("SET TRANSACTION SNAPSHOT '{snapshot}'"))
            .await?;
    }
    Ok(())
}
fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn decimal_key(value: &str) -> Result<Value> {
    let number = BigDecimal::from_str(value)?;
    let (coefficient, scale) = number.as_bigint_and_exponent();
    let mut digits = coefficient.to_string();
    let mut exponent = scale.checked_neg().context("decimal exponent overflow")?;
    if coefficient.to_string() == "0" {
        return Ok(json!(["0", 0]));
    }
    while digits.ends_with('0') {
        digits.pop();
        exponent = exponent
            .checked_add(1)
            .context("decimal exponent overflow")?;
    }
    Ok(json!([digits, exponent]))
}
fn canonical(value: &Value) -> Result<Value> {
    Ok(match value {
        Value::Null => json!(["null"]),
        Value::Bool(v) => json!(["bool", v]),
        Value::Number(v) => {
            let parts = decimal_key(&v.to_string())?;
            json!(["number", parts[0], parts[1]])
        }
        Value::String(v) => json!(["string", v]),
        Value::Array(v) => json!([
            "array",
            v.iter().map(canonical).collect::<Result<Vec<_>>>()?
        ]),
        Value::Object(v) => {
            let sorted: BTreeMap<_, _> = v.iter().collect();
            let items = sorted
                .into_iter()
                .map(|(k, v)| Ok(json!([k, canonical(v)?])))
                .collect::<Result<Vec<_>>>()?;
            json!(["object", items])
        }
    })
}
fn request_hash(rows: &[Value]) -> Result<String> {
    Ok(sha(&serde_json::to_vec(&canonical(&json!(rows))?)?))
}
async fn import_batch(catalog: &SqlCatalog, expected: Uuid) -> Result<Value> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: ImportRequest = serde_json::from_str(&input)?;
    ensure!(
        request.format == 1
            && request.target_id == expected
            && !request.source_checkpoint.is_empty(),
        "batch target/format/checkpoint mismatch"
    );
    ensure!(
        !request.observations.is_empty() && request.row_count == request.observations.len(),
        "empty/truncated import batch"
    );
    ensure!(
        request.fence.target_id == expected
            && request.fence.writes_paused
            && request.fence.pause_token.is_some(),
        "batch needs its exact paused write fence"
    );
    ensure!(
        request_hash(&request.observations)? == request.content_sha256,
        "batch content checksum mismatch"
    );
    let allowed: BTreeSet<_> = [
        "id",
        "series_id",
        "dataset_id",
        "date",
        "revision_date",
        "value",
        "is_original_release",
        "created_at",
        "updated_at",
        "attributes",
    ]
    .into_iter()
    .collect();
    let mut rows = vec![];
    for raw in &request.observations {
        let object = raw.as_object().context("observation must be an object")?;
        ensure!(
            object.keys().all(|k| allowed.contains(k.as_str()))
                && allowed.iter().all(|k| object.contains_key(*k)),
            "unknown or missing observation fields"
        );
        ensure!(
            raw["dataset_id"].as_str() == Some(request.dataset_id.to_string().as_str()),
            "batch dataset mismatch"
        );
        ensure!(
            raw["value"].is_null() || raw["value"].is_string(),
            "decimal value must be an exact string or null"
        );
        rows.push(serde_json::from_value::<Observation>(raw.clone())?);
    }
    let mut conn = catalog.pool.get().await?;
    conn.build_transaction().run::<_,anyhow::Error,_>(async |conn| {
        let state=guard(conn,expected,true,false).await?;
        ensure!(state.writes_paused && state.pause_token==request.fence.pause_token && state.epoch==request.fence.epoch,"stale paused import fence");
        let stored=data_points::table.filter(data_points::id.eq_any(rows.iter().map(|r|r.id).collect::<Vec<_>>())).select(DataPoint::as_select()).load::<DataPoint>(conn).await?;
        let mut stored:Vec<Observation>=stored.into_iter().map(Into::into).collect();
        ensure!(stored.len()==rows.len(),"retain/restore the PostgreSQL source rows before Iceberg backfill");
        // The retained PostgreSQL shadow is the source of truth at this drained checkpoint.
        // A stale export cannot overwrite newer shadow content during a resumed backfill.
        stored.sort_by_key(|r|r.id);
        let mut expected_rows=rows.clone(); expected_rows.sort_by_key(|r|r.id);
        ensure!(encoding::canonical_rows(&stored)?==encoding::canonical_rows(&expected_rows)?,"source export differs from paused PostgreSQL history");
        let prepared=catalog.prepare_append_conn(conn,request.batch_id,request.dataset_id,&rows).await?;
        let published=SqlCatalog::commit_prepared_conn(conn,&prepared).await?;
        Ok(json!({"format":1,"committed":true,"already_committed":!published,"batch_id":request.batch_id,
            "target_id":expected,"source_checkpoint":request.source_checkpoint,"dataset_id":request.dataset_id,
            "row_count":request.row_count,"content_sha256":request.content_sha256,"storage_content_sha256":prepared.content_sha256,
            "catalog_checkpoint":SqlCatalog::checkpoint_conn(conn).await?,"epoch":state.epoch}))
    }).await
}
async fn metadata(conn: &mut AsyncPgConnection, table: &str) -> Result<Value> {
    // Callers pass the two fixed table names below, never operator-provided SQL names.
    Ok(diesel::sql_query(format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY id),'[]'::jsonb) AS value FROM {table} t"
    ))
    .get_result::<Metadata>(conn)
    .await?
    .value)
}
async fn export(
    catalog: &SqlCatalog,
    expected: Uuid,
    output: &Path,
    sql_snapshot: Option<&str>,
) -> Result<Value> {
    let mut conn = catalog.pool.get().await?;
    conn.build_transaction().repeatable_read().read_only().run::<_,anyhow::Error,_>(async |conn| {
        import_snapshot(conn,sql_snapshot).await?;
        conn.batch_execute("SET LOCAL TIME ZONE 'UTC'").await?;
        let fence=guard(conn,expected,false,sql_snapshot.is_some()).await?;
        let checkpoint=SqlCatalog::checkpoint_conn(conn).await?;
        let datasets=metadata(conn,"datasets").await?;
        let series=metadata(conn,"economic_series").await?;
        let checkpoint_hash=sha(&serde_json::to_vec(&checkpoint)?);
        let partial=output.with_extension(format!("partial-{}",Uuid::new_v4()));
        let file=OpenOptions::new().write(true).create_new(true).open(&partial)?;
        let mut stream=BufWriter::new(file);
        writeln!(stream,"{}",json!({"format":1,"kind":"snapshot","target_id":expected,"checkpoint":checkpoint_hash,
            "fence":fence,"datasets":datasets,"series":series,"catalog_checkpoint":checkpoint}))?;
        let mut count=0usize;
        for pointer in &checkpoint.tables {
            for row in catalog.read_rows(pointer).await? {
                let mut value=serde_json::to_value(&row)?;
                value.as_object_mut().context("observation serialization")?.insert("dataset_id".into(),json!(pointer.dataset_id));
                writeln!(stream,"{value}")?; count+=1;
            }
        }
        stream.flush()?; stream.get_ref().sync_all()?;
        std::fs::hard_link(&partial,output)?; std::fs::remove_file(&partial)?;
        Ok(json!({"format":1,"target_id":expected,"checkpoint":checkpoint_hash,"rows":count}))
    }).await
}
fn object_relative(warehouse: &str, location: &str) -> Result<PathBuf> {
    let prefix = format!("{}/", warehouse.trim_end_matches('/'));
    let location = location
        .strip_prefix(&prefix)
        .context("catalog object is outside the configured warehouse namespace")?;
    let path = Path::new(location);
    ensure!(
        !path.as_os_str().is_empty()
            && path
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "invalid recovery object key"
    );
    Ok(path.to_path_buf())
}
async fn capture_objects(
    catalog: &SqlCatalog,
    expected: Uuid,
    output_root: &Path,
    snapshot: &str,
) -> Result<Value> {
    let mut conn = catalog.pool.get().await?;
    conn.build_transaction().repeatable_read().read_only().run::<_,anyhow::Error,_>(async |conn| {
        import_snapshot(conn,Some(snapshot)).await?;
        let fence=guard(conn,expected,false,true).await?;
        ensure!(fence.writes_paused,"consistent recovery copy requires a durable paused target");
        let checkpoint:CatalogCheckpoint=SqlCatalog::checkpoint_conn(conn).await?;
        std::fs::create_dir(output_root)?;
        let io=catalog.config.file_io()?;
        let mut copied=BTreeMap::new();
        for pointer in &checkpoint.tables {
            for object in catalog.reachable_objects(pointer).await? {
                let relative=object_relative(&catalog.config.warehouse,&object.location)?;
                let name=relative.to_str().context("non UTF-8 object key")?.to_string();
                if let Some(existing)=copied.get(&name) {
                    ensure!(existing==&object.sha256,"same object key has inconsistent hashes"); continue;
                }
                let target=output_root.join(&relative);
                std::fs::create_dir_all(target.parent().context("object parent")?)?;
                let mut destination=OpenOptions::new().write(true).create_new(true).open(target)?;
                let reader=io.new_input(&object.location)?.reader().await?;
                let mut digest=Sha256::new(); let mut offset=0u64;
                while offset<object.size_bytes {
                    let end=offset.saturating_add(8*1024*1024).min(object.size_bytes);
                    let bytes=reader.read(offset..end).await?;
                    ensure!(bytes.len() as u64==end-offset,"truncated object range");
                destination.write_all(&bytes)?; digest.update(&bytes); offset=end;
                }
                destination.sync_all()?;
                let actual:String=digest.finalize().iter().map(|b|format!("{b:02x}")).collect();
                ensure!(actual==object.sha256,"reachable object changed during recovery copy");
                copied.insert(name,actual);
            }
        }
        let mut objects=Vec::with_capacity(copied.len());
        for (path,sha256) in copied {
            let bytes=std::fs::metadata(output_root.join(&path))?.len();
            objects.push(json!({"bytes":bytes,"path":path,"sha256":sha256}));
        }
        Ok(json!({"format":1,"target_id":expected,"checkpoint":sha(&serde_json::to_vec(&checkpoint)?),
            "catalog_checkpoint":checkpoint,"warehouse":catalog.config.warehouse,"objects":objects,"fence":fence}))
    }).await
}

async fn bootstrap(catalog: &SqlCatalog, expected: Uuid, token: Uuid, epoch: i64) -> Result<Value> {
    let mut conn = catalog.pool.get().await?;
    conn.build_transaction()
        .run::<_, anyhow::Error, _>(async |conn| {
            let state = guard(conn, expected, true, false).await?;
            ensure!(
                state.writes_paused && state.pause_token == Some(token) && state.epoch == epoch,
                "stale paused bootstrap fence"
            );
            let ids = datasets::table
                .select(datasets::id)
                .load::<Uuid>(conn)
                .await?;
            for id in ids {
                catalog.ensure_dataset_conn(conn, id).await?;
            }
            Ok(json!({"format":1,"target_id":expected,"bootstrapped":true,"epoch":epoch}))
        })
        .await
}
async fn current_parity(
    catalog: &SqlCatalog,
    conn: &mut AsyncPgConnection,
    checkpoint: &CatalogCheckpoint,
) -> Result<()> {
    let ids = datasets::table
        .select(datasets::id)
        .load::<Uuid>(conn)
        .await?;
    for dataset in ids {
        let postgres = data_points::table
            .inner_join(economic_series::table)
            .filter(economic_series::dataset_id.eq(dataset))
            .select(DataPoint::as_select())
            .load::<DataPoint>(conn)
            .await?;
        let postgres: Vec<Observation> = postgres.into_iter().map(Into::into).collect();
        let pointer = checkpoint
            .tables
            .iter()
            .find(|p| p.dataset_id == dataset)
            .context("bootstrap every dataset before a backend transition")?;
        let iceberg = catalog.read_rows(pointer).await?;
        ensure!(
            encoding::canonical_rows(&postgres)? == encoding::canonical_rows(&iceberg)?,
            "current PostgreSQL shadow/Iceberg parity failed for dataset {dataset}"
        );
    }
    Ok(())
}
async fn apply_transition(catalog: &SqlCatalog, expected: Uuid) -> Result<Value> {
    let mut bytes = String::new();
    std::io::stdin().read_to_string(&mut bytes)?;
    let plan: Value = serde_json::from_str(&bytes)?;
    ensure!(
        plan["format"] == 1 && plan["kind"] == "transition-plan" && plan["apply"] == false,
        "requires a validated transition plan"
    );
    ensure!(
        plan["target_id"].as_str() == Some(expected.to_string().as_str()),
        "transition target mismatch"
    );
    let action = plan["action"].as_str().context("transition action")?;
    let (old_backend, new_backend) = match action {
        "cutover" => ("postgres", "iceberg"),
        "rollback" => ("iceberg", "postgres"),
        _ => anyhow::bail!("unsupported backend transition"),
    };
    ensure!(
        plan["rollback_strategy"] == "retained_atomic_postgres_shadow"
            && plan["recovery_policy"]["approved"] == true,
        "requires approved recovery objectives and retained write shadow"
    );
    let evidence = plan["evidence_sha256"]
        .as_object()
        .context("acceptance evidence hashes")?;
    for key in [
        "parity",
        "restore-drill",
        "rollback-drill",
        "release-e2e",
        "load-capacity",
    ] {
        let value = evidence
            .get(key)
            .and_then(Value::as_str)
            .context("missing exact acceptance evidence")?;
        ensure!(
            value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()),
            "invalid acceptance evidence hash"
        );
    }
    let wanted: Fence = serde_json::from_value(plan["fence"].clone())?;
    let expected_checkpoint: CatalogCheckpoint =
        serde_json::from_value(plan["catalog_checkpoint"].clone())
            .context("parity must retain exact catalog pointers")?;
    let mut conn = catalog.pool.get().await?;
    conn.build_transaction().run::<_,anyhow::Error,_>(async |conn| {
        let state=guard(conn,expected,true,false).await?;
        ensure!(state.writes_paused && state.pause_token.is_some() && state.pause_token==wanted.pause_token && state.epoch==wanted.epoch,"stale transition pause fence");
        ensure!(state.backend==old_backend,"backend already changed; inspect the durable transition ledger");
        let checkpoint=SqlCatalog::checkpoint_conn(conn).await?;
        ensure!(serde_json::to_value(&checkpoint)?==serde_json::to_value(&expected_checkpoint)?,"catalog changed after acceptance parity");
        current_parity(catalog,conn,&checkpoint).await?;
        let next=state.epoch.checked_add(1).context("recovery epoch exhausted")?;
        diesel::sql_query("UPDATE timeseries_recovery_state SET backend=$1,epoch=$2,checkpoint=$3,updated_at=now() WHERE singleton")
            .bind::<Text,_>(new_backend).bind::<BigInt,_>(next).bind::<Jsonb,_>(serde_json::to_value(&checkpoint)?).execute(conn).await?;
        diesel::sql_query("INSERT INTO timeseries_recovery_transitions(target_id,epoch,action,evidence) VALUES($1,$2,$3,$4)")
            .bind::<SqlUuid,_>(expected).bind::<BigInt,_>(next).bind::<Text,_>(action).bind::<Jsonb,_>(&plan).execute(conn).await?;
        Ok(json!({"format":1,"committed":true,"action":action,"target_id":expected,"backend":new_backend,"epoch":next,"writes_paused":true,"pause_token":state.pause_token,"catalog_checkpoint":checkpoint,"requires_matching_runtime_then_explicit_resume":true}))
    }).await
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let url = std::env::var("DATA_DATABASE_URL").context("explicit DATA_DATABASE_URL required")?;
    let catalog = SqlCatalog::new(
        econ_graph_core::create_pool(&url).await?,
        StorageConfig::from_env()?,
    );
    let result = match args.command {
        Command::ExportIceberg {
            expect_target,
            output,
            sql_snapshot,
        } => export(&catalog, expect_target, &output, sql_snapshot.as_deref()).await?,
        Command::Bootstrap {
            expect_target,
            pause_token,
            epoch,
        } => bootstrap(&catalog, expect_target, pause_token, epoch).await?,
        Command::ApplyTransition { expect_target } => {
            apply_transition(&catalog, expect_target).await?
        }
        Command::ImportBatch { expect_target } => import_batch(&catalog, expect_target).await?,
        Command::CaptureObjects {
            expect_target,
            output_root,
            sql_snapshot,
        } => capture_objects(&catalog, expect_target, &output_root, &sql_snapshot).await?,
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_request_hash_does_not_narrow_unknown_numbers() {
        let input: Value = serde_json::from_str(
            r#"[{"attributes":{"x":12345678901234567890.1234567890123456789},"value":"1.00"}]"#,
        )
        .unwrap();
        assert_eq!(
            input[0]["attributes"]["x"].to_string(),
            "12345678901234567890.1234567890123456789"
        );
        assert_eq!(decimal_key("1e1000000").unwrap(), json!(["1", 1000000]));
        let canonical = canonical(&input).unwrap();
        assert!(serde_json::to_string(&canonical)
            .unwrap()
            .contains("123456789012345678901234567890123456789"));
    }
    #[test]
    fn recovery_objects_cannot_escape_or_change_namespace() {
        assert!(object_relative("s3://bucket/data", "s3://bucket/data/table/a.parquet").is_ok());
        assert!(object_relative("s3://bucket/data", "s3://bucket/data2/a").is_err());
        assert!(object_relative("/warehouse", "/warehouse/../secret").is_err());
    }
}
