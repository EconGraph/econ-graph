// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! BEA (Bureau of Economic Analysis) adapter: curated NIPA and Regional tables, one series per
//! table line, with BEA's own table names, line numbers and area codes as ids.
//!
//! # Tables
//!
//! The tables are reference data in `bea_tables.csv` ([`TABLES_FILE`], in the
//! [reference data directory](crate::reference::data_dir)): BEA dataset (`NIPA` or `Regional`),
//! `TableName`, the frequencies crawled and a short title. See [`tables`].
//!
//! # Datasets and ids
//!
//! Two datasets, defined in `datasets/bea.toml` (see [`crate::dataset`]):
//!
//! - [`NIPA_DATASET`] (`bea_nipa`), dimensions `table_name`, `line_number` and `frequency` (BEA's
//!   `A`, `Q` or `M`): `bea_nipa/T10105.1.Q` is quarterly GDP in current dollars.
//! - [`REGIONAL_DATASET`] (`bea_regional`), dimensions `table_name`, `line_code` and `geo_fips`
//!   (BEA's five-digit area code: `00000` for the United States, `SS000` for a state, `9R000` for
//!   a BEA region): `bea_regional/SAGDP2N.1.06000` is California's GDP.
//!
//! Every value is BEA's own code: the table and frequency (NIPA) or the table, line code and area
//! (Regional) are `GetData` parameters, and a NIPA line number is the `LineNumber` of its rows.
//!
//! # Discovery
//!
//! - **NIPA.** The NIPA dataset has no line parameter (its parameters are `TableName`,
//!   `Frequency` and `Year`), so `GetParameterValuesFiltered` cannot list its lines. Instead one
//!   `GetData` per table and frequency for the last [`DISCOVERY_YEARS`] years lists the lines the
//!   table currently publishes, with their descriptions, series codes and units.
//! - **Regional.** `GetParameterValuesFiltered` lists the table's `LineCode`s and `GeoFips`; the
//!   areas kept are the nation, states and BEA regions (five digits ending in `000`). One series
//!   per line and area.
//!
//! Discovery is all or nothing: any failed request, or a table and frequency with no rows (for
//! example a table BEA renamed), fails the whole discovery. A successful discovery therefore lists
//! every series the adapter crawls ([`discovery_is_complete`](SourceAdapter::discovery_is_complete)),
//! so the worker retires series it no longer lists: lines or areas BEA dropped, and the made-up
//! ids (`NIPA_GDP_TOTAL`, ...) of the old hard-coded catalog.
//!
//! Titles are the line description, the table's short title and, for NIPA, the frequency. NIPA
//! tables repeat descriptions (`Goods` and `Services` under consumption, exports and imports), so
//! a description that occurs more than once in its table also gets `line N`.
//!
//! # Fetching
//!
//! [`batch_key`](SourceAdapter::batch_key) groups the series one `GetData` request returns:
//!
//! - NIPA: table and frequency. `GetData` returns every line of the table.
//! - Regional: table and line code, with the batch's areas as a comma-separated `GeoFips`. A
//!   Regional batch also requests the table's line codes (`GetParameterValuesFiltered`), since
//!   `GetData` rows carry no line description for the title.
//!
//! Every fetch asks for the whole history (`Year` `X` for NIPA, `ALL` for Regional) and ignores
//! `since`: BEA's annual updates revise several years and comprehensive updates revise (and
//! re-base) the whole history, so an incremental window would keep stale values. A table's full
//! history is still one modest request. A requested series with no rows in the response is
//! `NotFound`.
//!
//! `DataValue` has thousands separators and is scaled by `10^UNIT_MULT`, so values are in base
//! units (dollars, not millions of dollars). Units come from `CL_UNIT`, or `METRIC_NAME` when
//! `CL_UNIT` is `Level`, with a `Thousands of` / `Millions of` / `Billions of` prefix matching
//! `UNIT_MULT` removed. BEA's markers for a missing value (`(D)` suppressed, `(NA)`, `(NM)`,
//! `---` and the like: anything without a digit) become points with no value. `TimePeriod` is
//! `2024`, `2024Q3` or `2024M07`; the point date is the period's first day.
//!
//! A table dropped from `bea_tables.csv`, or a Regional line BEA no longer lists in
//! `GetParameterValuesFiltered`, still fetches its points (see [`fallback_table`]) but with no
//! metadata, so [`persist_series`](crate::persist::persist_series)'s COALESCE keeps the series'
//! already-stored title instead of overwriting it with a degraded placeholder.
//!
//! `NoteRef` is dropped: train 1 has no place for observation attributes, and the notes are table
//! footnotes or the `(D)` marker already reflected in the missing value.
//!
//! The API returns current estimates only (vintages are published as archived releases, not
//! through the API), so every point has `revision_date = date` and `is_original_release = true`,
//! like BLS; since every fetch covers the whole history, a revision overwrites the stored value on
//! the next crawl.
//!
//! Quarterly and monthly NIPA series are seasonally adjusted, and dollar levels are at annual
//! rates, which the series' seasonal adjustment says.
//!
//! # Keys and errors
//!
//! The key (`ctx.keys.bea`, `BEA_API_KEY`) is required: without it every call fails with
//! `Permanent` before any request. The `UserID` query parameter is redacted by the
//! [`HttpFetcher`](crate::HttpFetcher) in logs and errors.
//!
//! BEA reports request errors as HTTP 200 with
//! `{"BEAAPI":{..,"Error":{"APIErrorCode":..,"APIErrorDescription":..}}}` (under `BEAAPI` or
//! `BEAAPI.Results`). [`classify_bea_error`] maps a description that mentions the `UserId` to
//! `Auth`, a rejected parameter (invalid, not valid, does not exist, missing, required) to
//! `Permanent`, and anything else (BEA's generic failures, such as errors retrieving data
//! during a release) to `Transient`, so one hiccup doesn't permanently fail a whole batch.
//! HTTP-level errors use the fetcher's status mapping.
//!
//! # Rate limits
//!
//! BEA allows 100 requests a minute per key; the policy stays at 30 a minute
//! ([`SourcePolicy::default_for`](crate::SourcePolicy::default_for)), with batches of up to 64
//! series.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::sync::OnceLock;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::adapter::{
    BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite,
    SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::reference::{data_dir, DATA_DIR_ENV};
use crate::source::SourceId;

/// The real BEA API root.
pub const DEFAULT_BASE_URL: &str = "https://apps.bea.gov/api/data";

/// Dataset code of NIPA table lines.
pub const NIPA_DATASET: &str = "bea_nipa";

/// Dataset code of Regional table lines by area.
pub const REGIONAL_DATASET: &str = "bea_regional";

/// File name of the curated tables in the reference data directory.
pub const TABLES_FILE: &str = "bea_tables.csv";

/// Years of NIPA data (the current one included) discovery requests to list a table's lines.
pub const DISCOVERY_YEARS: i32 = 3;

/// Largest `UNIT_MULT` accepted (BEA uses 0, 3, 6 and 9).
const MAX_UNIT_MULT: u32 = 12;

/// A BEA dataset the adapter reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BeaDataset {
    /// National Income and Product Accounts (`NIPA`).
    Nipa,
    /// Regional Economic Accounts (`Regional`).
    Regional,
}

impl BeaDataset {
    /// BEA's `DatasetName`.
    pub fn api_name(self) -> &'static str {
        match self {
            BeaDataset::Nipa => "NIPA",
            BeaDataset::Regional => "Regional",
        }
    }

    /// Our dataset code.
    pub fn code(self) -> &'static str {
        match self {
            BeaDataset::Nipa => NIPA_DATASET,
            BeaDataset::Regional => REGIONAL_DATASET,
        }
    }
}

/// A BEA `Frequency` code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Frequency {
    /// `A`.
    Annual,
    /// `Q`.
    Quarterly,
    /// `M`.
    Monthly,
}

impl Frequency {
    /// Parses `A`, `Q` or `M`.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "A" => Some(Frequency::Annual),
            "Q" => Some(Frequency::Quarterly),
            "M" => Some(Frequency::Monthly),
            _ => None,
        }
    }

    /// BEA's code.
    pub fn code(self) -> &'static str {
        match self {
            Frequency::Annual => "A",
            Frequency::Quarterly => "Q",
            Frequency::Monthly => "M",
        }
    }

    /// The label stored as the series frequency.
    pub fn label(self) -> &'static str {
        match self {
            Frequency::Annual => "Annual",
            Frequency::Quarterly => "Quarterly",
            Frequency::Monthly => "Monthly",
        }
    }
}

/// One curated table from [`TABLES_FILE`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaTable {
    /// BEA dataset.
    pub dataset: BeaDataset,
    /// BEA `TableName`, e.g. `T10105` or `SAGDP2N`.
    pub table_name: String,
    /// Frequencies crawled, in file order. Regional tables are annual.
    pub frequencies: Vec<Frequency>,
    /// Short title used in series titles, e.g. `GDP, current dollars`.
    pub title: String,
}

/// The curated tables from [`TABLES_FILE`] in the reference data directory, read on first use
/// and cached. A missing or malformed file is a `Permanent` error with the path in the message.
pub fn tables() -> Result<&'static [BeaTable], CrawlError> {
    static TABLES: OnceLock<Result<Vec<BeaTable>, String>> = OnceLock::new();
    TABLES
        .get_or_init(|| load_tables(&data_dir().join(TABLES_FILE)))
        .as_deref()
        .map_err(|e| CrawlError::Permanent(e.clone()))
}

fn load_tables(path: &Path) -> Result<Vec<BeaTable>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e} (set {DATA_DIR_ENV})", path.display()))?;
    parse_tables(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parses `dataset,table_name,frequencies,title` rows after a header line. Blank lines and `#`
/// comments are skipped; the title may be wrapped in double quotes (it may contain commas).
/// Table names must be unique, and at least one table is required.
fn parse_tables(text: &str) -> Result<Vec<BeaTable>, String> {
    const HEADER: &str = "dataset,table_name,frequencies,title";
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, HEADER)) => {}
        Some((n, other)) => {
            return Err(format!("line {n}: expected header {HEADER}, got {other:?}"))
        }
        None => return Err("no header line".into()),
    }
    let mut tables = Vec::new();
    let mut seen = HashSet::new();
    for (n, line) in lines {
        let mut cols = line.splitn(4, ',').map(str::trim);
        let (Some(dataset), Some(table_name), Some(frequencies), Some(title)) =
            (cols.next(), cols.next(), cols.next(), cols.next())
        else {
            return Err(format!("line {n}: expected {HEADER}, got {line:?}"));
        };
        let dataset = match dataset {
            "NIPA" => BeaDataset::Nipa,
            "Regional" => BeaDataset::Regional,
            other => {
                return Err(format!(
                    "line {n}: unknown dataset {other:?} (NIPA or Regional)"
                ))
            }
        };
        if table_name.is_empty() || !table_name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(format!(
                "line {n}: table name {table_name:?} is not alphanumeric"
            ));
        }
        if !seen.insert(table_name) {
            return Err(format!("line {n}: table {table_name} is listed twice"));
        }
        let frequencies = frequencies
            .split_whitespace()
            .map(|f| {
                Frequency::from_code(f)
                    .ok_or_else(|| format!("line {n}: unknown frequency {f:?} (A, Q or M)"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if frequencies.is_empty() {
            return Err(format!("line {n}: table {table_name} has no frequency"));
        }
        if dataset == BeaDataset::Regional && frequencies != [Frequency::Annual] {
            return Err(format!(
                "line {n}: Regional table {table_name} must be annual (A)"
            ));
        }
        let title = title
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .unwrap_or(title)
            .trim();
        if title.is_empty() {
            return Err(format!("line {n}: table {table_name} has no title"));
        }
        tables.push(BeaTable {
            dataset,
            table_name: table_name.to_string(),
            frequencies,
            title: title.to_string(),
        });
    }
    if tables.is_empty() {
        return Err("no tables".into());
    }
    Ok(tables)
}

/// A BEA series id, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SeriesKey {
    Nipa {
        table: String,
        line: String,
        frequency: Frequency,
    },
    Regional {
        table: String,
        line: String,
        geo: String,
    },
}

impl SeriesKey {
    /// Parses `bea_nipa/{table}.{line}.{frequency}` or `bea_regional/{table}.{line}.{geo}`.
    fn parse(external_id: &str) -> Option<Self> {
        let (code, rest) = external_id.split_once('/')?;
        let mut parts = rest.split('.');
        let (Some(table), Some(line), Some(last), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        let alnum = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric());
        if !alnum(table) || !alnum(line) {
            return None;
        }
        match code {
            NIPA_DATASET => Some(SeriesKey::Nipa {
                table: table.into(),
                line: line.into(),
                frequency: Frequency::from_code(last)?,
            }),
            REGIONAL_DATASET if alnum(last) => Some(SeriesKey::Regional {
                table: table.into(),
                line: line.into(),
                geo: last.into(),
            }),
            _ => None,
        }
    }

    /// The series one `GetData` request returns together.
    fn batch_key(&self) -> String {
        match self {
            SeriesKey::Nipa {
                table, frequency, ..
            } => format!("{NIPA_DATASET}/{table}/{}", frequency.code()),
            SeriesKey::Regional { table, line, .. } => format!("{REGIONAL_DATASET}/{table}/{line}"),
        }
    }

    fn table(&self) -> &str {
        match self {
            SeriesKey::Nipa { table, .. } | SeriesKey::Regional { table, .. } => table,
        }
    }

    fn dataset(&self) -> SeriesDataset {
        match self {
            SeriesKey::Nipa {
                table,
                line,
                frequency,
            } => nipa_dimensions(table, line, *frequency),
            SeriesKey::Regional { table, line, geo } => regional_dimensions(table, line, geo),
        }
    }
}

fn nipa_dimensions(table: &str, line: &str, frequency: Frequency) -> SeriesDataset {
    SeriesDataset::new(
        NIPA_DATASET,
        [
            ("table_name", table),
            ("line_number", line),
            ("frequency", frequency.code()),
        ],
    )
}

fn regional_dimensions(table: &str, line: &str, geo: &str) -> SeriesDataset {
    SeriesDataset::new(
        REGIONAL_DATASET,
        [
            ("table_name", table),
            ("line_code", line),
            ("geo_fips", geo),
        ],
    )
}

/// BEA adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct BeaAdapter {
    base_url: String,
    /// Fixed "current year" for tests; `None` uses the clock.
    current_year: Option<i32>,
}

impl BeaAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            current_year: None,
        }
    }

    #[cfg(test)]
    fn with_current_year(mut self, year: i32) -> Self {
        self.current_year = Some(year);
        self
    }

    fn current_year(&self) -> i32 {
        self.current_year.unwrap_or_else(|| Utc::now().year())
    }

    fn api_key<'a>(&self, ctx: &'a CrawlCtx) -> Result<&'a str, CrawlError> {
        ctx.keys
            .bea
            .as_deref()
            .ok_or_else(|| CrawlError::Permanent("BEA_API_KEY not set".into()))
    }

    /// One BEA request: `GET {base}/?UserID=..&method=..&..&ResultFormat=JSON`, with in-body
    /// errors mapped by [`classify_bea_error`].
    async fn call<T: DeserializeOwned>(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, CrawlError> {
        let mut query = vec![("UserID", key), ("method", method)];
        query.extend_from_slice(params);
        query.push(("ResultFormat", "JSON"));
        let body: Envelope = ctx
            .http
            .get_json(SourceId::Bea, &format!("{}/", self.base_url), &query)
            .await?;
        let api = body.beaapi;
        if let Some(err) = &api.error {
            return Err(classify_bea_error(err));
        }
        // BEA sometimes wraps `Results` in a one-element array.
        let results = match api.results {
            Some(serde_json::Value::Array(items)) => items.into_iter().next(),
            other => other,
        }
        .ok_or_else(|| CrawlError::Parse(format!("BEA {method}: no Results")))?;
        if let Some(err) = results.get("Error") {
            let err: BeaError = serde_json::from_value(err.clone())
                .map_err(|e| CrawlError::Parse(format!("BEA {method}: bad Error: {e}")))?;
            return Err(classify_bea_error(&err));
        }
        serde_json::from_value(results)
            .map_err(|e| CrawlError::Parse(format!("BEA {method}: unexpected Results: {e}")))
    }

    async fn get_data<R: DeserializeOwned>(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<R>, CrawlError> {
        let data: DataResults<R> = self.call(ctx, key, "GetData", params).await?;
        Ok(data.data)
    }

    async fn param_values(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        table: &str,
        target: &str,
    ) -> Result<Vec<ParamValue>, CrawlError> {
        let values: ParamValues = self
            .call(
                ctx,
                key,
                "GetParameterValuesFiltered",
                &[
                    ("DatasetName", BeaDataset::Regional.api_name()),
                    ("TargetParameter", target),
                    ("TableName", table),
                ],
            )
            .await?;
        Ok(values.param_value)
    }

    async fn discover_nipa(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        def: &DatasetDef,
        table: &BeaTable,
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let end = self.current_year();
        let year = years(end - DISCOVERY_YEARS + 1, end);
        let mut found = Vec::new();
        for &frequency in &table.frequencies {
            let rows: Vec<NipaRow> = self
                .get_data(
                    ctx,
                    key,
                    &[
                        ("DatasetName", BeaDataset::Nipa.api_name()),
                        ("TableName", &table.table_name),
                        ("Frequency", frequency.code()),
                        ("Year", &year),
                    ],
                )
                .await?;
            if rows.is_empty() {
                // BEA answers Ok with no rows for a table it no longer publishes at this
                // frequency. Fail rather than skip: discovery is complete, so skipping would
                // retire the table's series.
                return Err(CrawlError::Permanent(format!(
                    "BEA NIPA {} ({}): GetData returned no rows; fix {TABLES_FILE}",
                    table.table_name,
                    frequency.code()
                )));
            }
            let repeated = repeated_descriptions(&rows);
            let mut seen = HashSet::new();
            for row in &rows {
                if !seen.insert(row.line_number.as_str()) {
                    continue;
                }
                let dataset = nipa_dimensions(&table.table_name, &row.line_number, frequency);
                let external_id = def
                    .external_id(&dataset.dimensions)
                    .map_err(|e| CrawlError::Permanent(format!("BEA {}: {e}", table.table_name)))?;
                let repeated = repeated.contains(row.line_description.trim());
                let meta = nipa_metadata(table, frequency, row, repeated)?;
                found.push(DiscoveredSeries {
                    external_id,
                    title: meta.title,
                    description: meta.description,
                    units: meta.units,
                    frequency: meta.frequency,
                    data_url: None,
                    dataset: Some(dataset),
                });
            }
        }
        Ok(found)
    }

    async fn discover_regional(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        def: &DatasetDef,
        table: &BeaTable,
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let lines = self
            .param_values(ctx, key, &table.table_name, "LineCode")
            .await?;
        let geos: Vec<ParamValue> = self
            .param_values(ctx, key, &table.table_name, "GeoFips")
            .await?
            .into_iter()
            .filter(|g| is_state_level_geo(&g.key))
            .collect();
        // Discovery is complete, so an empty list would retire every series of the table.
        if lines.is_empty() || geos.is_empty() {
            return Err(CrawlError::Permanent(format!(
                "BEA Regional {}: no line codes or areas; fix {TABLES_FILE}",
                table.table_name
            )));
        }
        let mut found = Vec::with_capacity(lines.len() * geos.len());
        for line in &lines {
            let line_desc = line_description(&line.desc);
            for geo in &geos {
                let dataset = regional_dimensions(&table.table_name, &line.key, &geo.key);
                let external_id = def
                    .external_id(&dataset.dimensions)
                    .map_err(|e| CrawlError::Permanent(format!("BEA {}: {e}", table.table_name)))?;
                found.push(DiscoveredSeries {
                    external_id,
                    title: regional_title(table, line_desc, &geo.desc),
                    description: Some(regional_description(table, &line.key, &geo.key)),
                    units: None,
                    frequency: Some(Frequency::Annual.label().into()),
                    data_url: None,
                    dataset: Some(dataset),
                });
            }
        }
        Ok(found)
    }

    /// Fetches one batch group (ids sharing a [`SeriesKey::batch_key`]).
    async fn fetch_group(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        group: &[(&String, SeriesKey)],
    ) -> Result<BatchFetch, CrawlError> {
        let table_name = group[0].1.table();
        let table = tables()?.iter().find(|t| t.table_name == table_name);
        match &group[0].1 {
            SeriesKey::Nipa { frequency, .. } => {
                self.fetch_nipa(ctx, key, table_name, table, *frequency, group)
                    .await
            }
            SeriesKey::Regional { line, .. } => {
                self.fetch_regional(ctx, key, table_name, table, line, group)
                    .await
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_nipa(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        table_name: &str,
        table: Option<&BeaTable>,
        frequency: Frequency,
        group: &[(&String, SeriesKey)],
    ) -> Result<BatchFetch, CrawlError> {
        let year = all_years(BeaDataset::Nipa);
        let rows: Vec<NipaRow> = self
            .get_data(
                ctx,
                key,
                &[
                    ("DatasetName", BeaDataset::Nipa.api_name()),
                    ("TableName", table_name),
                    ("Frequency", frequency.code()),
                    ("Year", year),
                ],
            )
            .await?;
        let mut by_line: HashMap<&str, Vec<&NipaRow>> = HashMap::new();
        for row in &rows {
            by_line
                .entry(row.line_number.as_str())
                .or_default()
                .push(row);
        }
        let repeated = repeated_descriptions(&rows);
        let fallback = fallback_table(BeaDataset::Nipa, table_name, frequency);
        // A table dropped from bea_tables.csv still has real rows and line descriptions from
        // BEA, but no curated short title; don't let the bare table name overwrite a good
        // stored title on every refresh.
        let stale_table = table.is_none();
        let table = table.unwrap_or(&fallback);
        let mut out = BatchFetch::with_capacity(group.len());
        for (id, series) in group {
            let SeriesKey::Nipa { line, .. } = series else {
                continue;
            };
            let result = match by_line.get(line.as_str()) {
                None => Err(CrawlError::NotFound(format!(
                    "BEA {id}: no line {line} in {table_name} ({})",
                    frequency.code()
                ))),
                Some(rows) => nipa_series(table, frequency, rows, &repeated).map(|mut s| {
                    if stale_table {
                        s.metadata = None;
                    }
                    s.dataset = Some(series.dataset());
                    s
                }),
            };
            out.insert((*id).clone(), result);
        }
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    async fn fetch_regional(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        table_name: &str,
        table: Option<&BeaTable>,
        line: &str,
        group: &[(&String, SeriesKey)],
    ) -> Result<BatchFetch, CrawlError> {
        let found_line = self
            .param_values(ctx, key, table_name, "LineCode")
            .await?
            .into_iter()
            .find(|v| v.key == line)
            .map(|v| line_description(&v.desc).to_string());
        // A line BEA dropped from the current parameter list has no real description; don't
        // let the placeholder overwrite a good stored title on every refresh.
        let stale_line = found_line.is_none();
        let line_desc = found_line.unwrap_or_else(|| format!("Line {line}"));
        let mut geos: Vec<&str> = group
            .iter()
            .filter_map(|(_, s)| match s {
                SeriesKey::Regional { geo, .. } => Some(geo.as_str()),
                SeriesKey::Nipa { .. } => None,
            })
            .collect();
        geos.sort_unstable();
        geos.dedup();
        let geo_fips = geos.join(",");
        let year = all_years(BeaDataset::Regional);
        let rows: Vec<RegionalRow> = self
            .get_data(
                ctx,
                key,
                &[
                    ("DatasetName", BeaDataset::Regional.api_name()),
                    ("TableName", table_name),
                    ("LineCode", line),
                    ("GeoFips", &geo_fips),
                    ("Year", year),
                ],
            )
            .await?;
        let mut by_geo: HashMap<&str, Vec<&RegionalRow>> = HashMap::new();
        for row in &rows {
            by_geo.entry(row.geo_fips.as_str()).or_default().push(row);
        }
        let fallback = fallback_table(BeaDataset::Regional, table_name, Frequency::Annual);
        let stale_table = table.is_none();
        let table = table.unwrap_or(&fallback);
        let mut out = BatchFetch::with_capacity(group.len());
        for (id, series) in group {
            let SeriesKey::Regional { geo, .. } = series else {
                continue;
            };
            let result = match by_geo.get(geo.as_str()) {
                None => Err(CrawlError::NotFound(format!(
                    "BEA {id}: no data for area {geo} in {table_name} line {line}"
                ))),
                Some(rows) => regional_series(table, &line_desc, line, geo, rows).map(|mut s| {
                    if stale_table || stale_line {
                        s.metadata = None;
                    }
                    s.dataset = Some(series.dataset());
                    s
                }),
            };
            out.insert((*id).clone(), result);
        }
        Ok(out)
    }
}

impl Default for BeaAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for BeaAdapter {
    fn id(&self) -> SourceId {
        SourceId::Bea
    }

    fn datasets(&self) -> &[&str] {
        &[NIPA_DATASET, REGIONAL_DATASET]
    }

    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let key = self.api_key(ctx)?;
        let tables = tables()?;
        let defs = crate::reference::datasets(SourceId::Bea)?;
        let def = |code: &str| {
            defs.iter().find(|d| d.code == code).ok_or_else(|| {
                CrawlError::Permanent(format!("BEA dataset {code} has no definition"))
            })
        };
        let (nipa, regional) = (def(NIPA_DATASET)?, def(REGIONAL_DATASET)?);

        let mut found = Vec::new();
        for table in tables {
            let series = match table.dataset {
                BeaDataset::Nipa => self.discover_nipa(ctx, key, nipa, table).await,
                BeaDataset::Regional => self.discover_regional(ctx, key, regional, table).await,
            }?;
            tracing::debug!(table = %table.table_name, series = series.len(), "BEA table");
            found.extend(series);
        }
        tracing::info!(
            tables = tables.len(),
            series = found.len(),
            "BEA discovery finished"
        );
        Ok(found)
    }

    /// Discovery fails whole on any error (see the module docs), so a successful one is complete.
    fn discovery_is_complete(&self) -> bool {
        true
    }

    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let ids = [external_id.to_string()];
        self.fetch_batch(ctx, &ids, since)
            .await?
            .remove(external_id)
            .unwrap_or_else(|| Err(CrawlError::NotFound(format!("BEA {external_id}"))))
    }

    fn batch_key(&self, external_id: &str) -> Option<String> {
        SeriesKey::parse(external_id).map(|k| k.batch_key())
    }

    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        // Ignored: every fetch covers the whole history (see the module docs).
        _since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let key = self.api_key(ctx)?;
        let mut out = BatchFetch::with_capacity(external_ids.len());
        // The worker only batches ids with one key; group anyway so any mix works.
        let mut groups: BTreeMap<String, Vec<(&String, SeriesKey)>> = BTreeMap::new();
        for id in external_ids {
            match SeriesKey::parse(id) {
                Some(series) => groups
                    .entry(series.batch_key())
                    .or_default()
                    .push((id, series)),
                None => {
                    out.insert(
                        id.clone(),
                        Err(CrawlError::NotFound(format!(
                            "{id} is not a BEA series id ({NIPA_DATASET}/table.line.frequency or \
                             {REGIONAL_DATASET}/table.line.geo)"
                        ))),
                    );
                }
            }
        }
        for group in groups.values() {
            match self.fetch_group(ctx, key, group).await {
                Ok(results) => out.extend(results),
                Err(e @ (CrawlError::RateLimited { .. } | CrawlError::Auth(_))) => return Err(e),
                Err(e) if groups.len() == 1 && out.is_empty() => return Err(e),
                Err(e) => {
                    for (id, _) in group {
                        out.insert((*id).clone(), Err(e.clone()));
                    }
                }
            }
        }
        Ok(out)
    }
}

/// A table missing from the CSV (removed after its series were discovered) still fetches, using
/// the table name as a placeholder title; the caller drops that placeholder from the resulting
/// metadata so a stored title is never overwritten with it.
fn fallback_table(dataset: BeaDataset, table_name: &str, frequency: Frequency) -> BeaTable {
    BeaTable {
        dataset,
        table_name: table_name.to_string(),
        frequencies: vec![frequency],
        title: table_name.to_string(),
    }
}

/// `Year` for a whole history: `X` for NIPA, `ALL` for Regional.
fn all_years(dataset: BeaDataset) -> &'static str {
    match dataset {
        BeaDataset::Nipa => "X",
        BeaDataset::Regional => "ALL",
    }
}

/// Line descriptions that occur on more than one line of a NIPA response.
fn repeated_descriptions(rows: &[NipaRow]) -> HashSet<&str> {
    let mut lines: HashMap<&str, HashSet<&str>> = HashMap::new();
    for row in rows {
        lines
            .entry(row.line_description.trim())
            .or_default()
            .insert(row.line_number.as_str());
    }
    lines
        .into_iter()
        .filter(|(_, l)| l.len() > 1)
        .map(|(d, _)| d)
        .collect()
}

/// `"{start},{start+1},..,{end}"`.
fn years(start: i32, end: i32) -> String {
    (start..=end)
        .map(|y| y.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// The nation (`00000`), a state (`SS000`) or a BEA region (`9R000`): five digits ending in
/// `000`. Excludes counties, metro areas and BEA's aggregate keywords (`STATE`, `COUNTY`).
fn is_state_level_geo(key: &str) -> bool {
    key.len() == 5 && key.bytes().all(|b| b.is_ascii_digit()) && key.ends_with("000")
}

/// `[SAGDP2N] All industry total` -> `All industry total`.
fn line_description(desc: &str) -> &str {
    let desc = desc.trim();
    match desc.strip_prefix('[').and_then(|d| d.split_once(']')) {
        Some((_, rest)) => rest.trim(),
        None => desc,
    }
}

/// `Alaska *` -> `Alaska` (BEA marks footnoted areas with an asterisk).
fn geo_name(name: &str) -> &str {
    name.trim().trim_end_matches('*').trim_end()
}

/// `Gross domestic product (GDP, current dollars, quarterly)`; a description the table repeats
/// gets its line: `Goods, line 17 (GDP, current dollars, quarterly)`.
fn nipa_title(
    table: &BeaTable,
    frequency: Frequency,
    line_desc: &str,
    line: &str,
    repeated: bool,
) -> String {
    let line = if repeated {
        format!(", line {line}")
    } else {
        String::new()
    };
    format!(
        "{}{line} ({}, {})",
        line_desc.trim(),
        table.title,
        frequency.label().to_lowercase()
    )
}

/// Quarterly and monthly NIPA series are seasonally adjusted; dollar levels are also at annual
/// rates (rates such as percent changes say so in their units). Annual series: none.
fn nipa_seasonal_adjustment(frequency: Frequency, row: &NipaRow) -> Option<String> {
    if frequency == Frequency::Annual {
        return None;
    }
    let level = row
        .cl_unit
        .as_deref()
        .is_none_or(|u| u.trim().eq_ignore_ascii_case("level"));
    let dollars = row
        .metric_name
        .as_deref()
        .is_some_and(|m| m.to_ascii_lowercase().contains("dollars"));
    Some(if level && dollars {
        "Seasonally adjusted at annual rates".into()
    } else {
        "Seasonally adjusted".into()
    })
}

fn regional_title(table: &BeaTable, line_desc: &str, geo: &str) -> String {
    format!("{}, {} ({})", line_desc.trim(), geo_name(geo), table.title)
}

fn regional_description(table: &BeaTable, line: &str, geo: &str) -> String {
    format!(
        "BEA Regional table {}, line {line}, area {geo}.",
        table.table_name
    )
}

fn nipa_metadata(
    table: &BeaTable,
    frequency: Frequency,
    row: &NipaRow,
    repeated: bool,
) -> Result<NewSeriesMetadataLite, CrawlError> {
    let mult = unit_mult(row.unit_mult.as_deref())?;
    let series_code = row
        .series_code
        .as_deref()
        .map(|c| format!(" (series code {c})"))
        .unwrap_or_default();
    Ok(NewSeriesMetadataLite {
        title: nipa_title(
            table,
            frequency,
            &row.line_description,
            &row.line_number,
            repeated,
        ),
        description: Some(format!(
            "BEA NIPA table {}, line {}{series_code}.",
            table.table_name, row.line_number
        )),
        units: units(row.metric_name.as_deref(), row.cl_unit.as_deref(), mult),
        frequency: Some(frequency.label().into()),
        seasonal_adjustment: nipa_seasonal_adjustment(frequency, row),
    })
}

fn nipa_series(
    table: &BeaTable,
    frequency: Frequency,
    rows: &[&NipaRow],
    repeated: &HashSet<&str>,
) -> Result<FetchedSeries, CrawlError> {
    let first = rows[0];
    let repeated = repeated.contains(first.line_description.trim());
    let metadata = nipa_metadata(table, frequency, first, repeated)?;
    let points = points(
        rows.iter()
            .map(|r| (&r.time_period, &r.data_value, r.unit_mult.as_deref())),
    )?;
    Ok(FetchedSeries {
        metadata: Some(metadata),
        points,
        dataset: None,
    })
}

fn regional_series(
    table: &BeaTable,
    line_desc: &str,
    line: &str,
    geo: &str,
    rows: &[&RegionalRow],
) -> Result<FetchedSeries, CrawlError> {
    let first = rows[0];
    let mult = unit_mult(first.unit_mult.as_deref())?;
    let metadata = NewSeriesMetadataLite {
        title: regional_title(table, line_desc, first.geo_name.as_deref().unwrap_or(geo)),
        description: Some(regional_description(table, line, geo)),
        units: units(None, first.cl_unit.as_deref(), mult),
        frequency: Some(Frequency::Annual.label().into()),
        seasonal_adjustment: None,
    };
    let points = points(
        rows.iter()
            .map(|r| (&r.time_period, &r.data_value, r.unit_mult.as_deref())),
    )?;
    Ok(FetchedSeries {
        metadata: Some(metadata),
        points,
        dataset: None,
    })
}

/// Parses rows of `(TimePeriod, DataValue, UNIT_MULT)` into points. A period repeated keeps its
/// last row.
fn points<'a>(
    rows: impl Iterator<Item = (&'a String, &'a String, Option<&'a str>)>,
) -> Result<Vec<FetchedPoint>, CrawlError> {
    let mut by_date = BTreeMap::new();
    for (period, value, mult) in rows {
        let date = parse_period(period)?;
        by_date.insert(date, parse_value(value, unit_mult(mult)?)?);
    }
    Ok(by_date
        .into_iter()
        .map(|(date, value)| FetchedPoint {
            date,
            value,
            revision_date: date,
            is_original_release: true,
        })
        .collect())
}

/// `2024` -> 2024-01-01, `2024Q3` -> 2024-07-01, `2024M07` -> 2024-07-01.
fn parse_period(period: &str) -> Result<NaiveDate, CrawlError> {
    let p = period.trim();
    let bad = || CrawlError::Parse(format!("BEA: invalid TimePeriod {period:?}"));
    let (year, month) = match p.get(4..5) {
        None => (p, 1),
        Some("Q") => {
            let q: u32 = p[5..].parse().map_err(|_| bad())?;
            if !(1..=4).contains(&q) {
                return Err(bad());
            }
            (&p[..4], (q - 1) * 3 + 1)
        }
        Some("M") => (&p[..4], p[5..].parse().map_err(|_| bad())?),
        Some(_) => return Err(bad()),
    };
    let year: i32 = year.parse().map_err(|_| bad())?;
    NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(bad)
}

/// `UNIT_MULT` as a power of ten; absent or blank means 0.
fn unit_mult(raw: Option<&str>) -> Result<u32, CrawlError> {
    match raw.map(str::trim) {
        None | Some("") => Ok(0),
        Some(m) => m
            .parse()
            .ok()
            .filter(|m| *m <= MAX_UNIT_MULT)
            .ok_or_else(|| CrawlError::Parse(format!("BEA: invalid UNIT_MULT {m:?}"))),
    }
}

/// Parses a `DataValue` and scales it by `10^mult`. BEA's markers for a missing value (`(D)`,
/// `(NA)`, `(NM)`, `---`: anything without a digit) are `None`. Thousands separators are ignored.
fn parse_value(raw: &str, mult: u32) -> Result<Option<BigDecimal>, CrawlError> {
    let v = raw.trim();
    if !v.bytes().any(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    let value = BigDecimal::from_str(&v.replace(',', ""))
        .map_err(|e| CrawlError::Parse(format!("BEA: invalid DataValue {raw:?}: {e}")))?;
    Ok(Some(value * BigDecimal::from(10u64.pow(mult))))
}

/// The units of a scaled value: `CL_UNIT`, or `METRIC_NAME` when `CL_UNIT` is `Level`, with a
/// `Thousands of` / `Millions of` / `Billions of` prefix that `mult` already applied removed.
fn units(metric_name: Option<&str>, cl_unit: Option<&str>, mult: u32) -> Option<String> {
    let cl_unit = cl_unit.map(str::trim).filter(|u| !u.is_empty());
    let base = match cl_unit {
        Some(u) if !u.eq_ignore_ascii_case("level") => u,
        _ => metric_name.map(str::trim).filter(|m| !m.is_empty())?,
    };
    let prefix = match mult {
        3 => "thousands of ",
        6 => "millions of ",
        9 => "billions of ",
        _ => return Some(base.to_string()),
    };
    match base.get(..prefix.len()) {
        Some(p) if p.eq_ignore_ascii_case(prefix) => {
            let rest = &base[prefix.len()..];
            let mut chars = rest.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
        }
        _ => Some(base.to_string()),
    }
}

/// Maps BEA's in-body error: a description about the `UserId` (missing / invalid / inactive
/// key) is `Auth`; a rejected parameter (invalid, not valid, does not exist, missing,
/// required) is `Permanent`; anything else is `Transient`. The message never contains the key.
pub fn classify_bea_error(err: &BeaError) -> CrawlError {
    let msg = format!(
        "BEA error {}: {}",
        err.code.as_deref().unwrap_or("?"),
        err.description.as_deref().unwrap_or("")
    );
    let desc = err
        .description
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    const BAD_PARAMETER: &[&str] = &[
        "invalid",
        "not valid",
        "does not exist",
        "missing",
        "required",
    ];
    if desc.contains("userid") {
        CrawlError::Auth(msg)
    } else if BAD_PARAMETER.iter().any(|p| desc.contains(p)) {
        CrawlError::Permanent(msg)
    } else {
        CrawlError::Transient(msg)
    }
}

// ---- Wire format ----

/// `{"BEAAPI":{"Results":..,"Error":..}}`. `Results` is kept as JSON until its error is checked,
/// since an error response has none of the method's fields.
#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "BEAAPI")]
    beaapi: Api,
}

#[derive(Debug, Deserialize)]
struct Api {
    #[serde(rename = "Results", default)]
    results: Option<serde_json::Value>,
    #[serde(rename = "Error", default)]
    error: Option<BeaError>,
}

#[derive(Debug, Deserialize)]
struct DataResults<R> {
    #[serde(rename = "Data")]
    data: Vec<R>,
}

#[derive(Debug, Deserialize)]
struct ParamValues {
    #[serde(rename = "ParamValue")]
    param_value: Vec<ParamValue>,
}

#[derive(Debug, Deserialize)]
struct ParamValue {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Desc", default)]
    desc: String,
}

/// One NIPA `GetData` row.
#[derive(Debug, Deserialize)]
struct NipaRow {
    #[serde(rename = "SeriesCode", default)]
    series_code: Option<String>,
    #[serde(rename = "LineNumber")]
    line_number: String,
    #[serde(rename = "LineDescription", default)]
    line_description: String,
    #[serde(rename = "TimePeriod")]
    time_period: String,
    #[serde(rename = "METRIC_NAME", default)]
    metric_name: Option<String>,
    #[serde(rename = "CL_UNIT", default)]
    cl_unit: Option<String>,
    #[serde(rename = "UNIT_MULT", default)]
    unit_mult: Option<String>,
    #[serde(rename = "DataValue")]
    data_value: String,
}

/// One Regional `GetData` row.
#[derive(Debug, Deserialize)]
struct RegionalRow {
    #[serde(rename = "GeoFips")]
    geo_fips: String,
    #[serde(rename = "GeoName", default)]
    geo_name: Option<String>,
    #[serde(rename = "TimePeriod")]
    time_period: String,
    #[serde(rename = "CL_UNIT", default)]
    cl_unit: Option<String>,
    #[serde(rename = "UNIT_MULT", default)]
    unit_mult: Option<String>,
    #[serde(rename = "DataValue")]
    data_value: String,
}

/// BEA's in-body error object.
#[derive(Debug, Clone, Deserialize)]
pub struct BeaError {
    /// `APIErrorCode`.
    #[serde(rename = "APIErrorCode", default)]
    pub code: Option<String>,
    /// `APIErrorDescription`.
    #[serde(rename = "APIErrorDescription", default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::contract::assert_discover_ok;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    const BAD_KEY: &str = include_str!("../../tests/fixtures/bea/error_invalid_userid.json");
    const GDP_Q: &str = include_str!("../../tests/fixtures/bea/nipa_t10105_q.json");
    const REGIONAL_LINE1: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2n_line1.json");
    const REGIONAL_LINES: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2n_linecodes.json");
    const REGIONAL_GEOS: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2n_geofips.json");

    /// Every NIPA table and frequency in `bea_tables.csv`, with its fixture.
    const NIPA_FIXTURES: &[(&str, &str, &str)] = &[
        (
            "T10101",
            "A",
            include_str!("../../tests/fixtures/bea/nipa_t10101_a.json"),
        ),
        (
            "T10101",
            "Q",
            include_str!("../../tests/fixtures/bea/nipa_t10101_q.json"),
        ),
        (
            "T10105",
            "A",
            include_str!("../../tests/fixtures/bea/nipa_t10105_a.json"),
        ),
        ("T10105", "Q", GDP_Q),
        (
            "T10106",
            "A",
            include_str!("../../tests/fixtures/bea/nipa_t10106_a.json"),
        ),
        (
            "T10106",
            "Q",
            include_str!("../../tests/fixtures/bea/nipa_t10106_q.json"),
        ),
        (
            "T20100",
            "A",
            include_str!("../../tests/fixtures/bea/nipa_t20100_a.json"),
        ),
        (
            "T20100",
            "Q",
            include_str!("../../tests/fixtures/bea/nipa_t20100_q.json"),
        ),
        (
            "T20600",
            "M",
            include_str!("../../tests/fixtures/bea/nipa_t20600_m.json"),
        ),
        (
            "T20804",
            "M",
            include_str!("../../tests/fixtures/bea/nipa_t20804_m.json"),
        ),
    ];

    fn adapter(mock: &MockSource) -> BeaAdapter {
        BeaAdapter::new(mock.base_url()).with_current_year(2024)
    }

    fn get_data() -> Route {
        Route::get("/")
            .query("UserID", TEST_API_KEY)
            .query("method", "GetData")
            .query("ResultFormat", "JSON")
    }

    fn nipa_route(table: &str, frequency: &str) -> Route {
        get_data()
            .query("DatasetName", "NIPA")
            .query("TableName", table)
            .query("Frequency", frequency)
    }

    fn regional_params(target: &str) -> Route {
        Route::get("/")
            .query("method", "GetParameterValuesFiltered")
            .query("DatasetName", "Regional")
            .query("TableName", "SAGDP2N")
            .query("TargetParameter", target)
    }

    fn regional_data() -> Route {
        get_data()
            .query("DatasetName", "Regional")
            .query("TableName", "SAGDP2N")
            .query("LineCode", "1")
    }

    async fn mount_discovery(mock: &MockSource) {
        for (table, frequency, body) in NIPA_FIXTURES {
            mock.mount(
                &nipa_route(table, frequency).query("Year", "2022,2023,2024"),
                Reply::json_str(*body),
            )
            .await;
        }
        mock.mount(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
        )
        .await;
        mock.mount(&regional_params("GeoFips"), Reply::json_str(REGIONAL_GEOS))
            .await;
    }

    fn dec(s: &str) -> Option<BigDecimal> {
        Some(BigDecimal::from_str(s).unwrap())
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(BeaAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(BeaAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(BeaAdapter::default().id(), SourceId::Bea);
        assert_eq!(
            BeaAdapter::default().datasets(),
            [NIPA_DATASET, REGIONAL_DATASET]
        );
    }

    #[test]
    fn curated_tables_file() {
        let tables = tables().unwrap();
        let names: Vec<&str> = tables.iter().map(|t| t.table_name.as_str()).collect();
        assert_eq!(
            names,
            ["T10101", "T10105", "T10106", "T20100", "T20600", "T20804", "SAGDP2N"]
        );
        let gdp = &tables[1];
        assert_eq!(gdp.dataset, BeaDataset::Nipa);
        assert_eq!(gdp.frequencies, [Frequency::Annual, Frequency::Quarterly]);
        assert_eq!(gdp.title, "GDP, current dollars");
        assert_eq!(tables[6].dataset, BeaDataset::Regional);
        // Every NIPA table and frequency has a discovery fixture.
        let listed: usize = tables
            .iter()
            .filter(|t| t.dataset == BeaDataset::Nipa)
            .map(|t| t.frequencies.len())
            .sum();
        assert_eq!(listed, NIPA_FIXTURES.len());
    }

    #[test]
    fn parse_tables_rejects_bad_rows() {
        let header = "dataset,table_name,frequencies,title\n";
        let ok = parse_tables(&format!("# c\n{header}NIPA,T1,A Q,\"A, b\"\n")).unwrap();
        assert_eq!(ok[0].title, "A, b");
        for (body, err) in [
            ("", "no header"),
            ("x,y\n", "expected header"),
            (header, "no tables"),
            ("NIPA,T1,A\n", "expected dataset"),
            ("ITA,T1,A,t\n", "unknown dataset"),
            ("NIPA,T-1,A,t\n", "not alphanumeric"),
            ("NIPA,T1,A,t\nNIPA,T1,Q,t\n", "listed twice"),
            ("NIPA,T1,W,t\n", "unknown frequency"),
            ("NIPA,T1, ,t\n", "no frequency"),
            ("Regional,S1,Q,t\n", "must be annual"),
            ("NIPA,T1,A,\"\"\n", "no title"),
        ] {
            let text = if body.starts_with("dataset") || body.is_empty() || body.starts_with("x,") {
                body.to_string()
            } else {
                format!("{header}{body}")
            };
            let e = parse_tables(&text).unwrap_err();
            assert!(e.contains(err), "{body:?}: {e}");
        }
    }

    #[test]
    fn series_ids_parse_and_batch() {
        let a = BeaAdapter::default();
        assert_eq!(
            a.batch_key("bea_nipa/T10105.1.Q").as_deref(),
            Some("bea_nipa/T10105/Q")
        );
        assert_eq!(
            a.batch_key("bea_nipa/T10105.22.Q"),
            a.batch_key("bea_nipa/T10105.1.Q")
        );
        assert_ne!(
            a.batch_key("bea_nipa/T10105.1.A"),
            a.batch_key("bea_nipa/T10105.1.Q")
        );
        assert_eq!(
            a.batch_key("bea_regional/SAGDP2N.1.06000").as_deref(),
            Some("bea_regional/SAGDP2N/1")
        );
        for bad in [
            "NIPA_GDP_TOTAL",
            "REG_GDP_TOTAL",
            "bea_nipa/T10105.1",
            "bea_nipa/T10105.1.X",
            "bea_nipa/T10105.1.Q.2",
            "bea_nipa/.1.Q",
            "bea_other/T10105.1.Q",
            "bea_regional/SAGDP2N.1.",
        ] {
            assert_eq!(a.batch_key(bad), None, "{bad}");
        }
    }

    #[test]
    fn periods_values_and_units() {
        assert_eq!(parse_period("2024").unwrap(), d("2024-01-01"));
        assert_eq!(parse_period("2024Q3").unwrap(), d("2024-07-01"));
        assert_eq!(parse_period("2024M07").unwrap(), d("2024-07-01"));
        for bad in ["2024Q5", "2024M13", "2024X1", "20x4", "2024Q"] {
            assert_eq!(parse_period(bad).unwrap_err().kind(), "parse", "{bad}");
        }

        assert_eq!(parse_value("28,624,069", 6).unwrap(), dec("28624069000000"));
        assert_eq!(parse_value("-1.5", 0).unwrap(), dec("-1.5"));
        assert_eq!(parse_value("12.5", 3).unwrap(), dec("12500"));
        for missing in ["(D)", "(NA)", "(NM)", "---", " "] {
            assert_eq!(parse_value(missing, 6).unwrap(), None, "{missing}");
        }
        assert_eq!(parse_value("1x2", 0).unwrap_err().kind(), "parse");
        assert_eq!(unit_mult(None).unwrap(), 0);
        assert_eq!(unit_mult(Some("6")).unwrap(), 6);
        assert_eq!(unit_mult(Some("99")).unwrap_err().kind(), "parse");

        let u = |m, c, x| units(m, c, x);
        assert_eq!(
            u(Some("Current Dollars"), Some("Level"), 6).as_deref(),
            Some("Current Dollars")
        );
        assert_eq!(
            u(
                Some("Fisher Quantity Index"),
                Some("Percent change, annual rate"),
                0
            )
            .as_deref(),
            Some("Percent change, annual rate")
        );
        assert_eq!(
            u(None, Some("Millions of current dollars"), 6).as_deref(),
            Some("Current dollars")
        );
        // A prefix the multiplier did not apply stays.
        assert_eq!(
            u(None, Some("Millions of current dollars"), 0).as_deref(),
            Some("Millions of current dollars")
        );
        assert_eq!(u(None, Some("Level"), 0), None);
        assert_eq!(
            line_description("[SAGDP2N] All industry total"),
            "All industry total"
        );
        assert_eq!(line_description("Plain"), "Plain");
        assert_eq!(geo_name("Alaska *"), "Alaska");
        assert!(is_state_level_geo("00000") && is_state_level_geo("06000"));
        assert!(is_state_level_geo("91000"));
        assert!(!is_state_level_geo("06037") && !is_state_level_geo("STATE"));
    }

    #[tokio::test]
    async fn discover_lists_every_curated_line() {
        let mock = MockSource::start().await;
        mount_discovery(&mock).await;
        let a = adapter(&mock);
        let found = assert_discover_ok(&a, &test_ctx(), &mock, 1000).await;
        // NIPA: 26 + 26 + 27 lines at A and Q, 35 at A, Q and M, 29 at M. Regional: 31 lines
        // by 60 areas (the nation, 50 states, DC and 8 regions).
        let nipa = found
            .iter()
            .filter(|s| s.external_id.starts_with("bea_nipa/"))
            .count();
        assert_eq!(nipa, 2 * (26 + 26 + 27 + 35) + 35 + 29);
        assert_eq!(found.len() - nipa, 31 * 60);
        assert_eq!(
            mock.received_requests().await.len(),
            NIPA_FIXTURES.len() + 2
        );

        let by_id = |id: &str| found.iter().find(|s| s.external_id == id).unwrap();
        let gdp = by_id("bea_nipa/T10105.1.Q");
        assert_eq!(
            gdp.title,
            "Gross domestic product (GDP, current dollars, quarterly)"
        );
        assert_eq!(gdp.units.as_deref(), Some("Current Dollars"));
        assert_eq!(gdp.frequency.as_deref(), Some("Quarterly"));
        assert_eq!(
            gdp.description.as_deref(),
            Some("BEA NIPA table T10105, line 1 (series code A191RC).")
        );
        assert_eq!(
            gdp.dataset,
            Some(SeriesDataset::new(
                NIPA_DATASET,
                [
                    ("table_name", "T10105"),
                    ("line_number", "1"),
                    ("frequency", "Q")
                ]
            ))
        );
        assert_eq!(
            by_id("bea_nipa/T10101.1.A").units.as_deref(),
            Some("Percent change, annual rate")
        );
        let ca = by_id("bea_regional/SAGDP2N.1.06000");
        assert_eq!(
            ca.title,
            "All industry total, California (GDP by state, current dollars)"
        );
        assert_eq!(ca.frequency.as_deref(), Some("Annual"));
        assert_eq!(
            by_id("bea_regional/SAGDP2N.86.02000").title,
            "State and local, Alaska (GDP by state, current dollars)"
        );
        assert!(found
            .iter()
            .any(|s| s.external_id == "bea_regional/SAGDP2N.1.00000"));
        assert!(found
            .iter()
            .any(|s| s.external_id == "bea_regional/SAGDP2N.1.98000"));
    }

    /// Discovery is all or nothing: a table BEA rejects fails it rather than being skipped, so
    /// a partial listing never retires the table's series.
    #[tokio::test]
    async fn discover_fails_on_any_table_error() {
        let mock = MockSource::start().await;
        assert!(adapter(&mock).discovery_is_complete());
        let unknown = serde_json::json!({"BEAAPI": {"Results": {"Error": {
            "APIErrorCode": "101", "APIErrorDescription": "Invalid TableName."}}}});
        mock.mount(&nipa_route("T20804", "M"), Reply::json(unknown))
            .await;
        mount_discovery(&mock).await;
        let e = adapter(&mock).discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");

        let mock = MockSource::start().await;
        mock.mount(
            &nipa_route("T10105", "A"),
            Reply::status(429).retry_after(1),
        )
        .await;
        mount_discovery(&mock).await;
        let e = adapter(&mock).discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "rate_limited", "{e}");
    }

    #[tokio::test]
    async fn fetch_nipa_series_scales_by_unit_mult() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &nipa_route("T10105", "Q").query("Year", "X"),
            Reply::json_str(GDP_Q),
            1,
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T10105.1.Q", None)
            .await
            .unwrap();
        let meta = s.metadata.unwrap();
        assert_eq!(
            meta.title,
            "Gross domestic product (GDP, current dollars, quarterly)"
        );
        assert_eq!(meta.units.as_deref(), Some("Current Dollars"));
        assert_eq!(meta.frequency.as_deref(), Some("Quarterly"));
        assert_eq!(
            meta.seasonal_adjustment.as_deref(),
            Some("Seasonally adjusted at annual rates")
        );
        assert_eq!(s.points.len(), 4);
        assert_eq!(s.points[0].date, d("2024-01-01"));
        assert_eq!(s.points[0].value, dec("28624069000000"));
        assert_eq!(s.points[3].date, d("2024-10-01"));
        assert!(s
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));
        assert_eq!(
            s.dataset.unwrap(),
            nipa_dimensions("T10105", "1", Frequency::Quarterly)
        );
        mock.server().verify().await;
    }

    /// Line 35 of T20100 is a percentage, not a dollar level: `CL_UNIT`/`METRIC_NAME` are
    /// `Percent` and `UNIT_MULT` is `0`, so the value is used as-is, unscaled.
    #[tokio::test]
    async fn fetch_nipa_percent_line_is_not_scaled() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &nipa_route("T20100", "A").query("Year", "X"),
            Reply::json_str(include_str!("../../tests/fixtures/bea/nipa_t20100_a.json")),
            1,
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T20100.35.A", None)
            .await
            .unwrap();
        let meta = s.metadata.unwrap();
        assert_eq!(meta.units.as_deref(), Some("Percent"));
        assert_eq!(s.points.len(), 2);
        assert_eq!(s.points[0].date, d("2023-01-01"));
        assert_eq!(s.points[0].value, dec("4.6"));
        assert_eq!(s.points[1].date, d("2024-01-01"));
        assert_eq!(s.points[1].value, dec("4.8"));
        mock.server().verify().await;
    }

    /// `since` is ignored: every fetch asks for the whole history and keeps every point, so
    /// revisions older than the lookback window are picked up.
    #[tokio::test]
    async fn fetch_since_still_requests_the_whole_history() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &nipa_route("T10105", "Q").query("Year", "X"),
            Reply::json_str(GDP_Q),
            1,
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T10105.1.Q", Some(d("2024-06-01")))
            .await
            .unwrap();
        assert_eq!(s.points.len(), 4);
        assert_eq!(s.points[0].date, d("2024-01-01"));
        mock.server().verify().await;
    }

    /// T10105 repeats `Goods` and `Services` under consumption, exports and imports; those
    /// titles carry their line so the three are told apart, while unique lines don't.
    #[tokio::test]
    async fn repeated_line_descriptions_get_their_line_number() {
        let mock = MockSource::start().await;
        mock.mount(&nipa_route("T10105", "Q"), Reply::json_str(GDP_Q))
            .await;
        let ids: Vec<String> = ["1", "3", "17", "20"]
            .map(|l| format!("bea_nipa/T10105.{l}.Q"))
            .to_vec();
        let out = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap();
        let title = |l: &str| {
            out[&format!("bea_nipa/T10105.{l}.Q")]
                .as_ref()
                .unwrap()
                .metadata
                .as_ref()
                .unwrap()
                .title
                .clone()
        };
        assert_eq!(
            title("1"),
            "Gross domestic product (GDP, current dollars, quarterly)"
        );
        assert_eq!(
            title("3"),
            "Goods, line 3 (GDP, current dollars, quarterly)"
        );
        assert_eq!(
            title("17"),
            "Goods, line 17 (GDP, current dollars, quarterly)"
        );
        assert_eq!(
            title("20"),
            "Goods, line 20 (GDP, current dollars, quarterly)"
        );
    }

    #[test]
    fn nipa_seasonal_adjustment_by_frequency_and_unit() {
        let row = |metric: &str, unit: &str| NipaRow {
            metric_name: Some(metric.into()),
            cl_unit: Some(unit.into()),
            ..serde_json::from_value(serde_json::json!({
                "LineNumber": "1", "LineDescription": "x", "TimePeriod": "2024", "DataValue": "1"
            }))
            .unwrap()
        };
        let saar = Some("Seasonally adjusted at annual rates".to_string());
        let sa = Some("Seasonally adjusted".to_string());
        let dollars = row("Chained Dollars", "Level");
        let rate = row("Fisher Quantity Index", "Percent change, annual rate");
        assert_eq!(nipa_seasonal_adjustment(Frequency::Annual, &dollars), None);
        assert_eq!(
            nipa_seasonal_adjustment(Frequency::Quarterly, &dollars),
            saar
        );
        assert_eq!(nipa_seasonal_adjustment(Frequency::Monthly, &dollars), saar);
        assert_eq!(nipa_seasonal_adjustment(Frequency::Quarterly, &rate), sa);
    }

    /// A monthly table fetches through `fetch_series`, not just through `parse_period` or
    /// discovery, with point dates on the first of each month.
    #[tokio::test]
    async fn fetch_monthly_series_parses_month_periods() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &nipa_route("T20600", "M"),
            Reply::json_str(include_str!("../../tests/fixtures/bea/nipa_t20600_m.json")),
            1,
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T20600.1.M", None)
            .await
            .unwrap();
        let dates: Vec<_> = s.points.iter().map(|p| p.date).collect();
        assert_eq!(dates, [d("2024-01-01"), d("2024-02-01"), d("2024-03-01")]);
        mock.server().verify().await;
    }

    /// `since` on a Regional fetch still requests `ALL` years and keeps every point.
    #[tokio::test]
    async fn fetch_regional_since_still_requests_all_years() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &regional_data()
                .query("GeoFips", "06000")
                .query("Year", "ALL"),
            Reply::json_str(REGIONAL_LINE1),
            1,
        )
        .await;
        mock.mount_expect(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
            1,
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(
                &test_ctx(),
                "bea_regional/SAGDP2N.1.06000",
                Some(d("2023-01-01")),
            )
            .await
            .unwrap();
        assert_eq!(s.points.first().unwrap().date, d("2021-01-01"));
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn fetch_batch_nipa_is_one_request_per_table_and_frequency() {
        let mock = MockSource::start().await;
        mock.mount_expect(&nipa_route("T10105", "Q"), Reply::json_str(GDP_Q), 1)
            .await;
        let ids: Vec<String> = [
            "bea_nipa/T10105.1.Q",
            "bea_nipa/T10105.2.Q",
            "bea_nipa/T10105.99.Q",
        ]
        .map(String::from)
        .to_vec();
        let out = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap();
        assert_eq!(out.len(), 3);
        let pce = out["bea_nipa/T10105.2.Q"].as_ref().unwrap();
        assert_eq!(
            pce.metadata.as_ref().unwrap().title,
            "Personal consumption expenditures (GDP, current dollars, quarterly)"
        );
        assert_eq!(out["bea_nipa/T10105.1.Q"].as_ref().unwrap().points.len(), 4);
        assert_eq!(
            out["bea_nipa/T10105.99.Q"].as_ref().unwrap_err().kind(),
            "not_found"
        );
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn fetch_batch_regional_requests_the_batch_areas() {
        let mock = MockSource::start().await;
        mock.mount_expect(
            &regional_data()
                .query("GeoFips", "00000,01000,06000,56000,72000")
                .query("Year", "ALL"),
            Reply::json_str(REGIONAL_LINE1),
            1,
        )
        .await;
        mock.mount_expect(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
            1,
        )
        .await;
        let ids: Vec<String> = [
            "bea_regional/SAGDP2N.1.06000",
            "bea_regional/SAGDP2N.1.00000",
            "bea_regional/SAGDP2N.1.56000",
            "bea_regional/SAGDP2N.1.01000",
            "bea_regional/SAGDP2N.1.72000",
        ]
        .map(String::from)
        .to_vec();
        let out = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap();
        let ca = out["bea_regional/SAGDP2N.1.06000"].as_ref().unwrap();
        let meta = ca.metadata.as_ref().unwrap();
        assert_eq!(
            meta.title,
            "All industry total, California (GDP by state, current dollars)"
        );
        assert_eq!(meta.units.as_deref(), Some("Current dollars"));
        assert_eq!(meta.frequency.as_deref(), Some("Annual"));
        assert_eq!(ca.points.len(), 3);
        assert_eq!(ca.points[0].date, d("2021-01-01"));
        assert_eq!(ca.points[0].value, dec("3598103000000"));
        assert_eq!(
            ca.dataset.as_ref().unwrap(),
            &regional_dimensions("SAGDP2N", "1", "06000")
        );
        // `(D)` (suppressed) is a point without a value.
        let wy = out["bea_regional/SAGDP2N.1.56000"].as_ref().unwrap();
        assert_eq!(wy.points.len(), 1);
        assert_eq!(wy.points[0].value, None);
        assert_eq!(
            out["bea_regional/SAGDP2N.1.72000"]
                .as_ref()
                .unwrap_err()
                .kind(),
            "not_found"
        );
        mock.server().verify().await;
    }

    /// A batch spanning two groups (different table/frequency) makes one request per group, and
    /// one group's failure fails only its own ids, leaving the other group's results intact.
    #[tokio::test]
    async fn fetch_batch_spans_two_groups_independently() {
        let mock = MockSource::start().await;
        mock.mount_expect(&nipa_route("T10105", "Q"), Reply::json_str(GDP_Q), 1)
            .await;
        mock.mount_expect(&nipa_route("T10101", "A"), Reply::status(404), 1)
            .await;
        let ids: Vec<String> = ["bea_nipa/T10105.1.Q", "bea_nipa/T10101.1.A"]
            .map(String::from)
            .to_vec();
        let out = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out["bea_nipa/T10105.1.Q"].as_ref().unwrap().points.len(), 4);
        assert_eq!(
            out["bea_nipa/T10101.1.A"].as_ref().unwrap_err().kind(),
            "not_found"
        );
        mock.server().verify().await;
    }

    /// A whole-batch failure with only one group in play fails the batch itself (`fetch_batch`
    /// returns `Err`), rather than reporting a per-id error.
    #[tokio::test]
    async fn fetch_batch_single_group_error_fails_the_whole_batch() {
        let mock = MockSource::start().await;
        mock.mount(&nipa_route("T10105", "Q"), Reply::status(404))
            .await;
        let ids: Vec<String> = ["bea_nipa/T10105.1.Q", "bea_nipa/T10105.2.Q"]
            .map(String::from)
            .to_vec();
        let e = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");
    }

    /// A table dropped from `bea_tables.csv` after its series were discovered still fetches real
    /// points, but doesn't overwrite a good stored title with the bare table name.
    #[tokio::test]
    async fn fetch_stale_table_keeps_points_but_omits_metadata() {
        let mock = MockSource::start().await;
        mock.mount(
            &nipa_route("T99999", "A"),
            Reply::json_str(
                r#"{"BEAAPI":{"Results":{"Data":[
                    {"TableName": "T99999", "SeriesCode": "X", "LineNumber": "1",
                     "LineDescription": "Something", "TimePeriod": "2024",
                     "METRIC_NAME": "Current Dollars", "CL_UNIT": "Level", "UNIT_MULT": "6",
                     "DataValue": "1,000"}
                ]}}}"#,
            ),
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T99999.1.A", None)
            .await
            .unwrap();
        assert!(s.metadata.is_none(), "{s:?}");
        assert!(!s.points.is_empty());
    }

    /// A line BEA no longer lists in `GetParameterValuesFiltered` still fetches real points (the
    /// `GetData` response still has rows for it), but its placeholder description doesn't
    /// overwrite a good stored title.
    #[tokio::test]
    async fn fetch_regional_stale_line_keeps_points_but_omits_metadata() {
        let mock = MockSource::start().await;
        mock.mount(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
        )
        .await;
        mock.mount(
            &get_data()
                .query("DatasetName", "Regional")
                .query("TableName", "SAGDP2N")
                .query("LineCode", "99"),
            Reply::json_str(
                r#"{"BEAAPI":{"Results":{"Data":[
                    {"GeoFips": "06000", "GeoName": "California", "TimePeriod": "2023",
                     "CL_UNIT": "Millions of current dollars", "UNIT_MULT": "6",
                     "DataValue": "1,000"}
                ]}}}"#,
            ),
        )
        .await;
        let s = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_regional/SAGDP2N.99.06000", None)
            .await
            .unwrap();
        assert!(s.metadata.is_none(), "{s:?}");
        assert!(!s.points.is_empty());
    }

    /// A table with no rows in a `GetData` response (BEA no longer publishes it at that
    /// frequency) fails discovery, so its series aren't retired by a partial listing.
    #[tokio::test]
    async fn discover_fails_on_a_table_with_no_rows() {
        let mock = MockSource::start().await;
        for (table, frequency, body) in NIPA_FIXTURES {
            let reply = if *table == "T10101" && *frequency == "A" {
                Reply::json_str(r#"{"BEAAPI":{"Results":{"Data":[]}}}"#)
            } else {
                Reply::json_str(*body)
            };
            mock.mount(
                &nipa_route(table, frequency).query("Year", "2022,2023,2024"),
                reply,
            )
            .await;
        }
        mock.mount(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
        )
        .await;
        mock.mount(&regional_params("GeoFips"), Reply::json_str(REGIONAL_GEOS))
            .await;
        let e = adapter(&mock).discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");
        assert!(e.to_string().contains("T10101 (A)"), "{e}");
    }

    /// A Regional table with no line codes or areas fails discovery too, rather than listing
    /// nothing and retiring all of its series.
    #[tokio::test]
    async fn discover_fails_on_a_regional_table_with_no_areas() {
        let mock = MockSource::start().await;
        mock.mount(
            &regional_params("GeoFips"),
            Reply::json_str(r#"{"BEAAPI":{"Results":{"ParamValue":[]}}}"#),
        )
        .await;
        mount_discovery(&mock).await;
        let e = adapter(&mock).discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "permanent", "{e}");
        assert!(e.to_string().contains("SAGDP2N"), "{e}");
    }

    #[tokio::test]
    async fn bad_ids_are_not_found_without_requests() {
        let mock = MockSource::start().await;
        let e = adapter(&mock)
            .fetch_series(&test_ctx(), "NIPA_GDP_TOTAL", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn missing_key_is_permanent_without_requests() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json_str(GDP_Q)).await;
        let mut ctx = test_ctx();
        ctx.keys.bea = None;
        let a = adapter(&mock);
        let expected = CrawlError::Permanent("BEA_API_KEY not set".into());
        assert_eq!(a.discover(&ctx).await.unwrap_err(), expected);
        assert_eq!(
            a.fetch_series(&ctx, "bea_nipa/T10105.1.Q", None)
                .await
                .unwrap_err(),
            expected
        );
        let ids = ["bea_nipa/T10105.1.Q".to_string()];
        assert_eq!(a.fetch_batch(&ctx, &ids, None).await.unwrap_err(), expected);
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn in_body_errors_are_classified_and_key_redacted() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/"), Reply::json_str(BAD_KEY)).await;
        let a = adapter(&mock);
        let e = a.discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");
        assert!(!e.to_string().contains(TEST_API_KEY));
        let e = a
            .fetch_series(&test_ctx(), "bea_nipa/T10105.1.Q", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "auth", "{e}");

        // Top-level errors, and `Results` as a one-element array.
        for body in [
            serde_json::json!({"BEAAPI": {"Error": {
                "APIErrorCode": "40", "APIErrorDescription": "The dataset requested is not valid."}}}),
            serde_json::json!({"BEAAPI": {"Results": [{"Error": {
                "APIErrorCode": "101", "APIErrorDescription": "Invalid TableName."}}]}}),
        ] {
            let mock = MockSource::start().await;
            mock.mount(&Route::get("/"), Reply::json(body)).await;
            let e = adapter(&mock)
                .fetch_series(&test_ctx(), "bea_nipa/T10105.1.Q", None)
                .await
                .unwrap_err();
            assert_eq!(e.kind(), "permanent", "{e}");
        }

        // BEA's generic failures are worth a retry.
        let mock = MockSource::start().await;
        let body = serde_json::json!({"BEAAPI": {"Results": {"Error": {
            "APIErrorCode": "201", "APIErrorDescription": "Error retrieving NIPA data."}}}});
        mock.mount(&Route::get("/"), Reply::json(body)).await;
        let e = adapter(&mock)
            .fetch_series(&test_ctx(), "bea_nipa/T10105.1.Q", None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
    }

    #[test]
    fn classify_bea_error_by_description() {
        for (desc, kind) in [
            ("Invalid API UserId.", "auth"),
            ("Invalid TableName.", "permanent"),
            ("The dataset requested is not valid.", "permanent"),
            ("The GeoFips parameter is required.", "permanent"),
            ("Missing parameter: Frequency.", "permanent"),
            ("Unknown error.", "transient"),
            ("Error retrieving Regional data.", "transient"),
        ] {
            let err = BeaError {
                code: Some("1".into()),
                description: Some(desc.into()),
            };
            assert_eq!(classify_bea_error(&err).kind(), kind, "{desc}");
        }
    }

    #[tokio::test]
    async fn http_errors_map_by_status_and_redact_key() {
        for (reply, kind) in [
            (Reply::status(429).retry_after(1), "rate_limited"),
            (Reply::text("internal error").with_status(500), "transient"),
            (Reply::status(404), "not_found"),
            (
                Reply::json_str(format!("<html>{TEST_API_KEY}</html>")),
                "parse",
            ),
        ] {
            let mock = MockSource::start().await;
            mock.mount(&Route::get("/"), reply).await;
            let e = adapter(&mock).discover(&test_ctx()).await.unwrap_err();
            assert_eq!(e.kind(), kind, "{e}");
            assert!(!e.to_string().contains(TEST_API_KEY), "{e}");
        }
    }
}

#[cfg(test)]
mod contract {
    use super::BeaAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| BeaAdapter::new(base_url),
        external_id: "bea_nipa/T10105.1.Q",
        route: Route::get("/").query("TableName", "T10105").query("Frequency", "Q"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/bea/nipa_t10105_q.json")),
        expect_points: 4,
    }
}
