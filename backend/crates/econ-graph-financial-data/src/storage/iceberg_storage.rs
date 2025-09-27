use anyhow::Result;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use super::FinancialDataStorage;
use crate::catalog::{IcebergCatalog, SeriesMetadata as CatalogSeriesMetadata};
use crate::models::{DataPoint, EconomicSeries};

/// Custom Time-Based Partitioning for Financial Data
///
/// This implementation provides:
/// - Time-based partition organization (year/month/day)
/// - Efficient range queries for time series data
/// - Simple data lifecycle management
/// - Direct file system access with minimal overhead
/// - Custom catalog for tracking series metadata
///
/// Partition Structure:
/// financial_data/
/// ├── year=2024/month=01/day=15/
/// │   ├── series_123e4567-e89b-12d3-a456-426614174000.parquet
/// │   ├── series_987fcdeb-51a2-43d1-b789-123456789abc.parquet
/// │   └── ...
/// ├── year=2024/month=01/day=16/
/// │   └── ...
/// └── metadata/
///     ├── series_catalog.json
///     └── partition_index.json
pub struct IcebergStorage {
    data_dir: PathBuf,
    catalog: IcebergCatalog,
}

/// Partition information for time-based organization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Partition {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Partition {
    pub fn from_date(date: NaiveDate) -> Self {
        Self {
            year: date.year(),
            month: date.month(),
            day: date.day(),
        }
    }

    pub fn to_path(&self) -> String {
        format!(
            "year={}/month={:02}/day={:02}",
            self.year, self.month, self.day
        )
    }

    pub fn to_path_buf(&self, base_dir: &Path) -> PathBuf {
        base_dir.join(self.to_path())
    }
}

impl IcebergStorage {
    pub async fn new(data_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        std::fs::create_dir_all(&data_dir)?;

        // Initialize the Iceberg catalog
        let catalog = IcebergCatalog::new(data_dir.clone()).await?;

        Ok(Self { data_dir, catalog })
    }

    /// Get partition path for a given date
    pub fn get_partition_path(&self, date: NaiveDate) -> PathBuf {
        let partition = Partition::from_date(date);
        partition.to_path_buf(&self.data_dir)
    }

    /// Get file path for a series in a specific partition
    pub fn get_series_file_path(&self, series_id: Uuid, partition: &Partition) -> PathBuf {
        let partition_path = partition.to_path_buf(&self.data_dir);
        partition_path.join(format!("series_{}.parquet", series_id))
    }

    /// Find all partitions that contain data for a date range
    pub fn get_partitions_for_date_range(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<Partition>> {
        let start = start_date.unwrap_or_else(|| NaiveDate::from_ymd_opt(1900, 1, 1).unwrap());
        let end = end_date.unwrap_or_else(|| Utc::now().date_naive());

        let mut partitions = Vec::new();
        let mut current = start;

        while current <= end {
            let partition = Partition::from_date(current);
            let partition_path = partition.to_path_buf(&self.data_dir);

            // Only include partitions that actually exist and have data
            if partition_path.exists() && !self.is_partition_empty(&partition_path)? {
                partitions.push(partition);
            }

            current = current
                .succ_opt()
                .ok_or_else(|| anyhow::anyhow!("Date overflow"))?;
        }

        Ok(partitions)
    }

    /// Check if a partition directory is empty
    fn is_partition_empty(&self, partition_path: &Path) -> Result<bool> {
        if !partition_path.exists() {
            return Ok(true);
        }

        let entries = std::fs::read_dir(partition_path)?;
        Ok(entries.count() == 0)
    }

    /// List all series files in a partition
    pub fn list_series_in_partition(&self, partition: &Partition) -> Result<Vec<Uuid>> {
        let partition_path = partition.to_path_buf(&self.data_dir);

        if !partition_path.exists() {
            return Ok(Vec::new());
        }

        let mut series_ids = Vec::new();
        let entries = std::fs::read_dir(partition_path)?;

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            if file_name_str.starts_with("series_") && file_name_str.ends_with(".parquet") {
                if let Some(id_str) = file_name_str
                    .strip_prefix("series_")
                    .and_then(|s| s.strip_suffix(".parquet"))
                {
                    if let Ok(series_id) = Uuid::parse_str(id_str) {
                        series_ids.push(series_id);
                    }
                }
            }
        }

        Ok(series_ids)
    }

    /// Find files containing data for a range of series IDs
    pub fn find_files_for_series_range(
        &self,
        start_series_id: Option<Uuid>,
        end_series_id: Option<Uuid>,
        date_range: Option<(NaiveDate, NaiveDate)>,
    ) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        // Get partitions for the date range
        let (start_date, end_date) = date_range.unwrap_or_else(|| {
            let today = Utc::now().date_naive();
            (today, today)
        });

        let partitions = self.get_partitions_for_date_range(Some(start_date), Some(end_date))?;

        for partition in partitions {
            let series_in_partition = self.list_series_in_partition(&partition)?;

            for series_id in series_in_partition {
                // Check if series ID is in range
                let include_series = match (start_series_id, end_series_id) {
                    (Some(start), Some(end)) => series_id >= start && series_id <= end,
                    (Some(start), None) => series_id >= start,
                    (None, Some(end)) => series_id <= end,
                    (None, None) => true,
                };

                if include_series {
                    let file_path = self.get_series_file_path(series_id, &partition);
                    if file_path.exists() {
                        files.push(file_path);
                    }
                }
            }
        }

        Ok(files)
    }

    /// Write partition data to Parquet file using Arrow
    async fn write_partition_data_to_parquet(
        &self,
        file_path: &Path,
        points: Vec<&DataPoint>,
    ) -> Result<()> {
        use arrow::array::{
            BooleanArray, Date32Array, Float64Array, StringArray, TimestampNanosecondArray,
        };
        use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::arrow_writer::ArrowWriter;
        use parquet::file::properties::WriterProperties;
        use std::fs::File;

        if points.is_empty() {
            return Ok(());
        }

        // Create Arrow schema
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("series_id", DataType::Utf8, false),
            Field::new("date", DataType::Date32, false),
            Field::new("value", DataType::Float64, true),
            Field::new("revision_date", DataType::Date32, false),
            Field::new("is_original_release", DataType::Boolean, false),
            Field::new(
                "created_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
            Field::new(
                "updated_at",
                DataType::Timestamp(TimeUnit::Nanosecond, None),
                false,
            ),
        ]));

        // Convert DataPoints to Arrow arrays
        let id_array: Vec<String> = points.iter().map(|p| p.id.to_string()).collect();
        let series_id_array: Vec<String> = points.iter().map(|p| p.series_id.to_string()).collect();
        let date_array: Vec<i32> = points.iter().map(|p| p.date.num_days_from_ce()).collect();
        let value_array: Vec<Option<f64>> = points
            .iter()
            .map(|p| p.value.as_ref().map(|v| v.0.to_f64().unwrap_or_default()))
            .collect();
        let revision_date_array: Vec<i32> = points
            .iter()
            .map(|p| p.revision_date.num_days_from_ce())
            .collect();
        let is_original_release_array: Vec<bool> =
            points.iter().map(|p| p.is_original_release).collect();
        let created_at_array: Vec<i64> = points
            .iter()
            .map(|p| p.created_at.timestamp_nanos_opt().unwrap_or_default())
            .collect();
        let updated_at_array: Vec<i64> = points
            .iter()
            .map(|p| p.updated_at.timestamp_nanos_opt().unwrap_or_default())
            .collect();

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(id_array)),
                Arc::new(StringArray::from(series_id_array)),
                Arc::new(Date32Array::from(date_array)),
                Arc::new(Float64Array::from(value_array)),
                Arc::new(Date32Array::from(revision_date_array)),
                Arc::new(BooleanArray::from(is_original_release_array)),
                Arc::new(TimestampNanosecondArray::from(created_at_array)),
                Arc::new(TimestampNanosecondArray::from(updated_at_array)),
            ],
        )?;

        // Write to Parquet file
        let file = File::create(file_path)?;
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
        writer.write(&batch)?;
        writer.close()?;

        tracing::debug!(
            "Wrote {} points to Parquet file: {:?}",
            points.len(),
            file_path
        );
        Ok(())
    }

    /// Read partition data from Parquet file using Arrow
    async fn read_partition_data_from_parquet(&self, file_path: &Path) -> Result<Vec<DataPoint>> {
        use arrow::array::{
            Array, BooleanArray, Date32Array, Float64Array, StringArray, TimestampNanosecondArray,
        };
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
        use std::fs::File;

        let file = File::open(file_path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let mut reader = builder.build()?;

        let mut points = Vec::new();

        while let Some(batch) = reader.next().transpose()? {
            // Convert Arrow RecordBatch back to DataPoints
            let id_array = batch
                .column(0)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let series_id_array = batch
                .column(1)
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let date_array = batch
                .column(2)
                .as_any()
                .downcast_ref::<Date32Array>()
                .unwrap();
            let value_array = batch
                .column(3)
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap();
            let revision_date_array = batch
                .column(4)
                .as_any()
                .downcast_ref::<Date32Array>()
                .unwrap();
            let is_original_release_array = batch
                .column(5)
                .as_any()
                .downcast_ref::<BooleanArray>()
                .unwrap();
            let created_at_array = batch
                .column(6)
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap();
            let updated_at_array = batch
                .column(7)
                .as_any()
                .downcast_ref::<TimestampNanosecondArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                let point = DataPoint {
                    id: Uuid::parse_str(id_array.value(i))?,
                    series_id: Uuid::parse_str(series_id_array.value(i))?,
                    date: {
                        let days_since_epoch = date_array.value(i);
                        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                        epoch + chrono::Duration::days(days_since_epoch as i64)
                    },
                    value: if value_array.is_null(i) {
                        None
                    } else {
                        Some(crate::models::DecimalScalar(
                            rust_decimal::Decimal::from_f64_retain(value_array.value(i))
                                .unwrap_or_default(),
                        ))
                    },
                    revision_date: {
                        let days_since_epoch = revision_date_array.value(i);
                        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                        epoch + chrono::Duration::days(days_since_epoch as i64)
                    },
                    is_original_release: is_original_release_array.value(i),
                    created_at: chrono::DateTime::from_timestamp_nanos(created_at_array.value(i)),
                    updated_at: chrono::DateTime::from_timestamp_nanos(updated_at_array.value(i)),
                };
                points.push(point);
            }
        }

        tracing::debug!(
            "Read {} points from Parquet file: {:?}",
            points.len(),
            file_path
        );
        Ok(points)
    }
}

// Note: Default implementation removed because IcebergStorage::new is async

#[async_trait]
impl FinancialDataStorage for IcebergStorage {
    async fn write_series(&self, series: &EconomicSeries) -> Result<()> {
        tracing::info!("Writing series to Iceberg storage: {}", series.id);

        // Convert EconomicSeries to CatalogSeriesMetadata
        let metadata = CatalogSeriesMetadata {
            series_id: series.id,
            external_id: series.external_id.clone(),
            title: series.title.clone(),
            description: series.description.clone(),
            units: series.units.clone(),
            frequency: series.frequency.clone(),
            seasonal_adjustment: series.seasonal_adjustment.clone(),
            source: "Financial Data Service".to_string(), // TODO: Get from series
            is_active: series.is_active,
            start_date: series.start_date,
            end_date: series.end_date,
            total_points: 0, // TODO: Calculate from actual data points
            created_at: series.created_at,
            updated_at: series.updated_at,
        };

        // Add to catalog
        self.catalog.add_series(metadata).await?;

        Ok(())
    }

    async fn read_series(&self, series_id: Uuid) -> Result<Option<EconomicSeries>> {
        tracing::info!("Reading series from Iceberg storage: {}", series_id);

        // Get series from catalog
        if let Some(metadata) = self.catalog.get_series(series_id).await? {
            // Convert CatalogSeriesMetadata back to EconomicSeries
            let series = EconomicSeries {
                id: metadata.series_id,
                source_id: Uuid::new_v4(), // TODO: Map from catalog metadata
                external_id: metadata.external_id,
                title: metadata.title,
                description: metadata.description,
                units: metadata.units,
                frequency: metadata.frequency,
                seasonal_adjustment: metadata.seasonal_adjustment,
                start_date: metadata.start_date,
                end_date: metadata.end_date,
                is_active: metadata.is_active,
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
            };
            Ok(Some(series))
        } else {
            Ok(None)
        }
    }

    async fn write_data_points(&self, _series_id: Uuid, _points: &[DataPoint]) -> Result<()> {
        // TODO: Implement Iceberg table write via Arrow Flight
        tracing::info!("IcebergStorage::write_data_points - V2 implementation pending");
        Ok(())
    }

    async fn read_data_points(
        &self,
        _series_id: Uuid,
        _start_date: Option<NaiveDate>,
        _end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        // TODO: Implement Iceberg table read via Arrow Flight
        tracing::info!("IcebergStorage::read_data_points - V2 implementation pending");
        Ok(vec![])
    }

    async fn list_series(&self) -> Result<Vec<EconomicSeries>> {
        tracing::info!("Listing series from Iceberg storage");

        // Get all series from catalog
        let metadata_list = self.catalog.list_series().await?;

        // Convert to EconomicSeries
        let series_list = metadata_list
            .into_iter()
            .map(|metadata| EconomicSeries {
                id: metadata.series_id,
                source_id: Uuid::new_v4(), // TODO: Map from catalog metadata
                external_id: metadata.external_id,
                title: metadata.title,
                description: metadata.description,
                units: metadata.units,
                frequency: metadata.frequency,
                seasonal_adjustment: metadata.seasonal_adjustment,
                start_date: metadata.start_date,
                end_date: metadata.end_date,
                is_active: metadata.is_active,
                created_at: metadata.created_at,
                updated_at: metadata.updated_at,
            })
            .collect();

        Ok(series_list)
    }
}
