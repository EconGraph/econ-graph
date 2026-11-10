// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! SEC companyfacts nightly load gauges, served by `crawler-worker` and polled from
//! `company_fact_load_runs` and `company_fact_conflicts` with the coverage gauges. The load
//! itself is a CronJob with no scrape target, so its outcome is read back from the database.
//!
//! - `crawler_companyfacts_last_run_timestamp_seconds`: when the newest finished load
//!   completed (a load still running after the 8-hour Job deadline counts as finished). 0 when
//!   none has run.
//! - `crawler_companyfacts_last_run_failed`: 1 when that load failed or never finished.
//! - `crawler_companyfacts_last_run_conflicts`: open conflicts that load saw; each left a
//!   stored row unchanged.
//! - `crawler_companyfacts_last_run_rows_written{table}`: rows that load inserted or updated in
//!   `company_facts` and `company_fact_filings`. A night with few new filings writes few rows;
//!   a value near the table size means unchanged rows are being rewritten again.
//! - `crawler_companyfacts_last_run_archive_stale`: 1 when that load's SEC archive was more than
//!   24 hours old at completion.
//! - `crawler_companyfacts_last_success_timestamp_seconds`: completion of the newest successful
//!   downloaded load (with or without conflicts). 0 when there is none.
//! - `crawler_companyfacts_open_conflicts`: conflicts awaiting an operator decision.
//! - `crawler_companyfacts_last_refresh_timestamp_seconds`: when these gauges were last read.

use once_cell::sync::Lazy;
use prometheus::{Gauge, IntGauge, IntGaugeVec, Opts, Registry};

use crate::DEFAULT_REGISTRY;

/// See the [module docs](self).
pub struct CompanyfactsMetrics {
    pub last_run_timestamp_seconds: Gauge,
    pub last_run_failed: IntGauge,
    pub last_run_conflicts: IntGauge,
    pub last_run_rows_written: IntGaugeVec,
    pub last_run_archive_stale: IntGauge,
    pub last_success_timestamp_seconds: Gauge,
    pub open_conflicts: IntGauge,
    pub last_refresh_timestamp_seconds: Gauge,
}

impl CompanyfactsMetrics {
    /// Creates the companyfacts metrics and registers them with `registry`.
    ///
    /// # Errors
    ///
    /// Returns an error if any metric fails to register with the provided registry
    pub fn new(registry: &Registry) -> anyhow::Result<Self> {
        let gauge = |name: &str, help: &str| -> anyhow::Result<Gauge> {
            let g = Gauge::with_opts(Opts::new(name, help))?;
            registry.register(Box::new(g.clone()))?;
            Ok(g)
        };
        let int_gauge = |name: &str, help: &str| -> anyhow::Result<IntGauge> {
            let g = IntGauge::with_opts(Opts::new(name, help))?;
            registry.register(Box::new(g.clone()))?;
            Ok(g)
        };
        let last_run_rows_written = IntGaugeVec::new(
            Opts::new(
                "crawler_companyfacts_last_run_rows_written",
                "Rows the newest finished SEC companyfacts load inserted or updated, by table",
            ),
            &["table"],
        )?;
        registry.register(Box::new(last_run_rows_written.clone()))?;
        Ok(Self {
            last_run_timestamp_seconds: gauge(
                "crawler_companyfacts_last_run_timestamp_seconds",
                "Unix time the newest finished SEC companyfacts load completed (0: none)",
            )?,
            last_run_failed: int_gauge(
                "crawler_companyfacts_last_run_failed",
                "1 when the newest SEC companyfacts load failed or did not finish within 8 hours",
            )?,
            last_run_conflicts: int_gauge(
                "crawler_companyfacts_last_run_conflicts",
                "Open conflicts the newest finished SEC companyfacts load recorded",
            )?,
            last_run_rows_written,
            last_run_archive_stale: int_gauge(
                "crawler_companyfacts_last_run_archive_stale",
                "1 when the newest finished SEC companyfacts load used an archive over 24 hours old",
            )?,
            last_success_timestamp_seconds: gauge(
                "crawler_companyfacts_last_success_timestamp_seconds",
                "Unix time the newest successful downloaded SEC companyfacts load completed (0: none)",
            )?,
            open_conflicts: int_gauge(
                "crawler_companyfacts_open_conflicts",
                "SEC companyfacts conflicts awaiting an operator decision",
            )?,
            last_refresh_timestamp_seconds: gauge(
                "crawler_companyfacts_last_refresh_timestamp_seconds",
                "Unix time the crawler_companyfacts_* gauges were last read from the database",
            )?,
        })
    }
}

/// Global companyfacts metrics, registered with [`DEFAULT_REGISTRY`].
///
/// # Panics
///
/// Panics if the metrics fail to initialize during lazy initialization
pub static COMPANYFACTS_METRICS: Lazy<CompanyfactsMetrics> = Lazy::new(|| {
    CompanyfactsMetrics::new(&DEFAULT_REGISTRY)
        .expect("Failed to initialize SEC companyfacts metrics")
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn companyfacts_metrics_register() {
        let registry = Registry::new();
        let m = CompanyfactsMetrics::new(&registry).unwrap();
        m.last_run_rows_written
            .with_label_values(&["company_facts"])
            .set(3);
        let names: Vec<String> = registry
            .gather()
            .iter()
            .map(|f| f.name().to_string())
            .collect();
        for n in [
            "crawler_companyfacts_last_run_timestamp_seconds",
            "crawler_companyfacts_last_run_failed",
            "crawler_companyfacts_last_run_conflicts",
            "crawler_companyfacts_last_run_rows_written",
            "crawler_companyfacts_last_run_archive_stale",
            "crawler_companyfacts_last_success_timestamp_seconds",
            "crawler_companyfacts_open_conflicts",
            "crawler_companyfacts_last_refresh_timestamp_seconds",
        ] {
            assert!(names.iter().any(|x| x == n), "missing {n}");
        }
    }
}
