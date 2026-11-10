// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The `stock_price_daily` dataset: daily bars from Massive, a market data vendor, as series and
//! observations.
//!
//! - One series per share class and price field. The external id is
//!   `{share_class_figi}:{field}` ([`external_id`]): the share-class FIGI names one security and
//!   survives ticker changes, and share classes of one company (GOOG and GOOGL) share a CIK.
//! - Dimensions `ticker`, `exchange`, `currency` and `field`, from the symbol table row
//!   ([`SymbolRow`]). The dataset definition is `data/datasets/massive.toml`.
//! - The 10-digit SEC CIK, the join key to company data, is the `cik10` attribute of every
//!   observation, and is also stated in the series description.
//! - Prices are unadjusted, as the vendor reports them on the day. Prices are in the row's
//!   currency and volume is in shares.
//! - Licence class [`LICENCE_CLASS`] is stated in the series and dataset descriptions. Access is
//!   gated by the source's catalog role, not by this text.
//! - A vendor correction is a new revision: the same series and date at a later
//!   `revision_date`. Through the worker, persistence stores a re-fetched bar only for the fields
//!   whose value changed ([`persist`](crate::persist) treats Massive as a source that restates
//!   its data).
//!
//! The adapter and the scheduled crawl use [`discovered_series`] and [`fetched_series`], which go
//! through the worker's normal publication path. The backfill writes Iceberg commits itself, with
//! [`ensure_series_conn`], [`dataset_id_conn`] and [`observations`].

use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, SubsecRound, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use econ_graph_core::error::{AppError, AppResult};
use econ_graph_core::models::Observation;
use econ_graph_core::DatabasePool;
use std::collections::BTreeSet;
use uuid::Uuid;

use super::symbols::SymbolRow;
use crate::adapter::{DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite};
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::series_id::stable_series_id;
use crate::source::SourceId;
use crate::units::{Scale, StructuredUnit, UnitKind, UnitsContext, UnitsSource};

/// The dataset code, defined in `data/datasets/massive.toml`.
pub const DATASET_CODE: &str = "stock_price_daily";

/// Licence class of every series in the dataset.
pub const LICENCE_CLASS: &str = "internal";

/// Frequency stored on every series.
pub const FREQUENCY: &str = "Daily";

/// Units of the `volume` field.
pub const VOLUME_UNITS: &str = "Shares";

/// Namespace for [`observation_id`]: UUIDv5 of `https://econgraph.com/ns/massive/observation` in
/// the standard URL namespace. Fixed forever: changing it changes every backfilled
/// observation's id.
pub const OBSERVATION_ID_NAMESPACE: Uuid = Uuid::from_u128(0x7499bcbf_03d3_5be6_8b3e_5781e8c23a0e);

/// One trading day's bar for one ticker, as Massive reports it, with exact decimals. The client
/// (`client.rs`) returns these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyBar {
    /// The ticker the bar was reported under.
    pub ticker: String,
    /// The trading day.
    pub date: NaiveDate,
    /// Opening price.
    pub open: BigDecimal,
    /// High price.
    pub high: BigDecimal,
    /// Low price.
    pub low: BigDecimal,
    /// Closing price.
    pub close: BigDecimal,
    /// Volume, in shares.
    pub volume: BigDecimal,
    /// Volume-weighted average price, when the vendor reports one.
    pub vwap: Option<BigDecimal>,
}

impl DailyBar {
    /// The bar's value for `field`.
    pub fn value(&self, field: PriceField) -> Option<&BigDecimal> {
        match field {
            PriceField::Open => Some(&self.open),
            PriceField::High => Some(&self.high),
            PriceField::Low => Some(&self.low),
            PriceField::Close => Some(&self.close),
            PriceField::Volume => Some(&self.volume),
            PriceField::Vwap => self.vwap.as_ref(),
        }
    }
}

/// The `field` dimension: one series per field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PriceField {
    /// Opening price.
    Open,
    /// High price.
    High,
    /// Low price.
    Low,
    /// Closing price.
    Close,
    /// Volume, in shares.
    Volume,
    /// Volume-weighted average price.
    Vwap,
}

impl PriceField {
    /// Every field, in dimension code order.
    pub const ALL: [PriceField; 6] = [
        PriceField::Open,
        PriceField::High,
        PriceField::Low,
        PriceField::Close,
        PriceField::Volume,
        PriceField::Vwap,
    ];

    /// The dimension code, as in the external id.
    pub fn as_str(self) -> &'static str {
        match self {
            PriceField::Open => "open",
            PriceField::High => "high",
            PriceField::Low => "low",
            PriceField::Close => "close",
            PriceField::Volume => "volume",
            PriceField::Vwap => "vwap",
        }
    }

    /// The field for a dimension code.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.as_str() == s)
    }

    /// Human-readable label, as in the dataset definition.
    pub fn label(self) -> &'static str {
        match self {
            PriceField::Open => "Open",
            PriceField::High => "High",
            PriceField::Low => "Low",
            PriceField::Close => "Close",
            PriceField::Volume => "Volume",
            PriceField::Vwap => "Volume-weighted average price",
        }
    }

    /// Units of this field's values: the currency code for a price, [`VOLUME_UNITS`] for volume.
    pub fn units(self, currency: &str) -> String {
        match self {
            PriceField::Volume => VOLUME_UNITS.to_string(),
            _ => currency.to_string(),
        }
    }

    fn structured_unit(self, currency: &str) -> StructuredUnit {
        match self {
            PriceField::Volume => StructuredUnit {
                kind: Some(UnitKind::Count),
                currency: None,
                scale: Some(Scale::Units),
                index_base: None,
            },
            _ => StructuredUnit {
                kind: Some(UnitKind::Currency),
                currency: Some(currency.to_string()),
                scale: Some(Scale::Units),
                index_base: None,
            },
        }
    }
}

impl std::fmt::Display for PriceField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The external id of a share class's `field` series: `{share_class_figi}:{field}`.
pub fn external_id(share_class_figi: &str, field: PriceField) -> String {
    format!("{share_class_figi}:{field}")
}

/// The share-class FIGI and field of an [`external_id`], or `None` if it isn't one.
pub fn parse_external_id(id: &str) -> Option<(&str, PriceField)> {
    let (figi, field) = id.rsplit_once(':')?;
    Some((figi, PriceField::parse(field)?)).filter(|(figi, _)| !figi.is_empty())
}

/// The stable `economic_series.id` of a share class's `field` series.
pub fn series_id(share_class_figi: &str, field: PriceField) -> Uuid {
    stable_series_id(SourceId::Massive, &external_id(share_class_figi, field))
}

/// The series' dataset and dimension values.
pub fn series_dataset(row: &SymbolRow, field: PriceField) -> SeriesDataset {
    SeriesDataset::new(
        DATASET_CODE,
        [
            ("ticker", row.ticker.as_str()),
            ("exchange", row.exchange.as_str()),
            ("currency", row.currency.as_str()),
            ("field", field.as_str()),
        ],
    )
}

fn title(row: &SymbolRow, field: PriceField) -> String {
    format!(
        "{} ({}): daily {}",
        row.name,
        row.ticker,
        field.label().to_lowercase()
    )
}

fn description(row: &SymbolRow, field: PriceField) -> String {
    let what = match field {
        PriceField::Volume => "Daily trading volume in shares".to_string(),
        _ => format!(
            "Daily {} price in {}, unadjusted for splits and dividends",
            field.label().to_lowercase(),
            row.currency
        ),
    };
    format!(
        "{what} for {} ({} on {}; share class FIGI {}; SEC CIK {}), from Massive, a market data \
         vendor. Licence class: {LICENCE_CLASS}.",
        row.name, row.ticker, row.exchange, row.share_class_figi, row.cik10
    )
}

/// Series-level metadata for a share class's `field` series: title, description (which states
/// the CIK and the licence class), units and frequency.
pub fn series_metadata(row: &SymbolRow, field: PriceField) -> NewSeriesMetadataLite {
    NewSeriesMetadataLite {
        title: title(row, field),
        description: Some(description(row, field)),
        units: Some(field.units(&row.currency)),
        units_source: UnitsSource::Structured,
        units_raw: None,
        structured_unit: Some(Box::new(field.structured_unit(&row.currency))),
        units_context: UnitsContext::NONE,
        frequency: Some(FREQUENCY.to_string()),
        seasonal_adjustment: None,
    }
}

/// The six series of one share class, for discovery.
pub fn discovered_series(row: &SymbolRow) -> Vec<DiscoveredSeries> {
    PriceField::ALL
        .into_iter()
        .map(|field| DiscoveredSeries {
            external_id: external_id(&row.share_class_figi, field),
            title: title(row, field),
            description: Some(description(row, field)),
            units: Some(field.units(&row.currency)),
            frequency: Some(FREQUENCY.to_string()),
            data_url: None,
            dataset: series_dataset(row, field),
        })
        .collect()
}

/// The attributes stored on every observation of `row`'s series.
pub fn observation_attributes(row: &SymbolRow) -> serde_json::Value {
    serde_json::json!({ "cik10": row.cik10 })
}

/// Checks that every bar is `row`'s ticker and no date repeats.
fn check_bars(row: &SymbolRow, bars: &[DailyBar]) -> Result<(), CrawlError> {
    let mut dates = BTreeSet::new();
    for bar in bars {
        if bar.ticker != row.ticker {
            return Err(CrawlError::Permanent(format!(
                "MASSIVE {}: bar for {} on {} is not this share class's ticker {}",
                row.share_class_figi, bar.ticker, bar.date, row.ticker
            )));
        }
        if !dates.insert(bar.date) {
            return Err(CrawlError::Permanent(format!(
                "MASSIVE {}: two bars for {} on {}",
                row.share_class_figi, row.ticker, bar.date
            )));
        }
    }
    Ok(())
}

/// `row`'s bars as one [`FetchedSeries`] per field, keyed by external id, for the worker's normal
/// publication path.
///
/// `bars` may hold one day (a grouped daily fetch) or many (a range). `revision_date` is the day
/// the values were fetched, so a later fetch with a corrected value is a new revision. A field a
/// bar lacks (VWAP) gives that series no point for the day. Every point is marked an original
/// release; persistence decides which ones are revisions from what is stored. Fails
/// (`Permanent`) if a bar's ticker is not `row.ticker` or a date repeats.
pub fn fetched_series(
    row: &SymbolRow,
    bars: &[DailyBar],
    revision_date: NaiveDate,
) -> Result<Vec<(String, FetchedSeries)>, CrawlError> {
    check_bars(row, bars)?;
    let attributes = observation_attributes(row);
    Ok(PriceField::ALL
        .into_iter()
        .map(|field| {
            let points = bars
                .iter()
                .filter_map(|bar| {
                    Some(FetchedPoint {
                        date: bar.date,
                        value: Some(bar.value(field)?.clone()),
                        revision_date,
                        is_original_release: true,
                        attributes: Some(attributes.clone()),
                    })
                })
                .collect();
            (
                external_id(&row.share_class_figi, field),
                FetchedSeries {
                    metadata: Some(series_metadata(row, field)),
                    points,
                    dataset: series_dataset(row, field),
                    validators: None,
                },
            )
        })
        .collect())
}

/// The stable id of one observation for writers that build Iceberg rows themselves (the
/// backfill): UUIDv5 of `{series_id}:{date}:{revision_date}:{original|revision}` in
/// [`OBSERVATION_ID_NAMESPACE`]. Rerunning a write for the same series, date and vintage gives the
/// same id, so the Iceberg write boundary treats it as the same observation.
pub fn observation_id(
    series_id: Uuid,
    date: NaiveDate,
    revision_date: NaiveDate,
    is_original_release: bool,
) -> Uuid {
    let kind = if is_original_release {
        "original"
    } else {
        "revision"
    };
    Uuid::new_v5(
        &OBSERVATION_ID_NAMESPACE,
        format!("{series_id}:{date}:{revision_date}:{kind}").as_bytes(),
    )
}

/// `row`'s bars as Iceberg observations, for writers that publish commits themselves (the
/// backfill). Ids come from [`observation_id`]. `created_at` and `updated_at` are `recorded_at`,
/// truncated to the microseconds Iceberg stores. Same checks as [`fetched_series`].
pub fn observations(
    row: &SymbolRow,
    bars: &[DailyBar],
    revision_date: NaiveDate,
    is_original_release: bool,
    recorded_at: DateTime<Utc>,
) -> Result<Vec<Observation>, CrawlError> {
    check_bars(row, bars)?;
    let recorded_at = recorded_at.trunc_subsecs(6);
    let attributes = observation_attributes(row);
    let mut rows = Vec::with_capacity(bars.len() * PriceField::ALL.len());
    for field in PriceField::ALL {
        let series_id = series_id(&row.share_class_figi, field);
        for bar in bars {
            let Some(value) = bar.value(field) else {
                continue;
            };
            rows.push(Observation {
                id: observation_id(series_id, bar.date, revision_date, is_original_release),
                series_id,
                date: bar.date,
                value: Some(value.clone()),
                revision_date,
                is_original_release,
                created_at: recorded_at,
                updated_at: recorded_at,
                attributes: Some(attributes.clone()),
            });
        }
    }
    Ok(rows)
}

/// `datasets.id` of `stock_price_daily`. Fails if the dataset catalog has not been synced
/// ([`persist::sync_datasets`](crate::persist::sync_datasets), at worker startup).
pub async fn dataset_id_conn(conn: &mut AsyncPgConnection) -> AppResult<Uuid> {
    use econ_graph_core::schema::{data_sources, datasets};
    let name = crate::persist::data_source_template(SourceId::Massive).name;
    datasets::table
        .inner_join(data_sources::table)
        .filter(data_sources::name.eq(&name))
        .filter(datasets::code.eq(DATASET_CODE))
        .select(datasets::id)
        .first::<Uuid>(conn)
        .await
        .optional()?
        .ok_or_else(|| {
            AppError::ValidationError(format!(
                "MASSIVE dataset {DATASET_CODE} is not in the datasets table (run sync_datasets \
                 at startup)"
            ))
        })
}

/// Creates or updates the six `economic_series` rows of `row`, with their stable ids, dataset,
/// dimensions and metadata, and returns their ids. It goes through the normal series persistence
/// with no points, so the write fence, lifecycle and units checks apply. Must run inside the
/// caller's transaction; `pool` is used only to publish an empty Iceberg table for the dataset
/// when mirroring is on.
pub async fn ensure_series_conn(
    conn: &mut AsyncPgConnection,
    pool: &DatabasePool,
    row: &SymbolRow,
) -> AppResult<Vec<(PriceField, Uuid)>> {
    let mut ids = Vec::with_capacity(PriceField::ALL.len());
    for field in PriceField::ALL {
        let fetched = FetchedSeries {
            metadata: Some(series_metadata(row, field)),
            points: Vec::new(),
            dataset: series_dataset(row, field),
            validators: None,
        };
        let id = external_id(&row.share_class_figi, field);
        let written = crate::persist::persist_series_conn(
            conn,
            pool,
            SourceId::Massive,
            &id,
            &fetched,
            false,
        )
        .await?;
        ids.push((field, written.series_id));
    }
    Ok(ids)
}

#[cfg(test)]
mod tests;
