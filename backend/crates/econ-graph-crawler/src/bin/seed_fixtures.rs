// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `seed-fixtures`: loads recorded fixtures into a database through the real adapters, with no
//! network access. Used by the release end-to-end stack (`frontend/tests/e2e/release/`).
//!
//! For each series in the manifest (default `tests/fixtures/e2e-seed.json`) it starts a local
//! mock upstream that serves only that entry's fixtures, points the source's adapter at it, calls
//! `fetch_series` and writes the result with [`persist::persist_series`], the worker's own
//! persistence. The seed fails when the adapter errors (a request the manifest doesn't cover gets
//! a 404), when it returns no observations, or when a listed fixture was never requested, so a
//! stale fixture or a changed adapter shows up here instead of as an empty page.
//!
//! Runs the database migrations first, so it works on an empty database, then syncs every
//! adapter's dataset declarations ([`DatasetCatalog::load`] and [`persist::sync_datasets`]) before
//! persisting any series, the same order the crawler worker starts up in. Re-running it is safe:
//! persistence upserts.
//!
//! ```bash
//! DATABASE_URL=postgres://... cargo run -p econ-graph-crawler --features testkit --bin seed-fixtures
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use clap::Parser;
use econ_graph_crawler::dataset::DatasetCatalog;
use econ_graph_crawler::persist;
use econ_graph_crawler::source::SourceId;
use econ_graph_crawler::sources::registry_at;
use econ_graph_crawler::testkit::{test_ctx, MockSource, Reply, Route};
use serde::Deserialize;

/// Loads recorded fixtures into `DATABASE_URL` through the source adapters.
#[derive(Debug, Parser)]
#[command(name = "seed-fixtures")]
struct Args {
    /// Postgres connection string.
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// Seed manifest. Fixture paths in it are relative to the manifest's directory.
    #[arg(long, default_value_os_t = default_manifest())]
    manifest: PathBuf,
}

fn default_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/e2e-seed.json")
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[serde(rename = "_comment", default)]
    _comment: Option<String>,
    series: Vec<SeedSeries>,
}

/// One series: which adapter fetches it, and the upstream responses it gets.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedSeries {
    source: SourceId,
    external_id: String,
    routes: Vec<FixtureRoute>,
}

/// One recorded upstream response.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRoute {
    /// `GET` or `POST`.
    method: String,
    /// Exact request path, without the query string.
    path: String,
    /// Query parameters the request must carry (others are ignored).
    #[serde(default)]
    query: BTreeMap<String, String>,
    /// JSON body file, relative to the manifest.
    fixture: PathBuf,
}

impl FixtureRoute {
    fn route(&self) -> anyhow::Result<Route> {
        let route = match self.method.to_ascii_uppercase().as_str() {
            "GET" => Route::get(&self.path),
            "POST" => Route::post(&self.path),
            other => bail!("unsupported method {other:?} for {}", self.path),
        };
        Ok(self
            .query
            .iter()
            .fold(route, |route, (k, v)| route.query(k, v)))
    }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();

    match run(Args::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("{e:#}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> anyhow::Result<()> {
    let manifest: Manifest = serde_json::from_str(
        &std::fs::read_to_string(&args.manifest)
            .with_context(|| format!("reading {}", args.manifest.display()))?,
    )
    .with_context(|| format!("parsing {}", args.manifest.display()))?;
    if manifest.series.is_empty() {
        bail!("{} lists no series", args.manifest.display());
    }
    let fixture_dir = args
        .manifest
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no directory"))?;

    econ_graph_core::run_migrations(&args.database_url)
        .await
        .context("running migrations")?;
    let pool = econ_graph_core::create_pool(&args.database_url)
        .await
        .context("connecting to the database")?;
    // The base URL here is unused: dataset declarations don't depend on it, only on the
    // adapter's id and its `datasets()` list.
    let datasets = DatasetCatalog::load(&registry_at("http://unused"))
        .context("loading dataset declarations")?;
    persist::sync_datasets(&pool, &datasets)
        .await
        .context("syncing datasets")?;
    // Fake API keys, fast rate limits and short timeouts; nothing here reaches a real upstream.
    let ctx = test_ctx();

    for entry in &manifest.series {
        let label = format!("{} {}", entry.source, entry.external_id);
        let mock = MockSource::start().await;
        for r in &entry.routes {
            let path = fixture_dir.join(&r.fixture);
            let body = std::fs::read_to_string(&path)
                .with_context(|| format!("{label}: reading {}", path.display()))?;
            mock.mount(&r.route()?, Reply::json_str(body)).await;
        }
        let adapter = registry_at(&mock.base_url())
            .get(entry.source)
            .ok_or_else(|| anyhow!("{label}: no adapter registered"))?;
        let fetched = adapter
            .fetch_series(&ctx, &entry.external_id, None)
            .await
            .with_context(|| format!("{label}: fetch_series against the fixtures"))?;
        if fetched.points.is_empty() {
            bail!("{label}: the fixtures produced no observations");
        }
        let received = mock.received_requests().await;
        for r in &entry.routes {
            let hit = received.iter().any(|req| {
                req.method.as_str().eq_ignore_ascii_case(&r.method)
                    && req.url.path() == r.path
                    && r.query
                        .iter()
                        .all(|(k, v)| req.url.query_pairs().any(|(qk, qv)| qk == *k && qv == *v))
            });
            if !hit {
                bail!(
                    "{label}: the adapter never requested {} {} ({})",
                    r.method,
                    r.path,
                    r.fixture.display()
                );
            }
        }
        datasets
            .check(entry.source, &entry.external_id, fetched.dataset.as_ref())
            .with_context(|| format!("{label}: dataset"))?;
        let write = persist::persist_series(&pool, entry.source, &entry.external_id, &fetched)
            .await
            .with_context(|| format!("{label}: persisting"))?;
        tracing::info!(
            series = %label,
            points = write.points_upserted,
            latest = ?write.latest_date,
            id = %write.series_id,
            "seeded"
        );
    }
    tracing::info!(series = manifest.series.len(), "seed complete");
    Ok(())
}
