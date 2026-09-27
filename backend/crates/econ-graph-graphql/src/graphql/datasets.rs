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
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use econ_graph_core::database::DatabasePool;
use econ_graph_core::models::{
    Code, ComponentType, Dataset, DatasetComponent, SeriesDimensions, COUNTRIES_CODELIST,
    US_STATES_CODELIST,
};
use econ_graph_core::reference::{self, Area, Areas};
use econ_graph_crawler::reference::{self as crawler_reference, UsState};

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

/// A coded value of a dimension, e.g. `06` / `California`.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "DimensionCode")]
pub struct DimensionCodeType {
    /// The value as stored in series dimensions.
    pub code: String,
    pub label: String,
    /// Unit of series with this value, e.g. `current US$` for a WDI indicator.
    pub unit: Option<String>,
    pub description: Option<String>,
}

impl From<&Code> for DimensionCodeType {
    fn from(code: &Code) -> Self {
        Self {
            code: code.code.clone(),
            label: code.label.clone(),
            unit: code.unit.clone(),
            description: code.description.clone(),
        }
    }
}

impl From<&UsState> for DimensionCodeType {
    fn from(state: &UsState) -> Self {
        Self {
            code: state.fips.clone(),
            label: state.name.clone(),
            unit: None,
            description: None,
        }
    }
}

impl From<&Area> for DimensionCodeType {
    fn from(area: &Area) -> Self {
        Self {
            code: area.key.clone(),
            label: area.name.clone(),
            unit: None,
            description: None,
        }
    }
}

/// Where a component's codes come from: its inline list or a shared code list.
enum CodeSource<'a> {
    Uncoded,
    Inline(&'a [Code]),
    Areas(&'static Areas),
    UsStates(&'static [UsState]),
}

impl<'a> CodeSource<'a> {
    /// Resolves a component's codes. Errors on an unknown code list name or an unreadable
    /// reference file.
    fn of(component: &'a DatasetComponent) -> Result<Self> {
        match (&component.codes, &component.codelist) {
            (Some(codes), _) => Ok(Self::Inline(codes)),
            (None, Some(name)) if name == COUNTRIES_CODELIST => {
                let areas = reference::areas().map_err(|e| {
                    async_graphql::Error::new(format!("code list {name:?} unavailable: {e}"))
                })?;
                Ok(Self::Areas(areas))
            }
            (None, Some(name)) if name == US_STATES_CODELIST => {
                let states = crawler_reference::us_states().map_err(|e| {
                    async_graphql::Error::new(format!("code list {name:?} unavailable: {e}"))
                })?;
                Ok(Self::UsStates(states))
            }
            (None, Some(name)) => Err(async_graphql::Error::new(format!(
                "dataset component {:?} uses unknown code list {name:?}",
                component.name
            ))),
            (None, None) => Ok(Self::Uncoded),
        }
    }

    /// Like [`Self::of`], but treats the component as uncoded on failure. Each distinct failure
    /// is logged once per process, not once per series in every response.
    fn of_or_uncoded(component: &'a DatasetComponent) -> Self {
        static LOGGED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        Self::of(component).unwrap_or_else(|e| {
            let message = format!("dataset component {:?}: {}", component.name, e.message);
            let mut logged = LOGGED
                .get_or_init(Default::default)
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if logged.insert(message.clone()) {
                tracing::warn!("{message}; treating it as uncoded");
            }
            Self::Uncoded
        })
    }

    /// Every code, in list order.
    fn all(&self) -> Vec<DimensionCodeType> {
        match self {
            Self::Uncoded => Vec::new(),
            Self::Inline(codes) => codes.iter().map(Into::into).collect(),
            Self::Areas(areas) => areas.all().iter().map(Into::into).collect(),
            Self::UsStates(states) => states.iter().map(Into::into).collect(),
        }
    }

    /// The code equal to `value`, if listed.
    fn get(&self, value: &str) -> Option<DimensionCodeType> {
        match self {
            Self::Uncoded => None,
            Self::Inline(codes) => codes.iter().find(|c| c.code == value).map(Into::into),
            Self::Areas(areas) => areas
                .by_key(value)
                .filter(|area| area.key == value)
                .map(Into::into),
            Self::UsStates(states) => states.iter().find(|s| s.fips == value).map(Into::into),
        }
    }
}

/// One dimension, measure or attribute of a dataset.
#[derive(Clone, Debug)]
pub struct DatasetComponentType(pub DatasetComponent);

#[Object(name = "DatasetComponent")]
impl DatasetComponentType {
    /// Machine name, e.g. `state`. Series dimension values are keyed by it.
    async fn name(&self) -> &str {
        &self.0.name
    }

    /// Human-readable label, e.g. `State`.
    async fn label(&self) -> &str {
        &self.0.label
    }

    /// Value type.
    #[graphql(name = "type")]
    async fn value_type(&self) -> DatasetComponentValueType {
        self.0.component_type.into()
    }

    /// Unit of a measure, e.g. `Percent`.
    async fn unit(&self) -> Option<&str> {
        self.0.unit.as_deref()
    }

    /// The values this component takes, with labels. Inline codes come in declared order and
    /// shared code lists (such as `countries`) in file order. Empty when the component is not
    /// coded.
    ///
    /// A code list that cannot be resolved (an unknown name, or an unreadable reference file) is
    /// logged and yields an empty list, so one misconfigured dataset doesn't fail the query. A
    /// client that cares about the difference checks `codelist`: non-null with empty `codes` is
    /// a degraded code list, not a component with none.
    async fn codes(&self) -> Vec<DimensionCodeType> {
        CodeSource::of_or_uncoded(&self.0).all()
    }

    /// Name of the shared code list the codes come from, e.g. `countries`. Null for inline or
    /// no codes. Clients use it to recognise a dimension, e.g. the map's country dimension.
    async fn codelist(&self) -> Option<&str> {
        self.0.codelist.as_deref()
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
        let source = loaders
            .dataset_source_loader
            .try_load(self.0.source_id)
            .await
            .map_err(|_| async_graphql::Error::new("failed to load data source"))?;
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
        components(&self.0.dimensions.0)
    }

    /// Observed values. In train 1 every dataset has the single measure `value`.
    async fn measures(&self) -> Vec<DatasetComponentType> {
        components(&self.0.measures.0)
    }

    /// Per-observation attributes, e.g. an observation status flag.
    async fn attributes(&self) -> Vec<DatasetComponentType> {
        components(&self.0.attributes.0)
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

fn components(components: &[DatasetComponent]) -> Vec<DatasetComponentType> {
    components
        .iter()
        .cloned()
        .map(DatasetComponentType)
        .collect()
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
    /// Unit of series with this value, e.g. `current US$` for a WDI indicator. Null when the
    /// code carries none; the series' own `units` then applies.
    pub value_unit: Option<String>,
}

/// Labels a series' dimension values with its dataset's definitions.
///
/// Values come in the dataset's dimension order; keys the dataset does not declare follow in
/// key order, unlabelled. A dimension whose code list cannot be resolved is logged and its
/// value left unlabelled.
pub fn label_dimensions(
    dimensions: &SeriesDimensions,
    dataset: Option<&Dataset>,
) -> Vec<SeriesDimensionType> {
    let mut remaining = dimensions.0.clone();
    let mut labelled = Vec::with_capacity(remaining.len());

    for component in dataset.iter().flat_map(|d| d.dimensions.0.iter()) {
        if let Some(value) = remaining.remove(&component.name) {
            let code = CodeSource::of_or_uncoded(component).get(&value);
            labelled.push(SeriesDimensionType {
                name: component.name.clone(),
                label: Some(component.label.clone()),
                value,
                value_label: code.as_ref().map(|c| c.label.clone()),
                value_unit: code.and_then(|c| c.unit),
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
                value_unit: None,
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
    use dataloader::non_cached::Loader as NonCachedLoader;
    use econ_graph_core::models::{DatasetComponents, VALUE_MEASURE};

    /// A batcher that finds nothing, however many keys it's asked for. Stands in for a batch
    /// query that failed: both loader batchers return an empty map on a connection or query
    /// error (see [`DatasetBatcher::load`] and [`SeriesDatasetFieldsBatcher::load`]).
    struct FindsNothing;

    impl<K, V> BatchFn<K, V> for FindsNothing {
        async fn load(&mut self, _keys: &[K]) -> HashMap<K, V> {
            HashMap::new()
        }
    }

    #[tokio::test]
    async fn load_dataset_and_load_series_dataset_fields_error_on_a_failed_batch() {
        // `load_dataset`/`load_series_dataset_fields` need a `Context`, which needs a running
        // schema; exercised end-to-end in datasets_db_tests.rs. What they rely on is that the
        // dataloader crate's `try_load` errs when a key is missing from the batch result
        // altogether (as opposed to being present and mapped to `None`), which is what a failed
        // batch looks like for both batchers here. That's what this checks directly.
        let dataset_loader: NonCachedLoader<Uuid, Option<Dataset>, _> =
            NonCachedLoader::new(FindsNothing);
        assert!(dataset_loader.try_load(Uuid::new_v4()).await.is_err());

        let fields_loader: NonCachedLoader<Uuid, Option<SeriesDatasetFields>, _> =
            NonCachedLoader::new(FindsNothing);
        assert!(fields_loader.try_load(Uuid::new_v4()).await.is_err());
    }

    fn dataset(dimensions: Vec<DatasetComponent>) -> Dataset {
        Dataset {
            id: Uuid::new_v4(),
            source_id: Uuid::new_v4(),
            code: "TEST".to_string(),
            name: "Test dataset".to_string(),
            description: None,
            dimensions: DatasetComponents(dimensions),
            measures: DatasetComponents(vec![DatasetComponent::value_measure()]),
            attributes: DatasetComponents::default(),
            default_measure: VALUE_MEASURE.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn bds() -> Dataset {
        let mut state = DatasetComponent::new("state", "State", ComponentType::String);
        state.codes = Some(vec![
            Code::new("36", "New York"),
            Code::new("06", "California"),
        ]);
        dataset(vec![
            DatasetComponent::new("geo_level", "Geography level", ComponentType::String),
            state,
            DatasetComponent::new("variable", "Variable", ComponentType::String),
        ])
    }

    fn wdi() -> Dataset {
        let mut indicator = DatasetComponent::new("indicator", "Indicator", ComponentType::String);
        let mut gdp = Code::new("NY.GDP.PCAP.CD", "GDP per capita");
        gdp.unit = Some("current US$".to_string());
        indicator.codes = Some(vec![gdp]);
        let mut area = DatasetComponent::new("area", "Country or area", ComponentType::String);
        area.codelist = Some(COUNTRIES_CODELIST.to_string());
        dataset(vec![indicator, area])
    }

    fn dims(pairs: &[(&str, &str)]) -> SeriesDimensions {
        pairs.iter().copied().collect()
    }

    fn unlabelled(name: &str, value: &str) -> SeriesDimensionType {
        SeriesDimensionType {
            name: name.into(),
            label: None,
            value: value.into(),
            value_label: None,
            value_unit: None,
        }
    }

    #[test]
    fn label_dimensions_follows_dataset_order_and_labels_codes() {
        let series = dims(&[
            ("variable", "ESTAB"),
            ("state", "06"),
            ("geo_level", "state"),
        ]);

        let labelled = label_dimensions(&series, Some(&bds()));

        let names: Vec<_> = labelled.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["geo_level", "state", "variable"]);
        assert_eq!(labelled[1].label.as_deref(), Some("State"));
        assert_eq!(labelled[1].value_label.as_deref(), Some("California"));
        // Uncoded dimension: dimension label, no value label.
        assert_eq!(labelled[2].label.as_deref(), Some("Variable"));
        assert_eq!(labelled[2].value_label, None);
    }

    #[test]
    fn label_dimensions_keeps_unknown_codes_and_undeclared_keys_unlabelled() {
        let series = dims(&[("state", "99"), ("zzz", "x"), ("aaa", "y")]);

        let labelled = label_dimensions(&series, Some(&bds()));
        assert_eq!(
            labelled,
            vec![
                SeriesDimensionType {
                    label: Some("State".into()),
                    ..unlabelled("state", "99")
                },
                unlabelled("aaa", "y"),
                unlabelled("zzz", "x"),
            ]
        );
    }

    #[test]
    fn label_dimensions_without_dataset_is_key_ordered() {
        let series = dims(&[("b", "2"), ("a", "1")]);
        assert_eq!(
            label_dimensions(&series, None),
            vec![unlabelled("a", "1"), unlabelled("b", "2")]
        );
    }

    #[test]
    fn label_dimensions_resolves_countries_code_list_and_code_units() {
        let series = dims(&[("area", "DEU"), ("indicator", "NY.GDP.PCAP.CD")]);

        let labelled = label_dimensions(&series, Some(&wdi()));

        assert_eq!(labelled[0].value_label.as_deref(), Some("GDP per capita"));
        assert_eq!(labelled[0].value_unit.as_deref(), Some("current US$"));
        assert_eq!(labelled[1].name, "area");
        assert_eq!(labelled[1].value_label.as_deref(), Some("Germany"));
        assert_eq!(labelled[1].value_unit, None);

        // Keys match exactly, as stored.
        let lower = dims(&[("area", "deu")]);
        let labelled = label_dimensions(&lower, Some(&wdi()));
        assert_eq!(labelled[0].value_label, None);
    }

    #[test]
    fn us_states_code_list_labels_fips_codes() {
        let mut state = DatasetComponent::new("state", "State", ComponentType::String);
        state.codelist = Some(US_STATES_CODELIST.to_string());
        let source = CodeSource::of(&state).unwrap();
        assert_eq!(source.all().len(), 51);
        assert_eq!(
            source.get("06").map(|c| c.label),
            Some("California".to_string())
        );
        assert_eq!(source.get("6"), None);
    }

    #[test]
    fn inline_codes_keep_declared_order() {
        let state = &bds().dimensions.0[1];
        let codes: Vec<_> = CodeSource::of(state).unwrap().all();
        let codes: Vec<_> = codes.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["36", "06"]);
    }

    #[test]
    fn countries_code_list_lists_countries_and_aggregates() {
        let area = &wdi().dimensions.0[1];
        let codes = CodeSource::of(area).unwrap().all();
        let germany = codes.iter().find(|c| c.code == "DEU").expect("DEU listed");
        assert_eq!(germany.label, "Germany");
        assert!(codes.iter().any(|c| c.code == "WLD"), "aggregates listed");
    }

    #[test]
    fn unknown_code_list_degrades_to_uncoded() {
        let mut area = DatasetComponent::new("area", "Area", ComponentType::String);
        area.codelist = Some("planets".to_string());
        let err = CodeSource::of(&area)
            .err()
            .expect("unknown code list rejected");
        assert!(err.message.contains("planets"), "{}", err.message);

        // Resolvers degrade instead: no codes, and values left unlabelled.
        assert!(CodeSource::of_or_uncoded(&area).all().is_empty());
        let series = dims(&[("area", "EARTH")]);
        assert_eq!(
            label_dimensions(&series, Some(&dataset(vec![area]))),
            vec![SeriesDimensionType {
                label: Some("Area".into()),
                ..unlabelled("area", "EARTH")
            }]
        );
    }
}
