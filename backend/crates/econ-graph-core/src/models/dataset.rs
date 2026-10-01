//! Datasets: groups of related series that share a schema.
//!
//! A dataset follows the SDMX model. Its dimensions identify a series within it, its measures
//! are the observed values and its attributes are per-observation flags. Each series records
//! its dataset and its dimension values (see [`SeriesDimensions`]). The design is in
//! `docs/roadmap/federation.md`, "Data model: datasets and series".
//!
//! In train 1 observations stay single-valued in `data_points`, so every dataset is stored long
//! and its default measure is [`VALUE_MEASURE`].

use chrono::{DateTime, Utc};
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Jsonb;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::schema::datasets;

/// Name of the single measure every train 1 dataset has.
pub const VALUE_MEASURE: &str = "value";

/// Shared code list of countries and areas (ISO 3166 alpha-3 plus World Bank aggregate codes).
pub const COUNTRIES_CODELIST: &str = "countries";
/// Shared code list of US states and DC by FIPS code.
pub const US_STATES_CODELIST: &str = "us_states";
/// Code lists a component may name in `codelist`; each is a reference data file loaded at runtime.
pub const KNOWN_CODELISTS: &[&str] = &[COUNTRIES_CODELIST, US_STATES_CODELIST];

/// Implements Diesel `Jsonb` conversion for a serde newtype.
macro_rules! jsonb_newtype {
    ($ty:ty) => {
        impl FromSql<Jsonb, Pg> for $ty {
            fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
                let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
                Ok(serde_json::from_value(value)?)
            }
        }

        impl ToSql<Jsonb, Pg> for $ty {
            fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
                let value = serde_json::to_value(self)?;
                <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, &mut out.reborrow())
            }
        }
    };
}

/// Value type of a dimension, measure or attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    String,
    Integer,
    Decimal,
    Date,
    Boolean,
}

/// One dimension, measure or attribute of a dataset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetComponent {
    /// Machine name, e.g. `state`. Unique across the dataset's components.
    pub name: String,
    /// Human-readable label, e.g. `State`.
    pub label: String,
    #[serde(rename = "type")]
    pub component_type: ComponentType,
    /// Unit of a measure, e.g. `Percent`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Inline code list: labels (and, for indicator-like dimensions, units) of the values
    /// this component takes. At most one of `codes` and `codelist` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codes: Option<Vec<Code>>,
    /// Name of a shared code list loaded at runtime from reference data, e.g. `countries`, so
    /// labels are not copied into every dataset that uses them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codelist: Option<String>,
}

/// One entry of an inline code list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Code {
    /// The value as stored in series dimensions, e.g. `06` or `NY.GDP.PCAP.CD`.
    pub code: String,
    pub label: String,
    /// Unit of series with this value, e.g. `current US$` for a WDI indicator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Code {
    /// A code with a label only.
    pub fn new(code: &str, label: &str) -> Self {
        Self {
            code: code.to_string(),
            label: label.to_string(),
            unit: None,
            description: None,
        }
    }
}

impl DatasetComponent {
    /// A component with no unit and no code list.
    pub fn new(name: &str, label: &str, component_type: ComponentType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            component_type,
            unit: None,
            codes: None,
            codelist: None,
        }
    }

    /// The single decimal `value` measure of a long dataset.
    pub fn value_measure() -> Self {
        Self::new(VALUE_MEASURE, "Value", ComponentType::Decimal)
    }
}

/// An ordered list of components, stored as a `jsonb` array.
///
/// Dimensions are kept in key order: the canonical series key lists values in this order.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow,
)]
#[diesel(sql_type = Jsonb)]
#[serde(transparent)]
pub struct DatasetComponents(pub Vec<DatasetComponent>);

jsonb_newtype!(DatasetComponents);

impl DatasetComponents {
    /// Whether a component with this name is present.
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|c| c.name == name)
    }
}

/// A series' dimension values within its dataset, stored as a flat `jsonb` object of strings,
/// e.g. `{"geo_level": "state", "state": "06", "variable": "ESTAB"}`. Empty for a series in a
/// dimensionless dataset (or a catalog row without a dataset).
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow,
)]
#[diesel(sql_type = Jsonb)]
#[serde(transparent)]
pub struct SeriesDimensions(pub BTreeMap<String, String>);

jsonb_newtype!(SeriesDimensions);

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for SeriesDimensions {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

/// A dataset row.
#[derive(Debug, Clone, PartialEq, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = datasets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Dataset {
    pub id: Uuid,
    pub source_id: Uuid,
    /// Code unique within the source, e.g. `BDS`.
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub dimensions: DatasetComponents,
    pub measures: DatasetComponents,
    pub attributes: DatasetComponents,
    /// The measure a chart plots when the user has not picked one.
    pub default_measure: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A dataset to insert.
#[derive(Debug, Clone, PartialEq, Insertable, Serialize, Deserialize)]
#[diesel(table_name = datasets)]
pub struct NewDataset {
    pub source_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub dimensions: DatasetComponents,
    pub measures: DatasetComponents,
    pub attributes: DatasetComponents,
    pub default_measure: String,
}

impl NewDataset {
    /// A long dataset with the given dimensions, one `value` measure and no attributes.
    pub fn long(
        source_id: Uuid,
        code: &str,
        name: &str,
        dimensions: Vec<DatasetComponent>,
    ) -> Self {
        Self {
            source_id,
            code: code.to_string(),
            name: name.to_string(),
            description: None,
            dimensions: DatasetComponents(dimensions),
            measures: DatasetComponents(vec![DatasetComponent::value_measure()]),
            attributes: DatasetComponents::default(),
            default_measure: VALUE_MEASURE.to_string(),
        }
    }

    /// Checks what the database cannot: component names are non-empty and unique across
    /// dimensions, measures and attributes; a component has at most one of `codes` and
    /// `codelist`, no repeated codes and only a [`KNOWN_CODELISTS`] name; and there is at least
    /// one measure, including the default one.
    pub fn validate_components(&self) -> AppResult<()> {
        let invalid = |msg: String| Err(AppError::ValidationError(msg));
        if self.code.trim().is_empty() {
            return invalid("dataset code must not be blank".to_string());
        }
        if self.measures.0.is_empty() {
            return invalid(format!("dataset {} declares no measures", self.code));
        }
        if !self.measures.contains(&self.default_measure) {
            return invalid(format!(
                "dataset {} default measure {:?} is not one of its measures",
                self.code, self.default_measure
            ));
        }
        let mut seen = HashSet::new();
        for component in self
            .dimensions
            .0
            .iter()
            .chain(&self.measures.0)
            .chain(&self.attributes.0)
        {
            if component.name.trim().is_empty() {
                return invalid(format!(
                    "dataset {} has a component with no name",
                    self.code
                ));
            }
            if !seen.insert(component.name.as_str()) {
                return invalid(format!(
                    "dataset {} declares component {:?} more than once",
                    self.code, component.name
                ));
            }
            if component.codes.is_some() && component.codelist.is_some() {
                return invalid(format!(
                    "dataset {} component {:?} sets both codes and codelist",
                    self.code, component.name
                ));
            }
            if let Some(codelist) = &component.codelist {
                if !KNOWN_CODELISTS.contains(&codelist.as_str()) {
                    return invalid(format!(
                        "dataset {} component {:?} names unknown codelist {:?}",
                        self.code, component.name, codelist
                    ));
                }
            }
            let mut codes = HashSet::new();
            for code in component.codes.iter().flatten() {
                if !codes.insert(code.code.as_str()) {
                    return invalid(format!(
                        "dataset {} component {:?} lists code {:?} more than once",
                        self.code, component.name, code.code
                    ));
                }
            }
        }
        Ok(())
    }
}

impl Dataset {
    /// Inserts a dataset after [`NewDataset::validate_components`].
    pub async fn create(pool: &DatabasePool, new_dataset: &NewDataset) -> AppResult<Self> {
        new_dataset.validate_components()?;
        let mut conn = pool.get().await?;
        let dataset = diesel::insert_into(datasets::table)
            .values(new_dataset)
            .returning(Self::as_returning())
            .get_result(&mut conn)
            .await?;
        Ok(dataset)
    }

    /// Finds a dataset by id.
    pub async fn find_by_id(pool: &DatabasePool, id: Uuid) -> AppResult<Option<Self>> {
        let mut conn = pool.get().await?;
        let dataset = datasets::table
            .find(id)
            .select(Self::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        Ok(dataset)
    }

    /// Finds a dataset by its source and code.
    pub async fn find_by_code(
        pool: &DatabasePool,
        source_id: Uuid,
        code: &str,
    ) -> AppResult<Option<Self>> {
        let mut conn = pool.get().await?;
        let dataset = datasets::table
            .filter(datasets::source_id.eq(source_id))
            .filter(datasets::code.eq(code))
            .select(Self::as_select())
            .first(&mut conn)
            .await
            .optional()?;
        Ok(dataset)
    }
}

#[cfg(test)]
mod tests;
