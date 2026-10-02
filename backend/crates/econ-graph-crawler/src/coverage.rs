// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Per-source data coverage and freshness, read from `series_metadata` and `economic_series`.
//!
//! Release exit criteria: at least 95% of each source's series have data, and scheduled refresh
//! keeps every series fresh. [`crawl_coverage`] computes both for each **covered source**: a
//! registry source the [refresh scheduler](crate::scheduler) refreshes
//! ([`supports_fetch`]) whose `data_sources` row is enabled
//! (or doesn't exist yet: discovery creates it).
//!
//! For one source:
//!
//! - **discovered**: distinct external ids among its active `series_metadata` rows (catalog
//!   discovery) and active `economic_series` rows (fetched series), except a series the source has
//!   confirmed doesn't exist (`crawl_status = 'not_found'`) and that never got any data (`end_date
//!   IS NULL`): that series is excluded entirely rather than counted as discovered-but-empty,
//!   whether it surfaces through its `series_metadata` row, its `economic_series` row, or (the
//!   usual case once both exist) both. A `series_metadata` row for a series the source confirmed
//!   NotFound before any `economic_series` row existed is deactivated at crawl time instead (see
//!   `persist::deactivate_metadata_conn`), so it already drops out of the `is_active` filter below;
//!   this exclusion additionally covers a `series_metadata` row that's still active alongside a
//!   `not_found`, dataless `economic_series` row (the series was fetched at least once before
//!   confirming NotFound). A series that *had* data before later turning up NotFound keeps counting
//!   (its `end_date` is still set), since its historical data is still real coverage.
//! - **with data**: active `economic_series` rows with at least one data point (`end_date` is set;
//!   persistence recomputes it from `data_points`).
//! - **overdue**: series with data whose last successful crawl (`last_crawled_at`, which a failed
//!   crawl leaves alone) is older than **twice** its refresh interval
//!   ([`refresh_interval`](crate::scheduler::refresh_interval), by frequency) — **ten times** for a
//!   `not_found` series, matching the scheduler's own multiplier for it (see
//!   [`scheduler`](crate::scheduler)) so a series that won't be retried for a while doesn't sit
//!   "overdue" in the meantime — or never recorded (the scheduler treats that as due too). The
//!   scheduler also considers a *failed* `fetch_series` attempt (not just a successful crawl), so a
//!   single failed or not_found refresh can make a series overdue here slightly before the
//!   scheduler actually retries it.
//! - **oldest success**: the earliest `last_crawled_at` among its active series.
//!
//! Census counts only the ids its adapter can fetch, as the scheduler does.

use chrono::{DateTime, Utc};
use diesel::sql_types::{Array, BigInt, Nullable, Text, Timestamptz};
use diesel::QueryableByName;
use diesel_async::RunQueryDsl;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::DatabasePool;
use serde::Serialize;
use std::sync::LazyLock;

use crate::persist::data_source_template;
use crate::scheduler::{frequency_days_sql, supports_fetch};
use crate::source::SourceId;

/// Release exit criterion: minimum percentage of a source's discovered series that have data.
pub const COVERAGE_TARGET_PERCENT: f64 = 95.0;

/// Coverage and freshness of one source. See the [module docs](self).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceCoverage {
    /// Source code, as stored in `crawl_queue.source` (e.g. `FRED`).
    pub source: String,
    /// Series discovered (catalog or fetched).
    pub discovered: i64,
    /// Series with at least one data point.
    pub with_data: i64,
    /// `with_data / discovered` in percent; `None` when nothing is discovered.
    pub percent: Option<f64>,
    /// Series with data whose last successful crawl is older than twice their refresh interval.
    pub overdue: i64,
    /// Earliest last successful crawl among the source's series.
    pub oldest_success: Option<DateTime<Utc>>,
}

impl SourceCoverage {
    /// `with_data / discovered` in `[0, 1]`, counting a source with nothing discovered as 0.
    pub fn ratio(&self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        match self.discovered {
            0 => 0.0,
            d => self.with_data as f64 / d as f64,
        }
    }

    /// Whether the source meets [`COVERAGE_TARGET_PERCENT`] (a source with nothing discovered
    /// doesn't).
    pub fn meets(&self, target_percent: f64) -> bool {
        self.percent.is_some_and(|p| p >= target_percent)
    }
}

#[derive(QueryableByName)]
struct CoverageRow {
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = BigInt)]
    discovered: i64,
    #[diesel(sql_type = BigInt)]
    with_data: i64,
    #[diesel(sql_type = BigInt)]
    overdue: i64,
    #[diesel(sql_type = Nullable<Timestamptz>)]
    oldest_success: Option<DateTime<Utc>>,
}

/// `$1` data_sources names, `$2` matching source codes, `$3` regex of fetchable Census ids.
static COVERAGE_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "WITH src AS ( \
             SELECT m.code, ds.id AS ds_id \
             FROM unnest($1::text[], $2::text[]) AS m(ds_name, code) \
             LEFT JOIN data_sources ds ON ds.name = m.ds_name \
             WHERE ds.id IS NULL OR ds.is_enabled \
         ), ids AS ( \
             SELECT src.code, sm.external_id::text AS external_id \
             FROM src JOIN series_metadata sm ON sm.source_id = src.ds_id \
             WHERE sm.is_active \
               AND NOT EXISTS ( \
                     SELECT 1 FROM economic_series es \
                     WHERE es.source_id = sm.source_id AND es.external_id = sm.external_id \
                       AND es.crawl_status = 'not_found' AND es.end_date IS NULL) \
             UNION \
             SELECT src.code, es.external_id::text \
             FROM src JOIN economic_series es ON es.source_id = src.ds_id \
             WHERE es.is_active \
               AND (es.crawl_status IS DISTINCT FROM 'not_found' OR es.end_date IS NOT NULL) \
         ), discovered AS ( \
             SELECT code, COUNT(*) AS n FROM ids \
             WHERE code <> '{census}' OR external_id ~ $3 \
             GROUP BY code \
         ), fetched AS ( \
             SELECT src.code, \
                    COUNT(*) FILTER (WHERE es.end_date IS NOT NULL) AS with_data, \
                    COUNT(*) FILTER (WHERE es.end_date IS NOT NULL \
                        AND COALESCE(es.last_crawled_at, '-infinity') \
                            <= NOW() - (CASE WHEN es.crawl_status = 'not_found' THEN 10 ELSE 2 END) \
                               * make_interval(days => ({days}))) \
                        AS overdue, \
                    MIN(es.last_crawled_at) AS oldest_success \
             FROM src JOIN economic_series es ON es.source_id = src.ds_id \
             WHERE es.is_active \
               AND (src.code <> '{census}' OR es.external_id ~ $3) \
             GROUP BY src.code \
         ) \
         SELECT src.code AS source, \
                COALESCE(d.n, 0) AS discovered, \
                COALESCE(f.with_data, 0) AS with_data, \
                COALESCE(f.overdue, 0) AS overdue, \
                f.oldest_success \
         FROM src \
         LEFT JOIN discovered d ON d.code = src.code \
         LEFT JOIN fetched f ON f.code = src.code \
         ORDER BY src.code",
        days = frequency_days_sql("lower(btrim(es.frequency))"),
        census = SourceId::Census.as_str(),
    )
});

/// The sources coverage is reported for, out of `registry_sources`: those the scheduler
/// refreshes, sorted and de-duplicated.
pub fn covered_sources(registry_sources: &[SourceId]) -> Vec<SourceId> {
    let mut out: Vec<SourceId> = registry_sources
        .iter()
        .copied()
        .filter(|&s| supports_fetch(s))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Coverage of each of `sources` whose `data_sources` row is enabled or missing, ordered by
/// source code. Pass [`covered_sources`] of the registry.
pub async fn crawl_coverage(
    pool: &DatabasePool,
    sources: &[SourceId],
) -> AppResult<Vec<SourceCoverage>> {
    let pairs: Vec<(String, String)> = sources
        .iter()
        .map(|&s| (s.as_str().to_string(), data_source_template(s).name))
        .collect();
    // Only read when Census is covered; the pattern is unused otherwise.
    let census_ids = if sources.contains(&SourceId::Census) {
        crate::sources::census::fetchable_id_regex()?
    } else {
        String::new()
    };
    coverage_for(pool, &pairs, &census_ids).await
}

/// [`crawl_coverage`] for explicit `(source code, data_sources name)` pairs.
async fn coverage_for(
    pool: &DatabasePool,
    pairs: &[(String, String)],
    census_ids: &str,
) -> AppResult<Vec<SourceCoverage>> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let (codes, names): (Vec<String>, Vec<String>) = pairs.iter().cloned().unzip();
    let mut conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(format!("failed to get database connection: {e}")))?;
    let rows: Vec<CoverageRow> = diesel::sql_query(COVERAGE_SQL.as_str())
        .bind::<Array<Text>, _>(&names)
        .bind::<Array<Text>, _>(&codes)
        .bind::<Text, _>(census_ids)
        .load(&mut conn)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            #[allow(clippy::cast_precision_loss)]
            let percent =
                (r.discovered > 0).then(|| 100.0 * r.with_data as f64 / r.discovered as f64);
            SourceCoverage {
                source: r.source,
                discovered: r.discovered,
                with_data: r.with_data,
                percent,
                overdue: r.overdue,
                oldest_success: r.oldest_success,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    //! DB-backed tests are skipped when `DATABASE_URL` is unset. They use their own
    //! `data_sources` rows (names prefixed `t_cov `), so rows other tests leave behind don't
    //! affect the counts.

    use super::*;
    use diesel::sql_types::{Bool, Double};
    use uuid::Uuid;

    const DAY: f64 = 24.0 * 60.0 * 60.0;

    #[derive(QueryableByName)]
    struct Id {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
    }

    async fn exec(pool: &DatabasePool, sql: &str) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(sql).execute(&mut conn).await.unwrap();
    }

    /// Recreates a test `data_sources` row; returns its id.
    async fn source(pool: &DatabasePool, name: &str, enabled: bool) -> Uuid {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM data_sources WHERE name = $1")
            .bind::<Text, _>(name)
            .execute(&mut conn)
            .await
            .unwrap();
        let row: Id = diesel::sql_query(
            "INSERT INTO data_sources (name, base_url, is_enabled) \
             VALUES ($1, 'https://example.test', $2) RETURNING id",
        )
        .bind::<Text, _>(name)
        .bind::<Bool, _>(enabled)
        .get_result(&mut conn)
        .await
        .unwrap();
        row.id
    }

    /// Inserts a catalog (discovery) row.
    async fn discovered(pool: &DatabasePool, source_id: Uuid, external_id: &str, active: bool) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "INSERT INTO series_metadata (source_id, external_id, title, is_active) \
             VALUES ($1, $2, $2, $3)",
        )
        .bind::<diesel::sql_types::Uuid, _>(source_id)
        .bind::<Text, _>(external_id)
        .bind::<Bool, _>(active)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    /// Inserts a fetched series. `crawled_days_ago = None` means never crawled successfully.
    async fn series(
        pool: &DatabasePool,
        source_id: Uuid,
        external_id: &str,
        frequency: &str,
        has_data: bool,
        crawled_days_ago: Option<f64>,
    ) {
        let mut conn = pool.get().await.unwrap();
        let dataset = econ_graph_core::test_utils::test_dataset_id(&mut conn, source_id).await;
        diesel::sql_query(
            "INSERT INTO economic_series \
               (source_id, external_id, title, frequency, is_active, end_date, last_crawled_at, \
                dataset_id) \
             VALUES ($1, $2, $2, $3, TRUE, CASE WHEN $4 THEN DATE '2026-01-01' END, \
                     NOW() - make_interval(secs => $5), $6)",
        )
        .bind::<diesel::sql_types::Uuid, _>(source_id)
        .bind::<Text, _>(external_id)
        .bind::<Text, _>(frequency)
        .bind::<Bool, _>(has_data)
        .bind::<Nullable<Double>, _>(crawled_days_ago.map(|d| d * DAY))
        .bind::<diesel::sql_types::Uuid, _>(dataset)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    fn pair(code: &str, name: &str) -> (String, String) {
        (code.to_string(), name.to_string())
    }

    #[test]
    fn covered_sources_are_the_refreshable_ones() {
        let got = covered_sources(&[
            SourceId::Sec,
            SourceId::Fred,
            SourceId::Bls,
            SourceId::Fred,
            SourceId::WorldBank,
        ]);
        assert_eq!(
            got,
            vec![SourceId::Fred, SourceId::Bls, SourceId::WorldBank]
        );
    }

    #[test]
    fn ratio_and_target() {
        let mut c = SourceCoverage {
            source: "FRED".into(),
            discovered: 20,
            with_data: 19,
            percent: Some(95.0),
            overdue: 0,
            oldest_success: None,
        };
        assert!((c.ratio() - 0.95).abs() < 1e-9);
        assert!(c.meets(COVERAGE_TARGET_PERCENT));
        c.with_data = 18;
        c.percent = Some(90.0);
        assert!(!c.meets(COVERAGE_TARGET_PERCENT));
        c.discovered = 0;
        c.with_data = 0;
        c.percent = None;
        assert_eq!(c.ratio(), 0.0);
        assert!(!c.meets(0.0), "nothing discovered never meets the target");
    }

    #[tokio::test]
    async fn coverage_from_seeded_rows() {
        let Some((url, _guard)) = crate::testkit::lock_test_db("coverage").await else {
            return;
        };
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");

        // A: 4 catalog ids (one inactive) and 4 fetched series, one of them also in the catalog
        // and one without data.
        let a = source(&pool, "t_cov A", true).await;
        discovered(&pool, a, "a1", true).await;
        discovered(&pool, a, "a2", true).await;
        discovered(&pool, a, "a3", true).await;
        discovered(&pool, a, "a_inactive", false).await;
        // a1: monthly (7-day interval), crawled 1 day ago: fresh.
        series(&pool, a, "a1", "Monthly", true, Some(1.0)).await;
        // a4: daily (1-day interval), crawled 3 days ago: overdue (> 2 days).
        series(&pool, a, "a4", "Daily", true, Some(3.0)).await;
        // a5: quarterly (14 days), crawled 20 days ago: not overdue (< 28 days).
        series(&pool, a, "a5", "Quarterly", true, Some(20.0)).await;
        // a6: fetched but no points.
        series(&pool, a, "a6", "Monthly", false, Some(0.5)).await;

        // B: nothing discovered yet.
        source(&pool, "t_cov B", true).await;
        // C: disabled, left out.
        let c = source(&pool, "t_cov C", false).await;
        discovered(&pool, c, "c1", true).await;
        // CENSUS (mapped to a test row): only ids matching the regex count.
        let census = source(&pool, "t_cov census", true).await;
        discovered(&pool, census, "CENSUS_OK_1", true).await;
        discovered(&pool, census, "CENSUS_OK_2", true).await;
        discovered(&pool, census, "CENSUS_LEVEL_ONLY", true).await;
        series(&pool, census, "CENSUS_OK_1", "Annual", true, Some(2.0)).await;
        series(
            &pool,
            census,
            "CENSUS_LEVEL_ONLY",
            "Annual",
            true,
            Some(2.0),
        )
        .await;

        let got = coverage_for(
            &pool,
            &[
                pair("A", "t_cov A"),
                pair("B", "t_cov B"),
                pair("C", "t_cov C"),
                pair("CENSUS", "t_cov census"),
                pair("D", "t_cov missing"),
            ],
            "^CENSUS_OK_[0-9]+$",
        )
        .await
        .unwrap();

        let sources: Vec<&str> = got.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, ["A", "B", "CENSUS", "D"], "disabled C is left out");

        let a = &got[0];
        // a1, a2, a3 (catalog) + a4, a5, a6 (fetched only); a_inactive excluded.
        assert_eq!((a.discovered, a.with_data, a.overdue), (6, 3, 1));
        assert!((a.percent.unwrap() - 50.0).abs() < 1e-9);
        let oldest = a.oldest_success.expect("oldest success");
        let expected = Utc::now() - chrono::Duration::days(20);
        assert!((oldest - expected).num_seconds().abs() < 60, "{oldest}");

        let b = &got[1];
        assert_eq!((b.discovered, b.with_data, b.overdue), (0, 0, 0));
        assert_eq!(b.percent, None);
        assert_eq!(b.oldest_success, None);

        let census = &got[2];
        assert_eq!(
            (census.discovered, census.with_data, census.overdue),
            (2, 1, 0)
        );

        // No data_sources row yet (discovery never ran): reported, empty.
        assert_eq!((got[3].discovered, got[3].percent), (0, None));

        exec(
            &pool,
            "DELETE FROM data_sources WHERE name LIKE 't\\_cov %'",
        )
        .await;
    }

    /// Sets `economic_series.crawl_status` for an already-inserted row.
    async fn set_crawl_status(
        pool: &DatabasePool,
        source_id: Uuid,
        external_id: &str,
        status: &str,
    ) {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query(
            "UPDATE economic_series SET crawl_status = $3 \
             WHERE source_id = $1 AND external_id = $2",
        )
        .bind::<diesel::sql_types::Uuid, _>(source_id)
        .bind::<Text, _>(external_id)
        .bind::<Text, _>(status)
        .execute(&mut conn)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn not_found_series_excluded_unless_it_has_data() {
        let Some((url, _guard)) = crate::testkit::lock_test_db("coverage").await else {
            return;
        };
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");

        let a = source(&pool, "t_cov notfound", true).await;
        // n1: confirmed NotFound, never got data: excluded entirely (ECO-254).
        series(&pool, a, "n1", "Monthly", false, Some(0.1)).await;
        set_crawl_status(&pool, a, "n1", "not_found").await;
        // n2: had data before later turning up NotFound: still counts, its history is real.
        series(&pool, a, "n2", "Monthly", true, Some(0.1)).await;
        set_crawl_status(&pool, a, "n2", "not_found").await;
        // n3: ordinary fetched series with data, for a sanity baseline.
        series(&pool, a, "n3", "Monthly", true, Some(0.1)).await;
        // n4: catalog-discovered (series_metadata still active) AND fetched once with no data,
        // now confirmed NotFound: the series_metadata side must be excluded too, not just the
        // economic_series side (n1's case), since both rows exist once a series has been fetched
        // at least once before going NotFound.
        discovered(&pool, a, "n4", true).await;
        series(&pool, a, "n4", "Monthly", false, Some(0.1)).await;
        set_crawl_status(&pool, a, "n4", "not_found").await;
        // n5: had data, now not_found, crawled 20 days ago (Monthly = 7-day interval): overdue at
        // twice the interval (14 days) but not at ten times (70), the multiplier a not_found
        // series actually gets, matching how long the scheduler waits before retrying it.
        series(&pool, a, "n5", "Monthly", true, Some(20.0)).await;
        set_crawl_status(&pool, a, "n5", "not_found").await;

        let got = coverage_for(&pool, &[pair("A", "t_cov notfound")], "")
            .await
            .unwrap();
        let a = &got[0];
        assert_eq!(
            (a.discovered, a.with_data, a.overdue),
            (3, 3, 0),
            "n1 and n4 excluded; n2, n3, n5 counted; n5 not overdue at the not_found multiplier"
        );

        exec(
            &pool,
            "DELETE FROM data_sources WHERE name = 't_cov notfound'",
        )
        .await;
    }

    #[tokio::test]
    async fn crawl_coverage_with_no_sources_is_empty() {
        let Some((url, _guard)) = crate::testkit::lock_test_db("coverage").await else {
            return;
        };
        let pool = econ_graph_core::create_pool(&url).await.expect("pool");
        assert!(crawl_coverage(&pool, &[]).await.unwrap().is_empty());
    }
}
