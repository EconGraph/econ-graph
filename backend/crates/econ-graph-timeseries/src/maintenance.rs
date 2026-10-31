//! Explicit, bounded maintenance using native Apache Iceberg metadata/manifests. No object,
//! backup, or retained observation revision is deleted by these operations.
use crate::{PreparedCommit, SqlCatalog, TablePointer};
use anyhow::{bail, Context, Result};
use arrow_array::{Array, Int64Array};
use chrono::Utc;
use iceberg::{
    spec::{ManifestListReader, ManifestListWriter, Operation, Snapshot, Summary},
    transaction::{ApplyTransactionAction, Transaction},
    Catalog, MetadataLocation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenancePlan {
    pub pointer: TablePointer,
    pub logical_rows: u64,
    pub physical_rows: u64,
    pub series: u64,
    pub retained_snapshots: usize,
    pub compact_recommended: bool,
}
impl SqlCatalog {
    pub async fn maintenance_plan(&self, dataset_id: Uuid) -> Result<MaintenancePlan> {
        let pointer = self
            .pointer(dataset_id)
            .await?
            .context("dataset has no Iceberg catalog")?;
        let table = self.load_at(&pointer).await?;
        let snapshots = table.metadata().snapshots().len();
        let ctx = crate::query::context()?;
        ctx.register_table(
            "observations",
            Arc::new(
                iceberg_datafusion::IcebergStaticTableProvider::try_new_from_table(table).await?,
            ),
        )?;
        let batches=ctx.sql("SELECT count(*) AS physical_rows,count(DISTINCT id) AS logical_rows,count(DISTINCT series_id) AS series FROM observations").await?.collect().await?;
        let first = batches.first().context("missing maintenance aggregate")?;
        let count = |name: &str| -> Result<u64> {
            let value = first
                .column_by_name(name)
                .context("missing maintenance count")?
                .as_any()
                .downcast_ref::<Int64Array>()
                .context("invalid maintenance count")?;
            Ok(value.value(0).try_into()?)
        };
        let physical_rows = count("physical_rows")?;
        let logical_rows = count("logical_rows")?;
        Ok(MaintenancePlan {
            pointer,
            logical_rows,
            physical_rows,
            series: count("series")?,
            retained_snapshots: snapshots,
            compact_recommended: physical_rows > logical_rows,
        })
    }
    /// Rewrite the current logical rows into a replacement snapshot. Old snapshots and their
    /// objects remain intact; publication still requires caller transaction + pointer CAS.
    pub async fn prepare_compaction(
        &self,
        commit_id: Uuid,
        dataset_id: Uuid,
        max_rows: u64,
    ) -> Result<PreparedCommit> {
        if max_rows == 0 {
            bail!("compaction requires an explicit positive row budget")
        }
        let plan = self.maintenance_plan(dataset_id).await?;
        if plan.logical_rows > max_rows {
            bail!("compaction exceeds the explicit row budget")
        }
        let rows = self.read_rows(&plan.pointer).await?;
        if rows.is_empty() {
            bail!("empty dataset does not need compaction")
        }
        let staged = self.prepare_append(commit_id, dataset_id, &rows).await?;
        if staged.base != plan.pointer {
            bail!("catalog changed during compaction staging; retry from a new checkpoint")
        }
        let base = self.load_at(&staged.base).await?;
        let append = self.load_at(&staged.next).await?;
        let current = append
            .metadata()
            .current_snapshot()
            .context("missing staged snapshot")?;
        let reader = append.manifest_list_reader(current);
        let list = ManifestListReader::load(&reader).await?;
        let added: Vec<_> = list
            .entries()
            .iter()
            .filter(|m| m.added_snapshot_id == current.snapshot_id())
            .cloned()
            .collect();
        if added.is_empty() {
            bail!("compaction did not produce replacement manifests")
        }
        let data_files: u64 = added
            .iter()
            .map(|m| u64::from(m.added_files_count.unwrap_or(0)))
            .sum();
        let location = format!(
            "{}/metadata/compact-{}.avro",
            base.metadata().location(),
            Uuid::new_v4()
        );
        let mut writer = ManifestListWriter::v2(
            base.file_io().new_output(&location)?.writer().await?,
            current.snapshot_id(),
            current.parent_snapshot_id(),
            current.sequence_number(),
        );
        writer.add_manifests(added.into_iter())?;
        writer.close().await?;
        let snapshot = Snapshot::builder()
            .with_snapshot_id(current.snapshot_id())
            .with_parent_snapshot_id(current.parent_snapshot_id())
            .with_sequence_number(current.sequence_number())
            .with_timestamp_ms(current.timestamp_ms())
            .with_manifest_list(location)
            .with_schema_id(base.metadata().current_schema_id())
            .with_summary(Summary {
                operation: Operation::Replace,
                additional_properties: HashMap::from([
                    ("total-records".into(), rows.len().to_string()),
                    ("total-data-files".into(), data_files.to_string()),
                    ("econgraph.maintenance".into(), "compaction".into()),
                ]),
            })
            .build();
        let metadata = base
            .metadata()
            .clone()
            .into_builder(Some(staged.base.metadata_location.clone()))
            .set_branch_snapshot(snapshot, "main")?
            .build()?
            .metadata;
        let metadata_location = MetadataLocation::from_str(&staged.base.metadata_location)?
            .with_next_version()
            .with_new_metadata(&metadata);
        metadata
            .write_to(base.file_io(), &metadata_location)
            .await?;
        let mut prepared = staged;
        prepared.next.metadata_location = metadata_location.to_string();
        // Bind the receipt to both operation and logical content, so an append UUID cannot be
        // reused to claim a maintenance rewrite.
        prepared.content_sha256 =
            Sha256::digest(format!("compact:{}", prepared.content_sha256).as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        Ok(prepared)
    }
    /// Expire snapshot references in new metadata only. All objects/SQL commit receipts are
    /// retained, and observation vintages remain in the current logical rows. An operator
    /// supplies cutoff and retention explicitly; upstream default age expiry is never used.
    pub async fn prepare_snapshot_expiry(
        &self,
        commit_id: Uuid,
        dataset_id: Uuid,
        older_than_ms: i64,
        retain_last: usize,
    ) -> Result<PreparedCommit> {
        if retain_last == 0 || older_than_ms > Utc::now().timestamp_millis() {
            bail!("expiry requires a past cutoff and retain_last >= 1")
        }
        let base = self
            .pointer(dataset_id)
            .await?
            .context("dataset has no Iceberg catalog")?;
        let staging = self.config.staging_catalog().await?;
        let table = self.load_at(&base).await?;
        let ident = table.identifier().clone();
        staging
            .create_namespace(ident.namespace(), HashMap::new())
            .await?;
        let table = staging
            .register_table(&ident, base.metadata_location.clone())
            .await?;
        let tx = Transaction::new(&table);
        let tx = tx
            .expire_snapshots()
            .expire_older_than_ms(older_than_ms)
            .retain_last(retain_last)
            .apply(tx)?;
        let table = tx.commit(&staging).await?;
        let content_sha256 =
            Sha256::digest(format!("expire:{dataset_id}:{older_than_ms}:{retain_last}").as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        Ok(PreparedCommit {
            next: TablePointer {
                dataset_id,
                metadata_location: table.metadata_location_result()?.into(),
                generation: base.generation + 1,
                commit_id,
            },
            base,
            rows: 0,
            content_sha256,
        })
    }
}
