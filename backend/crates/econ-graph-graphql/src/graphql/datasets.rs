//! # Dataset GraphQL types and loaders
//!
//! Datasets group series that share a schema (SDMX-style dimensions, measures and attributes).
//! See `docs/roadmap/federation.md`, "Data model: datasets and series".
//!
//! Clients use these types to:
//! - list a source's datasets and their dimension definitions, with code labels, for pickers
//!   such as the world map's indicator picker;
//! - read a series' dataset, its labelled dimension values and its default measure.
//!
//! The loaders here are non-caching: they batch loads made in the same resolver pass (one query
//! per batch) but keep nothing between requests, so a crawler updating a dataset's labels is
//! visible on the next request.

use async_graphql::{Context, Enum, Object, Result, SimpleObject, ID};
use chrono::{DateTime, Utc};
use dataloader::BatchFn;
use std::collections::HashMap;
use uuid::Uuid;

use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::{
    ComponentType, Dataset, DatasetComponent, EconomicSeries, SeriesDimensions,
};

use crate::graphql::schema::GraphQLContext;
use crate::types::DataSourceType;

/// Value type of a dataset component.
#[derive(Enum, Copy, Clone, Debug, Eq, PartialEq)]
#[graphql(name = "DatasetComponentValueType")]
pub enum DatasetComponentValueType {
    String,
    Integer,
    Decimal,
    Date,
    Boolean,
}

impl From<ComponentType> for DatasetComponentValueType {
    fn from(value: ComponentType) -> Self {
        match value {
            ComponentType::String => Self::String,
            ComponentType::Integer => Self::Integer,
            ComponentType::Decimal => Self::Decimal,
            ComponentType::Date => Self::Date,
            ComponentType::Boolean => Self::Boolean,
        }
    }
}

/// A coded value of a dimension and its label, e.g. `06` / `California`.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "DimensionCode")]
pub struct DimensionCodeType {
    pub code: String,
    pub label: String,
}

/// One dimension, measure or attribute of a dataset.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "DatasetComponent")]
pub struct DatasetComponentType {
    /// Machine name, e.g. `state`. Series dimension values are keyed by it.
    pub name: String,
    /// Human-readable label, e.g. `State`.
    pub label: String,
    /// Value type.
    #[graphql(name = "type")]
    pub value_type: DatasetComponentValueType,
    /// Unit of a measure, e.g. `Percent`.
    pub unit: Option<String>,
    /// Labels for coded values, sorted by code. Empty when the component is not coded.
    pub codes: Vec<DimensionCodeType>,
}

impl From<&DatasetComponent> for DatasetComponentType {
    fn from(component: &DatasetComponent) -> Self {
        Self {
            name: component.name.clone(),
            label: component.label.clone(),
            value_type: component.component_type.into(),
            unit: component.unit.clone(),
            codes: component
                .codes
                .iter()
                .flatten()
                .map(|(code, label)| DimensionCodeType {
                    code: code.clone(),
                    label: label.clone(),
                })
                .collect(),
        }
    }
}

/// GraphQL representation of a dataset.
#[derive(Clone, Debug)]
pub struct DatasetType(pub Dataset);

#[Object(name = "Dataset")]
impl DatasetType {
    async fn id(&self) -> ID {
        ID::from(self.0.id.to_string())
    }

    async fn source_id(&self) -> ID {
        ID::from(self.0.source_id.to_string())
    }

    /// The data source that publishes this dataset.
    async fn source(&self, ctx: &Context<'_>) -> Result<Option<DataSourceType>> {
        let loaders = &ctx.data::<GraphQLContext>()?.data_loaders;
        let source = loaders.data_source_loader.load(self.0.source_id).await;
        Ok(source.map(DataSourceType::from))
    }

    /// Code unique within the source, e.g. `BDS`.
    async fn code(&self) -> &str {
        &self.0.code
    }

    async fn name(&self) -> &str {
        &self.0.name
    }

    async fn description(&self) -> Option<&str> {
        self.0.description.as_deref()
    }

    /// Dimensions in key order. Together their values identify a series in the dataset.
    async fn dimensions(&self) -> Vec<DatasetComponentType> {
        self.0.dimensions.0.iter().map(Into::into).collect()
    }

    /// Observed values. In release 1 every dataset has the single measure `value`.
    async fn measures(&self) -> Vec<DatasetComponentType> {
        self.0.measures.0.iter().map(Into::into).collect()
    }

    /// Per-observation attributes, e.g. an observation status flag.
    async fn attributes(&self) -> Vec<DatasetComponentType> {
        self.0.attributes.0.iter().map(Into::into).collect()
    }

    /// The measure a chart plots when the user has not picked one.
    async fn default_measure(&self) -> &str {
        &self.0.default_measure
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.0.created_at
    }

    async fn updated_at(&self) -> DateTime<Utc> {
        self.0.updated_at
    }
}

/// A series' value for one dimension of its dataset, with labels resolved.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "SeriesDimension")]
pub struct SeriesDimensionType {
    /// Dimension name, e.g. `state`.
    pub name: String,
    /// Dimension label, e.g. `State`. Null when the dataset does not declare the dimension.
    pub label: Option<String>,
    /// Coded value, e.g. `06`.
    pub value: String,
    /// Label of the value, e.g. `California`. Null when the dimension has no label for it.
    pub value_label: Option<String>,
}

/// Labels a series' dimension values with its dataset's definitions.
///
/// Values come in the dataset's dimension order; keys the dataset does not declare follow in
/// key order, unlabelled.
pub fn label_dimensions(
    dimensions: &SeriesDimensions,
    dataset: Option<&Dataset>,
) -> Vec<SeriesDimensionType> {
    let mut remaining = dimensions.0.clone();
    let mut labelled = Vec::with_capacity(remaining.len());

    for component in dataset.iter().flat_map(|d| d.dimensions.0.iter()) {
        if let Some(value) = remaining.remove(&component.name) {
            let value_label = component
                .codes
                .as_ref()
                .and_then(|codes| codes.get(&value))
                .cloned();
            labelled.push(SeriesDimensionType {
                name: component.name.clone(),
                label: Some(component.label.clone()),
                value,
                value_label,
            });
        }
    }

    labelled.extend(
        remaining
            .into_iter()
            .map(|(name, value)| SeriesDimensionType {
                name,
                label: None,
                value,
                value_label: None,
            }),
    );
    labelled
}

/// The dataset columns of a series row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeriesDatasetFields {
    pub dataset_id: Option<Uuid>,
    pub dimensions: SeriesDimensions,
    /// Overrides the dataset's default measure when set.
    pub default_measure: Option<String>,
}

impl From<&EconomicSeries> for SeriesDatasetFields {
    fn from(series: &EconomicSeries) -> Self {
        Self {
            dataset_id: series.dataset_id,
            dimensions: series.dimensions.clone(),
            default_measure: series.default_measure.clone(),
        }
    }
}

/// Loads datasets by id, one query per batch. A missing dataset maps to `None`.
pub struct DatasetBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<Dataset>> for DatasetBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<Dataset>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::datasets;

            // On error return no entries: the resolver's try_load then fails with an error
            // rather than reporting the dataset as missing.
            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("dataset loader: no database connection: {e}");
                    return HashMap::new();
                }
            };
            let rows = match datasets::table
                .filter(datasets::id.eq_any(&keys))
                .select(Dataset::as_select())
                .load::<Dataset>(&mut conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!("dataset loader: query failed: {e}");
                    return HashMap::new();
                }
            };

            let found: HashMap<Uuid, Dataset> = rows.into_iter().map(|d| (d.id, d)).collect();
            keys.into_iter()
                .map(|key| (key, found.get(&key).cloned()))
                .collect()
        }
    }
}

/// Loads a series' dataset columns by series id, one query per batch. Used when a series was
/// built from a row that does not carry them (search results). A missing series maps to `None`.
pub struct SeriesDatasetFieldsBatcher {
    pub pool: DatabasePool,
}

impl BatchFn<Uuid, Option<SeriesDatasetFields>> for SeriesDatasetFieldsBatcher {
    fn load(
        &mut self,
        keys: &[Uuid],
    ) -> impl std::future::Future<Output = HashMap<Uuid, Option<SeriesDatasetFields>>> {
        let pool = self.pool.clone();
        let keys = keys.to_vec();
        async move {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::economic_series::dsl;

            let mut conn = match pool.get().await {
                Ok(conn) => conn,
                Err(e) => {
                    tracing::error!("series dataset loader: no database connection: {e}");
                    return HashMap::new();
                }
            };
            let rows = match dsl::economic_series
                .filter(dsl::id.eq_any(&keys))
                .select((
                    dsl::id,
                    dsl::dataset_id,
                    dsl::dimensions,
                    dsl::default_measure,
                ))
                .load::<(Uuid, Option<Uuid>, SeriesDimensions, Option<String>)>(&mut conn)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::error!("series dataset loader: query failed: {e}");
                    return HashMap::new();
                }
            };

            let found: HashMap<Uuid, SeriesDatasetFields> = rows
                .into_iter()
                .map(|(id, dataset_id, dimensions, default_measure)| {
                    (
                        id,
                        SeriesDatasetFields {
                            dataset_id,
                            dimensions,
                            default_measure,
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

/// Loads a dataset through the context's loader. Errors if the batch query failed.
pub async fn load_dataset(ctx: &Context<'_>, id: Uuid) -> Result<Option<Dataset>> {
    let loaders = &ctx.data::<GraphQLContext>()?.data_loaders;
    loaders
        .dataset_loader
        .try_load(id)
        .await
        .map_err(|_| async_graphql::Error::new("failed to load dataset"))
}

/// Loads a series' dataset columns through the context's loader. Errors if the batch query
/// failed.
pub async fn load_series_dataset_fields(
    ctx: &Context<'_>,
    series_id: Uuid,
) -> Result<Option<SeriesDatasetFields>> {
    let loaders = &ctx.data::<GraphQLContext>()?.data_loaders;
    loaders
        .series_dataset_fields_loader
        .try_load(series_id)
        .await
        .map_err(|_| async_graphql::Error::new("failed to load series dataset"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use econ_graph_core::models::{DatasetComponents, VALUE_MEASURE};
    use std::collections::BTreeMap;

    fn bds() -> Dataset {
        let mut state = DatasetComponent::new("state", "State", ComponentType::String);
        state.codes = Some(BTreeMap::from([
            ("06".to_string(), "California".to_string()),
            ("36".to_string(), "New York".to_string()),
        ]));
        Dataset {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            code: "BDS".to_string(),
            name: "Business Dynamics Statistics".to_string(),
            description: None,
            dimensions: DatasetComponents(vec![
                DatasetComponent::new("geo_level", "Geography level", ComponentType::String),
                state,
                DatasetComponent::new("variable", "Variable", ComponentType::String),
            ]),
            measures: DatasetComponents(vec![DatasetComponent::value_measure()]),
            attributes: DatasetComponents::default(),
            default_measure: VALUE_MEASURE.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn label_dimensions_follows_dataset_order_and_labels_codes() {
        let dims: SeriesDimensions = [
            ("variable", "ESTAB"),
            ("state", "06"),
            ("geo_level", "state"),
        ]
        .into_iter()
        .collect();

        let labelled = label_dimensions(&dims, Some(&bds()));

        let names: Vec<_> = labelled.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["geo_level", "state", "variable"]);
        assert_eq!(labelled[1].label.as_deref(), Some("State"));
        assert_eq!(labelled[1].value_label.as_deref(), Some("California"));
        // Uncoded dimension: dimension label, no value label.
        assert_eq!(labelled[2].label.as_deref(), Some("Variable"));
        assert_eq!(labelled[2].value_label, None);
    }

    #[test]
    fn label_dimensions_keeps_undeclared_keys_unlabelled() {
        let dims: SeriesDimensions = [("state", "99"), ("zzz", "x"), ("aaa", "y")]
            .into_iter()
            .collect();

        let labelled = label_dimensions(&dims, Some(&bds()));
        assert_eq!(
            labelled,
            vec![
                SeriesDimensionType {
                    name: "state".into(),
                    label: Some("State".into()),
                    value: "99".into(),
                    value_label: None,
                },
                SeriesDimensionType {
                    name: "aaa".into(),
                    label: None,
                    value: "y".into(),
                    value_label: None,
                },
                SeriesDimensionType {
                    name: "zzz".into(),
                    label: None,
                    value: "x".into(),
                    value_label: None,
                },
            ]
        );
    }

    #[test]
    fn label_dimensions_without_dataset_is_key_ordered() {
        let dims: SeriesDimensions = [("b", "2"), ("a", "1")].into_iter().collect();
        let names: Vec<_> = label_dimensions(&dims, None)
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn component_codes_are_sorted_by_code() {
        let state = &bds().dimensions.0[1];
        let component = DatasetComponentType::from(state);
        let codes: Vec<_> = component.codes.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["06", "36"]);
        assert_eq!(component.value_type, DatasetComponentValueType::String);
    }
}
