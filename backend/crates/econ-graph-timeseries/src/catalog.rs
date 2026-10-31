use crate::{config::StorageConfig, encoding};
use anyhow::{bail, Context, Result};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Text, Uuid as SqlUuid},
};
use diesel_async::{AnsiTransactionManager, AsyncPgConnection, RunQueryDsl, TransactionManager};
use econ_graph_core::{
    database::DatabasePool,
    models::{Dataset, EconomicSeries, Observation},
    schema::{datasets, economic_series},
};
use iceberg::{
    spec::{
        DataFileFormat, Literal, NullOrder, PartitionKey, SortDirection, SortField, SortOrder,
        Struct, Transform, UnboundPartitionSpec,
    },
    table::Table,
    transaction::{ApplyTransactionAction, Transaction},
    writer::{
        base_writer::data_file_writer::DataFileWriterBuilder,
        file_writer::{
            location_generator::{DefaultFileNameGenerator, DefaultLocationGenerator},
            rolling_writer::RollingFileWriterBuilder,
            ParquetWriterBuilder,
        },
        IcebergWriter, IcebergWriterBuilder,
    },
    Catalog, NamespaceIdent, TableCreation, TableIdent,
};
use parquet::file::properties::WriterProperties;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, QueryableByName)]
pub struct TablePointer {
    #[diesel(sql_type=SqlUuid)]
    pub dataset_id: Uuid,
    #[diesel(sql_type=Text)]
    pub metadata_location: String,
    #[diesel(sql_type=BigInt)]
    pub generation: i64,
    #[diesel(sql_type=SqlUuid)]
    pub commit_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogCheckpoint {
    pub format: u32,
    pub tables: Vec<TablePointer>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedCommit {
    pub base: TablePointer,
    pub next: TablePointer,
    /// Stored IDs and exact values written in this batch; useful for deterministic replay evidence.
    pub rows: usize,
    pub content_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReachableObject {
    pub location: String,
    pub size_bytes: u64,
    pub sha256: String,
}
fn require_transaction(conn: &mut AsyncPgConnection) -> Result<()> {
    if AnsiTransactionManager::transaction_manager_status_mut(conn)
        .transaction_depth()?
        .is_none()
    {
        bail!("catalog publication requires a caller-owned database transaction");
    }
    Ok(())
}
#[derive(Clone)]
pub struct SqlCatalog {
    pub pool: DatabasePool,
    pub config: StorageConfig,
}
impl SqlCatalog {
    pub fn new(pool: DatabasePool, config: StorageConfig) -> Self {
        Self { pool, config }
    }
    pub async fn pointer_conn(
        conn: &mut AsyncPgConnection,
        dataset_id: Uuid,
    ) -> Result<Option<TablePointer>> {
        Ok(diesel::sql_query("SELECT dataset_id,metadata_location,generation,commit_id FROM iceberg_catalog WHERE dataset_id=$1")
            .bind::<SqlUuid,_>(dataset_id).get_result(conn).await.optional()?)
    }
    pub async fn pointer(&self, dataset_id: Uuid) -> Result<Option<TablePointer>> {
        let mut conn = self.pool.get().await?;
        Self::pointer_conn(&mut conn, dataset_id).await
    }
    pub async fn checkpoint_conn(conn: &mut AsyncPgConnection) -> Result<CatalogCheckpoint> {
        let tables=diesel::sql_query("SELECT dataset_id,metadata_location,generation,commit_id FROM iceberg_catalog ORDER BY dataset_id").load::<TablePointer>(conn).await?;
        Ok(CatalogCheckpoint { format: 1, tables })
    }
    pub async fn checkpoint(&self) -> Result<CatalogCheckpoint> {
        let mut conn = self.pool.get().await?;
        conn.build_transaction()
            .repeatable_read()
            .read_only()
            .run::<_, anyhow::Error, _>(async |conn| Self::checkpoint_conn(conn).await)
            .await
    }
    fn ident(dataset_id: Uuid) -> TableIdent {
        TableIdent::new(
            NamespaceIdent::new("observations".into()),
            dataset_id.simple().to_string(),
        )
    }
    pub async fn load_at(&self, pointer: &TablePointer) -> Result<Table> {
        Self::load_at_config(&self.config, pointer).await
    }
    pub async fn load_at_config(config: &StorageConfig, pointer: &TablePointer) -> Result<Table> {
        let io = config.file_io()?;
        let metadata =
            iceberg::spec::TableMetadata::read_from(&io, &pointer.metadata_location).await?;
        Ok(Table::builder()
            .file_io(io)
            .metadata(metadata)
            .metadata_location(pointer.metadata_location.clone())
            .identifier(Self::ident(pointer.dataset_id))
            .runtime(iceberg::Runtime::current())
            .build()?)
    }
    /// Create a table only when absent. A losing creator leaves unreferenced immutable objects;
    /// it cannot replace the winner. No objects are removed automatically.
    pub async fn ensure_dataset(&self, dataset_id: Uuid) -> Result<TablePointer> {
        let mut conn = self.pool.get().await?;
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                self.ensure_dataset_conn(conn, dataset_id).await
            })
            .await
    }
    pub async fn ensure_dataset_conn(
        &self,
        conn: &mut AsyncPgConnection,
        dataset_id: Uuid,
    ) -> Result<TablePointer> {
        require_transaction(conn)?;
        if let Some(p) = Self::pointer_conn(conn, dataset_id).await? {
            return Ok(p);
        }
        let dataset = datasets::table
            .find(dataset_id)
            .select(Dataset::as_select())
            .first::<Dataset>(conn)
            .await?;
        let dimension_names: Vec<_> = dataset
            .dimensions
            .0
            .iter()
            .map(|d| d.name.clone())
            .collect();
        let schema = encoding::schema_with_dimensions(&dimension_names)?;
        let partition = UnboundPartitionSpec::builder()
            .add_partition_field(2, "series", Transform::Identity)?
            .build();
        let sorts = SortOrder {
            order_id: 1,
            fields: vec![3, 4]
                .into_iter()
                .map(|source_id| SortField {
                    source_id,
                    transform: Transform::Identity,
                    direction: SortDirection::Ascending,
                    null_order: NullOrder::First,
                })
                .collect(),
        };
        let staging = self.config.staging_catalog().await?;
        let ident = Self::ident(dataset_id);
        staging
            .create_namespace(ident.namespace(), HashMap::new())
            .await?;
        let creation = TableCreation::builder()
            .name(ident.name().to_string())
            .location(format!(
                "{}/datasets/{}/{}",
                self.config.warehouse.trim_end_matches('/'),
                dataset_id,
                Uuid::new_v4()
            ))
            .schema(schema)
            .partition_spec(partition)
            .sort_order(sorts)
            .properties([
                (
                    "econgraph.exact-decimal-encoding".into(),
                    "string-v1".into(),
                ),
                ("econgraph.dataset-id".into(), dataset_id.to_string()),
                (
                    "econgraph.dimension-columns".into(),
                    serde_json::to_string(&dimension_names)?,
                ),
                ("econgraph.dataset".into(), serde_json::to_string(&dataset)?),
            ])
            .build();
        let table = staging.create_table(ident.namespace(), creation).await?;
        let metadata = table.metadata_location_result()?.to_string();
        let commit_id = Uuid::new_v4();
        let n=diesel::sql_query("INSERT INTO iceberg_catalog(dataset_id,metadata_location,generation,commit_id) VALUES($1,$2,0,$3) ON CONFLICT(dataset_id) DO NOTHING")
            .bind::<SqlUuid,_>(dataset_id).bind::<Text,_>(&metadata).bind::<SqlUuid,_>(commit_id).execute(conn).await?;
        if n == 1 {
            diesel::sql_query("INSERT INTO iceberg_commits(commit_id,dataset_id,metadata_location,generation) VALUES($1,$2,$3,0)")
            .bind::<SqlUuid,_>(commit_id).bind::<SqlUuid,_>(dataset_id).bind::<Text,_>(&metadata).execute(conn).await?;
        }
        Self::pointer_conn(conn, dataset_id)
            .await?
            .context("catalog create lost its pointer")
    }
    /// Stage one accumulated dataset batch. Object writes and manifest generation happen before
    /// PostgreSQL publication, so a crash here leaves only unreachable immutable objects.
    pub async fn prepare_append(
        &self,
        commit_id: Uuid,
        dataset_id: Uuid,
        rows: &[Observation],
    ) -> Result<PreparedCommit> {
        let mut conn = self.pool.get().await?;
        conn.build_transaction()
            .run::<_, anyhow::Error, _>(async |conn| {
                self.prepare_append_conn(conn, commit_id, dataset_id, rows)
                    .await
            })
            .await
    }
    pub async fn prepare_append_conn(
        &self,
        conn: &mut AsyncPgConnection,
        commit_id: Uuid,
        dataset_id: Uuid,
        rows: &[Observation],
    ) -> Result<PreparedCommit> {
        require_transaction(conn)?;
        if rows.is_empty() {
            bail!("cannot prepare an empty observation batch")
        }
        if rows.len() > crate::query::QueryLimits::from_env()?.max_rows {
            bail!("observation batch exceeds ICEBERG_MAX_SCAN_ROWS")
        }

        let mut unique = std::collections::HashSet::new();
        for row in rows {
            if row.attributes.as_ref().is_some_and(|v| !v.is_object()) {
                bail!("observation attributes must be an object or absent")
            }
            if row.created_at.timestamp_subsec_nanos() % 1000 != 0
                || row.updated_at.timestamp_subsec_nanos() % 1000 != 0
            {
                bail!("observation audit timestamp exceeds stored microsecond precision")
            }

            if !unique.insert(row.id) {
                bail!("duplicate observation UUID in one prepared batch")
            }
        }
        use sha2::{Digest, Sha256};
        let content_sha256: String = Sha256::digest(encoding::canonical_rows(rows)?)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if let Some(next) = diesel::sql_query("SELECT dataset_id,metadata_location,generation,commit_id FROM iceberg_commits WHERE commit_id=$1")
            .bind::<SqlUuid,_>(commit_id).get_result::<TablePointer>(conn).await.optional()? {
            #[derive(QueryableByName)] struct Hash { #[diesel(sql_type=Nullable<Text>)] content_sha256: Option<String> }
            let stored = diesel::sql_query("SELECT content_sha256 FROM iceberg_commits WHERE commit_id=$1").bind::<SqlUuid,_>(commit_id).get_result::<Hash>(conn).await?;
            if next.dataset_id!=dataset_id || stored.content_sha256.as_deref()!=Some(content_sha256.as_str()) { bail!("commit identity was reused for different content") }
            let base=diesel::sql_query("SELECT dataset_id,metadata_location,generation,commit_id FROM iceberg_commits WHERE dataset_id=$1 AND generation=$2")
                .bind::<SqlUuid,_>(dataset_id).bind::<BigInt,_>(next.generation-1).get_result::<TablePointer>(conn).await?;
            return Ok(PreparedCommit{base,next,rows:rows.len(),content_sha256});
        }
        let base = self.ensure_dataset_conn(conn, dataset_id).await?;
        let staging = self.config.staging_catalog().await?;
        let ident = Self::ident(dataset_id);
        staging
            .create_namespace(ident.namespace(), HashMap::new())
            .await?;
        let table = staging
            .register_table(&ident, base.metadata_location.clone())
            .await?;
        self.validate_observation_identities(&table, rows).await?;
        let schema = table.current_schema_ref();
        let ids: Vec<_> = rows.iter().map(|r| r.series_id).collect();
        let series = economic_series::table
            .filter(economic_series::id.eq_any(ids))
            .select(EconomicSeries::as_select())
            .load::<EconomicSeries>(conn)
            .await?;
        let dimensions: BTreeMap<_, _> = series
            .into_iter()
            .map(|s| (s.id, (s.dataset_id, s.dimensions)))
            .collect();
        for row in rows {
            if dimensions.get(&row.series_id).map(|v| v.0) != Some(dataset_id) {
                bail!("batch series does not belong to dataset")
            }
        }
        let mut groups: BTreeMap<Uuid, Vec<Observation>> = BTreeMap::new();
        for r in rows {
            groups.entry(r.series_id).or_default().push(r.clone());
        }
        let mut files = vec![];
        for (series_id, mut group) in groups {
            group.sort_by_key(|r| (r.date, r.revision_date, r.id));
            let parquet =
                ParquetWriterBuilder::new(WriterProperties::builder().build(), schema.clone());
            let rolling = RollingFileWriterBuilder::new_with_default_file_size(
                parquet,
                table.file_io().clone(),
                DefaultLocationGenerator::new(table.metadata())?,
                DefaultFileNameGenerator::new(
                    commit_id.to_string(),
                    Some(Uuid::new_v4().to_string()),
                    DataFileFormat::Parquet,
                ),
            );
            let key = PartitionKey::new(
                table.metadata().default_partition_spec().as_ref().clone(),
                schema.clone(),
                Struct::from_iter([Some(Literal::string(series_id.to_string()))]),
            );
            let mut writer = DataFileWriterBuilder::new(rolling).build(Some(key)).await?;
            let dims = dimensions.get(&series_id).context("missing dimensions")?;
            let batch = encoding::to_batch_with_dimensions(
                &schema,
                &group,
                &dims.1 .0,
                base.generation + 1,
            )?;
            writer.write(batch).await?;
            files.extend(writer.close().await?);
        }
        let tx = Transaction::new(&table);
        let tx = tx.fast_append().add_data_files(files).apply(tx)?;
        let next_table = tx.commit(&staging).await?;
        let next = TablePointer {
            dataset_id,
            metadata_location: next_table.metadata_location_result()?.into(),
            generation: base
                .generation
                .checked_add(1)
                .context("catalog generation exhausted")?,
            commit_id,
        };
        Ok(PreparedCommit {
            base,
            next,
            rows: rows.len(),
            content_sha256,
        })
    }
    async fn validate_observation_identities(
        &self,
        table: &Table,
        rows: &[Observation],
    ) -> Result<()> {
        let ctx = crate::query::context()?;
        ctx.register_table(
            "observations",
            Arc::new(
                iceberg_datafusion::IcebergStaticTableProvider::try_new_from_table(table.clone())
                    .await?,
            ),
        )?;
        let ids = arrow_array::RecordBatch::try_new(
            Arc::new(arrow_schema::Schema::new(vec![arrow_schema::Field::new(
                "id",
                arrow_schema::DataType::Utf8,
                false,
            )])),
            vec![Arc::new(arrow_array::StringArray::from(
                rows.iter().map(|r| r.id.to_string()).collect::<Vec<_>>(),
            ))],
        )?;
        ctx.register_batch("candidate_ids", ids)?;
        let existing=ctx.sql("SELECT id,series_id,date,revision_date,value,is_original_release,created_at,updated_at,attributes FROM (SELECT o.*,row_number() OVER(PARTITION BY o.id ORDER BY o._commit_generation DESC) AS _rank FROM observations o INNER JOIN candidate_ids c ON o.id=c.id) WHERE _rank=1").await?.collect().await?;
        let by_id: HashMap<_, _> = rows.iter().map(|r| (r.id, r)).collect();
        for old in encoding::from_batches(&existing)? {
            let new = by_id
                .get(&old.id)
                .context("identity join returned unexpected UUID")?;
            if (
                old.series_id,
                old.date,
                old.revision_date,
                old.is_original_release,
            ) != (
                new.series_id,
                new.date,
                new.revision_date,
                new.is_original_release,
            ) {
                bail!("existing observation UUID cannot be retargeted to another series, date, vintage or release kind");
            }
        }
        Ok(())
    }
    /// Publish only in an existing caller-owned transaction, after its crawl claim has been
    /// locked and verified. This method never opens/commits a pool transaction. Errors must
    /// roll back that surrounding transaction, including summaries and queue completion.
    pub async fn commit_prepared_conn(
        conn: &mut AsyncPgConnection,
        prepared: &PreparedCommit,
    ) -> Result<bool> {
        require_transaction(conn)?;
        #[derive(QueryableByName)]
        struct Receipt {
            #[diesel(sql_type=SqlUuid)]
            dataset_id: Uuid,
            #[diesel(sql_type=Nullable<Text>)]
            content_sha256: Option<String>,
        }
        if let Some(done) = diesel::sql_query(
            "SELECT dataset_id,content_sha256 FROM iceberg_commits WHERE commit_id=$1",
        )
        .bind::<SqlUuid, _>(prepared.next.commit_id)
        .get_result::<Receipt>(conn)
        .await
        .optional()?
        {
            if done.dataset_id != prepared.next.dataset_id
                || done.content_sha256.as_deref() != Some(prepared.content_sha256.as_str())
            {
                bail!("commit identity was reused for different content")
            }
            return Ok(false);
        }
        let n=diesel::sql_query("UPDATE iceberg_catalog SET previous_metadata_location=metadata_location,metadata_location=$1,generation=$2,commit_id=$3,updated_at=CURRENT_TIMESTAMP WHERE dataset_id=$4 AND metadata_location=$5 AND generation=$6")
            .bind::<Text,_>(&prepared.next.metadata_location).bind::<BigInt,_>(prepared.next.generation).bind::<SqlUuid,_>(prepared.next.commit_id).bind::<SqlUuid,_>(prepared.next.dataset_id).bind::<Text,_>(&prepared.base.metadata_location).bind::<BigInt,_>(prepared.base.generation).execute(conn).await?;
        if n != 1 {
            bail!("Iceberg catalog commit conflict; rebase batch onto current pointer")
        }
        diesel::sql_query("INSERT INTO iceberg_commits(commit_id,dataset_id,metadata_location,parent_metadata_location,generation,content_sha256) VALUES($1,$2,$3,$4,$5,$6)")
            .bind::<SqlUuid,_>(prepared.next.commit_id).bind::<SqlUuid,_>(prepared.next.dataset_id).bind::<Text,_>(&prepared.next.metadata_location).bind::<Nullable<Text>,_>(Some(&prepared.base.metadata_location)).bind::<BigInt,_>(prepared.next.generation).bind::<Text,_>(&prepared.content_sha256).execute(conn).await?;
        Ok(true)
    }
    /// Traverse only objects reachable from this immutable pointer, including previous
    /// metadata/history and every retained snapshot. Hash data in bounded chunks.
    pub async fn reachable_objects(&self, pointer: &TablePointer) -> Result<Vec<ReachableObject>> {
        use sha2::{Digest, Sha256};
        let io = self.config.file_io()?;
        let mut locations = std::collections::BTreeSet::new();
        let mut pending = vec![pointer.metadata_location.clone()];
        while let Some(location) = pending.pop() {
            if !locations.insert(location.clone()) {
                continue;
            }
            let p = TablePointer {
                metadata_location: location,
                ..pointer.clone()
            };
            let table = self.load_at(&p).await?;
            let metadata = table.metadata();
            for old in metadata.metadata_log() {
                pending.push(old.metadata_file.clone());
            }
            for s in metadata.statistics_iter() {
                locations.insert(s.statistics_path.clone());
            }
            for s in metadata.partition_statistics_iter() {
                locations.insert(s.statistics_path.clone());
            }
            for snapshot in metadata.snapshots() {
                locations.insert(snapshot.manifest_list().to_string());
                let reader = table.manifest_list_reader(snapshot);
                let manifests = iceberg::spec::ManifestListReader::load(&reader).await?;
                for file in manifests.entries() {
                    locations.insert(file.manifest_path.clone());
                    let manifest = file.load_manifest(&io).await?;
                    for entry in manifest.entries() {
                        locations.insert(entry.data_file.file_path().to_string());
                    }
                }
            }
        }
        let mut objects = vec![];
        for location in locations {
            let input = io.new_input(&location)?;
            let size_bytes = input.metadata().await?.size;
            let reader = input.reader().await?;
            let mut digest = Sha256::new();
            let mut offset = 0;
            while offset < size_bytes {
                let end = offset.saturating_add(8 * 1024 * 1024).min(size_bytes);
                digest.update(reader.read(offset..end).await?);
                offset = end;
            }
            objects.push(ReachableObject {
                location,
                size_bytes,
                sha256: digest
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            });
        }
        Ok(objects)
    }
    pub async fn read_rows(&self, pointer: &TablePointer) -> Result<Vec<Observation>> {
        Self::read_matching_config(
            &self.config,
            pointer,
            None,
            None,
            None,
            crate::query::QueryLimits::from_env()?.max_rows,
        )
        .await
    }
    pub async fn read_rows_filtered(
        &self,
        pointer: &TablePointer,
        request: Option<&econ_graph_core::models::ObservationRequest>,
    ) -> Result<Vec<Observation>> {
        match request {
            Some(req) => {
                self.read_series_at(
                    pointer,
                    &[req.series_id],
                    req.start_date,
                    req.end_date,
                    crate::query::QueryLimits::from_env()?.max_rows,
                )
                .await
            }
            None => self.read_rows(pointer).await,
        }
    }
    pub async fn read_series_at(
        &self,
        pointer: &TablePointer,
        series: &[Uuid],
        start: Option<chrono::NaiveDate>,
        end: Option<chrono::NaiveDate>,
        max_rows: usize,
    ) -> Result<Vec<Observation>> {
        Self::read_series_at_config(&self.config, pointer, series, start, end, max_rows).await
    }
    /// Read a frozen metadata pointer without borrowing another database connection. Useful
    /// when a compatibility caller already owns the SQL snapshot which selected that pointer.
    pub async fn read_series_at_config(
        config: &StorageConfig,
        pointer: &TablePointer,
        series: &[Uuid],
        start: Option<chrono::NaiveDate>,
        end: Option<chrono::NaiveDate>,
        max_rows: usize,
    ) -> Result<Vec<Observation>> {
        if series.is_empty() {
            return Ok(vec![]);
        }
        Self::read_matching_config(config, pointer, Some(series), start, end, max_rows).await
    }
    async fn read_matching_config(
        config: &StorageConfig,
        pointer: &TablePointer,
        series: Option<&[Uuid]>,
        start: Option<chrono::NaiveDate>,
        end: Option<chrono::NaiveDate>,
        max_rows: usize,
    ) -> Result<Vec<Observation>> {
        if max_rows == 0 {
            bail!("snapshot read requires a positive row budget")
        }
        let budget = max_rows.min(crate::query::QueryLimits::from_env()?.max_rows);
        if series.is_some_and(|ids| ids.len() > budget) {
            bail!("snapshot series batch exceeds configured row budget")
        }
        let table = Self::load_at_config(config, pointer).await?;
        let ctx = crate::query::context()?;
        let provider =
            iceberg_datafusion::IcebergStaticTableProvider::try_new_from_table(table).await?;
        ctx.register_table("observations", Arc::new(provider))?;
        // The write boundary enforces immutable UUID/series identity. Identity filtering
        // before the window therefore enables safe physical partition pruning.
        let series = series
            .map(|ids| {
                format!(
                    " WHERE series_id IN ({})",
                    ids.iter()
                        .map(|id| format!("'{id}'"))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .unwrap_or_default();
        let mut bounds = String::new();
        if let Some(day) = start {
            bounds.push_str(&format!(" AND date>=DATE '{day}'"));
        }
        if let Some(day) = end {
            bounds.push_str(&format!(" AND date<=DATE '{day}'"));
        }
        let sql=format!("SELECT id,series_id,date,revision_date,value,is_original_release,created_at,updated_at,attributes FROM (SELECT *,row_number() OVER(PARTITION BY id ORDER BY _commit_generation DESC) AS _rank FROM observations{series}) WHERE _rank=1{bounds} ORDER BY series_id,date,revision_date,id LIMIT {}",budget+1);
        let batches = ctx.sql(&sql).await?.collect().await?;
        if batches.iter().map(|b| b.num_rows()).sum::<usize>() > budget {
            bail!("Iceberg snapshot exceeds the configured row budget; refusing to truncate observations or counts")
        }
        encoding::from_batches(&batches)
    }
}
