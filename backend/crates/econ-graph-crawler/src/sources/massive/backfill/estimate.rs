// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Object-store estimate for a Massive backfill (`crawler massive-backfill-estimate`).
//!
//! Rows are tickers × fields × trading days. Bytes come from [`MEASURED`], a model fitted to
//! what the real Iceberg writer stores (the `measured_storage_model_matches_the_writer` test
//! re-measures it): Parquet bytes per row and per data file, manifest bytes per commit, and the
//! table metadata file, which every commit rewrites with one more snapshot in it. Nothing
//! deletes old objects (docs/technical/ICEBERG_STORAGE.md), so the estimate adds:
//!
//! - **Compaction copies.** Each compaction writes a full new copy of the logical rows and keeps
//!   the old objects.
//! - **Daily refresh between compactions.** The worker's Iceberg mirror re-appends each
//!   refreshed series' whole PostgreSQL history on every publish (ECO-875). The backfill's rows
//!   are only in Iceberg, so only the daily crawl's own days are re-appended, but that history
//!   grows by a day per day, so the copies grow quadratically until a compaction.
//!
//! The approval rule (ECO-214): the run needs Joe's approval when the total is more than 10% of
//! the free space.

use std::path::Path;

use anyhow::{bail, Context as _};
use serde::Serialize;

use super::{FetchMode, FIELDS};

/// Bytes the Iceberg writer stores, as a linear model.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct StorageModel {
    /// Parquet bytes per observation row.
    pub data_bytes_per_row: f64,
    /// Parquet bytes per data file, beyond its rows (footer, schema, statistics).
    pub data_file_overhead_bytes: f64,
    /// Manifest and manifest-list bytes per commit, beyond the per-file entries.
    pub commit_overhead_bytes: f64,
    /// Manifest bytes per data file entry.
    pub manifest_bytes_per_file: f64,
    /// Table metadata file bytes with no snapshots.
    pub metadata_base_bytes: f64,
    /// Table metadata file bytes added by each retained snapshot.
    pub metadata_bytes_per_snapshot: f64,
}

/// The writer's storage, measured by `measured_storage_model_matches_the_writer` with a fake
/// random-walk price series. Re-measure after changing the Iceberg schema or writer settings.
pub const MEASURED: StorageModel = StorageModel {
    data_bytes_per_row: 70.0,
    data_file_overhead_bytes: 4_000.0,
    commit_overhead_bytes: 8_000.0,
    manifest_bytes_per_file: 400.0,
    metadata_base_bytes: 6_000.0,
    metadata_bytes_per_snapshot: 600.0,
};

/// What to estimate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EstimateInput {
    /// Ticker segments (share classes, plus one per ticker change).
    pub tickers: usize,
    /// History depth in years (for the report).
    pub years: u32,
    /// Trading days in the window.
    pub trading_days: usize,
    /// Request mode.
    pub mode: FetchMode,
    /// Observations per commit.
    pub batch_rows: usize,
    /// Compactions that will each write a full copy while the old objects are kept.
    pub compactions: u32,
    /// Days of daily refresh after the backfill before a compaction.
    pub refresh_days: u32,
}

/// The estimate, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Estimate {
    /// The input.
    pub input: EstimateInput,
    /// Logical observation rows.
    pub rows: u64,
    /// Iceberg commits the backfill makes.
    pub commits: u64,
    /// Data files the backfill writes.
    pub files: u64,
    /// Parquet data the backfill writes.
    pub data_bytes: f64,
    /// Manifests and table metadata files the backfill writes.
    pub metadata_bytes: f64,
    /// Full copies written by compactions.
    pub compaction_bytes: f64,
    /// The backfill's own share: data, metadata and compaction copies.
    pub backfill_bytes: f64,
    /// Daily-refresh re-appends before the next compaction.
    pub refresh_bytes: f64,
    /// Everything above.
    pub total_bytes: f64,
}

/// Estimates the object store bytes of a backfill, with `model`.
pub fn estimate(input: EstimateInput, model: &StorageModel) -> anyhow::Result<Estimate> {
    let series = (input.tickers * FIELDS) as u64;
    let days = input.trading_days as u64;
    let rows = series * days;
    let batch = input.batch_rows as u64;
    let (commits, files) = match input.mode {
        FetchMode::TickerRange => {
            // Whole share classes per commit: each series lands in one file.
            let per_ticker = (FIELDS as u64) * days;
            if per_ticker > batch {
                bail!("one ticker's {per_ticker} rows exceed the batch of {batch}");
            }
            let tickers_per_commit = batch / per_ticker.max(1);
            (
                (input.tickers as u64).div_ceil(tickers_per_commit.max(1)),
                series,
            )
        }
        FetchMode::GroupedDaily => {
            // Whole days per commit: every commit writes a file for every series.
            let per_day = series;
            if per_day > batch {
                bail!("one day's {per_day} rows exceed the batch of {batch}");
            }
            let commits = days.div_ceil((batch / per_day.max(1)).max(1));
            (commits, commits * series)
        }
    };
    let data_bytes =
        rows as f64 * model.data_bytes_per_row + files as f64 * model.data_file_overhead_bytes;
    let metadata_bytes = metadata_chain(model, 0, commits)
        + commits as f64 * model.commit_overhead_bytes
        + files as f64 * model.manifest_bytes_per_file;
    // Each compaction: one file per series holding every logical row, plus its own commit.
    let compaction_once = rows as f64 * model.data_bytes_per_row
        + series as f64 * (model.data_file_overhead_bytes + model.manifest_bytes_per_file)
        + model.commit_overhead_bytes;
    let compaction_bytes = f64::from(input.compactions) * compaction_once
        + metadata_chain(model, commits, u64::from(input.compactions));
    // Day k of the daily refresh re-appends k days for every series, in one file per series and
    // one commit per series (the worker publishes each series on its own).
    let refresh_days = u64::from(input.refresh_days);
    let refresh_rows = series * refresh_days * (refresh_days + 1) / 2;
    let refresh_commits = series * refresh_days;
    let refresh_bytes = refresh_rows as f64 * model.data_bytes_per_row
        + refresh_commits as f64
            * (model.data_file_overhead_bytes
                + model.manifest_bytes_per_file
                + model.commit_overhead_bytes)
        + metadata_chain(
            model,
            commits + u64::from(input.compactions),
            refresh_commits,
        );
    let backfill_bytes = data_bytes + metadata_bytes + compaction_bytes;
    let total_bytes = backfill_bytes + refresh_bytes;
    Ok(Estimate {
        input,
        rows,
        commits,
        files,
        data_bytes,
        metadata_bytes,
        compaction_bytes,
        backfill_bytes,
        refresh_bytes,
        total_bytes,
    })
}

/// Bytes of the table metadata files written by `commits` commits after `before` earlier ones:
/// each rewrites the file with every snapshot so far.
fn metadata_chain(model: &StorageModel, before: u64, commits: u64) -> f64 {
    // Sum over k = before+1 ..= before+commits of (base + k × per_snapshot).
    let first = before + 1;
    let last = before + commits;
    let snapshots = if commits == 0 {
        0.0
    } else {
        (first + last) as f64 * commits as f64 / 2.0
    };
    commits as f64 * model.metadata_base_bytes + snapshots * model.metadata_bytes_per_snapshot
}

/// Free space to compare the estimate with.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct FreeSpace {
    /// Bytes available.
    pub available: f64,
    /// Filesystem size, when known (from a path), for the 80% alert line.
    pub capacity: Option<f64>,
}

/// Parses `--free`: a byte count (with an optional K, M, G or T binary suffix), or a path whose
/// filesystem `df` reports on.
pub fn free_space(arg: &str) -> anyhow::Result<FreeSpace> {
    if let Some(bytes) = parse_bytes(arg) {
        return Ok(FreeSpace {
            available: bytes,
            capacity: None,
        });
    }
    let path = Path::new(arg);
    if !path.exists() {
        bail!("--free {arg:?} is neither a byte count nor an existing path");
    }
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .context("running df")?;
    if !out.status.success() {
        bail!(
            "df {arg:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    parse_df(&String::from_utf8_lossy(&out.stdout))
}

fn parse_bytes(s: &str) -> Option<f64> {
    let s = s.trim();
    let (digits, scale) = match s.char_indices().last()? {
        (i, 'K' | 'k') => (&s[..i], 1u64 << 10),
        (i, 'M' | 'm') => (&s[..i], 1 << 20),
        (i, 'G' | 'g') => (&s[..i], 1 << 30),
        (i, 'T' | 't') => (&s[..i], 1 << 40),
        _ => (s, 1),
    };
    let n: f64 = digits.trim().parse().ok()?;
    (n >= 0.0).then_some(n * scale as f64)
}

/// `df -Pk` output: a header, then `filesystem 1024-blocks used available capacity mount`.
fn parse_df(out: &str) -> anyhow::Result<FreeSpace> {
    let line = out
        .lines()
        .nth(1)
        .context("df printed no filesystem line")?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let kib = |i: usize| -> anyhow::Result<f64> {
        let v: f64 = fields
            .get(i)
            .context("short df line")?
            .parse()
            .context("df field is not a number")?;
        Ok(v * 1024.0)
    };
    Ok(FreeSpace {
        available: kib(3)?,
        capacity: Some(kib(1)?),
    })
}

/// Bytes in binary units, e.g. `1.23 GiB`.
pub fn human(bytes: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{v:.0} B")
    } else {
        format!("{v:.2} {}", UNITS[unit])
    }
}

/// The estimate as a short text table, compared with `free` when given.
pub fn to_text(estimates: &[Estimate], free: Option<FreeSpace>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:>5} {:>7} {:>5} {:>12} {:>7} {:>11} {:>11} {:>11} {:>11} {:>11} {:>11}  VS FREE",
        "YEARS",
        "TICKERS",
        "DAYS",
        "ROWS",
        "COMMITS",
        "DATA",
        "METADATA",
        "COMPACTION",
        "BACKFILL",
        "REFRESH",
        "TOTAL"
    );
    for e in estimates {
        let verdict = free.map_or_else(String::new, |f| verdict(e.total_bytes, f));
        let _ = writeln!(
            out,
            "{:>5} {:>7} {:>5} {:>12} {:>7} {:>11} {:>11} {:>11} {:>11} {:>11} {:>11}  {}",
            e.input.years,
            e.input.tickers,
            e.input.trading_days,
            e.rows,
            e.commits,
            human(e.data_bytes),
            human(e.metadata_bytes),
            human(e.compaction_bytes),
            human(e.backfill_bytes),
            human(e.refresh_bytes),
            human(e.total_bytes),
            verdict
        );
    }
    if let Some(e) = estimates.first() {
        let _ = writeln!(
            out,
            "\nmode {}, {} rows per commit, {} compaction copies kept, {} refresh days before \
             compaction (ECO-875 re-appends)",
            e.input.mode.as_str(),
            e.input.batch_rows,
            e.input.compactions,
            e.input.refresh_days
        );
    }
    if let Some(f) = free {
        let _ = write!(out, "free: {}", human(f.available));
        if let Some(capacity) = f.capacity {
            let _ = write!(
                out,
                " of {}; the 80% alert line leaves {}",
                human(capacity),
                human(alert_headroom(f).unwrap_or(0.0))
            );
        }
        let _ = writeln!(
            out,
            "\nJoe approves the run when its total is more than 10% of free space."
        );
    }
    out
}

/// Bytes left before the 80% used alert line, when the capacity is known.
fn alert_headroom(free: FreeSpace) -> Option<f64> {
    free.capacity
        .map(|capacity| (free.available - 0.2 * capacity).max(0.0))
}

fn verdict(total: f64, free: FreeSpace) -> String {
    let share = if free.available > 0.0 {
        100.0 * total / free.available
    } else {
        f64::INFINITY
    };
    let mut s = format!("{share:.1}% of free");
    if alert_headroom(free).is_some_and(|room| total > room) {
        s.push_str(", crosses the 80% alert line");
    }
    if share > 10.0 {
        s.push_str(", needs approval");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(mode: FetchMode, tickers: usize, days: usize) -> EstimateInput {
        EstimateInput {
            tickers,
            years: 1,
            trading_days: days,
            mode,
            batch_rows: 100_000,
            compactions: 1,
            refresh_days: 0,
        }
    }

    const UNIT: StorageModel = StorageModel {
        data_bytes_per_row: 1.0,
        data_file_overhead_bytes: 10.0,
        commit_overhead_bytes: 100.0,
        manifest_bytes_per_file: 0.0,
        metadata_base_bytes: 1000.0,
        metadata_bytes_per_snapshot: 1.0,
    };

    #[test]
    fn ticker_mode_packs_whole_tickers_into_each_commit() {
        // 10 tickers × 6 fields × 252 days = 15,120 rows; 1,512 per ticker, 66 per 100k commit.
        let e = estimate(input(FetchMode::TickerRange, 10, 252), &UNIT).unwrap();
        assert_eq!((e.rows, e.commits, e.files), (15_120, 1, 60));
        assert_eq!(e.data_bytes, 15_120.0 + 600.0);
        assert_eq!(e.metadata_bytes, 1000.0 + 1.0 + 100.0);
        // One compaction: a copy of every row, a file per series, one more commit (snapshot 2).
        assert_eq!(e.compaction_bytes, 15_120.0 + 600.0 + 100.0 + 1000.0 + 2.0);
        let e = estimate(input(FetchMode::TickerRange, 503, 1258), &UNIT).unwrap();
        // 7,548 rows per ticker: 13 per commit, 39 commits.
        assert_eq!(e.commits, 39);
    }

    #[test]
    fn grouped_mode_writes_a_file_per_series_per_commit() {
        // 503 × 6 = 3,018 rows a day; 33 days per 100k commit; 252 days need 8 commits.
        let e = estimate(input(FetchMode::GroupedDaily, 503, 252), &UNIT).unwrap();
        assert_eq!((e.commits, e.files), (8, 8 * 3_018));
    }

    #[test]
    fn refresh_re_appends_a_growing_history() {
        let mut i = input(FetchMode::TickerRange, 1, 10);
        i.refresh_days = 3;
        i.compactions = 0;
        let e = estimate(i, &UNIT).unwrap();
        // 6 series × (1 + 2 + 3) days; 18 commits, each with a file and a commit overhead.
        let chain = metadata_chain(&UNIT, 1, 18);
        assert_eq!(e.refresh_bytes, 36.0 + 18.0 * 110.0 + chain);
    }

    #[test]
    fn metadata_chain_sums_growing_snapshot_lists() {
        assert_eq!(metadata_chain(&UNIT, 0, 3), 3000.0 + 6.0);
        assert_eq!(metadata_chain(&UNIT, 2, 2), 2000.0 + 7.0);
        assert_eq!(metadata_chain(&UNIT, 5, 0), 0.0);
    }

    #[test]
    fn a_batch_smaller_than_one_unit_is_refused() {
        let mut i = input(FetchMode::TickerRange, 1, 252);
        i.batch_rows = 1000;
        estimate(i, &UNIT).unwrap_err();
    }

    #[test]
    fn free_space_parses_byte_counts_and_df() {
        assert_eq!(
            free_space("2G").unwrap().available,
            2.0 * 1024.0 * 1024.0 * 1024.0
        );
        assert_eq!(free_space("1500").unwrap().available, 1500.0);
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n\
                  /dev/sda1 1000 600 400 60% /\n";
        let f = parse_df(df).unwrap();
        assert_eq!(
            (f.available, f.capacity),
            (400.0 * 1024.0, Some(1000.0 * 1024.0))
        );
        // 400 KiB free of 1000 KiB: the 80% line is 200 KiB away.
        assert_eq!(alert_headroom(f), Some(200.0 * 1024.0));
        assert!(free_space("/no/such/path/here").is_err());
        assert!(free_space(".").unwrap().capacity.is_some());
    }

    #[test]
    fn verdict_flags_approval_and_the_alert_line() {
        let free = FreeSpace {
            available: 1000.0,
            capacity: Some(2000.0),
        };
        assert_eq!(verdict(50.0, free), "5.0% of free");
        assert_eq!(verdict(150.0, free), "15.0% of free, needs approval");
        assert_eq!(
            verdict(700.0, free),
            "70.0% of free, crosses the 80% alert line, needs approval"
        );
    }

    #[test]
    fn human_uses_binary_units() {
        assert_eq!(human(512.0), "512 B");
        assert_eq!(human(1536.0), "1.50 KiB");
        assert_eq!(human(3.0 * 1024.0 * 1024.0 * 1024.0), "3.00 GiB");
    }
}
