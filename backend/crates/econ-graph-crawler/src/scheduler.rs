// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Refresh scheduler: periodically **enqueues** crawl work; it never fetches anything itself.
//!
//! Each [`RefreshScheduler::tick`]:
//!
//! 1. **Series refresh** — enqueues a `fetch_series` job (priority [`DEFAULT_PRIORITY`]) for every
//!    active `economic_series` row that is *due*, oldest first, at most
//!    [`SchedulerConfig::batch_limit`] per tick. Only series of sources that are in the registry,
//!    can actually fetch series (not static catalogs, not [`FETCH_UNIMPLEMENTED`], not SEC) and
//!    whose `data_sources` row is enabled (`is_enabled`) are considered.
//!
//!    A series is due when it has never been crawled (`last_crawled_at IS NULL`) or
//!    `last_crawled_at` is older than its [`refresh_interval`] (by `frequency`):
//!
//!    | frequency                                   | interval |
//!    |---------------------------------------------|----------|
//!    | daily, business daily (`D`, `B`)            | 1 day    |
//!    | weekly, biweekly (`Weekly, Ending Friday`)  | 7 days   |
//!    | monthly (`M`)                               | 7 days   |
//!    | quarterly (`Q`)                             | 14 days  |
//!    | annual, semiannual (`A`, `SA`)              | 30 days  |
//!    | anything else                               | 7 days   |
//!
//!    Series whose `crawl_status` is `failed` back off: they are due only once
//!    `max(last_crawled_at, last crawl_attempts.attempted_at)` is older than **twice** the interval
//!    (a never-successful series therefore isn't retried every tick).
//!    Series that already have an active (pending | processing | retrying) job are not candidates;
//!    a concurrent duplicate rejected by the queue's partial unique index counts as skipped.
//!
//! 2. **Catalog discovery** — enqueues one `discover_catalog` job per registry source unless that
//!    source already has an active discovery job or one finished (completed or failed) within
//!    [`DISCOVERY_INTERVAL`], or its `data_sources` row exists and is disabled.
//!
//! Deployment runs a single `crawler-worker` replica (which hosts the scheduler), and duplicates
//! are prevented by the queue's unique index anyway, so there is no leader election.

use std::future::Future;
use std::sync::LazyLock;
use std::time::Duration;

use diesel::sql_types::{Array, BigInt, Double, Text};
use diesel::QueryableByName;
use diesel_async::RunQueryDsl;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{CrawlQueueItem, JobKind};
use econ_graph_core::DatabasePool;

use crate::adapter::AdapterRegistry;
use crate::cli::{new_job, CATALOG_SERIES_ID, DEFAULT_PRIORITY};
use crate::persist::data_source_template;
use crate::source::SourceId;
use crate::sources::static_catalogs::is_static_catalog_source;

/// Sources whose adapter supports discovery but whose `fetch_series` still returns
/// `CrawlError::Permanent("... not implemented yet")`. Enqueuing refreshes for them would only
/// produce failed jobs, so the scheduler skips them. Remove a source here once its adapter
/// implements `fetch_series`.
pub const FETCH_UNIMPLEMENTED: &[SourceId] = &[SourceId::WorldBank, SourceId::Imf, SourceId::Bea];

/// How often each source's catalog is re-discovered.
pub const DISCOVERY_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Default seconds between ticks.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(300);

/// Default maximum `fetch_series` jobs enqueued per tick.
pub const DEFAULT_BATCH_LIMIT: i64 = 500;

/// Refresh interval, in days, for a frequency that matches no rule.
pub const UNKNOWN_FREQUENCY_DAYS: i64 = 7;

/// One frequency bucket: matches if the lower-cased, trimmed frequency equals one of `codes` or
/// contains one of `contains`. Rules are tried in order.
struct FrequencyRule {
    codes: &'static [&'static str],
    contains: &'static [&'static str],
    days: i64,
}

/// Frequency buckets (FRED long names/codes, BLS names). Used by both [`refresh_interval`] and the
/// SQL `CASE` in the due query, so the two can't drift apart. Strings must not contain quotes.
const FREQUENCY_RULES: &[FrequencyRule] = &[
    // "Daily", "Daily, 7-Day", "Business Daily", FRED codes D / B.
    FrequencyRule {
        codes: &["d", "b", "bd"],
        contains: &["daily"],
        days: 1,
    },
    // "Weekly", "Weekly, Ending Friday", "Biweekly", FRED codes W / BW / WEF ...
    FrequencyRule {
        codes: &[
            "w", "bw", "wef", "weth", "wew", "wetu", "wem", "wesu", "wesa",
        ],
        contains: &["week"],
        days: 7,
    },
    // "Monthly", "Semimonthly", "Bimonthly", FRED code M.
    FrequencyRule {
        codes: &["m", "sm"],
        contains: &["month"],
        days: 7,
    },
    // "Quarterly", "Quarterly, End of Period", FRED code Q.
    FrequencyRule {
        codes: &["q"],
        contains: &["quarter"],
        days: 14,
    },
    // "Annual", "Semiannual" (BLS), "Yearly", FRED codes A / SA.
    FrequencyRule {
        codes: &["a", "sa", "y"],
        contains: &["annual", "year"],
        days: 30,
    },
];

fn frequency_days(frequency: &str) -> i64 {
    let f = frequency.trim().to_ascii_lowercase();
    FREQUENCY_RULES
        .iter()
        .find(|r| r.codes.contains(&f.as_str()) || r.contains.iter().any(|c| f.contains(c)))
        .map_or(UNKNOWN_FREQUENCY_DAYS, |r| r.days)
}

/// How long after a successful crawl a series with this `frequency` becomes due again
/// (see the table in the [module docs](self)). Case-insensitive.
pub fn refresh_interval(frequency: &str) -> Duration {
    let days = u64::try_from(frequency_days(frequency)).unwrap_or(7);
    Duration::from_secs(days * 24 * 60 * 60)
}

/// `CASE` expression over `f` (lower-cased, trimmed frequency) returning the interval in days.
fn frequency_days_sql(f: &str) -> String {
    let mut sql = String::from("CASE");
    for rule in FREQUENCY_RULES {
        let mut conds: Vec<String> = Vec::new();
        if !rule.codes.is_empty() {
            let list: Vec<String> = rule.codes.iter().map(|c| format!("'{c}'")).collect();
            conds.push(format!("{f} IN ({})", list.join(", ")));
        }
        conds.extend(rule.contains.iter().map(|c| format!("{f} LIKE '%{c}%'")));
        sql.push_str(&format!(" WHEN {} THEN {}", conds.join(" OR "), rule.days));
    }
    sql.push_str(&format!(" ELSE {UNKNOWN_FREQUENCY_DAYS} END"));
    sql
}

/// Due-series query. `$1` data_sources names, `$2` matching source codes, `$3` limit, `$4` the
/// regex of fetchable Census ids.
static DUE_SERIES_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "WITH src AS ( \
             SELECT m.code, ds.id AS ds_id \
             FROM unnest($1::text[], $2::text[]) AS m(ds_name, code) \
             JOIN data_sources ds ON ds.name = m.ds_name \
             WHERE ds.is_enabled \
         ), cand AS ( \
             SELECT src.code, es.id, es.external_id, es.last_crawled_at, es.crawl_status, \
                    make_interval(days => ({days})) AS refresh \
             FROM economic_series es \
             JOIN src ON es.source_id = src.ds_id \
             WHERE es.is_active \
               AND (src.code <> '{census}' OR es.external_id ~ $4) \
         ) \
         SELECT c.code AS source, c.external_id::text AS external_id \
         FROM cand c \
         WHERE NOT EXISTS ( \
                 SELECT 1 FROM crawl_queue q \
                 WHERE q.source = c.code AND q.series_id = c.external_id \
                   AND q.kind = 'fetch_series' \
                   AND q.status IN ('pending', 'processing', 'retrying')) \
           AND CASE WHEN c.crawl_status = 'failed' THEN \
                   COALESCE(GREATEST(c.last_crawled_at, \
                                     (SELECT max(ca.attempted_at) FROM crawl_attempts ca \
                                      WHERE ca.series_id = c.id)), \
                            '-infinity'::timestamptz) <= NOW() - 2 * c.refresh \
               ELSE c.last_crawled_at IS NULL OR c.last_crawled_at <= NOW() - c.refresh \
               END \
         ORDER BY c.last_crawled_at ASC NULLS FIRST, c.code, c.external_id \
         LIMIT $3",
        days = frequency_days_sql("lower(btrim(es.frequency))"),
        census = SourceId::Census.as_str(),
    )
});

/// Sources needing discovery. `$1` source codes, `$2` matching data_sources names, `$3` seconds.
const DISCOVERY_DUE_SQL: &str = "SELECT s.code AS source \
     FROM unnest($1::text[], $2::text[]) AS s(code, ds_name) \
     WHERE NOT EXISTS (SELECT 1 FROM data_sources ds \
                       WHERE ds.name = s.ds_name AND NOT ds.is_enabled) \
       AND NOT EXISTS (SELECT 1 FROM crawl_queue q \
                       WHERE q.source = s.code AND q.kind = 'discover_catalog' \
                         AND (q.status IN ('pending', 'processing', 'retrying') \
                              OR q.finished_at > NOW() - make_interval(secs => $3))) \
     ORDER BY s.code";

#[derive(QueryableByName)]
struct DueSeries {
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = Text)]
    external_id: String,
}

#[derive(QueryableByName)]
struct DueSource {
    #[diesel(sql_type = Text)]
    source: String,
}

/// Scheduler settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerConfig {
    /// Time between ticks.
    pub interval: Duration,
    /// Maximum `fetch_series` jobs enqueued per tick.
    pub batch_limit: i64,
    /// When false, [`RefreshScheduler::run`] returns immediately.
    pub enabled: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            batch_limit: DEFAULT_BATCH_LIMIT,
            enabled: true,
        }
    }
}

/// What one [`RefreshScheduler::tick`] did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SchedulerTickStats {
    /// Due series selected this tick (after `batch_limit`).
    pub series_due: usize,
    /// `fetch_series` jobs inserted.
    pub series_enqueued: usize,
    /// Due series whose insert was rejected because an active job already existed.
    pub series_skipped: usize,
    /// `discover_catalog` jobs inserted.
    pub discover_enqueued: usize,
    /// Due discoveries rejected because an active job already existed.
    pub discover_skipped: usize,
}

impl SchedulerTickStats {
    /// Whether anything was enqueued.
    pub fn enqueued_any(&self) -> bool {
        self.series_enqueued + self.discover_enqueued > 0
    }
}

/// Periodically enqueues due series refreshes and weekly catalog discovery. See the module docs.
pub struct RefreshScheduler {
    pool: DatabasePool,
    registry_sources: Vec<SourceId>,
    config: SchedulerConfig,
}

impl RefreshScheduler {
    /// Scheduler for `registry_sources` (normally `AdapterRegistry::ids()`).
    pub fn new(
        pool: DatabasePool,
        registry_sources: Vec<SourceId>,
        config: SchedulerConfig,
    ) -> Self {
        let mut registry_sources = registry_sources;
        registry_sources.sort_unstable();
        registry_sources.dedup();
        Self {
            pool,
            registry_sources,
            config,
        }
    }

    /// Scheduler for every source registered in `registry`.
    pub fn from_registry(
        pool: DatabasePool,
        registry: &AdapterRegistry,
        config: SchedulerConfig,
    ) -> Self {
        Self::new(pool, registry.ids(), config)
    }

    /// The configuration.
    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Registry sources whose series the scheduler refreshes.
    pub fn refreshable_sources(&self) -> Vec<SourceId> {
        self.registry_sources
            .iter()
            .copied()
            .filter(|&s| supports_fetch(s))
            .collect()
    }

    /// Registry sources whose catalogs the scheduler re-discovers.
    pub fn discovery_sources(&self) -> Vec<SourceId> {
        self.registry_sources
            .iter()
            .copied()
            .filter(|&s| s != SourceId::Sec)
            .collect()
    }

    /// One scheduling pass: enqueue due series refreshes, then due catalog discoveries.
    pub async fn tick(&self) -> AppResult<SchedulerTickStats> {
        let mut stats = SchedulerTickStats::default();
        self.enqueue_due_series(&mut stats).await?;
        self.enqueue_due_discovery(&mut stats).await?;
        Ok(stats)
    }

    async fn enqueue_due_series(&self, stats: &mut SchedulerTickStats) -> AppResult<()> {
        let sources = self.refreshable_sources();
        if sources.is_empty() || self.config.batch_limit <= 0 {
            return Ok(());
        }
        let names: Vec<String> = sources
            .iter()
            .map(|&s| data_source_template(s).name)
            .collect();
        let codes: Vec<String> = sources.iter().map(|s| s.as_str().to_string()).collect();
        // Only read when Census is scheduled; the pattern is unused otherwise.
        let census_ids = if sources.contains(&SourceId::Census) {
            crate::sources::census::fetchable_id_regex()?
        } else {
            String::new()
        };
        let due: Vec<DueSeries> = {
            let mut conn = self.pool.get().await.map_err(conn_err)?;
            diesel::sql_query(DUE_SERIES_SQL.as_str())
                .bind::<Array<Text>, _>(&names)
                .bind::<Array<Text>, _>(&codes)
                .bind::<BigInt, _>(self.config.batch_limit)
                .bind::<Text, _>(&census_ids)
                .load(&mut conn)
                .await?
        };
        stats.series_due = due.len();
        for row in due {
            let Ok(source) = row.source.parse::<SourceId>() else {
                continue;
            };
            let job = new_job(
                source,
                &row.external_id,
                JobKind::FetchSeries,
                DEFAULT_PRIORITY,
            );
            match CrawlQueueItem::enqueue(&self.pool, &job).await? {
                Some(_) => stats.series_enqueued += 1,
                None => stats.series_skipped += 1,
            }
        }
        Ok(())
    }

    async fn enqueue_due_discovery(&self, stats: &mut SchedulerTickStats) -> AppResult<()> {
        let sources = self.discovery_sources();
        if sources.is_empty() {
            return Ok(());
        }
        let codes: Vec<String> = sources.iter().map(|s| s.as_str().to_string()).collect();
        let names: Vec<String> = sources
            .iter()
            .map(|&s| data_source_template(s).name)
            .collect();
        let due: Vec<DueSource> = {
            let mut conn = self.pool.get().await.map_err(conn_err)?;
            diesel::sql_query(DISCOVERY_DUE_SQL)
                .bind::<Array<Text>, _>(&codes)
                .bind::<Array<Text>, _>(&names)
                .bind::<Double, _>(DISCOVERY_INTERVAL.as_secs_f64())
                .load(&mut conn)
                .await?
        };
        for row in due {
            let Ok(source) = row.source.parse::<SourceId>() else {
                continue;
            };
            let job = new_job(
                source,
                CATALOG_SERIES_ID,
                JobKind::DiscoverCatalog,
                DEFAULT_PRIORITY,
            );
            match CrawlQueueItem::enqueue(&self.pool, &job).await? {
                Some(_) => stats.discover_enqueued += 1,
                None => stats.discover_skipped += 1,
            }
        }
        Ok(())
    }

    /// Ticks every [`SchedulerConfig::interval`] until `shutdown` resolves (returns immediately
    /// when disabled). Tick errors are logged, never fatal.
    pub async fn run<F: Future<Output = ()>>(&self, shutdown: F) {
        if !self.config.enabled {
            tracing::info!("refresh scheduler disabled");
            return;
        }
        tracing::info!(
            interval_secs = self.config.interval.as_secs(),
            batch_limit = self.config.batch_limit,
            refresh = ?self.refreshable_sources(),
            discovery = ?self.discovery_sources(),
            "refresh scheduler started"
        );
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                result = self.tick() => match result {
                    Ok(stats) if stats.enqueued_any() => tracing::info!(?stats, "scheduler tick"),
                    Ok(stats) => tracing::debug!(?stats, "scheduler tick"),
                    Err(e) => tracing::warn!(error = %e, "scheduler tick failed"),
                },
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = tokio::time::sleep(self.config.interval) => {}
            }
        }
        tracing::info!("refresh scheduler stopped");
    }
}

/// Whether the scheduler should enqueue `fetch_series` for `source`: false for static catalogs,
/// [`FETCH_UNIMPLEMENTED`] sources and SEC (filings use `fetch_filing`, enqueued elsewhere).
pub fn supports_fetch(source: SourceId) -> bool {
    source != SourceId::Sec
        && !is_static_catalog_source(source)
        && !FETCH_UNIMPLEMENTED.contains(&source)
}

fn conn_err(e: impl std::fmt::Display) -> AppError {
    AppError::DatabaseError(format!("Failed to get database connection: {e}"))
}

#[cfg(test)]
mod tests {
    //! DB-backed tests need `DATABASE_URL` and are skipped when it is unset. Rows they create use
    //! external ids prefixed `t15c_`; `crawl_queue` is emptied before each test. Other tests may
    //! leave series in the database, so assertions only look at `t15c_` rows (or at sources no
    //! other test seeds).

    use std::collections::BTreeSet;

    use diesel::sql_types::{Bool, Nullable};
    use econ_graph_core::DatabasePool;
    use uuid::Uuid;

    use super::*;
    use crate::persist;

    const DAY: f64 = 24.0 * 60.0 * 60.0;

    struct Db {
        pool: DatabasePool,
        _guard: tokio::sync::MutexGuard<'static, ()>,
    }

    async fn db() -> Option<Db> {
        let (url, guard) = crate::testkit::lock_test_db("scheduler").await?;
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        exec(&pool, "DELETE FROM crawl_queue").await;
        exec(
            &pool,
            "DELETE FROM economic_series WHERE external_id LIKE 't15c\\_%'",
        )
        .await;
        Some(Db {
            pool,
            _guard: guard,
        })
    }

    async fn exec(pool: &DatabasePool, sql: &str) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(sql).execute(&mut conn).await.unwrap();
    }

    /// Inserts a series; `crawled_days_ago = None` means never crawled.
    async fn seed(
        pool: &DatabasePool,
        source: SourceId,
        external_id: &str,
        frequency: &str,
        crawled_days_ago: Option<f64>,
        crawl_status: Option<&str>,
    ) -> Uuid {
        seed_full(
            pool,
            source,
            external_id,
            frequency,
            crawled_days_ago,
            crawl_status,
            true,
        )
        .await
    }

    async fn seed_full(
        pool: &DatabasePool,
        source: SourceId,
        external_id: &str,
        frequency: &str,
        crawled_days_ago: Option<f64>,
        crawl_status: Option<&str>,
        is_active: bool,
    ) -> Uuid {
        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type = diesel::sql_types::Uuid)]
            id: Uuid,
        }
        let source_id = persist::data_source_id(pool, source).await.unwrap();
        let mut conn = pool.get().await.unwrap();
        let row: Id = diesel::sql_query(
            "INSERT INTO economic_series \
               (source_id, external_id, title, frequency, is_active, last_crawled_at, crawl_status) \
             VALUES ($1, $2, $2, $3, $4, NOW() - make_interval(secs => $5), $6) RETURNING id",
        )
        .bind::<diesel::sql_types::Uuid, _>(source_id)
        .bind::<Text, _>(external_id)
        .bind::<Text, _>(frequency)
        .bind::<Bool, _>(is_active)
        .bind::<Nullable<Double>, _>(crawled_days_ago.map(|d| d * DAY))
        .bind::<Nullable<Text>, _>(crawl_status)
        .get_result(&mut conn)
        .await
        .unwrap();
        row.id
    }

    async fn seed_attempt(pool: &DatabasePool, series_id: Uuid, days_ago: f64) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO crawl_attempts (series_id, attempted_at, crawl_method, success) \
             VALUES ($1, NOW() - make_interval(secs => $2), 'api', false)",
        )
        .bind::<diesel::sql_types::Uuid, _>(series_id)
        .bind::<Double, _>(days_ago * DAY)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    #[derive(QueryableByName)]
    struct QueueRow {
        #[diesel(sql_type = Text)]
        source: String,
        #[diesel(sql_type = Text)]
        series_id: String,
        #[diesel(sql_type = diesel::sql_types::Integer)]
        priority: i32,
    }

    async fn queue_rows(pool: &DatabasePool, kind: JobKind) -> Vec<QueueRow> {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "SELECT source, series_id::text AS series_id, priority FROM crawl_queue \
             WHERE kind = $1 AND status IN ('pending', 'processing', 'retrying') \
             ORDER BY source, series_id",
        )
        .bind::<Text, _>(kind.as_str())
        .load(&mut conn)
        .await
        .unwrap()
    }

    /// Active `fetch_series` jobs for `t15c_` series, as "SOURCE/external_id".
    async fn queued_series(pool: &DatabasePool) -> BTreeSet<String> {
        queue_rows(pool, JobKind::FetchSeries)
            .await
            .into_iter()
            .filter(|r| r.series_id.starts_with("t15c_"))
            .map(|r| format!("{}/{}", r.source, r.series_id))
            .collect()
    }

    async fn queued_discovery(pool: &DatabasePool) -> BTreeSet<String> {
        queue_rows(pool, JobKind::DiscoverCatalog)
            .await
            .into_iter()
            .map(|r| r.source)
            .collect()
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn scheduler(pool: &DatabasePool, sources: &[SourceId], batch_limit: i64) -> RefreshScheduler {
        RefreshScheduler::new(
            pool.clone(),
            sources.to_vec(),
            SchedulerConfig {
                batch_limit,
                ..SchedulerConfig::default()
            },
        )
    }

    #[test]
    fn refresh_interval_parses_fred_and_bls_frequencies() {
        let days = |f: &str| refresh_interval(f).as_secs() / 86_400;
        for (f, d) in [
            ("Daily", 1),
            ("daily, 7-day", 1),
            ("Business Daily", 1),
            ("D", 1),
            ("Weekly", 7),
            ("Weekly, Ending Friday", 7),
            ("Biweekly", 7),
            ("WEF", 7),
            ("Monthly", 7),
            ("  MONTHLY ", 7),
            ("M", 7),
            ("Quarterly", 14),
            ("Quarterly, End of Period", 14),
            ("q", 14),
            ("Annual", 30),
            ("Semiannual", 30),
            ("A", 30),
            ("SA", 30),
            ("Irregular", 7),
            ("Unknown", 7),
            ("", 7),
        ] {
            assert_eq!(days(f), d, "{f:?}");
        }
    }

    #[test]
    fn supports_fetch_excludes_static_unimplemented_and_sec() {
        for s in [
            SourceId::Fred,
            SourceId::Bls,
            SourceId::Census,
            SourceId::Fhfa,
        ] {
            assert!(supports_fetch(s), "{s}");
        }
        for s in [
            SourceId::WorldBank,
            SourceId::Imf,
            SourceId::Bea,
            SourceId::Sec,
            SourceId::Ecb,
            SourceId::Oecd,
            SourceId::UnStats,
        ] {
            assert!(!supports_fetch(s), "{s}");
        }
        let refreshable: Vec<SourceId> = crate::sources::default_registry()
            .ids()
            .into_iter()
            .filter(|&s| supports_fetch(s))
            .collect();
        assert_eq!(
            refreshable,
            vec![
                SourceId::Fred,
                SourceId::Bls,
                SourceId::Census,
                SourceId::Fhfa
            ]
        );
    }

    #[tokio::test]
    async fn sql_frequency_case_matches_rust() {
        let Some(db) = db().await else { return };
        #[derive(QueryableByName)]
        struct Days {
            #[diesel(sql_type = BigInt)]
            d: i64,
        }
        let sql = format!(
            "SELECT ({})::bigint AS d",
            frequency_days_sql("lower(btrim($1::text))")
        );
        let mut conn = db.pool.get().await.unwrap();
        for f in [
            "Daily",
            "Business Daily",
            "B",
            "Weekly, Ending Friday",
            "BW",
            "Monthly",
            "Semimonthly",
            "Quarterly",
            "Semiannual",
            "Annual",
            " a ",
            "Irregular",
        ] {
            let row: Days = diesel::sql_query(&sql)
                .bind::<Text, _>(f)
                .get_result(&mut conn)
                .await
                .unwrap();
            assert_eq!(row.d, frequency_days(f), "{f:?}");
        }
    }

    /// Only national and per-state Census ids are enqueued; bare levels and old-style ids are not.
    #[tokio::test]
    async fn skips_census_series_fetch_series_cannot_handle() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        let ids = [
            "CENSUS_BDS_T15CX_us",
            "CENSUS_BDS_T15CX_state_06",
            "CENSUS_BDS_T15CX_state_03",
            "CENSUS_BDS_T15CX_state",
            "CENSUS_BDS_T15CX_county",
            "CENSUS_BDS_T15CX_Alabama",
        ];
        let cleanup =
            "DELETE FROM economic_series WHERE external_id LIKE 'CENSUS\\_BDS\\_T15CX\\_%'";
        exec(p, cleanup).await;
        for id in ids {
            seed(p, SourceId::Census, id, "Annual", None, None).await;
        }

        scheduler(p, &[SourceId::Census], DEFAULT_BATCH_LIMIT)
            .tick()
            .await
            .unwrap();
        let queued: Vec<String> = queue_rows(p, JobKind::FetchSeries)
            .await
            .into_iter()
            .filter(|r| r.series_id.contains("T15CX"))
            .map(|r| r.series_id)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(
            queued,
            vec!["CENSUS_BDS_T15CX_state_06", "CENSUS_BDS_T15CX_us"]
        );
        exec(p, cleanup).await;
    }

    #[tokio::test]
    async fn enqueues_exactly_the_due_series_once() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        use SourceId::{Bls, Fhfa, Fred};
        seed(p, Fred, "t15c_never", "Monthly", None, None).await;
        seed(
            p,
            Fred,
            "t15c_daily_old",
            "Daily",
            Some(2.0),
            Some("success"),
        )
        .await;
        seed(
            p,
            Fred,
            "t15c_daily_fresh",
            "Daily",
            Some(0.5),
            Some("success"),
        )
        .await;
        seed(
            p,
            Fred,
            "t15c_wk_old",
            "Weekly, Ending Friday",
            Some(8.0),
            None,
        )
        .await;
        seed(
            p,
            Fred,
            "t15c_wk_fresh",
            "Weekly, Ending Friday",
            Some(6.0),
            None,
        )
        .await;
        seed(p, Bls, "t15c_m_old", "Monthly", Some(8.0), None).await;
        seed(p, Bls, "t15c_m_fresh", "Monthly", Some(5.0), None).await;
        seed(p, Bls, "t15c_q_old", "Quarterly", Some(15.0), None).await;
        seed(p, Bls, "t15c_q_fresh", "Quarterly", Some(13.0), None).await;
        seed(p, Bls, "t15c_semi_fresh", "Semiannual", Some(29.0), None).await;
        seed(p, Bls, "t15c_a_old", "Annual", Some(31.0), None).await;
        seed(p, Fhfa, "t15c_irr_old", "Irregular", Some(8.0), None).await;
        seed(p, Fhfa, "t15c_irr_fresh", "Irregular", Some(6.0), None).await;
        seed_full(p, Fred, "t15c_inactive", "Daily", None, None, false).await;

        let s = scheduler(p, &[Fred, Bls, Fhfa], DEFAULT_BATCH_LIMIT);
        let first = s.tick().await.unwrap();
        let expected = set(&[
            "BLS/t15c_a_old",
            "BLS/t15c_m_old",
            "BLS/t15c_q_old",
            "FHFA/t15c_irr_old",
            "FRED/t15c_daily_old",
            "FRED/t15c_never",
            "FRED/t15c_wk_old",
        ]);
        assert_eq!(queued_series(p).await, expected);
        assert!(first.series_enqueued >= expected.len(), "{first:?}");
        for row in queue_rows(p, JobKind::FetchSeries).await {
            assert_eq!(row.priority, DEFAULT_PRIORITY);
        }

        // Jobs still active: nothing new, nothing even selected.
        let second = s.tick().await.unwrap();
        assert_eq!(second.series_enqueued, 0, "{second:?}");
        assert_eq!(second.series_due, 0, "{second:?}");
        assert_eq!(queued_series(p).await, expected);
    }

    #[tokio::test]
    async fn skips_static_catalog_unimplemented_and_sec_sources() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        for (source, id) in [
            (SourceId::Ecb, "t15c_ecb"),
            (SourceId::Oecd, "t15c_oecd"),
            (SourceId::Imf, "t15c_imf"),
            (SourceId::Bea, "t15c_bea"),
            (SourceId::Sec, "t15c_sec"),
            (SourceId::Fred, "t15c_fred"),
        ] {
            seed(p, source, id, "Monthly", None, None).await;
        }
        let s = scheduler(
            p,
            &[
                SourceId::Ecb,
                SourceId::Oecd,
                SourceId::Imf,
                SourceId::Bea,
                SourceId::Sec,
                SourceId::Fred,
            ],
            DEFAULT_BATCH_LIMIT,
        );
        s.tick().await.unwrap();
        assert_eq!(queued_series(p).await, set(&["FRED/t15c_fred"]));
        // Series of sources that aren't in the registry are ignored too.
        seed(p, SourceId::Bls, "t15c_bls", "Monthly", None, None).await;
        s.tick().await.unwrap();
        assert_eq!(queued_series(p).await, set(&["FRED/t15c_fred"]));
    }

    #[tokio::test]
    async fn batch_limit_takes_oldest_first() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        let fhfa = persist::data_source_id(p, SourceId::Fhfa).await.unwrap();
        exec(
            p,
            &format!(
                "UPDATE economic_series SET is_active = false \
                 WHERE source_id = '{fhfa}' AND external_id NOT LIKE 't15c\\_%'"
            ),
        )
        .await;
        for (id, ago) in [
            ("t15c_6", Some(6.0)),
            ("t15c_10", Some(10.0)),
            ("t15c_8", Some(8.0)),
            ("t15c_null", None),
            ("t15c_9", Some(9.0)),
            ("t15c_7", Some(7.0)),
        ] {
            seed(p, SourceId::Fhfa, id, "Daily", ago, None).await;
        }
        let s = scheduler(p, &[SourceId::Fhfa], 3);
        let stats = s.tick().await.unwrap();
        assert_eq!(stats.series_due, 3);
        assert_eq!(stats.series_enqueued, 3);
        assert_eq!(
            queued_series(p).await,
            set(&["FHFA/t15c_null", "FHFA/t15c_10", "FHFA/t15c_9"])
        );
        s.tick().await.unwrap();
        assert_eq!(queued_series(p).await.len(), 6);
    }

    #[tokio::test]
    async fn failed_series_back_off_twice_the_interval() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        let f = Some("failed");
        // Monthly = 7 days, so failed series need 14.
        seed(p, SourceId::Fred, "t15c_f10", "Monthly", Some(10.0), f).await;
        seed(p, SourceId::Fred, "t15c_f15", "Monthly", Some(15.0), f).await;
        seed(
            p,
            SourceId::Fred,
            "t15c_ok10",
            "Monthly",
            Some(10.0),
            Some("success"),
        )
        .await;
        let recent = seed(p, SourceId::Fred, "t15c_never_recent", "Monthly", None, f).await;
        seed_attempt(p, recent, 3.0).await;
        let old = seed(p, SourceId::Fred, "t15c_never_old", "Monthly", None, f).await;
        seed_attempt(p, old, 20.0).await;
        seed(
            p,
            SourceId::Fred,
            "t15c_never_noattempt",
            "Monthly",
            None,
            f,
        )
        .await;
        // Last success long ago but a recent failed attempt: backs off.
        let mixed = seed(
            p,
            SourceId::Fred,
            "t15c_old_ok_recent_fail",
            "Monthly",
            Some(40.0),
            f,
        )
        .await;
        seed_attempt(p, mixed, 2.0).await;

        scheduler(p, &[SourceId::Fred], DEFAULT_BATCH_LIMIT)
            .tick()
            .await
            .unwrap();
        assert_eq!(
            queued_series(p).await,
            set(&[
                "FRED/t15c_f15",
                "FRED/t15c_never_noattempt",
                "FRED/t15c_never_old",
                "FRED/t15c_ok10",
            ])
        );
    }

    #[tokio::test]
    async fn discovery_is_enqueued_once_per_week() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        let wb = data_source_template(SourceId::WorldBank).name;
        persist::data_source_id(p, SourceId::WorldBank)
            .await
            .unwrap();
        exec(
            p,
            &format!("UPDATE data_sources SET is_enabled = false WHERE name = '{wb}'"),
        )
        .await;
        let s = scheduler(
            p,
            &[
                SourceId::Fred,
                SourceId::Bls,
                SourceId::Ecb,
                SourceId::WorldBank,
                SourceId::Sec,
            ],
            0,
        );
        let first = s.tick().await.unwrap();
        assert_eq!(first.discover_enqueued, 3, "{first:?}");
        assert_eq!(queued_discovery(p).await, set(&["BLS", "ECB", "FRED"]));
        for row in queue_rows(p, JobKind::DiscoverCatalog).await {
            assert_eq!(row.series_id, CATALOG_SERIES_ID);
        }

        // Active jobs: nothing new.
        assert_eq!(s.tick().await.unwrap().discover_enqueued, 0);

        // Finished just now (FRED completed, BLS failed, ECB completed): nothing new.
        exec(
            p,
            "UPDATE crawl_queue SET status = CASE WHEN source = 'BLS' THEN 'failed' ELSE 'completed' END, \
             finished_at = NOW() - interval '1 day' WHERE kind = 'discover_catalog'",
        )
        .await;
        assert_eq!(s.tick().await.unwrap().discover_enqueued, 0);
        assert!(queued_discovery(p).await.is_empty());

        // FRED's last discovery finished 8 days ago: re-discover FRED only.
        exec(
            p,
            "UPDATE crawl_queue SET finished_at = NOW() - interval '8 days' \
             WHERE kind = 'discover_catalog' AND source = 'FRED'",
        )
        .await;
        let stats = s.tick().await.unwrap();
        assert_eq!(stats.discover_enqueued, 1, "{stats:?}");
        assert_eq!(queued_discovery(p).await, set(&["FRED"]));
    }

    #[tokio::test]
    async fn run_ticks_until_shutdown_and_disabled_returns() {
        let Some(db) = db().await else { return };
        let p = &db.pool;
        let disabled = RefreshScheduler::new(
            p.clone(),
            vec![SourceId::Fred],
            SchedulerConfig {
                enabled: false,
                ..SchedulerConfig::default()
            },
        );
        // Would hang forever if it didn't return immediately.
        tokio::time::timeout(Duration::from_secs(5), disabled.run(std::future::pending()))
            .await
            .expect("disabled scheduler returns");
        assert!(queued_discovery(p).await.is_empty());

        let s = RefreshScheduler::new(
            p.clone(),
            vec![SourceId::Fred],
            SchedulerConfig {
                interval: Duration::from_secs(3600),
                ..SchedulerConfig::default()
            },
        );
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let pool = p.clone();
        let stopper = async move {
            for _ in 0..100 {
                if !queued_discovery(&pool).await.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            let _ = tx.send(());
        };
        let run = s.run(async move {
            let _ = rx.await;
        });
        tokio::time::timeout(Duration::from_secs(10), async {
            tokio::join!(run, stopper);
        })
        .await
        .expect("run stops on shutdown");
        assert_eq!(queued_discovery(p).await, set(&["FRED"]));
    }
}
