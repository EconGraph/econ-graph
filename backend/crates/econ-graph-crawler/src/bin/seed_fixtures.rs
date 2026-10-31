// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! `seed-fixtures`: loads recorded fixtures into a database through the real adapters, with no
//! network access. Used by the release end-to-end stack (`frontend/tests/e2e/release/`).
//!
//! For each series in the manifest (default `tests/fixtures/e2e-seed.json`) it starts a local
//! mock upstream that serves only that entry's fixtures, points the source's adapter at it, best-
//! effort refreshes its reference data against the same mock (as the worker does before
//! `discover`, see `worker.rs`'s `discover`; a source with nothing mounted for it, or no
//! `code_lists` at all, just keeps whatever it already has) — skipped entirely when any of the
//! adapter's `code_lists` points outside this entry's mock, since an adapter that hardcodes a
//! reference-data URL rather than deriving it from its own base URL (BLS) would otherwise have this
//! reach the real internet — calls `fetch_series` and writes the
//! result with [`persist::persist_series`], the worker's own persistence. An entry with
//! `discovery` fixtures first runs the adapter's catalog discovery against them and writes the
//! series' catalog row with [`persist::persist_discovered`], as the worker does before fetching;
//! adapters whose fetch carries no metadata (Census BDS) get their title, units and frequency
//! from that row. The seed fails when the adapter errors (a request the manifest doesn't cover
//! gets a 404), when it returns no observations, or when a listed fixture was never requested, so
//! a stale fixture or a changed adapter shows up here instead of as an empty page.
//!
//! Runs the data migrations first (never the app set), so it works on an empty data database,
//! then syncs every adapter's dataset declarations ([`DatasetCatalog::load`] and
//! [`persist::sync_datasets`]) before persisting any series, the same order the crawler worker
//! starts up in. Re-running it is safe: persistence upserts.
//!
//! ```bash
//! DATA_DATABASE_URL=postgres://... cargo run -p econ-graph-crawler --features testkit --bin seed-fixtures
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use clap::Parser;
use diesel::{ExpressionMethods, QueryDsl, QueryableByName};
use diesel_async::{AsyncConnection, RunQueryDsl, SimpleAsyncConnection};
use econ_graph_core::{DataPool, MigrationSet};
use econ_graph_crawler::persist;
use econ_graph_crawler::reference_file::CodeList;
use econ_graph_crawler::source::SourceId;
use econ_graph_crawler::sources::registry_at;
use econ_graph_crawler::testkit::{test_ctx, MockSource, Reply, Route};
use econ_graph_crawler::DatasetCatalog;
use serde::Deserialize;
use uuid::Uuid;

/// Loads recorded fixtures into `DATA_DATABASE_URL` through the source adapters.
#[derive(Debug, Parser)]
#[command(name = "seed-fixtures")]
struct Args {
    /// The data database's Postgres connection string. `DATABASE_URL` is not read.
    #[arg(long, env = "DATA_DATABASE_URL", hide_env_values = true)]
    database_url: Option<String>,

    /// Assign reproducible fixture-only row identities. Refuses a populated data database.
    #[arg(long)]
    deterministic_empty: bool,

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
    /// Responses `discover` gets, for an adapter whose fetch carries no metadata. Optional.
    #[serde(default)]
    discovery: Vec<FixtureRoute>,
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
    fn matches(&self, method: &str, url: &reqwest::Url) -> bool {
        method.eq_ignore_ascii_case(&self.method)
            && url.path() == self.path
            && self
                .query
                .iter()
                .all(|(k, v)| url.query_pairs().any(|(qk, qv)| qk == *k && qv == *v))
    }
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

    // A blank value (an empty `DATA_DATABASE_URL`) counts as unset, as in `data_database_url`.
    let database_url = match args.database_url.filter(|u| !u.trim().is_empty()) {
        Some(url) => url,
        None => econ_graph_core::config::data_database_url()
            .map_err(|e| anyhow!("{e} (or pass --database-url)"))?,
    };
    if args.deterministic_empty {
        #[derive(QueryableByName)]
        struct Existing {
            #[diesel(sql_type = diesel::sql_types::Bool)]
            occupied: bool,
        }
        let check_pool = econ_graph_core::create_pool(&database_url).await?;
        let mut conn = check_pool.get().await?;
        let existing = diesel::sql_query(
            "SELECT EXISTS(SELECT 1 FROM pg_tables WHERE schemaname = 'public') AS occupied",
        )
        .get_result::<Existing>(&mut conn)
        .await?;
        if existing.occupied {
            bail!("--deterministic-empty requires an entirely empty disposable database; no migrations or identity changes were applied");
        }
    }
    econ_graph_core::run_migration_set(&database_url, MigrationSet::Data)
        .await
        .context("running the data migrations")?;
    let pool = DataPool(
        econ_graph_core::create_pool(&database_url)
            .await
            .context("connecting to the data database")?,
    );
    // The base URL here is unused: dataset declarations don't depend on it, only on the
    // adapter's id and its `datasets()` list.
    let datasets = DatasetCatalog::load(&registry_at("http://unused"))
        .context("loading dataset declarations")?;
    if args.deterministic_empty {
        prepare_deterministic_empty(&pool, &datasets).await?;
    }
    persist::sync_datasets(&pool, &datasets)
        .await
        .context("syncing datasets")?;
    // Fake API keys, fast rate limits and short timeouts; nothing here reaches a real upstream.
    // The real pool, not `test_ctx`'s lazy one: `refresh_reference_data` reads and writes it.
    let mut ctx = test_ctx();
    ctx.pool = pool.0.clone();

    for entry in &manifest.series {
        let label = format!("{} {}", entry.source, entry.external_id);
        let mock = MockSource::start().await;
        for r in entry.discovery.iter().chain(&entry.routes) {
            let path = fixture_dir.join(&r.fixture);
            let body = std::fs::read_to_string(&path)
                .with_context(|| format!("{label}: reading {}", path.display()))?;
            mock.mount(&r.route()?, Reply::json_str(body)).await;
        }
        let adapter = registry_at(&mock.base_url())
            .get(entry.source)
            .ok_or_else(|| anyhow!("{label}: no adapter registered"))?;
        // Best-effort, same as the worker's own `discover` (worker.rs): a source with no
        // reference-data route mounted for this entry just keeps whatever it already has rather
        // than failing the seed. Only when every one of its code lists actually points at this
        // entry's mock, though: BLS hardcodes its reference-data URLs rather than deriving them
        // from the adapter's own base URL, so `registry_at` can't redirect those — calling `refresh_reference_data` on them here would reach the real
        // internet instead of staying within this "no network access" tool.
        let lists: Vec<CodeList> = adapter.code_lists(&ctx.keys);
        if lists.iter().all(|list| {
            list.request_url().starts_with(&mock.base_url())
                && (!args.deterministic_empty || {
                    reqwest::Url::parse(list.request_url()).is_ok_and(|url| {
                        entry
                            .discovery
                            .iter()
                            .chain(&entry.routes)
                            .any(|route| route.matches("GET", &url))
                    })
                })
        }) {
            if let Err(e) = adapter.refresh_reference_data(&ctx).await {
                if args.deterministic_empty {
                    bail!("{label}: declared reference fixture failed: {e}");
                }
                tracing::warn!(series = %label, error = %e, "refresh_reference_data failed");
            }
        } else {
            tracing::debug!(
                series = %label,
                "skipping refresh_reference_data: its code lists point outside this entry's mock"
            );
        }
        if !entry.discovery.is_empty() {
            let found = adapter
                .discover(&ctx)
                .await
                .with_context(|| format!("{label}: discover against the fixtures"))?;
            let listed = found
                .into_iter()
                .find(|d| d.external_id == entry.external_id)
                .ok_or_else(|| {
                    anyhow!("{label}: discovery against the fixtures doesn't list it")
                })?;
            datasets
                .check(entry.source, &entry.external_id, &listed.dataset)
                .with_context(|| format!("{label}: discovered dataset"))?;
            persist::persist_discovered(&pool, entry.source, &[listed])
                .await
                .with_context(|| format!("{label}: persisting its catalog row"))?;
        }
        let fetched = adapter
            .fetch_series(&ctx, &entry.external_id, None)
            .await
            .with_context(|| format!("{label}: fetch_series against the fixtures"))?;
        if fetched.points.is_empty() {
            bail!("{label}: the fixtures produced no observations");
        }
        let received = mock.received_requests().await;
        if args.deterministic_empty {
            for request in &received {
                if !entry
                    .discovery
                    .iter()
                    .chain(&entry.routes)
                    .any(|route| route.matches(request.method.as_str(), &request.url))
                {
                    bail!(
                        "{label}: unexpected fixture request {} {}",
                        request.method,
                        request.url.path()
                    );
                }
            }
        }
        for r in entry.discovery.iter().chain(&entry.routes) {
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
            .check(entry.source, &entry.external_id, &fetched.dataset)
            .with_context(|| format!("{label}: dataset"))?;
        let write = persist::persist_series(&pool, entry.source, &entry.external_id, &fetched)
            .await
            .with_context(|| format!("{label}: persisting"))?;
        if args.deterministic_empty {
            stabilize_fixture_observations(&pool, write.series_id).await?;
        }
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

// These helpers belong to this required-testkit binary, never the production writer. The
// opt-in mode is for a newly migrated disposable data DB, not a legacy-ID migration.
const FIXTURE_NAMESPACE: Uuid = Uuid::from_u128(0x35a99ed840a34cd39e150ad44b8a83b9);
fn fixture_id(key: impl serde::Serialize) -> Uuid {
    Uuid::new_v5(
        &FIXTURE_NAMESPACE,
        &serde_json::to_vec(&key).expect("fixture identity tuple serializes"),
    )
}

#[derive(QueryableByName)]
struct SourceIdentity {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    id: Uuid,
    #[diesel(sql_type = diesel::sql_types::Text)]
    name: String,
}
async fn prepare_deterministic_empty(
    pool: &DataPool,
    catalog: &DatasetCatalog,
) -> anyhow::Result<()> {
    use econ_graph_core::schema::data_sources;
    #[derive(QueryableByName)]
    struct Guard {
        #[diesel(sql_type = diesel::sql_types::Bool)]
        populated: bool,
    }
    let mut conn = pool.get().await?;
    conn.transaction::<(), anyhow::Error, _>(async |conn| {
        conn.batch_execute("LOCK TABLE data_sources, datasets, economic_series, data_points, series_metadata, financial_statements, financial_line_items, companies, crawl_queue, crawl_attempts IN ACCESS EXCLUSIVE MODE").await?;
        let guard = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM economic_series) OR EXISTS(SELECT 1 FROM datasets) OR EXISTS(SELECT 1 FROM data_points) OR EXISTS(SELECT 1 FROM companies) OR EXISTS(SELECT 1 FROM financial_statements) OR EXISTS(SELECT 1 FROM financial_line_items) OR EXISTS(SELECT 1 FROM crawl_queue) OR EXISTS(SELECT 1 FROM crawl_attempts) OR EXISTS(SELECT 1 FROM series_metadata WHERE dataset_id IS NOT NULL) OR to_regclass('public.users') IS NOT NULL AS populated").get_result::<Guard>(conn).await?;
        if guard.populated { bail!("--deterministic-empty requires a fresh disposable data database; existing identities/content are never renumbered"); }
        // Baseline discovery references already have stable IDs. Keep every field while only
        // replacing their freshly generated source foreign keys in this empty DB.
        conn.batch_execute("CREATE TEMP TABLE train3_seed_sources ON COMMIT DROP AS SELECT * FROM data_sources; CREATE TEMP TABLE train3_seed_metadata ON COMMIT DROP AS SELECT * FROM series_metadata; DELETE FROM series_metadata;").await?;
        let sources = diesel::sql_query("SELECT id, name FROM data_sources").load::<SourceIdentity>(conn).await?;
        for source in sources {
            diesel::update(data_sources::table.find(source.id)).set(data_sources::id.eq(fixture_id(("source", &source.name)))).execute(conn).await?;
        }
        conn.batch_execute("INSERT INTO series_metadata SELECT (jsonb_populate_record(NULL::series_metadata, jsonb_set(to_jsonb(m), '{source_id}', to_jsonb(s.id)))).* FROM train3_seed_metadata m JOIN train3_seed_sources old ON old.id = m.source_id JOIN data_sources s ON s.name = old.name;").await?;
        for (source, def) in catalog.iter() {
            let row = persist::data_source_template(source);
            let source_id = fixture_id(("source", &row.name));
            diesel::insert_into(data_sources::table).values((data_sources::id.eq(source_id), &row)).on_conflict(data_sources::name).do_nothing().execute(conn).await?;
            let dataset = def.to_new_dataset(source_id);
            use econ_graph_core::schema::datasets;
            diesel::insert_into(datasets::table).values((datasets::id.eq(fixture_id(("dataset", source_id, &dataset.code))), &dataset)).on_conflict((datasets::source_id, datasets::code)).do_nothing().execute(conn).await?;
        }
        Ok(())
    }).await
}

async fn stabilize_fixture_observations(pool: &DataPool, series: Uuid) -> anyhow::Result<()> {
    #[derive(QueryableByName)]
    struct PointIdentity {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
        #[diesel(sql_type = diesel::sql_types::Date)]
        date: chrono::NaiveDate,
        #[diesel(sql_type = diesel::sql_types::Date)]
        revision_date: chrono::NaiveDate,
        #[diesel(sql_type = diesel::sql_types::Bool)]
        is_original_release: bool,
    }
    let mut conn = pool.get().await?;
    conn.transaction::<(), anyhow::Error, _>(async |conn| {
        let rows = diesel::sql_query("SELECT id, date, revision_date, is_original_release FROM data_points WHERE series_id = $1")
            .bind::<diesel::sql_types::Uuid,_>(series).load::<PointIdentity>(conn).await?;
        for row in rows {
            let id = fixture_id(("observation", series, row.date, row.revision_date, row.is_original_release));
            diesel::sql_query("UPDATE data_points SET id = $1 WHERE id = $2")
                .bind::<diesel::sql_types::Uuid,_>(id).bind::<diesel::sql_types::Uuid,_>(row.id).execute(conn).await?;
        }
        Ok(())
    }).await
}

#[cfg(test)]
mod deterministic_identity_tests {
    use super::*;
    #[test]
    fn strict_routes_reject_unknown_method_path_and_query() {
        let route: FixtureRoute = serde_json::from_value(serde_json::json!({"method":"GET", "path":"/series", "query":{"id":"fixture"}, "fixture":"recorded.json"})).unwrap();
        assert!(route.matches(
            "GET",
            &reqwest::Url::parse("http://localhost/series?id=fixture&api_key=fake").unwrap()
        ));
        for (method, url) in [
            ("POST", "http://localhost/series?id=fixture"),
            ("GET", "http://localhost/unknown?id=fixture"),
            ("GET", "http://localhost/series?id=other"),
        ] {
            assert!(!route.matches(method, &reqwest::Url::parse(url).unwrap()));
        }
    }
    #[test]
    fn version_one_identity_keys_are_stable() {
        let source = fixture_id(("source", "Federal Reserve Economic Data (FRED)"));
        assert_eq!(source.to_string(), "4eb4de5e-9c0a-524a-917a-3141d33ed1e3");
        assert_eq!(
            fixture_id(("dataset", source, "FRED")).to_string(),
            "a9e88ece-ca29-59e2-b3ea-8dfdb5b1491e"
        );
        let date = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let revision = chrono::NaiveDate::from_ymd_opt(2020, 2, 1).unwrap();
        assert_eq!(
            fixture_id(("observation", Uuid::nil(), date, revision, true)).to_string(),
            "96197a15-2f4d-55d0-8873-fe2a2c9c18ee"
        );
        assert_ne!(
            fixture_id(("observation", Uuid::nil(), date, revision, true)),
            fixture_id(("observation", Uuid::nil(), date, revision, false))
        );
    }
}
