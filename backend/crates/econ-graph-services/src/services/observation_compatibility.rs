//! Selected-backend implementations of the legacy uncapped/batched observation surfaces.
//! Each call pins the durable selection, SQL identities and Iceberg pointers in one RR
//! snapshot. PostgreSQL retains grouped counts and indexed lateral latest reads.
use std::collections::{BTreeMap, HashMap, HashSet};

use super::series_service::{public_series_predicate, public_series_sql};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use econ_graph_core::{
    database::DatabasePool,
    error::{AppError, AppResult},
    models::{DataPoint, Observation, RevisionMode},
    observation_storage::{ObservationBackend, ObservationStorage},
    schema::{data_points, economic_series},
};
use econ_graph_timeseries::{SqlCatalog, StorageConfig};
use uuid::Uuid;

pub const MAX_COMPATIBILITY_ROWS: usize = 100_000;
pub const MAX_COMPATIBILITY_SERIES: usize = 2000;

fn budget_error() -> AppError {
    AppError::ServiceUnavailable(
        "observation compatibility scan budget exceeded; narrow the query".into(),
    )
}
fn check_keys(keys: &[Uuid]) -> AppResult<()> {
    if keys.len() > MAX_COMPATIBILITY_SERIES {
        return Err(budget_error());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub struct PointsQuery {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub revisions: RevisionMode,
    /// An all-revisions population may retain synthetic originals. Revision-resolving
    /// modes always exclude them, matching every existing public reader.
    pub exclude_synthetic: bool,
    pub descending: bool,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, QueryableByName)]
pub struct LatestObservation {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    pub series_id: Uuid,
    #[diesel(sql_type = diesel::sql_types::Date)]
    pub date: NaiveDate,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Numeric>)]
    pub value: Option<BigDecimal>,
    #[diesel(sql_type = diesel::sql_types::Date)]
    pub revision_date: NaiveDate,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Jsonb>)]
    pub attributes: Option<serde_json::Value>,
}

/// The caller owns an RR snapshot and has asserted its durable selection in that snapshot.
/// This function opens no additional pool connection. Missing tables fail closed.
pub async fn iceberg_rows_conn(
    conn: &mut AsyncPgConnection,
    keys: &[Uuid],
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> AppResult<Vec<Observation>> {
    check_keys(keys)?;
    let metadata = economic_series::table
        .filter(economic_series::id.eq_any(keys))
        .filter(public_series_predicate())
        .select((economic_series::id, economic_series::dataset_id))
        .load::<(Uuid, Uuid)>(conn)
        .await?;
    let mut datasets = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for (series, dataset) in metadata {
        datasets.entry(dataset).or_default().push(series);
    }
    let config = StorageConfig::from_env()
        .map_err(|_| AppError::ConfigError("invalid Iceberg storage configuration".into()))?;
    let mut result = Vec::new();
    for (dataset, series) in datasets {
        let pointer = SqlCatalog::pointer_conn(conn, dataset)
            .await
            .map_err(|_| AppError::ServiceUnavailable("could not pin Iceberg metadata".into()))?
            .ok_or_else(|| {
                AppError::ServiceUnavailable(
                    "selected Iceberg dataset has not been migrated".into(),
                )
            })?;
        let remaining = MAX_COMPATIBILITY_ROWS
            .checked_sub(result.len())
            .ok_or_else(budget_error)?;
        let rows =
            SqlCatalog::read_series_at_config(&config, &pointer, &series, start, end, remaining)
                .await
                .map_err(|_| {
                    AppError::ServiceUnavailable(
                        "Iceberg compatibility read unavailable or scan budget exceeded".into(),
                    )
                })?;
        result.extend(rows);
    }
    Ok(result)
}

/// Summary calls pin the same SQL identities/pointers as point reads, but their
/// budget measures output series, not historical revisions scanned by DataFusion.
async fn iceberg_summary_datasets(
    conn: &mut AsyncPgConnection,
    keys: &[Uuid],
) -> AppResult<Vec<(econ_graph_timeseries::TablePointer, Vec<Uuid>)>> {
    check_keys(keys)?;
    let metadata = economic_series::table
        .filter(economic_series::id.eq_any(keys))
        .filter(public_series_predicate())
        .select((economic_series::id, economic_series::dataset_id))
        .load::<(Uuid, Uuid)>(conn)
        .await?;
    let mut datasets = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for (series, dataset) in metadata {
        datasets.entry(dataset).or_default().push(series);
    }
    let mut result = Vec::new();
    for (dataset, series) in datasets {
        let pointer = SqlCatalog::pointer_conn(conn, dataset)
            .await
            .map_err(|_| AppError::ServiceUnavailable("could not pin Iceberg metadata".into()))?
            .ok_or_else(|| {
                AppError::ServiceUnavailable(
                    "selected Iceberg dataset has not been migrated".into(),
                )
            })?;
        result.push((pointer, series));
    }
    Ok(result)
}

fn summary_budget() -> AppResult<usize> {
    econ_graph_timeseries::query::QueryLimits::from_env()
        .map(|limits| limits.max_rows.min(MAX_COMPATIBILITY_ROWS))
        .map_err(|_| AppError::ConfigError("invalid Iceberg query budget".into()))
}

/// Select one observation per series without materializing historical rows. The
/// caller owns the repeatable-read transaction and asserts durable selection there.
pub async fn iceberg_latest_conn(
    conn: &mut AsyncPgConnection,
    keys: &[Uuid],
    on_date: Option<NaiveDate>,
    as_of: Option<NaiveDate>,
    non_null_current: bool,
) -> AppResult<Vec<Observation>> {
    let datasets = iceberg_summary_datasets(conn, keys).await?;
    let config = StorageConfig::from_env()
        .map_err(|_| AppError::ConfigError("invalid Iceberg storage configuration".into()))?;
    let budget = summary_budget()?;
    let mut result = Vec::new();
    for (pointer, series) in datasets {
        result.extend(
            econ_graph_timeseries::query::latest_at_config(
                &config,
                &pointer,
                &series,
                on_date,
                as_of,
                non_null_current,
                budget - result.len(),
            )
            .await
            .map_err(|_| {
                AppError::ServiceUnavailable(
                    "Iceberg latest summary unavailable or result budget exceeded".into(),
                )
            })?,
        );
    }
    Ok(result)
}

pub async fn points_batch(
    pool: &DatabasePool,
    keys: &[Uuid],
    query: PointsQuery,
) -> AppResult<HashMap<Uuid, Vec<DataPoint>>> {
    Ok(points_batch_inner(pool, keys, query, false).await?.0)
}

/// Return points and their original comparison from the same durable-selection snapshot.
pub async fn points_batch_with_revised(
    pool: &DatabasePool,
    keys: &[Uuid],
    query: PointsQuery,
) -> AppResult<(HashMap<Uuid, Vec<DataPoint>>, HashMap<Uuid, bool>)> {
    points_batch_inner(pool, keys, query, true).await
}

async fn points_batch_inner(
    pool: &DatabasePool,
    keys: &[Uuid],
    query: PointsQuery,
    with_revised: bool,
) -> AppResult<(HashMap<Uuid, Vec<DataPoint>>, HashMap<Uuid, bool>)> {
    check_keys(keys)?;
    if query.limit.is_some_and(|limit| limit < 0) {
        return Err(AppError::BadRequest(
            "observation limit must not be negative".into(),
        ));
    }
    let selection = ObservationStorage::from_env()?;
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .repeatable_read()
        .read_only()
        .run(async |conn| {
            selection.assert_read_selection_conn(conn).await?;
            if query.limit == Some(0) {
                return Ok((
                    keys.iter().map(|id| (*id, Vec::new())).collect(),
                    HashMap::new(),
                ));
            }
            let mut revised = HashMap::new();
            let rows = if selection.backend == ObservationBackend::Postgres {
                let mut sql = data_points::table
                    .filter(data_points::series_id.eq_any(keys))
                    .filter(
                        data_points::series_id.eq_any(
                            economic_series::table
                                .filter(public_series_predicate())
                                .select(economic_series::id),
                        ),
                    )
                    .into_boxed();
                if let Some(day) = query.start_date {
                    sql = sql.filter(data_points::date.ge(day));
                }
                if let Some(day) = query.end_date {
                    sql = sql.filter(data_points::date.le(day));
                }
                if query.exclude_synthetic {
                    sql = sql.filter(econ_graph_core::models::exclude_synthetic_legacy_rows());
                }
                let original = matches!(
                    query.revisions,
                    RevisionMode::Original
                        | RevisionMode::OriginalLatest
                        | RevisionMode::OriginalAsOf(_)
                );
                if original {
                    sql = sql.filter(data_points::is_original_release.eq(true));
                }
                let as_of = match query.revisions {
                    RevisionMode::AsOf(day) | RevisionMode::OriginalAsOf(day) => Some(day),
                    _ => None,
                };
                if matches!(
                    query.revisions,
                    RevisionMode::Latest
                        | RevisionMode::AsOf(_)
                        | RevisionMode::OriginalLatest
                        | RevisionMode::OriginalAsOf(_)
                ) {
                    sql = sql.filter(econ_graph_core::models::exclude_synthetic_legacy_rows());
                    sql = sql.filter(
                        econ_graph_core::models::revision_filter_excluding_synthetic(
                            as_of, original,
                        ),
                    );
                }
                sql = if query.descending {
                    sql.order((
                        data_points::date.desc(),
                        data_points::revision_date.desc(),
                        data_points::id.desc(),
                    ))
                } else {
                    sql.order((
                        data_points::date.asc(),
                        data_points::revision_date.asc(),
                        data_points::id.asc(),
                    ))
                };
                let budget = (MAX_COMPATIBILITY_ROWS + 1) as i64;
                let limit = if keys.len() == 1 {
                    query.limit.unwrap_or(budget).min(budget)
                } else {
                    budget
                };
                let rows = sql.limit(limit).load::<DataPoint>(conn).await?;
                if rows.len() > MAX_COMPATIBILITY_ROWS {
                    return Err(budget_error());
                }
                if with_revised {
                    for key in keys {
                        let selected: Vec<_> = rows
                            .iter()
                            .filter(|p| p.series_id == *key)
                            .cloned()
                            .collect();
                        revised.extend(
                            super::series_service::revised_from_original(conn, *key, &selected)
                                .await?,
                        );
                    }
                }
                rows
            } else {
                let rows = iceberg_rows_conn(conn, keys, query.start_date, query.end_date).await?;
                let selected = select_points(rows.clone(), query);
                if with_revised {
                    revised = econ_graph_timeseries::store::revised_flags(&rows, &selected);
                }
                selected.into_iter().map(DataPoint::from).collect()
            };
            let mut result: HashMap<_, Vec<_>> = keys.iter().map(|id| (*id, Vec::new())).collect();
            for row in rows {
                result.entry(row.series_id).or_default().push(row);
            }
            if let Some(limit) = query.limit {
                for rows in result.values_mut() {
                    rows.truncate(limit as usize);
                }
            }
            Ok((result, revised))
        })
        .await
}

/// Apply existing selection semantics to exact decoded values without limiting before revision
/// selection. In particular, synthetic checks see originals newer than an AsOf cutoff.
pub fn select_points(rows: Vec<Observation>, query: PointsQuery) -> Vec<Observation> {
    let original = matches!(
        query.revisions,
        RevisionMode::Original | RevisionMode::OriginalLatest | RevisionMode::OriginalAsOf(_)
    );
    let as_of = match query.revisions {
        RevisionMode::AsOf(day) | RevisionMode::OriginalAsOf(day) => Some(day),
        _ => None,
    };
    let one = matches!(
        query.revisions,
        RevisionMode::Latest
            | RevisionMode::AsOf(_)
            | RevisionMode::OriginalLatest
            | RevisionMode::OriginalAsOf(_)
    );
    let real: HashSet<_> = rows
        .iter()
        .filter(|p| p.is_original_release && p.revision_date != p.date)
        .map(|p| (p.series_id, p.date))
        .collect();
    let mut rows: Vec<_> = rows
        .into_iter()
        .filter(|p| {
            query.start_date.is_none_or(|day| p.date >= day)
                && query.end_date.is_none_or(|day| p.date <= day)
                && (!original || p.is_original_release)
                && as_of.is_none_or(|day| p.revision_date <= day)
                && (!(query.exclude_synthetic || one || original)
                    || !(p.is_original_release
                        && p.revision_date == p.date
                        && real.contains(&(p.series_id, p.date))))
        })
        .collect();
    if one {
        let mut selected = BTreeMap::new();
        for p in rows {
            let previous = selected
                .entry((p.series_id, p.date))
                .or_insert_with(|| p.clone());
            if (p.revision_date, !p.is_original_release, p.id)
                > (
                    previous.revision_date,
                    !previous.is_original_release,
                    previous.id,
                )
            {
                *previous = p;
            }
        }
        rows = selected.into_values().collect();
    }
    rows.sort_by_key(|p| (p.date, p.revision_date, p.id));
    if query.descending {
        rows.reverse();
    }
    rows
}

pub async fn counts_batch(pool: &DatabasePool, keys: &[Uuid]) -> AppResult<HashMap<Uuid, i64>> {
    check_keys(keys)?;
    let selection = ObservationStorage::from_env()?;
    let mut conn = pool.get().await?;
    conn.build_transaction()
        .repeatable_read()
        .read_only()
        .run(async |conn| {
            selection.assert_read_selection_conn(conn).await?;
            let mut result: HashMap<_, _> = keys.iter().map(|id| (*id, 0)).collect();
            if selection.backend == ObservationBackend::Postgres {
                for (id, count) in data_points::table
                    .filter(data_points::series_id.eq_any(keys))
                    .filter(
                        data_points::series_id.eq_any(
                            economic_series::table
                                .filter(public_series_predicate())
                                .select(economic_series::id),
                        ),
                    )
                    .group_by(data_points::series_id)
                    .select((data_points::series_id, diesel::dsl::count(data_points::id)))
                    .load::<(Uuid, i64)>(conn)
                    .await?
                {
                    result.insert(id, count);
                }
            } else {
                let datasets = iceberg_summary_datasets(conn, keys).await?;
                let config = StorageConfig::from_env().map_err(|_| {
                    AppError::ConfigError("invalid Iceberg storage configuration".into())
                })?;
                let budget = summary_budget()?;
                let mut returned = 0;
                for (pointer, series) in datasets {
                    for (id, count) in econ_graph_timeseries::query::counts_at_config(
                        &config,
                        &pointer,
                        &series,
                        budget - returned,
                    )
                    .await
                    .map_err(|_| {
                        AppError::ServiceUnavailable(
                            "Iceberg count summary unavailable or result budget exceeded".into(),
                        )
                    })? {
                        returned += 1;
                        result.insert(id, count);
                    }
                }
            }
            Ok(result)
        })
        .await
}

pub async fn latest_batch(
    pool: &DatabasePool,
    keys: &[Uuid],
) -> AppResult<HashMap<Uuid, Option<LatestObservation>>> {
    check_keys(keys)?;
    let selection = ObservationStorage::from_env()?;
    let mut conn = pool.get().await?;
    conn.build_transaction().repeatable_read().read_only().run(async |conn| {
        selection.assert_read_selection_conn(conn).await?;
        let mut result:HashMap<_,_>=keys.iter().map(|id|(*id,None)).collect();
        if selection.backend==ObservationBackend::Postgres {
            let rows=diesel::sql_query(format!("SELECT k.id AS series_id,dp.date,dp.value,dp.revision_date,dp.attributes FROM unnest($1::uuid[]) AS k(id) CROSS JOIN LATERAL (SELECT date,value,revision_date,attributes FROM data_points WHERE series_id=k.id AND NOT {} ORDER BY date DESC,revision_date DESC,is_original_release ASC,id DESC LIMIT 1) dp WHERE EXISTS (SELECT 1 FROM economic_series public_series WHERE public_series.id=k.id AND {})",econ_graph_core::models::synthetic_legacy_row("data_points","other_original"),public_series_sql("public_series")))
                .bind::<diesel::sql_types::Array<diesel::sql_types::Uuid>,_>(keys).load::<LatestObservation>(conn).await?;
            for row in rows {result.insert(row.series_id,Some(row));}
        } else {
            for row in iceberg_latest_conn(conn,keys,None,None,false).await? {result.insert(row.series_id,Some(LatestObservation{series_id:row.series_id,date:row.date,value:row.value,revision_date:row.revision_date,attributes:row.attributes}));}
        }
        Ok(result)
    }).await
}
