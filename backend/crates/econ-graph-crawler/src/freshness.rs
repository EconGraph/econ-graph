// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! How fresh a trading-day source's stored data is, read from Iceberg.
//!
//! For each source in [`FRESHNESS_DATASETS`] (Massive, a market data vendor), [`source_freshness`]
//! finds the newest trading day with a stored, non-NULL observation in the source's dataset, and
//! counts the trading days after it whose [refresh slot](crate::sources::massive::calendar) has
//! passed ([`observation_lag_trading_days`]). The crawler worker exports both on its coverage
//! loop, which runs once at startup and then on its interval, so a restart doesn't drop them:
//!
//! - `crawler_source_latest_observation_timestamp_seconds{source}`: that day at 00:00 UTC, absent
//!   when nothing recent is stored. For dashboards.
//! - `crawler_source_observation_lag_trading_days{source}`: the lag, 0 when current.
//!   `MassiveDataStale` fires when it is at least 1 for 6 hours.
//!
//! Only observations from the last [`FRESHNESS_WINDOW_DAYS`] are read, so the query stays small
//! on a table of many years. Data older than that counts as nothing stored: the timestamp is
//! absent and the lag is [`MAX_LAG_TRADING_DAYS`], far past the alert threshold either way.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};
use diesel::{OptionalExtension, QueryableByName};
use diesel_async::RunQueryDsl;
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::DatabasePool;
use uuid::Uuid;

use crate::persist::data_source_template;
use crate::source::SourceId;
use crate::sources::massive::calendar::{latest_settled_trading_day, observation_lag_trading_days};

pub use crate::sources::massive::calendar::MAX_LAG_TRADING_DAYS;

/// Sources whose freshness is reported, each with the code of the dataset that holds its data.
/// Each is refreshed on NYSE trading days.
pub const FRESHNESS_DATASETS: &[(SourceId, &str)] = &[(SourceId::Massive, "stock_price_daily")];

/// How many calendar days back from the latest settled trading day the query reads.
pub const FRESHNESS_WINDOW_DAYS: i64 = 45;

/// One source's freshness at a moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFreshness {
    /// The source.
    pub source: SourceId,
    /// Newest trading day with a stored observation, within the window.
    pub latest: Option<NaiveDate>,
    /// Trading days after `latest` whose refresh slot has passed.
    pub lag_trading_days: i64,
}

impl SourceFreshness {
    /// The freshness of `source` at `now` when its newest stored day is `latest`.
    pub fn at(source: SourceId, latest: Option<NaiveDate>, now: DateTime<Utc>) -> Self {
        Self {
            source,
            latest,
            lag_trading_days: observation_lag_trading_days(latest, now),
        }
    }
}

#[derive(QueryableByName)]
struct DatasetRow {
    #[diesel(sql_type = Bool)]
    enabled: bool,
    #[diesel(sql_type = diesel::sql_types::Nullable<SqlUuid>)]
    dataset_id: Option<Uuid>,
}

/// The freshness at `now` of each of `sources` that is in [`FRESHNESS_DATASETS`] and whose
/// `data_sources` row is enabled or doesn't exist yet (as for coverage). A source without its
/// dataset or Iceberg table yet has nothing stored. Pass the worker's covered sources.
///
/// # Errors
///
/// When the database or Iceberg can't be read, or the Iceberg storage configuration
/// (`ICEBERG_WAREHOUSE` and friends) is missing or invalid.
pub async fn source_freshness(
    pool: &DatabasePool,
    sources: &[SourceId],
    now: DateTime<Utc>,
) -> AppResult<Vec<SourceFreshness>> {
    let mut out = Vec::new();
    for &(source, dataset_code) in FRESHNESS_DATASETS {
        if !sources.contains(&source) {
            continue;
        }
        let row: Option<DatasetRow> = {
            let mut conn = pool.get().await.map_err(|e| {
                AppError::DatabaseError(format!("failed to get database connection: {e}"))
            })?;
            diesel::sql_query(
                "SELECT ds.is_enabled AS enabled, d.id AS dataset_id \
                 FROM data_sources ds \
                 LEFT JOIN datasets d ON d.source_id = ds.id AND d.code = $2 \
                 WHERE ds.name = $1",
            )
            .bind::<Text, _>(data_source_template(source).name)
            .bind::<Text, _>(dataset_code)
            .get_result(&mut conn)
            .await
            .optional()?
        };
        let dataset_id = match row {
            Some(DatasetRow { enabled: false, .. }) => continue,
            Some(DatasetRow { dataset_id, .. }) => dataset_id,
            None => None,
        };
        let latest = match dataset_id {
            Some(id) => latest_stored_date(pool, id, now).await?,
            None => None,
        };
        out.push(SourceFreshness::at(source, latest, now));
    }
    Ok(out)
}

/// The newest stored date in `dataset_id`'s Iceberg table within the window before `now`.
async fn latest_stored_date(
    pool: &DatabasePool,
    dataset_id: Uuid,
    now: DateTime<Utc>,
) -> AppResult<Option<NaiveDate>> {
    let iceberg = |e: anyhow::Error| {
        AppError::ServiceUnavailable(format!("reading Iceberg freshness failed: {e:#}"))
    };
    let config = econ_graph_timeseries::StorageConfig::from_env()
        .map_err(|_| AppError::ConfigError("invalid Iceberg storage configuration".into()))?;
    let catalog = econ_graph_timeseries::SqlCatalog::new(pool.clone(), config);
    let Some(pointer) = catalog.pointer(dataset_id).await.map_err(iceberg)? else {
        return Ok(None);
    };
    let since = latest_settled_trading_day(now) - Duration::days(FRESHNESS_WINDOW_DAYS);
    econ_graph_timeseries::query::latest_date_at_config(&catalog.config, &pointer, since)
        .await
        .map_err(iceberg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s)
            .expect("test timestamp")
            .with_timezone(&Utc)
    }

    fn d(s: &str) -> NaiveDate {
        s.parse().expect("test date")
    }

    #[test]
    fn freshness_lag_follows_the_trading_calendar() {
        // Thursday before Good Friday 2026 stored: current all weekend.
        let f = SourceFreshness::at(
            SourceId::Massive,
            Some(d("2026-04-02")),
            utc("2026-04-06T12:00:00Z"),
        );
        assert_eq!(f.lag_trading_days, 0);
        // Nothing stored yet.
        let f = SourceFreshness::at(SourceId::Massive, None, utc("2026-04-06T12:00:00Z"));
        assert_eq!(f.lag_trading_days, MAX_LAG_TRADING_DAYS);
    }

    #[test]
    fn massive_reads_stock_price_daily() {
        assert_eq!(
            FRESHNESS_DATASETS,
            &[(SourceId::Massive, "stock_price_daily")]
        );
    }
}
