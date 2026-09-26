// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Shared database writes for crawl results. Every source adapter's output goes through here,
//! so upsert semantics are defined exactly once.
//!
//! - [`data_source_id`]: the `data_sources` row for a [`SourceId`] (matches the seeded names, so
//!   FRED/BLS/... map onto the existing rows; created from the core template if missing).
//! - [`persist_series`]: upserts `economic_series` + `data_points` in one transaction.
//! - [`persist_discovered`]: upserts `series_metadata` rows from catalog discovery.
//! - [`record_attempt`]: one `crawl_attempts` row per processed job (when the series exists).
//! - [`latest_point_date`]: the `since` bound for incremental fetches.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Array, Bool, Date, Nullable, Numeric, Text, Uuid as SqlUuid};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{DataSource, NewCrawlAttempt, NewDataSource};
use econ_graph_core::schema::{data_sources, series_metadata};
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use crate::adapter::{DiscoveredSeries, FetchedPoint, FetchedSeries};
use crate::source::SourceId;

/// Rows per multi-row INSERT (keeps well under Postgres' 65535 bind-parameter limit).
pub const INSERT_CHUNK: usize = 1000;

/// Frequency stored for a new series whose source gave none (`economic_series.frequency` is NOT NULL).
pub const UNKNOWN_FREQUENCY: &str = "Unknown";

/// Result of [`persist_series`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeriesWrite {
    /// `economic_series.id`.
    pub series_id: Uuid,
    /// Whether the `economic_series` row was created by this call.
    pub series_created: bool,
    /// Data points inserted, or updated because their value changed (after de-duplicating the
    /// input). Points whose stored value is already identical are not rewritten or counted.
    pub points_upserted: usize,
    /// Data points that did not exist before (`points_upserted` minus revisions of existing rows).
    pub points_new: usize,
    /// Latest observation date in the input, if any.
    pub latest_date: Option<NaiveDate>,
}

/// One `crawl_attempts` row. See [`record_attempt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRecord {
    /// Whether the job succeeded.
    pub success: bool,
    /// [`CrawlError::kind`](crate::CrawlError::kind) on failure.
    pub error_kind: Option<String>,
    /// Error message on failure.
    pub error_message: Option<String>,
    /// Points upserted.
    pub points: usize,
    /// Points that were new.
    pub new_points: usize,
    /// Latest observation date fetched.
    pub latest_date: Option<NaiveDate>,
    /// Wall-clock job duration.
    pub duration: Duration,
    /// Queue retry count at the time of the attempt.
    pub retry_count: i32,
}

fn conn_err(e: impl std::fmt::Display) -> AppError {
    AppError::DatabaseError(format!("Failed to get database connection: {e}"))
}

/// Truncates `s` to at most `max` characters (the column limits are in characters).
fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((idx, _)) => s[..idx].to_string(),
        None => s.to_string(),
    }
}

fn clip_opt(s: Option<&str>, max: usize) -> Option<String> {
    s.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| clip(v, max))
}

/// The `data_sources` template for `source`. Names match the rows seeded by the migrations and
/// the `DataSource::<source>()` constructors in econ-graph-core, so lookups hit existing rows.
pub fn data_source_template(source: SourceId) -> NewDataSource {
    match source {
        SourceId::Fred => DataSource::fred(),
        SourceId::Bls => DataSource::bls(),
        SourceId::Bea => DataSource::bea(),
        SourceId::Census => DataSource::census(),
        SourceId::WorldBank => DataSource::world_bank(),
        SourceId::Imf => DataSource::imf(),
        SourceId::Ecb => DataSource::ecb(),
        SourceId::Oecd => DataSource::oecd(),
        SourceId::Boe => DataSource::boe(),
        SourceId::Boj => DataSource::boj(),
        SourceId::Boc => DataSource::boc(),
        SourceId::Rba => DataSource::rba(),
        SourceId::Snb => DataSource::snb(),
        SourceId::UnStats => DataSource::unstats(),
        SourceId::Ilo => DataSource::ilo(),
        SourceId::Wto => DataSource::wto(),
        SourceId::Fhfa => DataSource::fhfa(),
        // Seeded by the initial migration; there is no core constructor for it.
        SourceId::Sec => NewDataSource {
            name: "SEC EDGAR".to_string(),
            description: Some(
                "SEC Electronic Data Gathering, Analysis, and Retrieval system for XBRL financial filings"
                    .to_string(),
            ),
            base_url: "https://www.sec.gov/edgar".to_string(),
            api_key_required: false,
            rate_limit_per_minute: 10,
            is_visible: true,
            is_enabled: true,
            requires_admin_approval: false,
            crawl_frequency_hours: 24,
            api_documentation_url: Some(
                "https://www.sec.gov/search-filings/edgar-application-programming-interfaces"
                    .to_string(),
            ),
            api_key_name: None,
        },
    }
}

/// `data_sources.id` for `source`: the existing row with the template's name, or a new one.
/// Race-free (`INSERT ... ON CONFLICT (name) DO NOTHING` then `SELECT`).
pub async fn data_source_id(pool: &DatabasePool, source: SourceId) -> AppResult<Uuid> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    data_source_id_conn(&mut conn, source).await
}

async fn data_source_id_conn(conn: &mut AsyncPgConnection, source: SourceId) -> AppResult<Uuid> {
    use data_sources::dsl;
    let template = data_source_template(source);
    if let Some(id) = dsl::data_sources
        .filter(dsl::name.eq(&template.name))
        .select(dsl::id)
        .first::<Uuid>(conn)
        .await
        .optional()?
    {
        return Ok(id);
    }
    diesel::insert_into(dsl::data_sources)
        .values(&template)
        .on_conflict(dsl::name)
        .do_nothing()
        .execute(conn)
        .await?;
    Ok(dsl::data_sources
        .filter(dsl::name.eq(&template.name))
        .select(dsl::id)
        .first::<Uuid>(conn)
        .await?)
}

/// `economic_series.id` for `(source, external_id)`, if the series exists.
pub async fn find_series_id(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<Uuid>> {
    use econ_graph_core::schema::economic_series::dsl as es;
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    Ok(es::economic_series
        .filter(es::source_id.eq(source_id))
        .filter(es::external_id.eq(external_id))
        .select(es::id)
        .first::<Uuid>(&mut conn)
        .await
        .optional()?)
}

/// Latest stored observation date for `(source, external_id)`; `None` if the series or its
/// points don't exist. Used as the `since` bound for incremental fetches.
pub async fn latest_point_date(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<NaiveDate>> {
    let Some(series_id) = find_series_id(pool, source, external_id).await? else {
        return Ok(None);
    };
    latest_point_date_by_id(pool, series_id).await
}

#[derive(QueryableByName)]
struct UpsertedSeries {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Bool)]
    inserted: bool,
}

#[derive(QueryableByName)]
struct WrittenPoint {
    #[diesel(sql_type = Bool)]
    inserted: bool,
}

/// Upserts one chunk of points for `series_id` and returns one row per point actually written.
///
/// A conflicting row is only rewritten when its value changed, so re-crawling unchanged data
/// creates no dead tuples. PostgreSQL 18's `RETURNING old.*` tells inserts from revisions in the
/// same statement.
async fn upsert_points(
    conn: &mut AsyncPgConnection,
    series_id: Uuid,
    points: &[&FetchedPoint],
) -> QueryResult<Vec<WrittenPoint>> {
    diesel::sql_query(
        "INSERT INTO data_points (series_id, date, value, revision_date, is_original_release) \
         SELECT $1, t.date, t.value, t.revision_date, t.is_original_release \
         FROM UNNEST($2::date[], $3::numeric[], $4::date[], $5::bool[]) \
             AS t(date, value, revision_date, is_original_release) \
         ON CONFLICT (series_id, date, revision_date, is_original_release) DO UPDATE \
             SET value = EXCLUDED.value, updated_at = NOW() \
             WHERE data_points.value IS DISTINCT FROM EXCLUDED.value \
         RETURNING (old.id IS NULL) AS inserted",
    )
    .bind::<SqlUuid, _>(series_id)
    .bind::<Array<Date>, _>(points.iter().map(|p| p.date).collect::<Vec<_>>())
    .bind::<Array<Nullable<Numeric>>, _>(points.iter().map(|p| p.value.clone()).collect::<Vec<_>>())
    .bind::<Array<Date>, _>(points.iter().map(|p| p.revision_date).collect::<Vec<_>>())
    .bind::<Array<Bool>, _>(
        points
            .iter()
            .map(|p| p.is_original_release)
            .collect::<Vec<_>>(),
    )
    .load(conn)
    .await
}

/// Upserts the `economic_series` row for `(source, external_id)` and all `fetched.points`, in one
/// transaction.
///
/// - Series: created if missing (title falls back to `external_id`, frequency to
///   [`UNKNOWN_FREQUENCY`]). When `fetched.metadata` is present, its non-empty fields replace the
///   stored ones; absent fields keep their stored values. Always sets `last_crawled_at`,
///   `last_updated`, `crawl_status = 'success'` and clears `crawl_error_message`.
/// - Points: upserted on the `data_points` unique key `(series_id, date, revision_date,
///   is_original_release)` in chunks of [`INSERT_CHUNK`]; a conflicting row gets the new value
///   only if it differs.
///   Duplicate keys in the input keep the last occurrence.
/// - `start_date` / `end_date` are recomputed from the stored points.
pub async fn persist_series(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
    fetched: &FetchedSeries,
) -> AppResult<SeriesWrite> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    let meta = fetched.metadata.as_ref();
    let title = meta.and_then(|m| clip_opt(Some(&m.title), 500));
    let description = meta.and_then(|m| clip_opt(m.description.as_deref(), 2000));
    let units = meta.and_then(|m| clip_opt(m.units.as_deref(), 100));
    let frequency = meta.and_then(|m| clip_opt(m.frequency.as_deref(), 50));
    let seasonal = meta.and_then(|m| clip_opt(m.seasonal_adjustment.as_deref(), 100));
    let external_id = clip(external_id, 255);

    // Last occurrence wins for duplicate keys (one INSERT can't touch a row twice).
    let mut unique = BTreeMap::new();
    for p in &fetched.points {
        unique.insert((p.date, p.revision_date, p.is_original_release), p);
    }
    let latest_date = unique.keys().map(|k| k.0).max();

    conn.transaction::<SeriesWrite, AppError, _>(async move |conn| {
            let row: UpsertedSeries = diesel::sql_query(
                "INSERT INTO economic_series (source_id, external_id, title, description, units, \
                     frequency, seasonal_adjustment, is_active, first_discovered_at, last_crawled_at, \
                     last_updated, crawl_status, crawl_error_message) \
                 VALUES ($1, $2, COALESCE($3, $2), $4, $5, COALESCE($6, $7), $8, TRUE, NOW(), NOW(), \
                     NOW(), 'success', NULL) \
                 ON CONFLICT (source_id, external_id) DO UPDATE SET \
                     title = COALESCE($3, economic_series.title), \
                     description = COALESCE($4, economic_series.description), \
                     units = COALESCE($5, economic_series.units), \
                     frequency = COALESCE($6, economic_series.frequency), \
                     seasonal_adjustment = COALESCE($8, economic_series.seasonal_adjustment), \
                     last_crawled_at = NOW(), last_updated = NOW(), \
                     crawl_status = 'success', crawl_error_message = NULL \
                 RETURNING id, (old.id IS NULL) AS inserted",
            )
            .bind::<SqlUuid, _>(source_id)
            .bind::<Text, _>(&external_id)
            .bind::<Nullable<Text>, _>(title.as_deref())
            .bind::<Nullable<Text>, _>(description.as_deref())
            .bind::<Nullable<Text>, _>(units.as_deref())
            .bind::<Nullable<Text>, _>(frequency.as_deref())
            .bind::<Text, _>(UNKNOWN_FREQUENCY)
            .bind::<Nullable<Text>, _>(seasonal.as_deref())
            .get_result(conn)
            .await?;
            let series_id = row.id;

            let points: Vec<&FetchedPoint> = unique.values().copied().collect();
            let (mut upserted, mut new) = (0usize, 0usize);
            for chunk in points.chunks(INSERT_CHUNK) {
                let written = upsert_points(conn, series_id, chunk).await?;
                upserted += written.len();
                new += written.iter().filter(|w| w.inserted).count();
            }

            if !points.is_empty() {
                diesel::sql_query(
                    "UPDATE economic_series SET \
                         start_date = (SELECT MIN(date) FROM data_points WHERE series_id = $1), \
                         end_date = (SELECT MAX(date) FROM data_points WHERE series_id = $1) \
                     WHERE id = $1",
                )
                .bind::<SqlUuid, _>(series_id)
                .execute(conn)
                .await?;
            }

            Ok(SeriesWrite {
                series_id,
                series_created: row.inserted,
                points_upserted: upserted,
                points_new: new,
                latest_date,
            })
    })
    .await
}

/// Upserts one `series_metadata` row per discovered series (key `(source_id, external_id)`), in
/// one transaction, chunked. Existing rows get the new title/description/units/frequency/data_url,
/// `last_discovered_at = NOW()` and `is_active = TRUE`. Duplicate ids in the input keep the last.
/// Returns the number of rows written.
pub async fn persist_discovered(
    pool: &DatabasePool,
    source: SourceId,
    discovered: &[DiscoveredSeries],
) -> AppResult<usize> {
    use diesel::upsert::excluded;
    use econ_graph_core::models::NewSeriesMetadata;
    use series_metadata::dsl as sm;

    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;

    let mut unique = BTreeMap::new();
    for d in discovered {
        if !d.external_id.trim().is_empty() {
            unique.insert(d.external_id.as_str(), d);
        }
    }
    let rows: Vec<NewSeriesMetadata> = unique
        .values()
        .map(|d| NewSeriesMetadata {
            source_id,
            external_id: clip(&d.external_id, 255),
            title: clip_opt(Some(&d.title), 500).unwrap_or_else(|| clip(&d.external_id, 500)),
            description: clip_opt(d.description.as_deref(), usize::MAX),
            units: clip_opt(d.units.as_deref(), 100),
            frequency: clip_opt(d.frequency.as_deref(), 50),
            geographic_level: None,
            data_url: clip_opt(d.data_url.as_deref(), usize::MAX),
            api_endpoint: None,
            is_active: true,
        })
        .collect();

    conn.transaction::<usize, AppError, _>(async move |conn| {
        let mut written = 0;
        for chunk in rows.chunks(INSERT_CHUNK) {
            let now = Utc::now();
            written += diesel::insert_into(sm::series_metadata)
                .values(chunk)
                .on_conflict((sm::source_id, sm::external_id))
                .do_update()
                .set((
                    sm::title.eq(excluded(sm::title)),
                    sm::description.eq(excluded(sm::description)),
                    sm::units.eq(excluded(sm::units)),
                    sm::frequency.eq(excluded(sm::frequency)),
                    sm::data_url.eq(excluded(sm::data_url)),
                    sm::is_active.eq(true),
                    sm::last_discovered_at.eq(now),
                    sm::updated_at.eq(now),
                ))
                .execute(conn)
                .await?;
        }
        Ok(written)
    })
    .await
}

/// Records one `crawl_attempts` row for `series_id` (the table requires an existing series) and,
/// on failure, sets `economic_series.crawl_status = 'failed'` with the error message.
pub async fn record_attempt(
    pool: &DatabasePool,
    series_id: Uuid,
    attempt: &AttemptRecord,
) -> AppResult<()> {
    use econ_graph_core::schema::crawl_attempts;
    let now = Utc::now();
    let started = now
        - chrono::Duration::from_std(attempt.duration).unwrap_or_else(|_| chrono::Duration::zero());
    let row = NewCrawlAttempt {
        series_id,
        attempted_at: Some(started),
        completed_at: Some(now),
        crawl_method: "api".to_string(),
        crawl_url: None,
        http_status_code: None,
        data_found: Some(attempt.points > 0),
        new_data_points: Some(i32::try_from(attempt.new_points).unwrap_or(i32::MAX)),
        latest_data_date: attempt.latest_date,
        data_freshness_hours: None,
        success: Some(attempt.success),
        error_type: attempt.error_kind.as_deref().map(|k| clip(k, 50)),
        error_message: attempt.error_message.clone(),
        retry_count: Some(attempt.retry_count),
        response_time_ms: Some(i32::try_from(attempt.duration.as_millis()).unwrap_or(i32::MAX)),
        data_size_bytes: None,
        rate_limit_remaining: None,
        user_agent: None,
        request_headers: None,
        response_headers: None,
    };
    let mut conn = pool.get().await.map_err(conn_err)?;
    diesel::insert_into(crawl_attempts::table)
        .values(&row)
        .execute(&mut conn)
        .await?;
    if !attempt.success {
        diesel::sql_query(
            "UPDATE economic_series SET crawl_status = 'failed', crawl_error_message = $2 \
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(series_id)
        .bind::<Nullable<Text>, _>(attempt.error_message.as_deref())
        .execute(&mut conn)
        .await?;
    }
    Ok(())
}

/// Latest stored date for `series_id`, if any (helper for callers that already hold the id).
pub async fn latest_point_date_by_id(
    pool: &DatabasePool,
    series_id: Uuid,
) -> AppResult<Option<NaiveDate>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    #[derive(QueryableByName)]
    struct MaxRow {
        #[diesel(sql_type = Nullable<Date>)]
        d: Option<NaiveDate>,
    }
    let row: MaxRow =
        diesel::sql_query("SELECT MAX(date) AS d FROM data_points WHERE series_id = $1")
            .bind::<SqlUuid, _>(series_id)
            .get_result(&mut conn)
            .await?;
    Ok(row.d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_counts_characters() {
        assert_eq!(clip("héllo", 2), "hé");
        assert_eq!(clip("abc", 10), "abc");
        assert_eq!(clip_opt(Some("  "), 5), None);
        assert_eq!(clip_opt(Some(" ab "), 5).as_deref(), Some("ab"));
    }

    #[test]
    fn every_source_has_a_template_with_a_distinct_name() {
        let mut names: Vec<String> = SourceId::ALL
            .into_iter()
            .map(|s| data_source_template(s).name)
            .collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SourceId::ALL.len());
        assert_eq!(
            data_source_template(SourceId::Fred).name,
            "Federal Reserve Economic Data (FRED)"
        );
        assert_eq!(data_source_template(SourceId::Sec).name, "SEC EDGAR");
    }
}
