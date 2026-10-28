# EconGraph Metrics

This crate defines Prometheus metrics for EconGraph crawlers and provides the shared `DEFAULT_REGISTRY`. Crawlers and the refresh scheduler record observations through these metrics; the crawler worker polls the database for queue and coverage gauges and exposes the shared registry at `/metrics`.

## Metrics and registration

The [crawler module](src/crawler.rs) defines five groups:

- **HTTP requests (`CrawlerMetrics`)**: Request counts and duration histograms, downloaded bytes, collected items, errors, rate limit hits, retries, and timeouts.
- **Queue (`CrawlerQueueMetrics`)**: Job outcomes, queue counts by source and status, recent failures, and the latest success and failure timestamps.
- **Coverage (`CrawlerCoverageMetrics`)**: Discovered series, series with data, coverage ratios, overdue series, and freshness timestamps.
- **Discovery (`CrawlerDiscoveryMetrics`)**: Curated or configured entries missing from the source's catalog at the last discovery.
- **Refresh scheduler (`CrawlerSchedulerMetrics`)**: Scheduling passes, jobs enqueued, and the latest successful discovery in retained queue history.

Each group's `new(&Registry)` constructor registers its metrics with the supplied registry. The lazy globals `CRAWLER_METRICS`, `CRAWLER_QUEUE_METRICS`, `CRAWLER_COVERAGE_METRICS`, `CRAWLER_DISCOVERY_METRICS`, and `CRAWLER_SCHEDULER_METRICS` register with `DEFAULT_REGISTRY` when initialized. Callers use recording helpers or set the metric fields directly to supply observations.

The [worker metrics module](../econ-graph-crawler-worker/src/metrics.rs) implements the HTTP endpoint and database polling. See the [monitoring guide](../../../docs/monitoring/README.md) for scrape configuration and operational guidance.

## Testing

The three unit tests in `crawler::queue_metrics_tests` each create a fresh Prometheus registry, register queue, coverage, or discovery metrics, and record sample observations. They gather the registry's metric families and assert that selected family names are present.

From `backend/`, run the package's tests:

```bash
cargo test -p econ-graph-metrics
```

To select just those three unit tests:

```bash
cargo test -p econ-graph-metrics queue_metrics_tests
```

## License

The repository's [LICENSE](../../../LICENSE) is headed Microsoft Reference Source License (MS-RSL). See that file for the terms and conditions.
