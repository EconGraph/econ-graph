// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Chunked catalog discovery run metrics, served by `crawler-worker`.
//!
//! Counted by the worker as runs and chunk jobs commit:
//! - `crawler_discovery_runs_total{source,outcome}`: runs by outcome (`started`, `complete`,
//!   `failed`, `superseded`).
//! - `crawler_discovery_chunks_total{source,outcome}`: chunk jobs by outcome (`completed`,
//!   `failed`, `discarded`: the chunk's run had already failed or been superseded, or the chunk
//!   was already done).
//! - `crawler_discovery_chunk_duration_seconds{source}`: time one chunk's upstream calls took.
//! - `crawler_discovery_run_duration_seconds{source}`: start to completion of complete runs.
//!
//! Polled from `discovery_runs` with the queue gauges:
//! - `crawler_discovery_run_chunks{source,state}`: the newest run's chunk counts (`total`,
//!   `done`, `failed`).
//! - `crawler_discovery_run_active{source}`: 1 while the newest run is enumerating, running or
//!   finalizing, else 0.
//! - `crawler_discovery_last_complete_run_timestamp_seconds{source}`: when the newest complete
//!   run finished. A source with no complete run reports when its oldest retained run started, so
//!   a source whose runs never complete still ages.
//!
//! Only sources whose adapter discovers in chunks have runs, so only they have these series.

use once_cell::sync::Lazy;
use prometheus::{
    GaugeVec, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
};

use crate::DEFAULT_REGISTRY;

/// See the [module docs](self).
pub struct CrawlerDiscoveryRunMetrics {
    /// Runs by source and outcome.
    pub runs_total: IntCounterVec,
    /// Chunk jobs by source and outcome.
    pub chunks_total: IntCounterVec,
    /// Upstream time of one chunk, by source.
    pub chunk_duration_seconds: HistogramVec,
    /// Start to completion of a complete run, by source.
    pub run_duration_seconds: HistogramVec,
    /// The newest run's chunk counts, by source and state (`total`, `done`, `failed`).
    pub run_chunks: IntGaugeVec,
    /// 1 while the source's newest run is not final, else 0.
    pub run_active: IntGaugeVec,
    /// When the source's newest complete run finished (or its oldest retained run started).
    pub last_complete_run_timestamp_seconds: GaugeVec,
}

impl CrawlerDiscoveryRunMetrics {
    /// Creates the run metrics and registers them with `registry`.
    ///
    /// # Errors
    ///
    /// Returns an error if any metric fails to register with the provided registry
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let runs_total = IntCounterVec::new(
            Opts::new(
                "crawler_discovery_runs_total",
                "Chunked discovery runs by outcome (started, complete, failed, superseded)",
            ),
            &["source", "outcome"],
        )?;
        registry.register(Box::new(runs_total.clone()))?;
        let chunks_total = IntCounterVec::new(
            Opts::new(
                "crawler_discovery_chunks_total",
                "Chunked discovery chunk jobs by outcome (completed, failed, discarded)",
            ),
            &["source", "outcome"],
        )?;
        registry.register(Box::new(chunks_total.clone()))?;
        let chunk_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "crawler_discovery_chunk_duration_seconds",
                "Time the upstream calls of one discovery chunk took",
            )
            .buckets(vec![
                1.0, 5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1200.0,
            ]),
            &["source"],
        )?;
        registry.register(Box::new(chunk_duration_seconds.clone()))?;
        let run_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "crawler_discovery_run_duration_seconds",
                "Start to completion of chunked discovery runs that completed",
            )
            .buckets(vec![
                60.0, 300.0, 900.0, 1800.0, 3600.0, 7200.0, 14400.0, 43200.0, 86400.0, 259_200.0,
            ]),
            &["source"],
        )?;
        registry.register(Box::new(run_duration_seconds.clone()))?;
        let run_chunks = IntGaugeVec::new(
            Opts::new(
                "crawler_discovery_run_chunks",
                "Chunk counts of the newest chunked discovery run, by state (total, done, failed)",
            ),
            &["source", "state"],
        )?;
        registry.register(Box::new(run_chunks.clone()))?;
        let run_active = IntGaugeVec::new(
            Opts::new(
                "crawler_discovery_run_active",
                "1 while the newest chunked discovery run is enumerating, running or finalizing",
            ),
            &["source"],
        )?;
        registry.register(Box::new(run_active.clone()))?;
        let last_complete_run_timestamp_seconds = GaugeVec::new(
            Opts::new(
                "crawler_discovery_last_complete_run_timestamp_seconds",
                "Unix time the newest complete chunked discovery run finished (with no complete \
                 run: when the oldest retained run started)",
            ),
            &["source"],
        )?;
        registry.register(Box::new(last_complete_run_timestamp_seconds.clone()))?;
        Ok(Self {
            runs_total,
            chunks_total,
            chunk_duration_seconds,
            run_duration_seconds,
            run_chunks,
            run_active,
            last_complete_run_timestamp_seconds,
        })
    }
}

/// Global run metrics, registered with [`DEFAULT_REGISTRY`].
///
/// # Panics
///
/// Panics if the metrics fail to initialize during lazy initialization
pub static CRAWLER_DISCOVERY_RUN_METRICS: Lazy<CrawlerDiscoveryRunMetrics> = Lazy::new(|| {
    CrawlerDiscoveryRunMetrics::new(&DEFAULT_REGISTRY)
        .expect("Failed to initialize crawler discovery run metrics")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_run_metrics_register() {
        let registry = Registry::new();
        let m = CrawlerDiscoveryRunMetrics::new(&registry).unwrap();
        m.runs_total.with_label_values(&["FRED", "started"]).inc();
        m.chunks_total
            .with_label_values(&["FRED", "completed"])
            .inc();
        m.chunk_duration_seconds
            .with_label_values(&["FRED"])
            .observe(2.0);
        m.run_duration_seconds
            .with_label_values(&["FRED"])
            .observe(60.0);
        m.run_chunks.with_label_values(&["FRED", "total"]).set(3);
        m.run_active.with_label_values(&["FRED"]).set(1);
        m.last_complete_run_timestamp_seconds
            .with_label_values(&["FRED"])
            .set(1.0);
        let names: Vec<String> = registry
            .gather()
            .iter()
            .map(|f| f.name().to_string())
            .collect();
        for n in [
            "crawler_discovery_runs_total",
            "crawler_discovery_chunks_total",
            "crawler_discovery_chunk_duration_seconds",
            "crawler_discovery_run_duration_seconds",
            "crawler_discovery_run_chunks",
            "crawler_discovery_run_active",
            "crawler_discovery_last_complete_run_timestamp_seconds",
        ] {
            assert!(names.contains(&n.to_string()), "{n} missing: {names:?}");
        }
    }
}
