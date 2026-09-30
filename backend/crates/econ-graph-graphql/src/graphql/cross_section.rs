//! # Cross-section query
//!
//! GraphQL types for `crossSection`: one measure of one dataset, for every value of one
//! dimension, at one date. The world map reads it, one area per key. The logic lives in
//! [`econ_graph_services::services::cross_section_service`].

use std::collections::BTreeMap;

use async_graphql::{Context, Enum, ErrorExtensions, InputObject, SimpleObject, ID};
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use uuid::Uuid;

use econ_graph_core::{
    database::DatabasePool,
    error::AppError,
    reference::{self, Area, AreaKind},
};
use econ_graph_services::services::cross_section_service::{
    self, CrossSectionDate, CrossSectionRequest, CrossSectionRow,
};

/// One dimension pinned to a value, e.g. `{dimension: "indicator", value: "NY.GDP.PCAP.CD"}`.
#[derive(InputObject, Debug, Clone)]
pub struct DimensionFilterInput {
    pub dimension: String,
    pub value: String,
}

/// Whether an area is a country or an aggregate of countries.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum AreaKindType {
    /// An ISO 3166-1 country or territory, or Kosovo.
    Country,
    /// A group of countries published as one area, e.g. `WLD` (World).
    Aggregate,
}

impl From<AreaKind> for AreaKindType {
    fn from(kind: AreaKind) -> Self {
        match kind {
            AreaKind::Country => Self::Country,
            AreaKind::Aggregate => Self::Aggregate,
        }
    }
}

/// A country or aggregate from the shared countries reference file.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "Area")]
pub struct AreaType {
    /// Display name, e.g. `United States`.
    pub name: String,
    /// ISO 3166-1 alpha-3, e.g. `USA`; null for aggregates.
    pub iso3: Option<String>,
    /// ISO 3166-1 numeric, e.g. `840` (world-atlas feature ids are this, zero-padded to
    /// three digits); null for aggregates and Kosovo.
    pub iso_numeric: Option<i32>,
    pub kind: AreaKindType,
}

impl From<Area> for AreaType {
    fn from(area: Area) -> Self {
        Self {
            name: area.name,
            iso3: area.iso3,
            iso_numeric: area.iso_numeric.map(i32::from),
            kind: area.kind.into(),
        }
    }
}

/// An observation attribute, e.g. `{name: "obs_status", value: "E"}`.
#[derive(SimpleObject, Clone, Debug)]
pub struct ObservationFlag {
    pub name: String,
    pub value: String,
}

/// One key of a cross-section.
#[derive(SimpleObject, Clone, Debug)]
pub struct CrossSectionEntry {
    /// The series' value of the `across` dimension, e.g. `USA`.
    pub key: String,
    /// The area for `key` when `across` uses the countries code list; otherwise null.
    pub area: Option<AreaType>,
    pub series_id: ID,
    /// The requested date, or with `latest` the date of the key's latest non-null value
    /// (null when it has none).
    pub date: Option<NaiveDate>,
    /// Null when the series has no value at the date.
    pub value: Option<BigDecimal>,
    /// Observation attributes. Always empty until datasets store attributes.
    pub flags: Vec<ObservationFlag>,
}

impl From<CrossSectionRow> for CrossSectionEntry {
    fn from(row: CrossSectionRow) -> Self {
        Self {
            key: row.key,
            area: row.area.map(AreaType::from),
            series_id: ID(row.series_id.to_string()),
            date: row.date,
            value: row.value,
            flags: Vec::new(),
        }
    }
}

/// Resolves `crossSection`; see [`crate::graphql::query::Query`].
pub(crate) async fn resolve(
    ctx: &Context<'_>,
    dataset_id: ID,
    measure: Option<String>,
    filter: Vec<DimensionFilterInput>,
    across: String,
    date: Option<NaiveDate>,
    latest: Option<bool>,
) -> async_graphql::Result<Vec<CrossSectionEntry>> {
    let dataset_id = Uuid::parse_str(&dataset_id)
        .map_err(|_| bad_request(format!("datasetId {:?} is not a UUID", dataset_id.as_str())))?;
    let date = match (date, latest) {
        (Some(date), None | Some(false)) => CrossSectionDate::On(date),
        (None, Some(true)) => CrossSectionDate::Latest,
        _ => return Err(bad_request("give exactly one of date and latest: true")),
    };
    let mut pinned = BTreeMap::new();
    for f in filter {
        if pinned.insert(f.dimension.clone(), f.value).is_some() {
            return Err(bad_request(format!(
                "filter sets dimension {:?} more than once",
                f.dimension
            )));
        }
    }
    let request = CrossSectionRequest {
        dataset_id,
        measure,
        filter: pinned,
        across,
        date,
    };

    let areas = reference::areas().map_err(|e| {
        tracing::error!("crossSection: {e}");
        async_graphql::Error::new("reference data unavailable")
    })?;
    let pool = ctx.data::<DatabasePool>()?;
    let mut conn = pool.get().await.map_err(|e| {
        tracing::error!("crossSection: database pool: {e}");
        async_graphql::Error::new("database unavailable")
    })?;
    let rows = cross_section_service::cross_section(&mut conn, areas, &request)
        .await
        .map_err(to_graphql_error)?;
    Ok(rows.into_iter().map(CrossSectionEntry::from).collect())
}

fn bad_request(message: impl Into<String>) -> async_graphql::Error {
    async_graphql::Error::new(message.into()).extend_with(|_, e| e.set("code", "BAD_REQUEST"))
}

/// Client errors keep their message; anything else is logged and reported generically.
fn to_graphql_error(err: AppError) -> async_graphql::Error {
    match err {
        AppError::BadRequest(msg) => bad_request(msg),
        AppError::NotFound(msg) => {
            async_graphql::Error::new(msg).extend_with(|_, e| e.set("code", "NOT_FOUND"))
        }
        other => {
            tracing::error!("crossSection: {other}");
            async_graphql::Error::new("internal error")
        }
    }
}

#[cfg(test)]
mod tests;
