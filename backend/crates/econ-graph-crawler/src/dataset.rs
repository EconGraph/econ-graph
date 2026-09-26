// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Datasets: groups of series that share a schema (SDMX-style dimensions, measures and
//! attributes), and the contract adapters follow when they write series into one.
//!
//! - Definitions are reference data. They live in `datasets/<source>.toml` under the reference
//!   data directory ([`crate::reference::datasets`]), not in code.
//! - Each adapter declares the dataset codes it writes ([`SourceAdapter::datasets`]).
//! - [`DatasetCatalog::load`] reads the definitions for every registered adapter at startup and
//!   rejects a declared code without a definition, or a definition no adapter declares.
//!   [`crate::persist::sync_datasets`] then upserts them into the `datasets` table.
//! - Every discovered or fetched series names its dataset and dimension values
//!   ([`SeriesDataset`]). [`DatasetCatalog::check`] rejects a dataset the adapter did not declare,
//!   or dimension keys that differ from the definition, before anything is written.
//!
//! Train 1 stores every dataset long: `data_points` holds one value, so a source with several
//! measures publishes each measure as its own series with the measure as a dimension, and every
//! dataset declares the single measure [`DEFAULT_MEASURE`].

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::adapter::{AdapterRegistry, SourceAdapter};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The one measure every train 1 dataset declares, and so its `default_measure`.
pub const DEFAULT_MEASURE: &str = "value";

/// Which dataset a series belongs to, and its value for each of the dataset's dimensions.
///
/// A dimensionless dataset (one series per source id, e.g. FRED) has an empty `dimensions` map.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeriesDataset {
    /// Dataset code, unique within the source (e.g. `wdi`).
    pub code: String,
    /// Dimension name to value (e.g. `indicator` -> `NY.GDP.PCAP.CD`, `area` -> `USA`).
    pub dimensions: BTreeMap<String, String>,
}

impl SeriesDataset {
    /// A series in dataset `code` with the given dimension values.
    pub fn new<K, V>(code: impl Into<String>, dimensions: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            code: code.into(),
            dimensions: dimensions
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// The dimension values as the JSON object stored in `dimensions`.
    pub fn dimensions_json(&self) -> serde_json::Value {
        serde_json::Value::Object(
            self.dimensions
                .iter()
                .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                .collect(),
        )
    }
}

/// One dimension, measure or attribute of a dataset (an SDMX component).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Name used as the key in a series' `dimensions` (e.g. `area`).
    pub name: String,
    /// Human-readable label (e.g. `Country or area`).
    pub label: String,
    /// Value type, e.g. `code`, `string` or `number`. Informational.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    /// Unit of measure, for measures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Labels for known code values (code -> label). Not a closed list: values outside it are
    /// accepted.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub codes: BTreeMap<String, String>,
}

/// A dataset definition, as written in `datasets/<source>.toml` and stored in `datasets`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetDef {
    /// Code, unique within the source (e.g. `wdi`). ASCII letters, digits, `_` and `-`.
    pub code: String,
    /// Human-readable name.
    pub name: String,
    /// Longer description.
    #[serde(default)]
    pub description: Option<String>,
    /// Dimensions, in the order the canonical external id lists their values.
    #[serde(default)]
    pub dimensions: Vec<Component>,
    /// Measures. Train 1 allows exactly one, named [`DEFAULT_MEASURE`]; omitted means that one.
    #[serde(default = "default_measures")]
    pub measures: Vec<Component>,
    /// Attributes (observation or series flags).
    #[serde(default)]
    pub attributes: Vec<Component>,
    /// The measure a series of this dataset stores. Omitted means [`DEFAULT_MEASURE`].
    #[serde(default = "default_measure")]
    pub default_measure: String,
}

fn default_measures() -> Vec<Component> {
    vec![Component {
        name: DEFAULT_MEASURE.into(),
        label: "Value".into(),
        value_type: Some("number".into()),
        unit: None,
        codes: BTreeMap::new(),
    }]
}

fn default_measure() -> String {
    DEFAULT_MEASURE.into()
}

/// The shape of a `datasets/<source>.toml` file: one `[[dataset]]` table per dataset.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DatasetFile {
    #[serde(default)]
    dataset: Vec<DatasetDef>,
}

fn is_code(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_component_name(s: &str) -> bool {
    s.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl DatasetDef {
    /// Dimension names in declared order.
    pub fn dimension_names(&self) -> impl Iterator<Item = &str> {
        self.dimensions.iter().map(|d| d.name.as_str())
    }

    /// Checks the definition on its own: a valid code, lowercase `snake_case` component names that
    /// are unique across dimensions, measures and attributes, and train 1's single
    /// [`DEFAULT_MEASURE`].
    pub fn validate(&self) -> Result<(), String> {
        let code = &self.code;
        if !is_code(code) {
            return Err(format!(
                "dataset code {code:?} must be ASCII letters, digits, '_' or '-'"
            ));
        }
        if self.name.trim().is_empty() {
            return Err(format!("dataset {code}: empty name"));
        }
        let mut seen = HashSet::new();
        for c in self
            .dimensions
            .iter()
            .chain(&self.measures)
            .chain(&self.attributes)
        {
            if !is_component_name(&c.name) {
                return Err(format!(
                    "dataset {code}: component name {:?} must be lowercase snake_case",
                    c.name
                ));
            }
            if c.label.trim().is_empty() {
                return Err(format!(
                    "dataset {code}: component {} has an empty label",
                    c.name
                ));
            }
            if !seen.insert(c.name.as_str()) {
                return Err(format!(
                    "dataset {code}: component {} is declared twice",
                    c.name
                ));
            }
        }
        // Train 1: data_points holds one value, so datasets are stored long (see module docs).
        match self.measures.as_slice() {
            [m] if m.name == DEFAULT_MEASURE && self.default_measure == DEFAULT_MEASURE => Ok(()),
            _ => Err(format!(
                "dataset {code}: train 1 stores one measure named {DEFAULT_MEASURE:?} (with \
                 default_measure {DEFAULT_MEASURE:?}); publish other measures as a dimension"
            )),
        }
    }

    /// Checks that `dimensions` has exactly this dataset's dimension keys.
    pub fn check_dimensions(&self, dimensions: &BTreeMap<String, String>) -> Result<(), String> {
        let expected: BTreeSet<&str> = self.dimension_names().collect();
        let got: BTreeSet<&str> = dimensions.keys().map(String::as_str).collect();
        if expected == got {
            return Ok(());
        }
        let missing: Vec<_> = expected.difference(&got).collect();
        let unexpected: Vec<_> = got.difference(&expected).collect();
        Err(format!(
            "dataset {}: wrong dimension keys (missing {missing:?}, unexpected {unexpected:?}; \
             declared {:?})",
            self.code,
            self.dimension_names().collect::<Vec<_>>()
        ))
    }

    /// The canonical external id of the series with these dimension values:
    /// `{code}/{v1}.{v2}...`, values in declared dimension order (the SDMX series key).
    ///
    /// Adapters of dimensioned datasets use this rather than formatting ids by hand. Fails if the
    /// keys differ from the definition, if the dataset has no dimensions (its series keep the
    /// source's own ids), or if a value contains `/`. An empty value is allowed (e.g. no state for
    /// a national series). Values may contain `.` (WDI indicator codes do), so an id is not parsed
    /// back into values; the stored `dimensions` are the source of truth.
    pub fn external_id(&self, dimensions: &BTreeMap<String, String>) -> Result<String, String> {
        self.check_dimensions(dimensions)?;
        let values: Vec<&str> = self
            .dimension_names()
            .map(|name| dimensions[name].as_str())
            .collect();
        canonical_external_id(&self.code, &values)
    }

    /// [`external_id`](Self::external_id) and the matching [`SeriesDataset`], for adapters that
    /// build both from the same values.
    pub fn series<K, V>(
        &self,
        dimensions: impl IntoIterator<Item = (K, V)>,
    ) -> Result<(String, SeriesDataset), String>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let dataset = SeriesDataset::new(self.code.clone(), dimensions);
        let id = self.external_id(&dataset.dimensions)?;
        Ok((id, dataset))
    }
}

/// `{code}/{v1}.{v2}...` from values already in declared dimension order. Prefer
/// [`DatasetDef::external_id`], which also checks the keys.
pub fn canonical_external_id(code: &str, values: &[&str]) -> Result<String, String> {
    if !is_code(code) {
        return Err(format!(
            "dataset code {code:?} must be ASCII letters, digits, '_' or '-'"
        ));
    }
    if values.is_empty() {
        return Err(format!(
            "dataset {code} has no dimensions; its series keep the source's own ids"
        ));
    }
    if let Some(v) = values.iter().find(|v| v.contains('/')) {
        return Err(format!(
            "dataset {code}: dimension value {v:?} contains '/'"
        ));
    }
    Ok(format!("{code}/{}", values.join(".")))
}

/// Parses a `datasets/<source>.toml` file and validates each definition. Codes must be unique.
pub fn parse_dataset_file(text: &str) -> Result<Vec<DatasetDef>, String> {
    let file: DatasetFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut codes = HashSet::new();
    for def in &file.dataset {
        def.validate()?;
        if !codes.insert(def.code.as_str()) {
            return Err(format!("dataset {} is defined twice", def.code));
        }
    }
    Ok(file.dataset)
}

/// Every registered adapter's dataset definitions, checked against its declarations.
#[derive(Debug, Clone, Default)]
pub struct DatasetCatalog {
    sources: HashMap<SourceId, BTreeMap<String, DatasetDef>>,
}

impl DatasetCatalog {
    /// A catalog with no datasets: every series that names a dataset is rejected.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads the definitions for each adapter in `registry` from the reference data directory.
    /// See [`insert`](Self::insert) for the checks.
    pub fn load(registry: &AdapterRegistry) -> Result<Self, CrawlError> {
        let mut catalog = Self::empty();
        for id in registry.ids() {
            if let Some(adapter) = registry.get(id) {
                catalog.load_adapter(&*adapter)?;
            }
        }
        Ok(catalog)
    }

    /// Loads one adapter's definitions from the reference data directory (nothing is read when it
    /// declares no datasets).
    pub fn load_adapter(&mut self, adapter: &dyn SourceAdapter) -> Result<(), CrawlError> {
        let declared = adapter.datasets();
        if declared.is_empty() {
            return Ok(());
        }
        let defs = crate::reference::datasets(adapter.id())?;
        self.insert(adapter.id(), &declared, defs)
            .map_err(CrawlError::Permanent)
    }

    /// Adds `source`'s definitions. Every code in `declared` must have exactly one definition, and
    /// every definition must be declared, so the file and the adapter cannot drift apart.
    pub fn insert(
        &mut self,
        source: SourceId,
        declared: &[&str],
        defs: Vec<DatasetDef>,
    ) -> Result<(), String> {
        let mut by_code = BTreeMap::new();
        for def in defs {
            def.validate().map_err(|e| format!("{source}: {e}"))?;
            let code = def.code.clone();
            if by_code.insert(code.clone(), def).is_some() {
                return Err(format!("{source}: dataset {code} is defined twice"));
            }
        }
        let declared_set: BTreeSet<&str> = declared.iter().copied().collect();
        if declared_set.len() != declared.len() {
            return Err(format!(
                "{source}: adapter declares a dataset twice: {declared:?}"
            ));
        }
        if let Some(code) = declared_set.iter().find(|c| !by_code.contains_key(**c)) {
            return Err(format!(
                "{source}: adapter declares dataset {code}, which has no definition in {}",
                crate::reference::datasets_file(source).display()
            ));
        }
        if let Some(code) = by_code.keys().find(|c| !declared_set.contains(c.as_str())) {
            return Err(format!(
                "{source}: dataset {code} is defined but the adapter does not declare it"
            ));
        }
        self.sources.insert(source, by_code);
        Ok(())
    }

    /// The definition of `source`'s dataset `code`, if declared.
    pub fn get(&self, source: SourceId, code: &str) -> Option<&DatasetDef> {
        self.sources.get(&source)?.get(code)
    }

    /// Every `(source, definition)` in the catalog, sources and codes sorted.
    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &DatasetDef)> {
        let mut sources: Vec<_> = self.sources.iter().collect();
        sources.sort_unstable_by_key(|(s, _)| **s);
        sources
            .into_iter()
            .flat_map(|(s, defs)| defs.values().map(move |d| (*s, d)))
    }

    /// Number of datasets in the catalog.
    pub fn len(&self) -> usize {
        self.sources.values().map(BTreeMap::len).sum()
    }

    /// Whether the catalog holds no datasets.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks one series an adapter emitted. `None` (no dataset) passes, for now: the dataset
    /// becomes required once every adapter declares one. A dataset the adapter did not declare, or
    /// dimension keys other than the definition's, is a `Permanent` error naming the series.
    pub fn check(
        &self,
        source: SourceId,
        external_id: &str,
        dataset: Option<&SeriesDataset>,
    ) -> Result<Option<&DatasetDef>, CrawlError> {
        let Some(dataset) = dataset else {
            return Ok(None);
        };
        let def = self.get(source, &dataset.code).ok_or_else(|| {
            CrawlError::Permanent(format!(
                "{source} series {external_id}: dataset {} is not declared by the adapter",
                dataset.code
            ))
        })?;
        def.check_dimensions(&dataset.dimensions)
            .map_err(|e| CrawlError::Permanent(format!("{source} series {external_id}: {e}")))?;
        Ok(Some(def))
    }

    /// [`check`](Self::check) for a batch of series, which also rejects two series with the same
    /// dataset and dimension values (the database allows one series per key). Series of a
    /// dimensionless dataset are keyed by their external id alone, so they are not compared.
    pub fn check_all<'a>(
        &self,
        source: SourceId,
        series: impl IntoIterator<Item = (&'a str, Option<&'a SeriesDataset>)>,
    ) -> Result<(), CrawlError> {
        let mut keys: HashMap<(&str, &BTreeMap<String, String>), &str> = HashMap::new();
        for (external_id, dataset) in series {
            if self.check(source, external_id, dataset)?.is_none() {
                continue;
            }
            let dataset = dataset.expect("check returned a definition");
            if dataset.dimensions.is_empty() {
                continue;
            }
            if let Some(other) =
                keys.insert((dataset.code.as_str(), &dataset.dimensions), external_id)
            {
                if other != external_id {
                    return Err(CrawlError::Permanent(format!(
                        "{source}: series {other} and {external_id} have the same dataset {} \
                         and dimensions {:?}",
                        dataset.code, dataset.dimensions
                    )));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WDI: &str = r#"
[[dataset]]
code = "wdi"
name = "World Development Indicators"
description = "World Bank indicators by country"

[[dataset.dimensions]]
name = "indicator"
label = "Indicator"
type = "code"

[[dataset.dimensions]]
name = "area"
label = "Country or area"
type = "code"
codes = { USA = "United States" }

[[dataset.attributes]]
name = "obs_status"
label = "Observation status"
"#;

    fn wdi() -> DatasetDef {
        parse_dataset_file(WDI).unwrap().remove(0)
    }

    fn dims(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parses_a_dataset_file_with_default_measure() {
        let d = wdi();
        assert_eq!(d.code, "wdi");
        assert_eq!(
            d.dimension_names().collect::<Vec<_>>(),
            ["indicator", "area"]
        );
        assert_eq!(d.measures.len(), 1);
        assert_eq!(d.measures[0].name, DEFAULT_MEASURE);
        assert_eq!(d.default_measure, DEFAULT_MEASURE);
        assert_eq!(d.dimensions[1].codes["USA"], "United States");
        assert_eq!(d.attributes[0].name, "obs_status");
    }

    #[test]
    fn empty_file_has_no_datasets() {
        assert!(parse_dataset_file("").unwrap().is_empty());
    }

    #[test]
    fn rejects_invalid_definitions() {
        let base = "[[dataset]]\ncode = \"x\"\nname = \"X\"\n";
        for (text, needle) in [
            (
                "[[dataset]]\ncode = \"a/b\"\nname = \"X\"\n",
                "dataset code",
            ),
            ("[[dataset]]\ncode = \"x\"\nname = \" \"\n", "empty name"),
            (
                &format!("{base}[[dataset.dimensions]]\nname = \"Area\"\nlabel = \"A\"\n"),
                "snake_case",
            ),
            (
                &format!("{base}[[dataset.dimensions]]\nname = \"area\"\nlabel = \"\"\n"),
                "empty label",
            ),
            (
                &format!(
                    "{base}[[dataset.dimensions]]\nname = \"area\"\nlabel = \"A\"\n\
                     [[dataset.attributes]]\nname = \"area\"\nlabel = \"A\"\n"
                ),
                "declared twice",
            ),
            (
                &format!("{base}[[dataset.measures]]\nname = \"level\"\nlabel = \"L\"\n"),
                "train 1",
            ),
            (&format!("{base}default_measure = \"level\"\n"), "train 1"),
            (&format!("{base}{base}"), "defined twice"),
            (&format!("{base}colour = \"red\"\n"), "unknown field"),
        ] {
            let e = parse_dataset_file(text).unwrap_err();
            assert!(e.contains(needle), "{text:?}: {e}");
        }
    }

    #[test]
    fn canonical_external_id_uses_declared_order() {
        let d = wdi();
        // BTreeMap order is alphabetical (area first); the id follows declared order.
        let id = d
            .external_id(&dims(&[("area", "USA"), ("indicator", "NY.GDP.PCAP.CD")]))
            .unwrap();
        assert_eq!(id, "wdi/NY.GDP.PCAP.CD.USA");
        let (id2, ds) = d
            .series([("indicator", "SP.POP.TOTL"), ("area", "FRA")])
            .unwrap();
        assert_eq!(id2, "wdi/SP.POP.TOTL.FRA");
        assert_eq!(ds.code, "wdi");
        assert_eq!(
            ds.dimensions_json(),
            serde_json::json!({"indicator": "SP.POP.TOTL", "area": "FRA"})
        );
    }

    #[test]
    fn canonical_external_id_rejects_bad_input() {
        let d = wdi();
        assert!(d
            .external_id(&dims(&[("indicator", "X")]))
            .unwrap_err()
            .contains("missing [\"area\"]"));
        assert!(d
            .external_id(&dims(&[("indicator", "X"), ("area", "a/b")]))
            .unwrap_err()
            .contains("'/'"));
        assert!(canonical_external_id("FRED", &[])
            .unwrap_err()
            .contains("no dimensions"));
        assert!(canonical_external_id("a/b", &["x"])
            .unwrap_err()
            .contains("dataset code"));
        // Empty values are allowed (e.g. no state for a national series).
        assert_eq!(
            canonical_external_id("bds", &["national", "", "ESTAB"]).unwrap(),
            "bds/national..ESTAB"
        );
    }

    fn catalog() -> DatasetCatalog {
        let mut c = DatasetCatalog::empty();
        c.insert(
            SourceId::WorldBank,
            &["wdi"],
            parse_dataset_file(WDI).unwrap(),
        )
        .unwrap();
        c
    }

    #[test]
    fn insert_requires_declarations_and_definitions_to_match() {
        let defs = || parse_dataset_file(WDI).unwrap();
        let mut c = DatasetCatalog::empty();
        let e = c
            .insert(SourceId::WorldBank, &["wdi", "ids"], defs())
            .unwrap_err();
        assert!(
            e.contains("declares dataset ids") && e.contains("world_bank.toml"),
            "{e}"
        );
        let e = c.insert(SourceId::WorldBank, &[], defs()).unwrap_err();
        assert!(e.contains("does not declare it"), "{e}");
        let e = c
            .insert(SourceId::WorldBank, &["wdi", "wdi"], defs())
            .unwrap_err();
        assert!(e.contains("declares a dataset twice"), "{e}");
        assert!(c.is_empty());
        c.insert(SourceId::WorldBank, &["wdi"], defs()).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(
            c.iter()
                .map(|(s, d)| (s, d.code.as_str()))
                .collect::<Vec<_>>(),
            [(SourceId::WorldBank, "wdi")]
        );
    }

    #[test]
    fn check_accepts_declared_dataset_and_no_dataset() {
        let c = catalog();
        let ds = SeriesDataset::new("wdi", [("indicator", "X"), ("area", "USA")]);
        assert_eq!(
            c.check(SourceId::WorldBank, "wdi/X.USA", Some(&ds))
                .unwrap()
                .unwrap()
                .code,
            "wdi"
        );
        assert!(c
            .check(SourceId::WorldBank, "legacy", None)
            .unwrap()
            .is_none());
    }

    #[test]
    fn check_rejects_undeclared_dataset_and_wrong_keys() {
        let c = catalog();
        let other = SeriesDataset::new("ids", [("indicator", "X")]);
        let e = c
            .check(SourceId::WorldBank, "s1", Some(&other))
            .unwrap_err();
        assert_eq!(e.kind(), "permanent");
        assert!(e.to_string().contains("dataset ids is not declared"), "{e}");
        // Right code, wrong source.
        let ds = SeriesDataset::new("wdi", [("indicator", "X"), ("area", "USA")]);
        assert!(c.check(SourceId::Imf, "s1", Some(&ds)).is_err());
        let wrong = SeriesDataset::new("wdi", [("indicator", "X"), ("country", "USA")]);
        let e = c
            .check(SourceId::WorldBank, "s2", Some(&wrong))
            .unwrap_err();
        assert!(
            e.to_string()
                .contains("missing [\"area\"], unexpected [\"country\"]"),
            "{e}"
        );
    }

    #[test]
    fn check_all_rejects_duplicate_keys() {
        let c = catalog();
        let a = SeriesDataset::new("wdi", [("indicator", "X"), ("area", "USA")]);
        let b = a.clone();
        c.check_all(
            SourceId::WorldBank,
            [("a", Some(&a)), ("a", Some(&a)), ("n", None)],
        )
        .unwrap();
        let e = c
            .check_all(SourceId::WorldBank, [("a", Some(&a)), ("b", Some(&b))])
            .unwrap_err();
        assert!(e.to_string().contains("same dataset wdi"), "{e}");
    }

    /// A dimensionless dataset (e.g. FRED) holds many series, keyed by external id.
    #[test]
    fn check_all_allows_many_series_in_a_dimensionless_dataset() {
        let mut c = DatasetCatalog::empty();
        c.insert(
            SourceId::Fred,
            &["FRED"],
            parse_dataset_file("[[dataset]]\ncode = \"FRED\"\nname = \"FRED\"\n").unwrap(),
        )
        .unwrap();
        let flat = SeriesDataset::new("FRED", Vec::<(String, String)>::new());
        c.check_all(
            SourceId::Fred,
            [("GDP", Some(&flat)), ("UNRATE", Some(&flat))],
        )
        .unwrap();
    }
}
