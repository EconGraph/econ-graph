//! A series' units in the crawler's fixed vocabulary (`econ_graph_crawler::units`), and where
//! they came from.

use std::collections::HashMap;

use async_graphql::{Context, Enum, Result, SimpleObject};
use dataloader::BatchFn;
use econ_graph_core::database::DatabasePool;
use econ_graph_crawler::units::{self as crawler_units, Scale, Unit, UnitKind};
use uuid::Uuid;

/// Where a series' units came from.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "UnitsSource")]
pub enum UnitsSourceType {
    /// A field of the source's own that holds units (FRED's units, BEA's CL_UNIT), or the
    /// crawler's curated list of what it fetches.
    Structured,
    /// Parsed by the crawler out of free text that also says other things, such as a
    /// description.
    ParsedFromText,
    /// The series has no units.
    Unknown,
}

impl UnitsSourceType {
    /// The source for a stored `economic_series.units_source`; `Unknown` for anything else.
    pub fn from_db(s: &str) -> Self {
        match crawler_units::UnitsSource::from_db(s) {
            Some(crawler_units::UnitsSource::Structured) => Self::Structured,
            Some(crawler_units::UnitsSource::ParsedFromText) => Self::ParsedFromText,
            Some(crawler_units::UnitsSource::Unknown) | None => Self::Unknown,
        }
    }
}

/// What kind of quantity a series measures.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "UnitKind")]
pub enum UnitKindType {
    /// An amount of money at the prices of each period.
    Currency,
    /// An amount of money at the prices of a reference period (constant or chained).
    ConstantPriceCurrency,
    /// A percentage level or share.
    Percent,
    /// A percentage change between periods.
    PercentChange,
    /// A difference between two percentages.
    PercentagePoints,
    /// An index number.
    Index,
    /// A number of people or things.
    Count,
    /// Events per fixed number of a population, such as per 1000 people.
    Rate,
    /// A duration in years.
    Years,
}

impl From<UnitKind> for UnitKindType {
    fn from(k: UnitKind) -> Self {
        match k {
            UnitKind::Currency => Self::Currency,
            UnitKind::ConstantPriceCurrency => Self::ConstantPriceCurrency,
            UnitKind::Percent => Self::Percent,
            UnitKind::PercentChange => Self::PercentChange,
            UnitKind::PercentagePoints => Self::PercentagePoints,
            UnitKind::Index => Self::Index,
            UnitKind::Count => Self::Count,
            UnitKind::Rate => Self::Rate,
            UnitKind::Years => Self::Years,
        }
    }
}

/// The power of ten a series' values are in.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "UnitScale")]
pub enum UnitScaleType {
    /// Values as they are.
    Units,
    /// Values in thousands.
    Thousands,
    /// Values in millions.
    Millions,
    /// Values in billions (10^9).
    Billions,
    /// Values in trillions (10^12).
    Trillions,
}

impl From<Scale> for UnitScaleType {
    fn from(s: Scale) -> Self {
        match s {
            Scale::Units => Self::Units,
            Scale::Thousands => Self::Thousands,
            Scale::Millions => Self::Millions,
            Scale::Billions => Self::Billions,
            Scale::Trillions => Self::Trillions,
        }
    }
}

/// A series' units in a fixed vocabulary.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "ParsedUnit")]
pub struct ParsedUnitType {
    /// What the series measures.
    pub kind: UnitKindType,
    /// ISO 4217 code of a currency, when the source names it. Null for a bare "dollars".
    pub currency_code: Option<String>,
    /// The power of ten the values are in.
    pub scale: UnitScaleType,
    /// For an index: the period it equals its base value in, and that value, e.g. `2017=100`.
    pub index_base: Option<String>,
    /// For a constant-price amount: the year whose prices it uses.
    pub price_reference_year: Option<i32>,
}

impl From<Unit> for ParsedUnitType {
    fn from(u: Unit) -> Self {
        Self {
            kind: u.kind.into(),
            currency_code: u.currency,
            scale: u.scale.into(),
            index_base: u.index_base,
            price_reference_year: u.price_reference_year,
        }
    }
}

/// A series' units columns, as stored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeriesUnitsFields {
    /// `economic_series.units_source`.
    pub units_source: String,
    /// `economic_series.units_raw`.
    pub units_raw: Option<String>,
    /// `economic_series.units_parsed`.
    pub units_parsed: Option<serde_json::Value>,
}

impl SeriesUnitsFields {
    /// The series' parsed unit: the stored one, else `units` parsed now (for rows whose units
    /// came from discovery, or no fetch has written since the column was added). Null when the
    /// series has no units or they are not in the vocabulary.
    pub fn parsed_unit(&self, units: Option<&str>) -> Option<ParsedUnitType> {
        if UnitsSourceType::from_db(&self.units_source) == UnitsSourceType::Unknown {
            return None;
        }
        self.units_parsed
            .as_ref()
            .and_then(|v| serde_json::from_value::<Unit>(v.clone()).ok())
            .or_else(|| units.and_then(crawler_units::parse))
            .map(Into::into)
    }
}

/// Loads a series' units columns by series id, one query per batch. Used when a series was
/// built from a row that does not carry them (search results). A missing series maps to `None`.
pub struct SeriesUnitsBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<SeriesUnitsFields>> for SeriesUnitsBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<SeriesUnitsFields>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::economic_series::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("series units loader: no database connection: {e}");
                    return HashMap::new();
                }
            };
            let rows = match dsl::economic_series
                .filter(dsl::id.eq_any(&keys))
                .select((
                    dsl::id,
                    dsl::units_source,
                    dsl::units_raw,
                    dsl::units_parsed,
                ))
                .load::<(Uuid, String, Option<String>, Option<serde_json::Value>)>(&mut conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!("series units loader: query failed: {e}");
                    return HashMap::new();
                }
            };
            let found: HashMap<Uuid, SeriesUnitsFields> = rows
                .into_iter()
                .map(|(id, units_source, units_raw, units_parsed)| {
                    (
                        id,
                        SeriesUnitsFields {
                            units_source,
                            units_raw,
                            units_parsed,
                        },
                    )
                })
                .collect();
            keys.into_iter()
                .map(|key| (key, found.get(&key).cloned()))
                .collect()
        }
    }
}

/// Loads a series' units columns through the context's loader. Errors if the batch query
/// failed.
pub async fn load_series_units_fields(
    ctx: &Context<'_>,
    series_id: Uuid,
) -> Result<Option<SeriesUnitsFields>> {
    let loaders = crate::graphql::context::data_loaders(ctx)?;
    loaders
        .series_units_loader
        .try_load(series_id)
        .await
        .map_err(|_| async_graphql::Error::new("failed to load series units"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(source: &str, parsed: Option<serde_json::Value>) -> SeriesUnitsFields {
        SeriesUnitsFields {
            units_source: source.into(),
            units_raw: None,
            units_parsed: parsed,
        }
    }

    /// The stored unit wins over the text; without one, the text is parsed; without units,
    /// nothing.
    #[test]
    fn parsed_unit_prefers_the_stored_unit() {
        let stored = serde_json::json!({"kind": "currency", "currency": "USD", "scale": "units"});
        let unit = fields("structured", Some(stored))
            .parsed_unit(Some("Millions of current dollars"))
            .unwrap();
        assert_eq!(unit.scale, UnitScaleType::Units);
        assert_eq!(unit.currency_code.as_deref(), Some("USD"));

        let unit = fields("structured", None)
            .parsed_unit(Some("Index 2017=100"))
            .unwrap();
        assert_eq!(unit.kind, UnitKindType::Index);
        assert_eq!(unit.index_base.as_deref(), Some("2017=100"));

        assert_eq!(fields("structured", None).parsed_unit(Some("Rate")), None);
        assert_eq!(fields("unknown", None).parsed_unit(Some("Percent")), None);
        assert_eq!(fields("structured", None).parsed_unit(None), None);
        // A stored value from a newer vocabulary falls back to the text.
        let unit = fields("structured", Some(serde_json::json!({"kind": "novel"})))
            .parsed_unit(Some("Percent"))
            .unwrap();
        assert_eq!(unit.kind, UnitKindType::Percent);
    }

    #[test]
    fn units_source_from_db() {
        assert_eq!(
            UnitsSourceType::from_db("structured"),
            UnitsSourceType::Structured
        );
        assert_eq!(
            UnitsSourceType::from_db("parsed_from_text"),
            UnitsSourceType::ParsedFromText
        );
        assert_eq!(
            UnitsSourceType::from_db("unknown"),
            UnitsSourceType::Unknown
        );
        assert_eq!(UnitsSourceType::from_db("other"), UnitsSourceType::Unknown);
    }
}
