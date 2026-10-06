//! Cross-sections: one measure of one dataset, for every value of one dimension, at one date.
//!
//! This backs the GraphQL `crossSection` query (the world map reads it), decision 3 of
//! `docs/roadmap/global-analysis.md`. A request names a dataset, pins every dimension but one
//! with `filter`, and asks for the remaining dimension (`across`) at a fixed date or at each
//! key's latest non-null observation.
//!
//! A request costs two SQL statements whatever the number of keys: one reads the dataset,
//! one reads every matching series with its observation (a lateral join over `data_points`).
//!
//! Observations are read at their current revision: for each date the newest `revision_date`
//! wins, and on the same revision date a revision beats the original release (the order
//! [`econ_graph_core::models::revision_filter`] uses). `asOf` arrives with that filter.

use std::collections::BTreeMap;

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use diesel::prelude::*;
use diesel::sql_types::{Jsonb, Nullable, Text};
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;

use econ_graph_core::{
    error::{AppError, AppResult},
    models::dataset::{Dataset, VALUE_MEASURE},
    reference::{Area, Areas},
    schema::datasets,
};

/// Name of the shared code list whose keys are [`Area::key`]s.
pub use econ_graph_core::models::dataset::COUNTRIES_CODELIST;

/// Which observation of each series a cross-section returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSectionDate {
    /// The observation on this date.
    On(NaiveDate),
    /// Each series' most recent non-null observation, with its own date.
    Latest,
}

/// A cross-section request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossSectionRequest {
    pub dataset_id: Uuid,
    /// Measure to read; `None` uses the dataset's default measure.
    pub measure: Option<String>,
    /// A value for every dataset dimension except `across`.
    pub filter: BTreeMap<String, String>,
    /// The dimension the result varies over, e.g. `area`.
    pub across: String,
    pub date: CrossSectionDate,
    /// Read each series as it was known on this day: its newest revision published on or
    /// before it (PR #184's `revision_filter`). Observations first published after this day
    /// are treated as not yet known. `None` reads the current revision.
    pub as_of: Option<NaiveDate>,
}

/// One key of a cross-section.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossSectionRow {
    /// The series' value of the `across` dimension, e.g. `USA`.
    pub key: String,
    /// The reference area for `key` when `across` uses the countries code list and the key is
    /// in it, with the name the dataset gives it
    /// ([`DatasetComponent::area_label`](econ_graph_core::models::DatasetComponent::area_label));
    /// otherwise `None`.
    pub area: Option<Area>,
    pub series_id: Uuid,
    /// The observation's date: the requested date for [`CrossSectionDate::On`]; for
    /// [`CrossSectionDate::Latest`], the latest non-null observation's date, or `None` when
    /// the series has none.
    pub date: Option<NaiveDate>,
    /// `None` when the series has no non-null value at the date.
    pub value: Option<BigDecimal>,
}

/// Reads a cross-section, ordered by key (byte order, via `COLLATE "C"`, so it doesn't depend
/// on the database's collation).
///
/// Every active series of the dataset whose dimensions match `filter` and set `across` gets
/// a row, including series with no value at the date, so a map can tell "no data" from "not
/// in the dataset". An inactive series is left out entirely.
///
/// Errors: `NotFound` for an unknown dataset; `BadRequest` when `across` or a filter
/// dimension is not one of the dataset's dimensions, `filter` sets `across` or leaves another
/// dimension unset, or the measure is not declared (or, in train 1, is not `value`).
pub async fn cross_section(
    conn: &mut AsyncPgConnection,
    areas: &Areas,
    request: &CrossSectionRequest,
) -> AppResult<Vec<CrossSectionRow>> {
    let dataset = datasets::table
        .find(request.dataset_id)
        .select(Dataset::as_select())
        .first(conn)
        .await
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("dataset {} not found", request.dataset_id)))?;

    let uses_countries = validate(&dataset, request)?;

    let filter = serde_json::to_value(&request.filter)?;
    let on_date = match request.date {
        CrossSectionDate::On(date) => Some(date),
        CrossSectionDate::Latest => None,
    };

    // For each date the current revision wins (the inner ORDER BY; `id DESC` only makes that
    // order total, since the unique key on (series_id, date, revision_date,
    // is_original_release) already leaves one row per date). Of those current values, the
    // latest non-null one (or the one on `on_date`, null or not) is the series' observation;
    // `value IS NOT NULL` runs after the current revision is picked, so a null current value
    // never lets an earlier, superseded value show through.
    //
    // With `asOf` ($5), revisions first published after it are invisible (`dp.revision_date <=
    // $5`). A pre-vintage-tracking synthetic row (`revision_date = date`, tagged
    // `is_original_release`) is also dropped when the same date has another original-release
    // row at a different `revision_date` — see
    // [`econ_graph_core::models::exclude_synthetic_legacy_rows`] — so the synthetic row can't
    // stand in for "known on `$5`" when it really only means "current as of an old crawl".
    let rows: Vec<SqlRow> = diesel::sql_query(
        "SELECT s.id AS series_id, s.dimensions ->> $2 AS key, obs.date, obs.value \
         FROM economic_series s \
         LEFT JOIN LATERAL ( \
             SELECT cur.date, cur.value FROM ( \
                 SELECT DISTINCT ON (dp.date) dp.date, dp.value \
                 FROM data_points dp \
                 WHERE dp.series_id = s.id AND ($4::date IS NULL OR dp.date = $4) \
                   AND ($5::date IS NULL OR dp.revision_date <= $5) \
                   AND ($5::date IS NULL OR NOT ( \
                         dp.revision_date = dp.date AND dp.is_original_release \
                         AND EXISTS ( \
                           SELECT 1 FROM data_points other_original \
                           WHERE other_original.series_id = dp.series_id \
                             AND other_original.date = dp.date \
                             AND other_original.is_original_release \
                             AND other_original.revision_date <> dp.date \
                         ) \
                       )) \
                 ORDER BY dp.date DESC, dp.revision_date DESC, dp.is_original_release ASC, \
                          dp.id DESC \
             ) cur \
             WHERE $4::date IS NOT NULL OR cur.value IS NOT NULL \
             ORDER BY cur.date DESC \
             LIMIT 1 \
         ) obs ON TRUE \
         WHERE s.dataset_id = $1 AND s.is_active \
           AND s.dimensions @> $3 AND s.dimensions ? $2 \
         ORDER BY (s.dimensions ->> $2) COLLATE \"C\", s.id",
    )
    .bind::<diesel::sql_types::Uuid, _>(dataset.id)
    .bind::<Text, _>(&request.across)
    .bind::<Jsonb, _>(filter)
    .bind::<Nullable<diesel::sql_types::Date>, _>(on_date)
    .bind::<Nullable<diesel::sql_types::Date>, _>(request.as_of)
    .load(conn)
    .await?;

    // The across dimension when it uses the countries code list, for each key's area, named as
    // the dataset names it (the World Bank's own names for its aggregates).
    let across = dataset
        .dimensions
        .0
        .iter()
        .find(|d| d.name == request.across)
        .filter(|_| uses_countries);
    Ok(rows
        .into_iter()
        .map(|row| CrossSectionRow {
            area: across.and_then(|dim| {
                areas.by_key(&row.key).map(|area| Area {
                    name: dim.area_label(area).to_string(),
                    ..area.clone()
                })
            }),
            key: row.key,
            series_id: row.series_id,
            date: on_date.or(row.date),
            value: row.value,
        })
        .collect())
}

/// Checks the request against the dataset. Returns whether `across` uses the countries code
/// list.
fn validate(dataset: &Dataset, request: &CrossSectionRequest) -> AppResult<bool> {
    let bad = |msg: String| Err(AppError::BadRequest(msg));
    let code = &dataset.code;

    let measure = request
        .measure
        .as_deref()
        .unwrap_or(&dataset.default_measure);
    if !dataset.measures.contains(measure) {
        return bad(format!("dataset {code} has no measure {measure:?}"));
    }
    // Train 1 stores every dataset long, so data_points holds the value measure only.
    if measure != VALUE_MEASURE {
        return bad(format!(
            "measure {measure:?} of dataset {code} is not stored yet; only {VALUE_MEASURE:?} is"
        ));
    }

    let Some(across) = dataset
        .dimensions
        .0
        .iter()
        .find(|d| d.name == request.across)
    else {
        return bad(format!(
            "dataset {code} has no dimension {:?}",
            request.across
        ));
    };
    if request.filter.contains_key(&request.across) {
        return bad(format!(
            "filter must not set the across dimension {:?}",
            request.across
        ));
    }
    for name in request.filter.keys() {
        if !dataset.dimensions.contains(name) {
            return bad(format!("dataset {code} has no dimension {name:?}"));
        }
    }
    // Pinning every other dimension leaves at most one series per key, provided each series'
    // dimensions match the dataset's declared ones (nothing here checks that; a series with
    // an extra undeclared key could still collide with another on the `across` value).
    for dimension in &dataset.dimensions.0 {
        if dimension.name != request.across && !request.filter.contains_key(&dimension.name) {
            return bad(format!(
                "filter must set dimension {:?} of dataset {code}",
                dimension.name
            ));
        }
    }

    Ok(across.codelist.as_deref() == Some(COUNTRIES_CODELIST))
}

#[derive(QueryableByName)]
struct SqlRow {
    #[diesel(sql_type = diesel::sql_types::Uuid)]
    series_id: Uuid,
    #[diesel(sql_type = Text)]
    key: String,
    #[diesel(sql_type = Nullable<diesel::sql_types::Date>)]
    date: Option<NaiveDate>,
    #[diesel(sql_type = Nullable<diesel::sql_types::Numeric>)]
    value: Option<BigDecimal>,
}

#[cfg(test)]
mod tests;
