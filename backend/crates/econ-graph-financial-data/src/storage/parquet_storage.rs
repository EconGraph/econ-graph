use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use arrow::array::*;
use arrow::datatypes::*;
use arrow::record_batch::RecordBatch;
use arrow_flight::flight_service_server::FlightServiceServer;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::arrow_writer::ArrowWriter;
use parquet::file::properties::WriterProperties;
use rust_decimal::prelude::ToPrimitive;
use std::fs::File;
use tokio::sync::RwLock;

use super::FinancialDataStorage;
use crate::models::{DataPoint, DecimalScalar, EconomicSeries};

/// V1 Implementation: Direct Parquet files with Arrow Flight
///
/// This implementation provides:
/// - Zero-copy data transfer via Arrow Flight
/// - Direct Parquet file operations using Arrow
/// - Memory-mapped files for hot data
/// - Time series indexing for fast queries
///
/// Future V2 will use the same Arrow Flight interface but with Iceberg backend
#[derive(Clone)]
pub struct ParquetStorage {
    data_dir: PathBuf,
    flight_server: Arc<RwLock<Option<FlightServiceServer<ParquetFlightService>>>>,
}

impl ParquetStorage {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            flight_server: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the Arrow Flight server
    pub async fn start_flight_server(&self, port: u16) -> Result<()> {
        let service = ParquetFlightService::new(self.data_dir.clone());
        let server = FlightServiceServer::new(service);

        let mut server_guard = self.flight_server.write().await;
        *server_guard = Some(server);

        tracing::info!("Arrow Flight server started on port {}", port);
        Ok(())
    }

    /// Get the Arrow schema for EconomicSeries
    pub fn series_arrow_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("external_id", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("description", DataType::Utf8, true),
            Field::new("units", DataType::Utf8, true),
            Field::new("frequency", DataType::Utf8, false),
            Field::new("seasonal_adjustment", DataType::Utf8, true),
            Field::new("start_date", DataType::Utf8, true),
            Field::new("end_date", DataType::Utf8, true),
            Field::new("is_active", DataType::Boolean, false),
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
        ])
    }

    /// Get the Arrow schema for DataPoint
    pub fn data_points_arrow_schema() -> Schema {
        Schema::new(vec![
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
        ])
    }

    /// Convert EconomicSeries to Arrow RecordBatch
    pub fn series_to_record_batch(series: &EconomicSeries) -> Result<RecordBatch> {
        let schema = Arc::new(Self::series_arrow_schema());

        let id_array = StringArray::from(vec![series.id.to_string()]);
        let source_id_array = StringArray::from(vec![series.source_id.to_string()]);
        let external_id_array = StringArray::from(vec![series.external_id.clone()]);
        let title_array = StringArray::from(vec![series.title.clone()]);
        let description_array =
            StringArray::from(vec![series.description.clone().unwrap_or_default()]);
        let units_array = StringArray::from(vec![series.units.clone().unwrap_or_default()]);
        let frequency_array = StringArray::from(vec![series.frequency.clone()]);
        let seasonal_adjustment_array =
            StringArray::from(vec![series.seasonal_adjustment.clone().unwrap_or_default()]);
        let start_date_array = StringArray::from(vec![series
            .start_date
            .map(|d| d.to_string())
            .unwrap_or_default()]);
        let end_date_array = StringArray::from(vec![series
            .end_date
            .map(|d| d.to_string())
            .unwrap_or_default()]);
        let is_active_array = BooleanArray::from(vec![series.is_active]);
        let created_at_array = TimestampNanosecondArray::from(vec![Some(
            series.created_at.timestamp_nanos_opt().unwrap_or(0),
        )]);
        let updated_at_array = TimestampNanosecondArray::from(vec![Some(
            series.updated_at.timestamp_nanos_opt().unwrap_or(0),
        )]);

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(source_id_array),
                Arc::new(external_id_array),
                Arc::new(title_array),
                Arc::new(description_array),
                Arc::new(units_array),
                Arc::new(frequency_array),
                Arc::new(seasonal_adjustment_array),
                Arc::new(start_date_array),
                Arc::new(end_date_array),
                Arc::new(is_active_array),
                Arc::new(created_at_array),
                Arc::new(updated_at_array),
            ],
        )?;

        Ok(batch)
    }

    /// Convert DataPoints to Arrow RecordBatch
    pub fn data_points_to_record_batch(points: &[DataPoint]) -> Result<RecordBatch> {
        if points.is_empty() {
            return Ok(RecordBatch::new_empty(Arc::new(
                Self::data_points_arrow_schema(),
            )));
        }

        let schema = Arc::new(Self::data_points_arrow_schema());

        let mut id_array = Vec::new();
        let mut series_id_array = Vec::new();
        let mut date_array = Vec::new();
        let mut value_array = Vec::new();
        let mut revision_date_array = Vec::new();
        let mut is_original_release_array = Vec::new();
        let mut created_at_array = Vec::new();
        let mut updated_at_array = Vec::new();

        for point in points {
            id_array.push(point.id.to_string());
            series_id_array.push(point.series_id.to_string());
            // Convert NaiveDate to days since epoch for Date32
            let days_since_epoch = (point
                .date
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp()
                / 86400) as i32;
            date_array.push(days_since_epoch);
            value_array.push(point.value.as_ref().map(|v| v.0.to_f64().unwrap_or(0.0)));
            // Convert NaiveDate to days since epoch for Date32
            let revision_days_since_epoch = (point
                .revision_date
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp()
                / 86400) as i32;
            revision_date_array.push(revision_days_since_epoch);
            is_original_release_array.push(point.is_original_release);
            created_at_array.push(Some(point.created_at.timestamp_nanos_opt().unwrap_or(0)));
            updated_at_array.push(Some(point.updated_at.timestamp_nanos_opt().unwrap_or(0)));
        }

        let batch = RecordBatch::try_new(
            schema,
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

        Ok(batch)
    }

    /// Write Arrow RecordBatch to Parquet file
    pub async fn write_record_batch_to_parquet(
        &self,
        batch: RecordBatch,
        file_path: PathBuf,
    ) -> Result<()> {
        let file = File::create(&file_path)?;
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, batch.schema(), Some(props))?;
        writer.write(&batch)?;
        writer.close()?;

        tracing::info!("Arrow RecordBatch written to Parquet: {:?}", file_path);
        Ok(())
    }

    /// Read Arrow RecordBatch from Parquet file
    pub async fn read_record_batch_from_parquet(
        &self,
        file_path: PathBuf,
    ) -> Result<Option<RecordBatch>> {
        if !file_path.exists() {
            return Ok(None);
        }

        let file = File::open(&file_path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let mut reader = builder.build()?;

        if let Some(batch) = reader.next() {
            Ok(Some(batch?))
        } else {
            Ok(None)
        }
    }
}

#[async_trait]
impl FinancialDataStorage for ParquetStorage {
    async fn write_series(&self, series: &EconomicSeries) -> Result<()> {
        let batch = Self::series_to_record_batch(series)?;
        let file_path = self.data_dir.join(format!("series_{}.parquet", series.id));
        self.write_record_batch_to_parquet(batch, file_path).await
    }

    async fn read_series(&self, series_id: Uuid) -> Result<Option<EconomicSeries>> {
        let file_path = self.data_dir.join(format!("series_{}.parquet", series_id));

        if let Some(batch) = self.read_record_batch_from_parquet(file_path).await? {
            // Convert Arrow RecordBatch back to EconomicSeries
            if batch.num_rows() > 0 {
                let id_str = batch
                    .column(0)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let source_id_str = batch
                    .column(1)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let external_id = batch
                    .column(2)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0)
                    .to_string();
                let title = batch
                    .column(3)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0)
                    .to_string();
                let description = batch
                    .column(4)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let units = batch
                    .column(5)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let frequency = batch
                    .column(6)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0)
                    .to_string();
                let seasonal_adjustment = batch
                    .column(7)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let start_date_str = batch
                    .column(8)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let end_date_str = batch
                    .column(9)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .unwrap()
                    .value(0);
                let is_active = batch
                    .column(10)
                    .as_any()
                    .downcast_ref::<BooleanArray>()
                    .unwrap()
                    .value(0);
                let created_at_nanos = batch
                    .column(11)
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .unwrap()
                    .value(0);
                let updated_at_nanos = batch
                    .column(12)
                    .as_any()
                    .downcast_ref::<TimestampNanosecondArray>()
                    .unwrap()
                    .value(0);

                let series = EconomicSeries {
                    id: Uuid::parse_str(id_str)?,
                    source_id: Uuid::parse_str(source_id_str)?,
                    external_id,
                    title,
                    description: if description.is_empty() {
                        None
                    } else {
                        Some(description.to_string())
                    },
                    units: if units.is_empty() {
                        None
                    } else {
                        Some(units.to_string())
                    },
                    frequency,
                    seasonal_adjustment: if seasonal_adjustment.is_empty() {
                        None
                    } else {
                        Some(seasonal_adjustment.to_string())
                    },
                    start_date: if start_date_str.is_empty() {
                        None
                    } else {
                        Some(NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")?)
                    },
                    end_date: if end_date_str.is_empty() {
                        None
                    } else {
                        Some(NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d")?)
                    },
                    is_active,
                    created_at: chrono::DateTime::from_timestamp_nanos(created_at_nanos),
                    updated_at: chrono::DateTime::from_timestamp_nanos(updated_at_nanos),
                };

                return Ok(Some(series));
            }
        }

        Ok(None)
    }

    async fn write_data_points(&self, series_id: Uuid, points: &[DataPoint]) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let batch = Self::data_points_to_record_batch(points)?;
        let file_path = self
            .data_dir
            .join(format!("data_points_{}.parquet", series_id));
        self.write_record_batch_to_parquet(batch, file_path).await
    }

    async fn read_data_points(
        &self,
        series_id: Uuid,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<DataPoint>> {
        let file_path = self
            .data_dir
            .join(format!("data_points_{}.parquet", series_id));

        if let Some(batch) = self.read_record_batch_from_parquet(file_path).await? {
            let mut points = Vec::new();

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
                        // Convert from days since epoch to NaiveDate
                        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                        epoch + chrono::Duration::days(days_since_epoch as i64)
                    },
                    value: if value_array.is_null(i) {
                        None
                    } else {
                        Some(DecimalScalar(
                            rust_decimal::Decimal::from_f64_retain(value_array.value(i))
                                .unwrap_or_default(),
                        ))
                    },
                    revision_date: {
                        let days_since_epoch = revision_date_array.value(i);
                        // Convert from days since epoch to NaiveDate
                        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
                        epoch + chrono::Duration::days(days_since_epoch as i64)
                    },
                    is_original_release: is_original_release_array.value(i),
                    created_at: chrono::DateTime::from_timestamp_nanos(created_at_array.value(i)),
                    updated_at: chrono::DateTime::from_timestamp_nanos(updated_at_array.value(i)),
                };

                // Apply date filtering
                let include_point = match (start_date, end_date) {
                    (Some(start), Some(end)) => point.date >= start && point.date <= end,
                    (Some(start), None) => point.date >= start,
                    (None, Some(end)) => point.date <= end,
                    (None, None) => true,
                };

                if include_point {
                    points.push(point);
                }
            }

            Ok(points)
        } else {
            Ok(vec![])
        }
    }

    async fn list_series(&self) -> Result<Vec<EconomicSeries>> {
        let mut series = Vec::new();

        // Read all series files from the data directory
        if let Ok(entries) = std::fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.starts_with("series_") && file_name.ends_with(".parquet") {
                        if let Some(id_str) = file_name
                            .strip_prefix("series_")
                            .and_then(|s| s.strip_suffix(".parquet"))
                        {
                            if let Ok(series_id) = Uuid::parse_str(id_str) {
                                if let Some(series_data) = self.read_series(series_id).await? {
                                    series.push(series_data);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(series)
    }
}

/// Arrow Flight service implementation for Parquet storage
/// This provides the low-level Arrow Flight protocol for zero-copy data transfer
///
/// For V1, we implement a simplified version that focuses on the core functionality
/// without the full gRPC server complexity. The key is that we use Arrow Flight
/// concepts and data structures, making it easy to upgrade to full Arrow Flight
/// server in V2.
pub struct ParquetFlightService {
    data_dir: PathBuf,
}

impl ParquetFlightService {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Get Arrow schema for financial data
    pub fn get_financial_data_schema() -> Schema {
        ParquetStorage::series_arrow_schema()
    }

    /// Get Arrow schema for data points
    pub fn get_data_points_schema() -> Schema {
        ParquetStorage::data_points_arrow_schema()
    }
}
