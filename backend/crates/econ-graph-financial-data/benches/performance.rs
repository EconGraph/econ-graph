use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use chrono::{NaiveDate, Utc};
use econ_graph_financial_data::database::Database;
use econ_graph_financial_data::models::{DataPoint, DecimalScalar, EconomicSeries};
use econ_graph_financial_data::storage::{FinancialDataStorage, ParquetStorage};
use rust_decimal::Decimal;
use uuid::Uuid;

/// Benchmark suite for Arrow Flight + Parquet performance
///
/// This benchmark suite measures:
/// - Arrow RecordBatch creation and serialization
/// - Parquet file write/read performance
/// - GraphQL query execution times
/// - Memory usage and efficiency
/// - Concurrent operation performance

fn create_test_series(id: Uuid, source_id: Uuid, count: usize) -> EconomicSeries {
    EconomicSeries {
        id,
        source_id,
        external_id: format!("BENCH_{}", count),
        title: format!("Benchmark Series {}", count),
        description: Some("Performance test series".to_string()),
        units: Some("Index".to_string()),
        frequency: "monthly".to_string(),
        seasonal_adjustment: Some("Not Seasonally Adjusted".to_string()),
        start_date: Some(NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()),
        end_date: Some(NaiveDate::from_ymd_opt(2023, 12, 31).unwrap()),
        is_active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn create_test_data_points(series_id: Uuid, count: usize) -> Vec<DataPoint> {
    (0..count)
        .map(|i| {
            let date =
                NaiveDate::from_ymd_opt(2023, 1, 1).unwrap() + chrono::Duration::days(i as i64);
            DataPoint {
                id: Uuid::new_v4(),
                series_id,
                date,
                value: Some(DecimalScalar(Decimal::new(1000 + i as i64, 0))),
                revision_date: date,
                is_original_release: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        })
        .collect()
}

/// Benchmark Arrow RecordBatch creation performance
fn bench_arrow_record_batch_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("arrow_record_batch_creation");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("series", size), size, |b, &size| {
            b.iter(|| {
                let series = create_test_series(Uuid::new_v4(), Uuid::new_v4(), size);
                black_box(ParquetStorage::series_to_record_batch(&series).unwrap())
            })
        });

        group.bench_with_input(BenchmarkId::new("data_points", size), size, |b, &size| {
            let series_id = Uuid::new_v4();
            let data_points = create_test_data_points(series_id, size);

            b.iter(|| black_box(ParquetStorage::data_points_to_record_batch(&data_points).unwrap()))
        });
    }

    group.finish();
}

/// Benchmark Parquet file write performance
fn bench_parquet_write_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("parquet_write_performance");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("series_write", size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
            let series = create_test_series(Uuid::new_v4(), Uuid::new_v4(), size);

            b.iter(|| {
                rt.block_on(async {
                    let _: () = storage.write_series(&series).await.unwrap();
                    black_box(())
                })
            })
        });

        group.bench_with_input(
            BenchmarkId::new("data_points_write", size),
            size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
                let series_id = Uuid::new_v4();
                let data_points = create_test_data_points(series_id, size);

                b.iter(|| {
                    rt.block_on(async {
                        let _: () = storage
                            .write_data_points(series_id, &data_points)
                            .await
                            .unwrap();
                        black_box(())
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark Parquet file read performance
fn bench_parquet_read_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("parquet_read_performance");

    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("series_read", size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
            let series = create_test_series(Uuid::new_v4(), Uuid::new_v4(), size);

            // Pre-write the data
            rt.block_on(async { storage.write_series(&series).await.unwrap() });

            b.iter(|| {
                rt.block_on(async { black_box(storage.read_series(series.id).await.unwrap()) })
            })
        });

        group.bench_with_input(
            BenchmarkId::new("data_points_read", size),
            size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
                let series_id = Uuid::new_v4();
                let data_points = create_test_data_points(series_id, size);

                // Pre-write the data
                rt.block_on(async {
                    storage
                        .write_data_points(series_id, &data_points)
                        .await
                        .unwrap()
                });

                b.iter(|| {
                    rt.block_on(async {
                        black_box(
                            storage
                                .read_data_points(series_id, None, None)
                                .await
                                .unwrap(),
                        )
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");

    for concurrency in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_writes", concurrency),
            concurrency,
            |b, &concurrency| {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParquetStorage::new(temp_dir.path().to_path_buf());

                b.iter(|| {
                    rt.block_on(async {
                        let handles: Vec<_> = (0..concurrency)
                            .map(|_| {
                                let storage = storage.clone();
                                let series =
                                    create_test_series(Uuid::new_v4(), Uuid::new_v4(), 100);
                                tokio::spawn(async move { storage.write_series(&series).await })
                            })
                            .collect();

                        for handle in handles {
                            let _: () = handle.await.unwrap().unwrap();
                            black_box(());
                        }
                    })
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", concurrency),
            concurrency,
            |b, &concurrency| {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
                let series_id = Uuid::new_v4();
                let data_points = create_test_data_points(series_id, 1000);

                // Pre-write the data
                rt.block_on(async {
                    storage
                        .write_data_points(series_id, &data_points)
                        .await
                        .unwrap()
                });

                b.iter(|| {
                    rt.block_on(async {
                        let handles: Vec<_> = (0..concurrency)
                            .map(|_| {
                                let storage = storage.clone();
                                tokio::spawn(async move {
                                    storage.read_data_points(series_id, None, None).await
                                })
                            })
                            .collect();

                        for handle in handles {
                            let _: Vec<_> = handle.await.unwrap().unwrap();
                            black_box(());
                        }
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_usage");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("memory_efficiency", size),
            size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();
                let storage = ParquetStorage::new(temp_dir.path().to_path_buf());
                let series_id = Uuid::new_v4();
                let data_points = create_test_data_points(series_id, size);

                b.iter(|| {
                    rt.block_on(async {
                        // Write data
                        storage
                            .write_data_points(series_id, &data_points)
                            .await
                            .unwrap();

                        // Read data multiple times to test memory efficiency
                        for _ in 0..10 {
                            let _result = storage
                                .read_data_points(series_id, None, None)
                                .await
                                .unwrap();
                        }

                        black_box(())
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark GraphQL query performance
fn bench_graphql_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("graphql_performance");

    for query_complexity in [1, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("graphql_queries", query_complexity),
            query_complexity,
            |b, &complexity| {
                let temp_dir = TempDir::new().unwrap();
                let database = rt.block_on(async { Database::new_in_memory().await.unwrap() });

                // Pre-populate with test data
                rt.block_on(async {
                    for i in 0..complexity {
                        let series = create_test_series(Uuid::new_v4(), Uuid::new_v4(), 100);
                        database.create_series(series).await.unwrap();
                    }
                });

                b.iter(|| {
                    rt.block_on(async {
                        // Simulate GraphQL query execution
                        let series_list = database.list_series().await.unwrap();
                        black_box(series_list)
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark Arrow Flight concepts
fn bench_arrow_flight_concepts(c: &mut Criterion) {
    let mut group = c.benchmark_group("arrow_flight_concepts");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("arrow_schema_creation", size),
            size,
            |b, &size| {
                b.iter(|| {
                    // Benchmark Arrow schema creation
                    let _series_schema = ParquetStorage::series_arrow_schema();
                    let _data_points_schema = ParquetStorage::data_points_arrow_schema();
                    black_box(())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("arrow_record_batch_operations", size),
            size,
            |b, &size| {
                let data_points = create_test_data_points(Uuid::new_v4(), size);

                b.iter(|| {
                    let batch = ParquetStorage::data_points_to_record_batch(&data_points).unwrap();
                    let num_rows = batch.num_rows();
                    let num_columns = batch.num_columns();
                    black_box((num_rows, num_columns))
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_arrow_record_batch_creation,
    bench_parquet_write_performance,
    bench_parquet_read_performance,
    bench_concurrent_operations,
    bench_memory_usage,
    bench_graphql_performance,
    bench_arrow_flight_concepts
);

criterion_main!(benches);
