//! Explicit, bounded maintenance using native Apache Iceberg metadata/manifests. No object,
//! backup, or retained observation revision is deleted by these operations.
use crate::{PreparedCommit, SqlCatalog, TablePointer};
use anyhow::{bail, Context, Result};
use arrow_array::{Array, Int64Array};
use async_trait::async_trait;
use chrono::Utc;
use iceberg::{
    memory::MemoryCatalog,
    spec::{ManifestListReader, ManifestListWriter, Operation, Snapshot, Summary},
    table::Table,
    transaction::{ApplyTransactionAction, Transaction},
    Catalog, MetadataLocation, Namespace, NamespaceIdent, TableCommit, TableCreation, TableIdent,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use uuid::Uuid;

/// Native expiry computes the complete retention plan. Its memory catalog otherwise writes
/// a new metadata version even for an empty update list; intercept only that genuine no-op.
/// Nonempty updates, including reference expiry, use the public native metadata updates.
#[derive(Debug)]
struct ExpiryCatalog(MemoryCatalog);

#[async_trait]
impl Catalog for ExpiryCatalog {
    async fn list_namespaces(
        &self,
        parent: Option<&NamespaceIdent>,
    ) -> iceberg::Result<Vec<NamespaceIdent>> {
        self.0.list_namespaces(parent).await
    }
    async fn create_namespace(
        &self,
        namespace: &NamespaceIdent,
        properties: HashMap<String, String>,
    ) -> iceberg::Result<Namespace> {
        self.0.create_namespace(namespace, properties).await
    }
    async fn get_namespace(&self, namespace: &NamespaceIdent) -> iceberg::Result<Namespace> {
        self.0.get_namespace(namespace).await
    }
    async fn namespace_exists(&self, namespace: &NamespaceIdent) -> iceberg::Result<bool> {
        self.0.namespace_exists(namespace).await
    }
    async fn update_namespace(
        &self,
        namespace: &NamespaceIdent,
        properties: HashMap<String, String>,
    ) -> iceberg::Result<()> {
        self.0.update_namespace(namespace, properties).await
    }
    async fn drop_namespace(&self, namespace: &NamespaceIdent) -> iceberg::Result<()> {
        self.0.drop_namespace(namespace).await
    }
    async fn list_tables(&self, namespace: &NamespaceIdent) -> iceberg::Result<Vec<TableIdent>> {
        self.0.list_tables(namespace).await
    }
    async fn create_table(
        &self,
        namespace: &NamespaceIdent,
        creation: TableCreation,
    ) -> iceberg::Result<Table> {
        self.0.create_table(namespace, creation).await
    }
    async fn load_table(&self, table: &TableIdent) -> iceberg::Result<Table> {
        self.0.load_table(table).await
    }
    async fn drop_table(&self, table: &TableIdent) -> iceberg::Result<()> {
        self.0.drop_table(table).await
    }
    async fn purge_table(&self, table: &TableIdent) -> iceberg::Result<()> {
        self.0.purge_table(table).await
    }
    async fn table_exists(&self, table: &TableIdent) -> iceberg::Result<bool> {
        self.0.table_exists(table).await
    }
    async fn rename_table(&self, src: &TableIdent, dest: &TableIdent) -> iceberg::Result<()> {
        self.0.rename_table(src, dest).await
    }
    async fn register_table(
        &self,
        table: &TableIdent,
        metadata_location: String,
    ) -> iceberg::Result<Table> {
        self.0.register_table(table, metadata_location).await
    }
    async fn update_table(&self, mut commit: TableCommit) -> iceberg::Result<Table> {
        let updates = commit.take_updates();
        let requirements = commit.take_requirements();
        let table = self.0.load_table(commit.identifier()).await?;
        self.apply_updates(table, requirements, updates).await
    }
}

impl ExpiryCatalog {
    async fn apply_updates(
        &self,
        table: Table,
        requirements: Vec<iceberg::TableRequirement>,
        updates: Vec<iceberg::TableUpdate>,
    ) -> iceberg::Result<Table> {
        for requirement in requirements {
            requirement.check(Some(table.metadata()))?;
        }
        if updates.is_empty() {
            return Ok(table);
        }
        // TableCommit cannot be rebuilt outside Iceberg. Apply its public native updates
        // exactly as TableCommit::apply does, after validating every requirement above.
        let mut builder = table
            .metadata()
            .clone()
            .into_builder(Some(table.metadata_location_result()?.into()));
        for update in updates {
            builder = update.apply(builder)?;
        }
        let metadata = builder.build()?.metadata;
        let location = MetadataLocation::from_str(table.metadata_location_result()?)?
            .with_next_version()
            .with_new_metadata(&metadata);
        metadata.write_to(table.file_io(), &location).await?;
        // This catalog is owned by one expiry transaction; no observer can see the
        // intermediate in-memory registration state. No object is deleted by drop_table.
        self.0.drop_table(table.identifier()).await?;
        self.0
            .register_table(table.identifier(), location.to_string())
            .await
    }
}

async fn expire_snapshot_table(
    config: &crate::StorageConfig,
    table: &Table,
    older_than_ms: i64,
    retain_last: usize,
) -> Result<Option<Table>> {
    let staging = ExpiryCatalog(config.staging_catalog().await?);
    let ident = table.identifier();
    staging
        .create_namespace(ident.namespace(), HashMap::new())
        .await?;
    let table = staging
        .register_table(ident, table.metadata_location_result()?.into())
        .await?;
    let previous_location = table.metadata_location_result()?.to_string();
    let tx = Transaction::new(&table);
    let tx = tx
        .expire_snapshots()
        .expire_older_than_ms(older_than_ms)
        .retain_last(retain_last)
        .apply(tx)?;
    let table = tx.commit(&staging).await?;
    if table.metadata_location_result()? == previous_location {
        return Ok(None);
    }
    Ok(Some(table))
}

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
    /// `None` means the native retention plan has no updates: no objects or SQL receipt are
    /// written, and callers must not publish a catalog generation for that no-op.
    pub async fn prepare_snapshot_expiry(
        &self,
        commit_id: Uuid,
        dataset_id: Uuid,
        older_than_ms: i64,
        retain_last: usize,
    ) -> Result<Option<PreparedCommit>> {
        if retain_last == 0 || older_than_ms > Utc::now().timestamp_millis() {
            bail!("expiry requires a past cutoff and retain_last >= 1")
        }
        let base = self
            .pointer(dataset_id)
            .await?
            .context("dataset has no Iceberg catalog")?;
        let table = self.load_at(&base).await?;
        let Some(table) =
            expire_snapshot_table(&self.config, &table, older_than_ms, retain_last).await?
        else {
            return Ok(None);
        };
        let content_sha256 =
            Sha256::digest(format!("expire:{dataset_id}:{older_than_ms}:{retain_last}").as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
        Ok(Some(PreparedCommit {
            next: TablePointer {
                dataset_id,
                metadata_location: table.metadata_location_result()?.into(),
                generation: base
                    .generation
                    .checked_add(1)
                    .context("catalog generation exhausted")?,
                commit_id,
            },
            base,
            rows: 0,
            content_sha256,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iceberg::spec::{SnapshotReference, SnapshotRetention};
    use iceberg::TableRequirement;
    use std::path::{Path, PathBuf};

    fn objects(path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                files.extend(objects(&entry.path())?);
            } else {
                files.push(entry.path());
            }
        }
        files.sort();
        Ok(files)
    }

    async fn fixture() -> Result<(
        tempfile::TempDir,
        crate::StorageConfig,
        MemoryCatalog,
        Table,
    )> {
        let temp = tempfile::tempdir()?;
        let config = crate::StorageConfig::local(temp.path().to_str().unwrap());
        let catalog = config.staging_catalog().await?;
        let namespace = NamespaceIdent::new("expiry".into());
        catalog.create_namespace(&namespace, HashMap::new()).await?;
        let table = catalog
            .create_table(
                &namespace,
                TableCreation::builder()
                    .name("snapshots".into())
                    .schema(crate::encoding::schema()?)
                    .build(),
            )
            .await?;
        let now = Utc::now().timestamp_millis();
        let mut builder = table
            .metadata()
            .clone()
            .into_builder(Some(table.metadata_location_result()?.into()));
        for id in 1..=2 {
            let snapshot = Snapshot::builder()
                .with_snapshot_id(id)
                .with_parent_snapshot_id(if id == 1 { None } else { Some(1) })
                .with_sequence_number(id)
                .with_timestamp_ms(now - (3 - id) * 1000)
                .with_manifest_list(format!("{}/unused-{id}.avro", config.warehouse))
                .with_schema_id(table.metadata().current_schema_id())
                .with_summary(Summary {
                    operation: Operation::Append,
                    additional_properties: HashMap::new(),
                })
                .build();
            builder = builder.set_branch_snapshot(snapshot, "main")?;
        }
        let metadata = builder.build()?.metadata;
        let table = write_fixture_metadata(&catalog, &table, metadata).await?;
        Ok((temp, config, catalog, table))
    }

    async fn write_fixture_metadata(
        catalog: &MemoryCatalog,
        table: &Table,
        metadata: iceberg::spec::TableMetadata,
    ) -> Result<Table> {
        let location = MetadataLocation::from_str(table.metadata_location_result()?)?
            .with_next_version()
            .with_new_metadata(&metadata);
        metadata.write_to(table.file_io(), &location).await?;
        catalog.drop_table(table.identifier()).await?;
        Ok(catalog
            .register_table(table.identifier(), location.to_string())
            .await?)
    }

    #[tokio::test]
    async fn expiry_noops_preserve_objects_for_age_retention_and_protected_refs() -> Result<()> {
        let (temp, config, catalog, mut table) = fixture().await?;
        let before = objects(temp.path())?;
        // More snapshots than retain_last, but none strictly older than the cutoff.
        let earliest = table
            .metadata()
            .snapshots()
            .map(|s| s.timestamp_ms())
            .min()
            .unwrap();
        for cutoff in [0, earliest] {
            assert!(expire_snapshot_table(&config, &table, cutoff, 1)
                .await?
                .is_none());
            assert_eq!(objects(temp.path())?, before);
        }
        // Age would expire the older snapshot, but explicit retention protects it.
        assert!(
            expire_snapshot_table(&config, &table, Utc::now().timestamp_millis(), 2)
                .await?
                .is_none()
        );
        assert_eq!(objects(temp.path())?, before);

        let metadata = table
            .metadata()
            .clone()
            .into_builder(Some(table.metadata_location_result()?.into()))
            .set_ref(
                "recovery",
                SnapshotReference::new(
                    1,
                    SnapshotRetention::Tag {
                        max_ref_age_ms: None,
                    },
                ),
            )?
            .build()?
            .metadata;
        table = write_fixture_metadata(&catalog, &table, metadata).await?;
        let tagged_objects = objects(temp.path())?;
        assert!(
            expire_snapshot_table(&config, &table, Utc::now().timestamp_millis(), 1)
                .await?
                .is_none()
        );
        assert_eq!(objects(temp.path())?, tagged_objects);
        Ok(())
    }

    #[tokio::test]
    async fn expiry_still_publishes_real_changes_and_repeated_expiry_is_noop() -> Result<()> {
        let (temp, config, _, table) = fixture().await?;
        let before = objects(temp.path())?;
        let expired = expire_snapshot_table(&config, &table, Utc::now().timestamp_millis(), 1)
            .await?
            .expect("older snapshot should expire");
        assert_eq!(expired.metadata().snapshots().len(), 1);
        assert_eq!(expired.metadata().current_snapshot_id(), Some(2));
        assert_ne!(
            expired.metadata_location_result()?,
            table.metadata_location_result()?
        );
        let after = objects(temp.path())?;
        assert_eq!(after.len(), before.len() + 1);
        assert!(
            before.iter().all(|path| after.contains(path)),
            "expiry never deletes objects"
        );
        assert!(
            expire_snapshot_table(&config, &expired, Utc::now().timestamp_millis(), 1)
                .await?
                .is_none()
        );
        assert_eq!(objects(temp.path())?, after);
        Ok(())
    }

    #[tokio::test]
    async fn expiry_empty_update_still_checks_commit_requirements() -> Result<()> {
        let (temp, _, catalog, table) = fixture().await?;
        let before = objects(temp.path())?;
        let staging = ExpiryCatalog(catalog);
        let requirements = vec![TableRequirement::UuidMatch {
            uuid: Uuid::new_v4(),
        }];
        assert!(staging
            .apply_updates(table, requirements, vec![])
            .await
            .is_err());
        assert_eq!(objects(temp.path())?, before);
        Ok(())
    }
}
