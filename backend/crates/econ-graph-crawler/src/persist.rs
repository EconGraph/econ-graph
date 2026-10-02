// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Shared database writes for crawl results. Every source adapter's output goes through here,
//! so upsert semantics are defined exactly once. Series rows get [stable ids](crate::series_id),
//! and series are never deleted.
//!
//! - [`data_source_id`]: the `data_sources` row for a [`SourceId`] (matches the seeded names, so
//!   FRED/BLS/... map onto the existing rows; created from the core template if missing).
//! - [`sync_datasets`]: upserts the `datasets` rows from a [`DatasetCatalog`] (at startup).
//! - [`persist_series`]: upserts `economic_series` + `data_points` in one transaction.
//! - [`persist_discovered`]: upserts `series_metadata` rows from catalog discovery.
//! - [`retire_unlisted`]: marks series a complete catalog no longer lists inactive (never deletes).
//! - [`record_attempt`]: one `crawl_attempts` row per processed job (when the series exists).
//! - [`latest_point_date`]: the `since` bound for incremental fetches.
//! - [`latest_revision_date`]: the known-vintage bound for sources that track vintages.
//!
//! Every series is written with its dataset's ([`SeriesDataset`]) id and its dimension values.
//! `default_measure` on a series is an override of the dataset's and is not written: train 1
//! datasets have the single measure `value`. Callers check the series against the
//! [`DatasetCatalog`] first; persistence only resolves the synced row.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use diesel::prelude::*;
use diesel::sql_types::{
    Array, Bool, Date, Integer, Jsonb, Nullable, Numeric, Text, Uuid as SqlUuid,
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::{
    Code, DataSource, DatasetComponents, NewCrawlAttempt, NewDataSource,
};
use econ_graph_core::schema::{data_sources, datasets, series_metadata};
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use crate::adapter::{DiscoveredSeries, FetchedPoint, FetchedSeries};
use crate::dataset::{DatasetCatalog, SeriesDataset};
use crate::series_id::stable_series_id;
use crate::source::SourceId;

diesel::define_sql_function! {
    /// `substr(string, start, count)`, used by [`retire_unlisted`] to match a scope prefix
    /// literally (unlike `LIKE`, which treats `%`/`_` in the prefix as wildcards).
    fn substr(string: Text, start: Integer, count: Integer) -> Text;
}
diesel::define_sql_function! {
    /// `length(string)`, used by [`retire_unlisted`] alongside [`substr()`].
    fn length(string: Text) -> Integer;
}

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
    /// Latest observation date in the input, if any. For an incremental fetch from a
    /// vintage-tracking adapter this is only the input's latest date, which may be an old date
    /// (a revision) or absent, not the series' overall latest date; read
    /// `economic_series.end_date` for that.
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
        .execute(&mut *conn)
        .await?;
    Ok(dsl::data_sources
        .filter(dsl::name.eq(&template.name))
        .select(dsl::id)
        .first::<Uuid>(conn)
        .await?)
}

/// Upserts one `datasets` row per definition in `catalog`, keyed by `(source_id, code)`, in one
/// transaction. Existing rows get the file's name, description, components and default measure,
/// and are only rewritten (bumping `updated_at` through its trigger) when one of them changed.
/// Returns the number of definitions synced.
pub async fn sync_datasets(pool: &DatabasePool, catalog: &DatasetCatalog) -> AppResult<usize> {
    let defs: Vec<_> = catalog.iter().collect();
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<usize, AppError, _>(async move |conn| {
        for (source, def) in &defs {
            let source_id = data_source_id_conn(conn, *source).await?;
            let mut row = def.to_new_dataset(source_id);
            row.validate_components()?;
            // Preserve any codes a previous `refresh_reference_data` run merged in for a code
            // the file doesn't list (e.g. a BLS code the toml hasn't been updated for yet): the
            // file's own codes still win where both define the same one.
            merge_existing_codes(conn, source_id, &row.code, &mut row.dimensions).await?;
            diesel::sql_query(
                "INSERT INTO datasets (source_id, code, name, description, dimensions, measures, \
                     attributes, default_measure) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (source_id, code) DO UPDATE SET \
                     name = EXCLUDED.name, description = EXCLUDED.description, \
                     dimensions = EXCLUDED.dimensions, measures = EXCLUDED.measures, \
                     attributes = EXCLUDED.attributes, default_measure = EXCLUDED.default_measure \
                 WHERE (datasets.name, datasets.description, datasets.dimensions, \
                         datasets.measures, datasets.attributes, datasets.default_measure) \
                     IS DISTINCT FROM (EXCLUDED.name, EXCLUDED.description, EXCLUDED.dimensions, \
                         EXCLUDED.measures, EXCLUDED.attributes, EXCLUDED.default_measure)",
            )
            .bind::<SqlUuid, _>(row.source_id)
            .bind::<Text, _>(&row.code)
            .bind::<Text, _>(clip(&row.name, 500))
            .bind::<Nullable<Text>, _>(clip_opt(row.description.as_deref(), usize::MAX))
            .bind::<Jsonb, DatasetComponents>(row.dimensions)
            .bind::<Jsonb, DatasetComponents>(row.measures)
            .bind::<Jsonb, DatasetComponents>(row.attributes)
            .bind::<Text, _>(&row.default_measure)
            .execute(conn)
            .await?;
        }
        Ok(defs.len())
    })
    .await
}

/// Adds any code in the stored `(source_id, code)` dataset's dimensions that `dimensions`
/// doesn't already have for that dimension name, so a dynamically fetched code
/// ([`merge_dataset_dimension_codes`]) survives the next [`sync_datasets`] run even if the
/// checked-in file hasn't been updated for it. A code `dimensions` already defines is left as
/// the file wrote it. No-op if the dataset has no existing row (first sync).
async fn merge_existing_codes(
    conn: &mut AsyncPgConnection,
    source_id: Uuid,
    code: &str,
    dimensions: &mut DatasetComponents,
) -> AppResult<()> {
    use datasets::dsl;
    let existing: Option<DatasetComponents> = dsl::datasets
        .filter(dsl::source_id.eq(source_id))
        .filter(dsl::code.eq(code))
        .select(dsl::dimensions)
        // Locked against a concurrent `merge_dataset_dimension_codes` until this sync commits.
        .for_update()
        .first(conn)
        .await
        .optional()?;
    let Some(existing) = existing else {
        return Ok(());
    };
    for dim in &mut dimensions.0 {
        let Some(existing_codes) = existing
            .0
            .iter()
            .find(|d| d.name == dim.name)
            .and_then(|d| d.codes.as_ref())
        else {
            continue;
        };
        let codes = dim.codes.get_or_insert_with(Vec::new);
        for existing_code in existing_codes {
            if !codes.iter().any(|c| c.code == existing_code.code) {
                codes.push(existing_code.clone());
            }
        }
        codes.sort_unstable_by(|a, b| a.code.cmp(&b.code));
    }
    Ok(())
}

/// Cached `ETag` for `url`, one of `source`'s own reference files (e.g. a BLS code list), from
/// the last successful [`set_reference_file_etag`]. `None` if never fetched or the source sent
/// no `ETag`.
pub async fn reference_file_etag(
    pool: &DatabasePool,
    source: SourceId,
    url: &str,
) -> AppResult<Option<String>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    #[derive(QueryableByName)]
    struct Row {
        #[diesel(sql_type = Nullable<Text>)]
        etag: Option<String>,
    }
    let row: Option<Row> = diesel::sql_query(
        "SELECT etag FROM reference_file_cache WHERE source_id = $1 AND url = $2",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(url)
    .get_result(&mut conn)
    .await
    .optional()?;
    Ok(row.and_then(|r| r.etag))
}

/// Records `etag` (the response header from the fetch that found `url` changed, or `None` if the
/// source sent none) as the validator to send next time.
pub async fn set_reference_file_etag(
    pool: &DatabasePool,
    source: SourceId,
    url: &str,
    etag: Option<&str>,
) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    diesel::sql_query(
        "INSERT INTO reference_file_cache (source_id, url, etag, fetched_at) \
             VALUES ($1, $2, $3, NOW()) \
         ON CONFLICT (source_id, url) DO UPDATE SET \
             etag = EXCLUDED.etag, fetched_at = EXCLUDED.fetched_at",
    )
    .bind::<SqlUuid, _>(source_id)
    .bind::<Text, _>(url)
    .bind::<Nullable<Text>, _>(etag)
    .execute(&mut conn)
    .await?;
    Ok(())
}

/// [`merge_dataset_dimension_code_entries`] for plain `(code, label)` pairs.
pub async fn merge_dataset_dimension_codes(
    pool: &DatabasePool,
    source: SourceId,
    dataset_code: &str,
    dimension_name: &str,
    labels: &[(String, String)],
) -> AppResult<bool> {
    let entries: Vec<Code> = labels.iter().map(|(c, l)| Code::new(c, l)).collect();
    merge_dataset_dimension_code_entries(pool, source, dataset_code, dimension_name, &entries).await
}

/// Merges `entries` into `source`'s dataset `dataset_code`, dimension `dimension_name`: adds a
/// code that isn't there yet, and for one that is, updates its label plus its `unit` and
/// `description` where the entry has one (a missing one keeps what is stored). Skips the write
/// (but still returns `true`) when the merged codes equal what's already stored, so
/// `updated_at` doesn't move on every refresh of a source whose files carry no `ETag` of their
/// own for [`reference_file::refresh`](crate::reference_file::refresh) to short-circuit on.
/// Returns whether anything was merged: `false` (nothing written) when the dataset or dimension
/// isn't declared (the catalog hasn't synced yet, or the caller mis-named one) or the dimension
/// uses a shared `codelist`. The caller should then not treat the fetch that produced `entries`
/// as consumed, e.g. by caching its `ETag`.
pub async fn merge_dataset_dimension_code_entries(
    pool: &DatabasePool,
    source: SourceId,
    dataset_code: &str,
    dimension_name: &str,
    entries: &[Code],
) -> AppResult<bool> {
    use datasets::dsl;
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    conn.transaction::<bool, AppError, _>(async move |conn| {
        let row: Option<(Uuid, DatasetComponents)> = dsl::datasets
            .filter(dsl::source_id.eq(source_id))
            .filter(dsl::code.eq(dataset_code))
            .select((dsl::id, dsl::dimensions))
            // Locked: two refreshes merging different dimensions of one dataset (the worker's
            // startup refresh and a discovery job) would otherwise each write back the whole
            // `dimensions` array and drop the other's codes.
            .for_update()
            .first(conn)
            .await
            .optional()?;
        let Some((id, mut dims)) = row else {
            return Ok(false);
        };
        let Some(dim) = dims.0.iter_mut().find(|d| d.name == dimension_name) else {
            return Ok(false);
        };
        // A dimension labelled by a shared code list takes no inline codes (it may not have
        // both); like `seed_reference_codes`, store nothing, so the ETag isn't cached either.
        if dim.codelist.is_some() {
            return Ok(false);
        }
        let mut before = dim.codes.clone().unwrap_or_default();
        before.sort_unstable_by(|a, b| a.code.cmp(&b.code));
        let mut codes = before.clone();
        for entry in entries {
            match codes.iter_mut().find(|c| c.code == entry.code) {
                Some(existing) => {
                    existing.label = entry.label.clone();
                    if entry.unit.is_some() {
                        existing.unit = entry.unit.clone();
                    }
                    if entry.description.is_some() {
                        existing.description = entry.description.clone();
                    }
                }
                None => codes.push(entry.clone()),
            }
        }
        codes.sort_unstable_by(|a, b| a.code.cmp(&b.code));
        if codes == before {
            // Nothing changed: skip the write so `updated_at` doesn't move on every refresh of a
            // source (like FHFA's) with no conditional GET to short-circuit on first. The
            // dataset and dimension were still found (and not codelist-backed), so this is
            // `true`, not the "nothing to merge into" `false` above.
            return Ok(true);
        }
        dim.codes = Some(codes);
        diesel::update(dsl::datasets.filter(dsl::id.eq(id)))
            .set(dsl::dimensions.eq(dims))
            .execute(conn)
            .await?;
        Ok(true)
    })
    .await
}

/// Current inline codes of `source`'s dataset `dataset_code`, dimension `dimension_name`, by
/// code: whatever [`merge_dataset_dimension_codes`] last merged in, or the dataset file's own
/// codes if no merge has happened yet. Empty if the dataset or dimension isn't declared, or has
/// no inline codes. For an adapter that needs a code's current label or description to build a
/// series' own title or description (not just the dimension's code list).
pub async fn dataset_dimension_codes(
    pool: &DatabasePool,
    source: SourceId,
    dataset_code: &str,
    dimension_name: &str,
) -> AppResult<HashMap<String, Code>> {
    use datasets::dsl;
    let mut conn = pool.get().await.map_err(conn_err)?;
    let source_id = data_source_id_conn(&mut conn, source).await?;
    let row: Option<DatasetComponents> = dsl::datasets
        .filter(dsl::source_id.eq(source_id))
        .filter(dsl::code.eq(dataset_code))
        .select(dsl::dimensions)
        .first(&mut conn)
        .await
        .optional()?;
    Ok(row
        .and_then(|dims| dims.0.into_iter().find(|d| d.name == dimension_name))
        .and_then(|dim| dim.codes)
        .into_iter()
        .flatten()
        .map(|c| (c.code.clone(), c))
        .collect())
}

/// `datasets.id` of each of `source_id`'s datasets named in `codes`. A code without a row means
/// the catalog was not synced ([`sync_datasets`]): an error, not a silent `NULL`.
async fn dataset_ids(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    source_id: Uuid,
    codes: &[&str],
) -> AppResult<BTreeMap<String, Uuid>> {
    use econ_graph_core::schema::datasets::dsl as ds;
    if codes.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows: Vec<(String, Uuid)> = ds::datasets
        .filter(ds::source_id.eq(source_id))
        .filter(ds::code.eq_any(codes))
        .select((ds::code, ds::id))
        .load(conn)
        .await?;
    let found: BTreeMap<String, Uuid> = rows.into_iter().collect();
    if let Some(code) = codes.iter().find(|c| !found.contains_key(**c)) {
        return Err(AppError::ValidationError(format!(
            "{source} dataset {code} is not in the datasets table (run sync_datasets at startup)"
        )));
    }
    Ok(found)
}

/// The distinct dataset codes named by `datasets`.
fn dataset_codes<'a>(datasets: impl IntoIterator<Item = &'a SeriesDataset>) -> Vec<&'a str> {
    let codes: BTreeSet<&str> = datasets.into_iter().map(|d| d.code.as_str()).collect();
    codes.into_iter().collect()
}

/// `economic_series.id` for `(source, external_id)`, if the series exists.
pub async fn find_series_id(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<Uuid>> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    find_series_id_conn(&mut conn, source, external_id).await
}

/// Read-only connection lookup; fetching must not create a source before lease validation.
pub(crate) async fn find_series_id_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<Uuid>> {
    use econ_graph_core::schema::economic_series::dsl as es;
    Ok(es::economic_series
        .inner_join(data_sources::table)
        .filter(data_sources::name.eq(data_source_template(source).name))
        .filter(es::external_id.eq(external_id))
        .select(es::id)
        .first::<Uuid>(conn)
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

/// Newest stored `revision_date` for `(source, external_id)`; `None` if the series or its points
/// don't exist. The known-vintage bound for sources that
/// [track vintages](crate::SourceAdapter::tracks_vintages).
///
/// Rows with `revision_date = date` that are original releases are ignored. FRED used to store
/// every point that way (current values, no vintages), and counting them would pass the newest
/// observation date off as a known vintage, so the first vintage fetch would skip all history.
/// A real vintage published on its own observation date is ignored too; that only makes the
/// bound earlier, which re-reads vintages the upsert already has. A series whose every vintage
/// is published on its own observation date never gets a bound and is fetched in full on every
/// crawl: correct, just costly, and rare on FRED.
pub async fn latest_revision_date(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
) -> AppResult<Option<NaiveDate>> {
    let Some(series_id) = find_series_id(pool, source, external_id).await? else {
        return Ok(None);
    };
    let mut conn = pool.get().await.map_err(conn_err)?;
    #[derive(QueryableByName)]
    struct MaxRow {
        #[diesel(sql_type = Nullable<Date>)]
        d: Option<NaiveDate>,
    }
    let row: MaxRow = diesel::sql_query(
        "SELECT MAX(revision_date) AS d FROM data_points \
         WHERE series_id = $1 AND NOT (revision_date = date AND is_original_release)",
    )
    .bind::<SqlUuid, _>(series_id)
    .get_result(&mut conn)
    .await?;
    Ok(row.d)
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
/// - Series: created if missing, with the [stable id](crate::series_id) of `(source, external_id)`.
///   An existing row keeps its id. Each of title, description, units and frequency comes from
///   `fetched.metadata` when it has the field, else from the series' `series_metadata` row
///   ([`persist_discovered`]) when discovery listed it, else stays as stored. Adapters whose
///   fetch response carries no metadata (Census BDS) rely on that row. A new row with neither
///   gets `external_id` as its title and [`UNKNOWN_FREQUENCY`]. Always sets `last_crawled_at`,
///   `last_updated`, `crawl_status = 'success'` and clears `crawl_error_message`.
/// - Points: upserted on the `data_points` unique key `(series_id, date, revision_date,
///   is_original_release)` in chunks of [`INSERT_CHUNK`]; a conflicting row gets the new value
///   only if it differs.
///   Duplicate keys in the input keep the last occurrence.
/// - `start_date` / `end_date` are recomputed from the stored points.
/// - Dataset: `dataset_id` (from the synced `datasets` row) and `dimensions` are written from
///   `fetched.dataset`. `default_measure` is a per-series override and is left alone, so the
///   dataset's applies.
pub async fn persist_series(
    pool: &DatabasePool,
    source: SourceId,
    external_id: &str,
    fetched: &FetchedSeries,
) -> AppResult<SeriesWrite> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<SeriesWrite, AppError, _>(async move |conn| {
        persist_series_conn(conn, source, external_id, fetched).await
    })
    .await
}

/// Connection form; caller must wrap all related writes in a transaction.
pub(crate) async fn persist_series_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    external_id: &str,
    fetched: &FetchedSeries,
) -> AppResult<SeriesWrite> {
    let source_id = data_source_id_conn(conn, source).await?;
    let meta = fetched.metadata.as_ref();
    let title = meta.and_then(|m| clip_opt(Some(&m.title), 500));
    let description = meta.and_then(|m| clip_opt(m.description.as_deref(), 2000));
    let units = meta.and_then(|m| clip_opt(m.units.as_deref(), 100));
    let frequency = meta.and_then(|m| clip_opt(m.frequency.as_deref(), 50));
    let seasonal = meta.and_then(|m| clip_opt(m.seasonal_adjustment.as_deref(), 100));
    let external_id = clip(external_id, 255);
    let id = stable_series_id(source, &external_id);

    // Last occurrence wins for duplicate keys (one INSERT can't touch a row twice).
    let mut unique = BTreeMap::new();
    for p in &fetched.points {
        unique.insert((p.date, p.revision_date, p.is_original_release), p);
    }
    let latest_date = unique.keys().map(|k| k.0).max();

    let codes = dataset_codes([&fetched.dataset]);
    let ids = dataset_ids(&mut *conn, source, source_id, &codes).await?;
    let dataset_id = ids[&fetched.dataset.code];
    let row: UpsertedSeries = diesel::sql_query(
        // `discovered`: the series' catalog row, if discovery listed it, for the fields the fetch
        // left out. A FROM-less SELECT so the row is inserted whether or not there is one.
        "WITH discovered AS ( \
                     SELECT title, LEFT(description, 2000) AS description, units, frequency \
                     FROM series_metadata WHERE source_id = $1 AND external_id = $2) \
                 INSERT INTO economic_series (id, source_id, external_id, title, description, units, \
                     frequency, seasonal_adjustment, is_active, first_discovered_at, last_crawled_at, \
                     last_updated, crawl_status, crawl_error_message, dataset_id, dimensions) \
                 SELECT $9, $1, $2, COALESCE($3, (SELECT title FROM discovered), $2), \
                     COALESCE($4, (SELECT description FROM discovered)), \
                     COALESCE($5, (SELECT units FROM discovered)), \
                     COALESCE($6, (SELECT frequency FROM discovered), $7), $8, TRUE, NOW(), \
                     NOW(), NOW(), 'success', NULL, $10, $11 \
                 ON CONFLICT (source_id, external_id) DO UPDATE SET \
                     title = COALESCE($3, (SELECT title FROM discovered), economic_series.title), \
                     description = COALESCE($4, (SELECT description FROM discovered), \
                         economic_series.description), \
                     units = COALESCE($5, (SELECT units FROM discovered), economic_series.units), \
                     frequency = COALESCE($6, (SELECT frequency FROM discovered), \
                         economic_series.frequency), \
                     seasonal_adjustment = COALESCE($8, economic_series.seasonal_adjustment), \
                     dataset_id = $10, dimensions = $11, \
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
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(dataset_id)
    .bind::<Jsonb, _>(&fetched.dataset.dimensions)
    .get_result(&mut *conn)
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
        .execute(&mut *conn)
        .await?;
    }

    Ok(SeriesWrite {
        series_id,
        series_created: row.inserted,
        points_upserted: upserted,
        points_new: new,
        latest_date,
    })
}

/// Upserts one `series_metadata` row per discovered series (key `(source_id, external_id)`), in
/// one transaction, chunked. Every row gets the [stable id](crate::series_id) of its key. Existing
/// rows get the new title/description/units/frequency/data_url, `last_discovered_at = NOW()` and
/// `is_active = TRUE`. Duplicate ids in the input keep the last.
/// Every row gets its series' `dataset_id` and `dimensions`. `default_measure` (a per-series
/// override) is left alone.
/// Returns the number of rows written.
pub async fn persist_discovered(
    pool: &DatabasePool,
    source: SourceId,
    discovered: &[DiscoveredSeries],
) -> AppResult<usize> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<usize, AppError, _>(async move |conn| {
        persist_discovered_conn(conn, source, discovered).await
    })
    .await
}

/// Connection form; caller must wrap all related writes in a transaction.
pub(crate) async fn persist_discovered_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    discovered: &[DiscoveredSeries],
) -> AppResult<usize> {
    use diesel::upsert::excluded;
    use econ_graph_core::models::NewSeriesMetadata;
    use series_metadata::dsl as sm;

    let source_id = data_source_id_conn(conn, source).await?;

    // Keyed by the stored (clipped) id: one INSERT can't touch a row twice.
    let mut unique = BTreeMap::new();
    for d in discovered {
        if !d.external_id.trim().is_empty() {
            unique.insert(clip(&d.external_id, 255), d);
        }
    }
    let codes = dataset_codes(unique.values().map(|d| &d.dataset));
    let ids = dataset_ids(&mut *conn, source, source_id, &codes).await?;
    let rows: Vec<_> = unique
        .into_iter()
        .map(|(external_id, d)| {
            let id = sm::id.eq(stable_series_id(source, &external_id));
            (
                id,
                NewSeriesMetadata {
                    source_id,
                    title: clip_opt(Some(&d.title), 500)
                        .unwrap_or_else(|| clip(&d.external_id, 500)),
                    description: clip_opt(d.description.as_deref(), usize::MAX),
                    units: clip_opt(d.units.as_deref(), 100),
                    frequency: clip_opt(d.frequency.as_deref(), 50),
                    geographic_level: None,
                    data_url: clip_opt(d.data_url.as_deref(), usize::MAX),
                    api_endpoint: None,
                    is_active: true,
                    dataset_id: Some(ids[&d.dataset.code]),
                    dimensions: d.dataset.dimensions.clone(),
                    default_measure: None,
                    external_id,
                },
            )
        })
        .collect();
    let mut written = 0;
    for chunk in rows.chunks(INSERT_CHUNK) {
        let now = Utc::now();
        written += diesel::insert_into(sm::series_metadata)
            .values(chunk)
            .on_conflict((sm::source_id, sm::external_id))
            .do_update()
            .set((
                // Nothing references series_metadata.id, so a row created before stable ids
                // (the seeds, an older database) is moved onto its stable id here.
                sm::id.eq(excluded(sm::id)),
                sm::title.eq(excluded(sm::title)),
                sm::description.eq(excluded(sm::description)),
                sm::units.eq(excluded(sm::units)),
                sm::frequency.eq(excluded(sm::frequency)),
                sm::data_url.eq(excluded(sm::data_url)),
                sm::dataset_id.eq(excluded(sm::dataset_id)),
                sm::dimensions.eq(excluded(sm::dimensions)),
                sm::is_active.eq(true),
                sm::last_discovered_at.eq(now),
                sm::updated_at.eq(now),
            ))
            .execute(&mut *conn)
            .await?;
    }
    Ok(written)
}

/// Result of [`retire_unlisted`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retirement {
    /// `series_metadata` rows marked inactive.
    pub metadata_retired: usize,
    /// `economic_series` rows marked inactive.
    pub series_retired: usize,
    /// `economic_series` rows marked active again because the catalog lists them again.
    pub series_reactivated: usize,
}

/// Applies a complete catalog of `source` (see [`SourceAdapter::discovery_is_complete`]): its
/// `series_metadata` and `economic_series` rows that `listed` doesn't contain are marked
/// inactive, and its inactive `economic_series` rows that `listed` contains are marked active
/// again. Nothing is deleted, so a retired series keeps its id and its data points.
///
/// `scope_prefix` (see [`SourceAdapter::retirement_scope_prefix`]) restricts every check to
/// `external_id`s starting with it, so an adapter that doesn't own its whole `SourceId` (Census
/// BDS alongside seeded ACS rows) never retires or reactivates rows it didn't discover. `None`
/// scopes to the whole source.
///
/// Does nothing when `listed` is empty: a source that suddenly lists nothing is far more likely
/// broken than retired.
///
/// [`SourceAdapter::discovery_is_complete`]: crate::adapter::SourceAdapter::discovery_is_complete
/// [`SourceAdapter::retirement_scope_prefix`]: crate::adapter::SourceAdapter::retirement_scope_prefix
pub async fn retire_unlisted(
    pool: &DatabasePool,
    source: SourceId,
    listed: &[DiscoveredSeries],
    scope_prefix: Option<&str>,
) -> AppResult<Retirement> {
    if !listed.iter().any(|d| !d.external_id.trim().is_empty()) {
        return Ok(Retirement::default());
    }
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<Retirement, AppError, _>(async move |conn| {
        retire_unlisted_conn(conn, source, listed, scope_prefix).await
    })
    .await
}

/// Connection form; caller must wrap catalog writes and retirement in a transaction.
pub(crate) async fn retire_unlisted_conn(
    conn: &mut AsyncPgConnection,
    source: SourceId,
    listed: &[DiscoveredSeries],
    scope_prefix: Option<&str>,
) -> AppResult<Retirement> {
    use diesel::dsl::not;
    use econ_graph_core::schema::economic_series::dsl as es;
    use series_metadata::dsl as sm;

    let ids: Vec<String> = listed
        .iter()
        .filter(|d| !d.external_id.trim().is_empty())
        .map(|d| clip(&d.external_id, 255))
        .collect();
    if ids.is_empty() {
        return Ok(Retirement::default());
    }
    // Matched with `substr(external_id, 1, length(prefix)) = prefix` rather than LIKE, so a
    // prefix containing '%' or '_' (SQL LIKE wildcards) is still matched literally. An empty
    // prefix (the "whole source" case) matches every external_id, since substr(_, 1, 0) = ''.
    let prefix = scope_prefix.unwrap_or("").to_string();
    let source_id = data_source_id_conn(conn, source).await?;
    let sm_in_scope = || substr(sm::external_id, 1, length(prefix.clone())).eq(prefix.clone());
    let es_in_scope = || substr(es::external_id, 1, length(prefix.clone())).eq(prefix.clone());
    let metadata_retired = diesel::update(sm::series_metadata)
        .filter(sm::source_id.eq(source_id))
        .filter(sm::is_active)
        .filter(not(sm::external_id.eq_any(ids.clone())))
        .filter(sm_in_scope())
        .set(sm::is_active.eq(false))
        .execute(&mut *conn)
        .await?;
    let series_retired = diesel::update(es::economic_series)
        .filter(es::source_id.eq(source_id))
        .filter(es::is_active)
        .filter(not(es::external_id.eq_any(ids.clone())))
        .filter(es_in_scope())
        .set(es::is_active.eq(false))
        .execute(&mut *conn)
        .await?;
    let series_reactivated = diesel::update(es::economic_series)
        .filter(es::source_id.eq(source_id))
        .filter(not(es::is_active))
        .filter(es::external_id.eq_any(ids.clone()))
        .filter(es_in_scope())
        .set(es::is_active.eq(true))
        .execute(&mut *conn)
        .await?;
    let retirement = Retirement {
        metadata_retired,
        series_retired,
        series_reactivated,
    };
    if retirement != Retirement::default() {
        tracing::info!(%source, ?retirement, "applied complete catalog");
    }
    Ok(retirement)
}

/// Records one `crawl_attempts` row for `series_id` (the table requires an existing series) and,
/// on failure, sets `economic_series.crawl_status = 'failed'` with the error message.
pub async fn record_attempt(
    pool: &DatabasePool,
    series_id: Uuid,
    attempt: &AttemptRecord,
) -> AppResult<()> {
    let mut conn = pool.get().await.map_err(conn_err)?;
    conn.transaction::<(), AppError, _>(async move |conn| {
        record_attempt_conn(conn, series_id, attempt).await
    })
    .await
}

/// Connection form; caller must wrap all related writes in a transaction.
pub(crate) async fn record_attempt_conn(
    conn: &mut AsyncPgConnection,
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
    diesel::insert_into(crawl_attempts::table)
        .values(&row)
        .execute(&mut *conn)
        .await?;
    if !attempt.success {
        diesel::sql_query(
            "UPDATE economic_series SET crawl_status = 'failed', crawl_error_message = $2 \
             WHERE id = $1",
        )
        .bind::<SqlUuid, _>(series_id)
        .bind::<Nullable<Text>, _>(attempt.error_message.as_deref())
        .execute(&mut *conn)
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

#[cfg(test)]
mod reference_data_tests;
#[cfg(test)]
mod release_sources_tests;
#[cfg(test)]
pub(crate) mod stable_id_tests;
