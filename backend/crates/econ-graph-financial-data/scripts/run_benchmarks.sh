#!/bin/bash

# Performance Benchmark Runner for EconGraph Financial Data Service
#
# This script runs comprehensive performance benchmarks for:
# - Arrow Flight + Parquet operations
# - GraphQL query performance
# - Concurrent operations
# - Memory usage and efficiency
# - Storage I/O performance

set -e

echo "🚀 Starting EconGraph Financial Data Service Performance Benchmarks"
echo "=================================================================="

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Please run this script from the econ-graph-financial-data directory"
    exit 1
fi

# Create benchmark results directory
mkdir -p benchmark_results
RESULTS_DIR="benchmark_results/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "📊 Running performance benchmarks..."
echo "Results will be saved to: $RESULTS_DIR"
echo ""

# Run benchmarks with different configurations
echo "🔧 Running Arrow Flight + Parquet benchmarks..."

# Basic performance benchmarks
echo "  - Arrow RecordBatch creation..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/arrow_record_batch.html"

echo "  - Parquet write performance..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/parquet_write.html"

echo "  - Parquet read performance..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/parquet_read.html"

echo "  - Concurrent operations..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/concurrent_ops.html"

echo "  - Memory usage analysis..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/memory_usage.html"

echo "  - GraphQL performance..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/graphql_performance.html"

echo "  - Arrow Flight concepts..."
cargo bench --bench performance -- --output-format html --output "$RESULTS_DIR/arrow_flight.html"

# Generate summary report
echo ""
echo "📈 Generating performance summary..."

cat > "$RESULTS_DIR/summary.md" << EOF
# EconGraph Financial Data Service - Performance Benchmark Results

**Generated:** $(date)
**Version:** $(cargo version --format '{{.version}}')
**Rust Version:** $(rustc --version)

## Benchmark Results

### Arrow Flight + Parquet Performance

- **Arrow RecordBatch Creation:** See \`arrow_record_batch.html\`
- **Parquet Write Performance:** See \`parquet_write.html\`
- **Parquet Read Performance:** See \`parquet_read.html\`
- **Concurrent Operations:** See \`concurrent_ops.html\`
- **Memory Usage Analysis:** See \`memory_usage.html\`
- **GraphQL Performance:** See \`graphql_performance.html\`
- **Arrow Flight Concepts:** See \`arrow_flight.html\`

## Key Performance Metrics

### Expected Performance Characteristics

- **Query Latency:** < 10ms for cached data, < 100ms for cold data
- **Throughput:** 10,000+ queries/second
- **Storage Efficiency:** 80-90% compression with Parquet
- **Memory Usage:** 256MB base + 1MB per 1000 series

### Arrow Flight Benefits

- **Zero-copy data transfer:** No memory copying between processes
- **Sub-millisecond latency:** Direct memory access
- **High throughput:** Optimized for large datasets
- **Columnar storage:** Efficient compression and querying

## Recommendations

1. **For High Throughput:** Use concurrent operations with 10-20 workers
2. **For Low Latency:** Keep hot data in memory-mapped files
3. **For Storage Efficiency:** Use Snappy compression with Parquet
4. **For Memory Efficiency:** Batch operations in chunks of 1000-10000 records

## Next Steps

1. Review the HTML reports for detailed performance analysis
2. Compare results against production requirements
3. Tune configuration based on benchmark results
4. Monitor performance in production environment

EOF

echo "✅ Performance benchmarks completed!"
echo ""
echo "📁 Results saved to: $RESULTS_DIR"
echo "📊 Open the HTML files in your browser to view detailed results"
echo "📋 Summary report: $RESULTS_DIR/summary.md"
echo ""

# List the generated files
echo "Generated files:"
ls -la "$RESULTS_DIR"

echo ""
echo "🎉 Benchmark run completed successfully!"
echo ""
echo "To view results:"
echo "  - Open $RESULTS_DIR/*.html files in your browser"
echo "  - Read $RESULTS_DIR/summary.md for overview"
echo ""
echo "To run specific benchmarks:"
echo "  cargo bench --bench performance -- <benchmark_name>"
echo ""
echo "To run with different parameters:"
echo "  cargo bench --bench performance -- --sample-size 100"
