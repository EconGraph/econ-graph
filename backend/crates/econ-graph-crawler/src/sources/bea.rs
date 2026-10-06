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
//! `TableName` and the frequencies crawled. See [`tables`]. Each table's title comes from BEA's
//! own `GetParameterValues(TableName)`, refreshed each crawl (see
//! [`BeaAdapter::refresh_table_titles`]), not from a curated column.
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
//! A table whose title isn't known yet (BEA no longer lists it in
//! `GetParameterValues(TableName)`, or `refresh_table_titles` hasn't synced it this process)
//! fails discovery outright (see Discovery above) rather than writing a placeholder title that
//! would overwrite every one of its series' stored titles. A fetch for such a table (discovery
//! ran earlier, successfully, before the title went stale) still returns its points but with no
//! metadata, so [`persist_series`](crate::persist::persist_series)'s COALESCE keeps the series'
//! already-stored title instead of overwriting it with the table name.
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
    ApiKeys, BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::persist;
use crate::reference::{data_dir, DATA_DIR_ENV};
#[cfg(test)]
use crate::reference_file::{download_seed_entries, seed_migration_sql};
use crate::reference_file::{labels_only, refresh_code_list, CodeList};
use crate::source::SourceId;
use econ_graph_core::error::AppError;

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

/// One curated table from [`TABLES_FILE`]. Its title, used in series titles, is not here: it
/// comes from BEA's own `GetParameterValues(TableName)` on each crawl (see
/// [`BeaAdapter::refresh_table_titles`]), not from a curated column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaTable {
    /// BEA dataset.
    pub dataset: BeaDataset,
    /// BEA `TableName`, e.g. `T10105` or `SAGDP2N`.
    pub table_name: String,
    /// Frequencies crawled, in file order. Regional tables are annual.
    pub frequencies: Vec<Frequency>,
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

/// Parses `dataset,table_name,frequencies` rows after a header line. Blank lines and `#`
/// comments are skipped. Table names must be unique, and at least one table is required.
fn parse_tables(text: &str) -> Result<Vec<BeaTable>, String> {
    const HEADER: &str = "dataset,table_name,frequencies";
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
        let mut cols = line.splitn(3, ',').map(str::trim);
        let (Some(dataset), Some(table_name), Some(frequencies)) =
            (cols.next(), cols.next(), cols.next())
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
        tables.push(BeaTable {
            dataset,
            table_name: table_name.to_string(),
            frequencies,
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
#[derive(Debug)]
pub struct BeaAdapter {
    base_url: String,
    /// Fixed "current year" for tests; `None` uses the clock.
    current_year: Option<i32>,
    /// BEA's own titles for NIPA and Regional tables, from the last successful
    /// [`refresh_table_titles`](Self::refresh_table_titles) in this adapter's lifetime (empty
    /// until then). `discover` and `fetch_*` only ever read this in-process cache: they never
    /// touch the database or the network for a title, so a BEA or DB hiccup there can't fail a
    /// `fetch` already in flight, only make it omit metadata for a table whose title isn't
    /// cached (see the module docs); `discover` instead fails outright on such a table, rather
    /// than persisting the bare table name as a real title.
    titles: tokio::sync::RwLock<HashMap<BeaDataset, HashMap<String, String>>>,
}

impl BeaAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            current_year: None,
            titles: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    fn with_current_year(mut self, year: i32) -> Self {
        self.current_year = Some(year);
        self
    }

    /// Seeds the in-process title cache directly, bypassing BEA and the database, as if
    /// [`refresh_table_titles`](Self::refresh_table_titles) had already run.
    #[cfg(test)]
    fn with_table_titles(self, dataset: BeaDataset, titles: &[(&str, &str)]) -> Self {
        let mut map = self.titles.into_inner();
        map.entry(dataset).or_default().extend(
            titles
                .iter()
                .map(|(code, label)| (code.to_string(), label.to_string())),
        );
        Self {
            titles: tokio::sync::RwLock::new(map),
            ..self
        }
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
        let results = decode_results(body, method)?;
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

    /// Conditionally re-fetches BEA's own titles for every `TableName` of `list`'s dataset
    /// (`GetParameterValues`, `ParameterName=TableName`; its `ParamValue` rows are named after
    /// the parameter itself, `TableName`/`Description`, for NIPA, or the generic `Key`/`Desc`
    /// every other `GetParameterValues*` call on Regional uses — [`TableNameValue`] accepts
    /// either), merges them into the dataset's `table_name` dimension
    /// ([`refresh_code_list`], so `bea_tables.csv` doesn't need a curated title column and the
    /// GraphQL code list carries them too — a response with no rows is a `Parse` error, same as
    /// any other empty code list, never a silent no-op) and refreshes the in-process cache
    /// [`table_titles`](Self::table_titles) reads. The only method that touches the database or
    /// fetches titles over the network; called by
    /// [`refresh_reference_data`](SourceAdapter::refresh_reference_data), so its failure (BEA or
    /// the database) never fails a `discover` or `fetch_*` already in flight — it still reloads
    /// the cache from the database either way (below), so only a database that truly has nothing
    /// yet leaves the cache as it was.
    ///
    /// Reloads the in-process cache from the database after every refresh attempt, applied or
    /// not (a `304`, or an `apply` that found nowhere to merge into), rather than trusting
    /// whatever this process instance already has in memory, so a fresh worker process picks up
    /// another instance's last merge instead of placeholder titles.
    async fn refresh_table_titles(
        &self,
        ctx: &CrawlCtx,
        list: &CodeList,
        dataset: BeaDataset,
    ) -> Result<(), CrawlError> {
        // `refresh_code_list`'s own HTTP layer already counts a transport/status error; a `parse`
        // error (BEA answered 200 with an in-body error envelope) comes from `apply`, after that,
        // so it needs its own record here or it's silently dropped from `crawler_errors_total`.
        let refreshed = refresh_code_list(ctx, SourceId::Bea, list)
            .await
            .inspect_err(|e| {
                if e.kind() == "parse" {
                    ctx.http
                        .record_response_error(SourceId::Bea, list.request_url(), e);
                }
            });
        let dataset_code = dataset.code();
        // Reload from the database either way, even on a failed refresh (BEA down, rate
        // limited, a bad body): a prior process instance's merge, or this process's own earlier
        // one, may already be there, and skipping the reload would otherwise blank the
        // in-process cache (and so fail every subsequent `discover`) on a single transient BEA
        // error. After a successful merge this is the dataset's canonical (sorted, deduplicated)
        // code list rather than just this fetch's rows; after a no-op (304, or nothing to merge
        // into), it's the best available titles. Keep whatever this process already has only if
        // the database truly has nothing yet.
        let from_db =
            persist::dataset_dimension_labels(&ctx.pool, SourceId::Bea, dataset_code, "table_name")
                .await
                .map_err(db_err)?;
        let titles = if from_db.is_empty() {
            self.table_titles(dataset).await
        } else {
            from_db
        };
        self.titles.write().await.insert(dataset, titles);
        refreshed?;
        Ok(())
    }

    /// `table_name` -> BEA's title, from the last call to
    /// [`refresh_table_titles`](Self::refresh_table_titles) in this adapter's lifetime, whether
    /// or not it succeeded (that function reloads the cache from the database either way). Empty
    /// before the first call.
    async fn table_titles(&self, dataset: BeaDataset) -> HashMap<String, String> {
        self.titles
            .read()
            .await
            .get(&dataset)
            .cloned()
            .unwrap_or_default()
    }

    async fn discover_nipa(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        def: &DatasetDef,
        table: &BeaTable,
        title: &str,
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
                let meta = nipa_metadata(&table.table_name, title, frequency, row, repeated)?;
                found.push(DiscoveredSeries {
                    external_id,
                    title: meta.title,
                    description: meta.description,
                    units: meta.units,
                    frequency: meta.frequency,
                    data_url: None,
                    dataset,
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
        title: &str,
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
                    title: regional_title(title, line_desc, &geo.desc),
                    description: Some(regional_description(&table.table_name, &line.key, &geo.key)),
                    units: None,
                    frequency: Some(Frequency::Annual.label().into()),
                    data_url: None,
                    dataset,
                });
            }
        }
        Ok(found)
    }

    /// Fetches one batch group (ids sharing a [`SeriesKey::batch_key`]). `titles` is this
    /// crawl's title map for the group's dataset (NIPA or Regional), from
    /// [`table_titles`](Self::table_titles).
    async fn fetch_group(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        titles: &HashMap<String, String>,
        group: &[(&String, SeriesKey)],
    ) -> Result<BatchFetch, CrawlError> {
        let table_name = group[0].1.table();
        let (title, title_known) = title_for(titles, table_name);
        match &group[0].1 {
            SeriesKey::Nipa { frequency, .. } => {
                self.fetch_nipa(ctx, key, table_name, &title, title_known, *frequency, group)
                    .await
            }
            SeriesKey::Regional { line, .. } => {
                self.fetch_regional(ctx, key, table_name, &title, title_known, line, group)
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
        title: &str,
        title_known: bool,
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
        // A table BEA no longer lists in GetParameterValues still has real rows and line
        // descriptions, but only the bare table name as a placeholder title; don't let it
        // overwrite a good stored title on every refresh.
        let stale_table = !title_known;
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
                Some(rows) => nipa_series(
                    table_name,
                    title,
                    frequency,
                    rows,
                    &repeated,
                    series.dataset(),
                )
                .map(|mut s| {
                    if stale_table {
                        s.metadata = None;
                    }
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
        title: &str,
        title_known: bool,
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
        // A table BEA no longer lists in GetParameterValues still has real rows, but only the
        // bare table name as a placeholder title; don't let it overwrite a good stored title.
        let stale_table = !title_known;
        let mut out = BatchFetch::with_capacity(group.len());
        for (id, series) in group {
            let SeriesKey::Regional { geo, .. } = series else {
                continue;
            };
            let result = match by_geo.get(geo.as_str()) {
                None => Err(CrawlError::NotFound(format!(
                    "BEA {id}: no data for area {geo} in {table_name} line {line}"
                ))),
                Some(rows) => regional_series(
                    table_name,
                    title,
                    &line_desc,
                    line,
                    geo,
                    rows,
                    series.dataset(),
                )
                .map(|mut s| {
                    if stale_table || stale_line {
                        s.metadata = None;
                    }
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
        let nipa_titles = self.table_titles(BeaDataset::Nipa).await;
        let regional_titles = self.table_titles(BeaDataset::Regional).await;

        let mut found = Vec::new();
        for table in tables {
            let titles = match table.dataset {
                BeaDataset::Nipa => &nipa_titles,
                BeaDataset::Regional => &regional_titles,
            };
            let (title, known) = title_for(titles, &table.table_name);
            // Discovery is all or nothing (see the module docs), so a table whose title isn't
            // known yet (refresh_reference_data never ran, or failed) fails discovery rather
            // than persisting the bare-table-name placeholder as every one of its series'
            // titles: `persist_discovered` writes `DiscoveredSeries::title` unconditionally, so
            // a degraded discovery would overwrite real stored titles on the very next fetch,
            // defeating the staleness protection `fetch_nipa`/`fetch_regional` rely on.
            if !known {
                return Err(CrawlError::Transient(format!(
                    "BEA {} {}: title not yet known; refresh_reference_data hasn't synced it",
                    table.dataset.api_name(),
                    table.table_name
                )));
            }
            let series = match table.dataset {
                BeaDataset::Nipa => self.discover_nipa(ctx, key, nipa, table, &title).await,
                BeaDataset::Regional => {
                    self.discover_regional(ctx, key, regional, table, &title)
                        .await
                }
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

    /// BEA's titles for every NIPA and Regional `TableName`, one [`CodeList`] per dataset.
    /// `request_url` carries the API key (`UserID`), since BEA's `GetParameterValues` needs one
    /// and it must never land in `url`, the `reference_file_cache` key (`HttpFetcher` redacts it
    /// from logs and errors regardless, the same as every other BEA request). No key, no lists:
    /// there is nothing this adapter could seed or refresh without one, same as every other BEA
    /// request ([`api_key`](Self::api_key)'s `Permanent` error when `BEA_API_KEY` isn't set).
    fn code_lists(&self, keys: &ApiKeys) -> Vec<CodeList> {
        let Some(key) = keys.bea.as_deref() else {
            return Vec::new();
        };
        [BeaDataset::Nipa, BeaDataset::Regional]
            .into_iter()
            .map(|dataset| CodeList {
                request_url: Some(format!(
                    "{}/?UserID={key}&method=GetParameterValues&DatasetName={}&ParameterName=TableName&ResultFormat=JSON",
                    self.base_url,
                    dataset.api_name(),
                )),
                ..CodeList::new(
                    table_titles_cache_key(dataset),
                    dataset.code(),
                    "table_name",
                    labels_only(parse_table_titles),
                )
            })
            .collect()
    }

    /// Fetches BEA's own titles for every NIPA and Regional `TableName` and merges them into
    /// `bea_tables.csv`'s curated tables' `table_name` dimension (see
    /// [`refresh_table_titles`](Self::refresh_table_titles)). No key: the same `Permanent` error
    /// [`api_key`](Self::api_key) always gives BEA requests without one, since
    /// [`code_lists`](Self::code_lists) returns nothing to refresh.
    async fn refresh_reference_data(&self, ctx: &CrawlCtx) -> Result<(), CrawlError> {
        let lists = self.code_lists(&ctx.keys);
        if lists.is_empty() {
            return Err(self.api_key(ctx).unwrap_err());
        }
        let mut errors = Vec::new();
        for list in &lists {
            let dataset = if list.dataset == NIPA_DATASET {
                BeaDataset::Nipa
            } else {
                BeaDataset::Regional
            };
            if let Err(e) = self.refresh_table_titles(ctx, list, dataset).await {
                errors.push(format!("{}: {e}", dataset.api_name()));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CrawlError::Transient(format!(
                "BEA table titles (used by {TABLES_FILE}'s curated tables): {}",
                errors.join("; ")
            )))
        }
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
        // Both maps are small clones of the in-process cache; reading them once up front is
        // simpler than scanning `groups` to fetch only the dataset(s) actually present.
        let nipa_titles = self.table_titles(BeaDataset::Nipa).await;
        let regional_titles = self.table_titles(BeaDataset::Regional).await;
        for group in groups.values() {
            let group_titles = match &group[0].1 {
                SeriesKey::Nipa { .. } => &nipa_titles,
                SeriesKey::Regional { .. } => &regional_titles,
            };
            match self.fetch_group(ctx, key, group_titles, group).await {
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
    title: &str,
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
        "{}{line} ({title}, {})",
        line_desc.trim(),
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

fn regional_title(title: &str, line_desc: &str, geo: &str) -> String {
    format!("{}, {} ({title})", line_desc.trim(), geo_name(geo))
}

fn regional_description(table_name: &str, line: &str, geo: &str) -> String {
    format!("BEA Regional table {table_name}, line {line}, area {geo}.")
}

fn nipa_metadata(
    table_name: &str,
    title: &str,
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
            title,
            frequency,
            &row.line_description,
            &row.line_number,
            repeated,
        ),
        description: Some(format!(
            "BEA NIPA table {table_name}, line {}{series_code}.",
            row.line_number
        )),
        units: units(row.metric_name.as_deref(), row.cl_unit.as_deref(), mult),
        frequency: Some(frequency.label().into()),
        seasonal_adjustment: nipa_seasonal_adjustment(frequency, row),
    })
}

fn nipa_series(
    table_name: &str,
    title: &str,
    frequency: Frequency,
    rows: &[&NipaRow],
    repeated: &HashSet<&str>,
    dataset: SeriesDataset,
) -> Result<FetchedSeries, CrawlError> {
    let first = rows[0];
    let repeated = repeated.contains(first.line_description.trim());
    let metadata = nipa_metadata(table_name, title, frequency, first, repeated)?;
    let points = points(
        rows.iter()
            .map(|r| (&r.time_period, &r.data_value, r.unit_mult.as_deref())),
    )?;
    Ok(FetchedSeries {
        metadata: Some(metadata),
        points,
        dataset,
        validators: None,
    })
}

fn regional_series(
    table_name: &str,
    title: &str,
    line_desc: &str,
    line: &str,
    geo: &str,
    rows: &[&RegionalRow],
    dataset: SeriesDataset,
) -> Result<FetchedSeries, CrawlError> {
    let first = rows[0];
    let mult = unit_mult(first.unit_mult.as_deref())?;
    let metadata = NewSeriesMetadataLite {
        title: regional_title(title, line_desc, first.geo_name.as_deref().unwrap_or(geo)),
        description: Some(regional_description(table_name, line, geo)),
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
        dataset,
        validators: None,
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

/// Wraps a reference-data DB error as [`CrawlError::Transient`] (retried on the next scheduled
/// discovery, same as any other reference-data fetch failure).
fn db_err(e: AppError) -> CrawlError {
    CrawlError::Transient(format!("BEA reference data: {e}"))
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

/// Checks a decoded `BEAAPI` envelope for an in-body error (top-level, or inside `Results` once
/// BEA's occasional one-element array is unwrapped) and returns the decoded `Results` value.
/// Shared by [`BeaAdapter::call`] and [`parse_table_titles`], which decodes its own conditional
/// GET response the same way.
fn decode_results(envelope: Envelope, method: &str) -> Result<serde_json::Value, CrawlError> {
    let api = envelope.beaapi;
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
    Ok(results)
}

/// Logical cache key for [`persist::reference_file_etag`]: `GetParameterValues` takes the key in
/// the query string, which must never be persisted, so this is a stable label, not the literal
/// request URL.
fn table_titles_cache_key(dataset: BeaDataset) -> &'static str {
    match dataset {
        BeaDataset::Nipa => "bea:GetParameterValues:NIPA:TableName",
        BeaDataset::Regional => "bea:GetParameterValues:Regional:TableName",
    }
}

/// `TableName` -> BEA's own title, from a `GetParameterValues(TableName)` response body.
/// Table names or titles BEA sent empty are dropped.
fn parse_table_titles(body: &str) -> Result<Vec<(String, String)>, CrawlError> {
    let envelope: Envelope = serde_json::from_str(body)
        .map_err(|e| CrawlError::Parse(format!("BEA GetParameterValues: {e}")))?;
    let results = decode_results(envelope, "GetParameterValues")?;
    let values: TableNameValues = serde_json::from_value(results).map_err(|e| {
        CrawlError::Parse(format!("BEA GetParameterValues: unexpected Results: {e}"))
    })?;
    Ok(values
        .param_value
        .into_iter()
        .filter_map(|v| {
            if v.table_name.is_empty() {
                return None;
            }
            let title = strip_table_number_prefix(&v.description).to_string();
            if title.is_empty() {
                return None;
            }
            Some((v.table_name, title))
        })
        .collect())
}

/// Strips a leading `Table 1.1.5.` or `Table 6.1D.` (NIPA's `TableName` descriptions repeat the
/// table's own number, with an occasional trailing letter such as `6.1D` or `7.2.5A`, before its
/// title): `nipa_title` already combines this title with the line description and frequency, so
/// the number would just be redundant clutter. A description with no such prefix (Regional's, or
/// a NIPA one BEA formats differently) passes through unchanged, as does one that's only a table
/// number with no title following it (filtered out by the caller as empty, same as BEA sending
/// no description at all).
fn strip_table_number_prefix(description: &str) -> &str {
    let Some(rest) = description.strip_prefix("Table ") else {
        return description;
    };
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && !c.is_ascii_uppercase())
        .unwrap_or(rest.len());
    let number = &rest[..end];
    if !number.contains('.') || !number.chars().any(|c| c.is_ascii_digit()) {
        return description;
    }
    rest[end..].trim_start_matches(['.', ' '])
}

/// `titles.get(table_name)`, or the bare table name as a placeholder if BEA hasn't published (or
/// [`BeaAdapter::refresh_table_titles`] hasn't yet fetched) a title for it. The `bool` is false
/// in the fallback case, so the caller can avoid overwriting a previously stored good title with
/// the placeholder (see the module docs).
fn title_for(titles: &HashMap<String, String>, table_name: &str) -> (String, bool) {
    match titles.get(table_name) {
        Some(title) => (title.clone(), true),
        None => (table_name.to_string(), false),
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

/// `GetParameterValues(DatasetName, ParameterName=TableName)`'s results: unlike
/// `GetParameterValuesFiltered`, its `ParamValue` rows are named after the parameter itself.
#[derive(Debug, Deserialize)]
struct TableNameValues {
    #[serde(rename = "ParamValue")]
    param_value: Vec<TableNameValue>,
}

/// NIPA's `GetParameterValues(ParameterName=TableName)` rows are named after the parameter
/// itself (`TableName`/`Description`); Regional's are the generic `Key`/`Desc` that every other
/// `GetParameterValuesFiltered`/`GetParameterValues` call on that dataset uses (confirmed against
/// `regional_sagdp2n_linecodes.json`'s real `LineCode` response) — accept either name for either
/// dataset rather than assuming NIPA's naming applies everywhere.
#[derive(Debug, Deserialize)]
struct TableNameValue {
    #[serde(rename = "TableName", alias = "Key")]
    table_name: String,
    #[serde(rename = "Description", alias = "Desc", default)]
    description: String,
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
    const NIPA_TABLE_TITLES: &str = include_str!("../../tests/fixtures/bea/nipa_table_titles.json");
    const REGIONAL_TABLE_TITLES: &str =
        include_str!("../../tests/fixtures/bea/regional_table_titles.json");

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

    /// Curated tables' titles, as if [`BeaAdapter::refresh_table_titles`] had already run
    /// (`discover`/`fetch_*` read only the in-process cache, never the network, for a title).
    fn adapter(mock: &MockSource) -> BeaAdapter {
        BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .with_table_titles(
                BeaDataset::Nipa,
                &[
                    ("T10101", "Real GDP, percent change from preceding period"),
                    ("T10105", "GDP, current dollars"),
                    ("T10106", "Real GDP, chained dollars"),
                    ("T20100", "Personal income and its disposition"),
                    ("T20600", "Personal income and its disposition"),
                    ("T20804", "PCE price indexes"),
                ],
            )
            .with_table_titles(
                BeaDataset::Regional,
                &[("SAGDP2N", "GDP by state, current dollars")],
            )
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

    /// `GetParameterValues(DatasetName, ParameterName=TableName)` for `dataset`.
    fn table_titles_route(dataset: &str) -> Route {
        Route::get("/")
            .query("UserID", TEST_API_KEY)
            .query("method", "GetParameterValues")
            .query("DatasetName", dataset)
            .query("ParameterName", "TableName")
            .query("ResultFormat", "JSON")
    }

    /// The [`CodeList`] `adapter.code_lists()` builds for `dataset`, with `TEST_API_KEY`.
    fn table_titles_code_list(adapter: &BeaAdapter, dataset: BeaDataset) -> CodeList {
        let keys = ApiKeys {
            bea: Some(TEST_API_KEY.to_string()),
            ..ApiKeys::default()
        };
        adapter
            .code_lists(&keys)
            .into_iter()
            .find(|list| {
                list.dataset
                    == match dataset {
                        BeaDataset::Nipa => NIPA_DATASET,
                        BeaDataset::Regional => REGIONAL_DATASET,
                    }
            })
            .unwrap()
    }

    /// `code_lists()`'s `url` (the `reference_file_cache` key and seed-migration identity) never
    /// carries the `UserID` key that `request_url` needs, and neither does a seed recorded from
    /// it: not `SeedEntry::url`, nor the generated migration SQL.
    #[tokio::test]
    async fn code_lists_never_puts_the_api_key_in_the_seed() {
        let mock = MockSource::start().await;
        mock.mount(
            &table_titles_route("NIPA"),
            Reply::json_str(NIPA_TABLE_TITLES),
        )
        .await;
        let adapter = BeaAdapter::new(mock.base_url());
        let list = table_titles_code_list(&adapter, BeaDataset::Nipa);
        assert!(!list.url.contains(TEST_API_KEY), "{}", list.url);
        assert!(list.request_url().contains(TEST_API_KEY));

        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog
            .insert(
                SourceId::Bea,
                &[NIPA_DATASET, REGIONAL_DATASET],
                crate::dataset::parse_dataset_file(
                    &std::fs::read_to_string(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("data/datasets/bea.toml"),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        let download = download_seed_entries(&test_ctx().http, SourceId::Bea, &[list], &catalog)
            .await
            .unwrap();
        assert!(download.failures.is_empty(), "{:?}", download.failures);
        assert!(!download.entries[0].url.contains(TEST_API_KEY));

        let at = "2026-10-02T03:00:00Z".parse().unwrap();
        let (up, down) = seed_migration_sql(SourceId::Bea, at, &download.entries).unwrap();
        assert!(!up.contains(TEST_API_KEY), "{up}");
        assert!(!down.contains(TEST_API_KEY), "{down}");
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
        let header = "dataset,table_name,frequencies\n";
        let ok = parse_tables(&format!("# c\n{header}NIPA,T1,A Q\n")).unwrap();
        assert_eq!(ok[0].table_name, "T1");
        for (body, err) in [
            ("", "no header"),
            ("x,y\n", "expected header"),
            (header, "no tables"),
            ("NIPA,T1\n", "expected dataset"),
            ("ITA,T1,A\n", "unknown dataset"),
            ("NIPA,T-1,A\n", "not alphanumeric"),
            ("NIPA,T1,A\nNIPA,T1,Q\n", "listed twice"),
            ("NIPA,T1,W\n", "unknown frequency"),
            ("NIPA,T1, \n", "no frequency"),
            ("Regional,S1,Q\n", "must be annual"),
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
            SeriesDataset::new(
                NIPA_DATASET,
                [
                    ("table_name", "T10105"),
                    ("line_number", "1"),
                    ("frequency", "Q")
                ]
            )
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

    /// A curated table whose title the in-process cache doesn't have yet (no
    /// `refresh_table_titles` has run in this process) fails discovery rather than writing the
    /// bare table name as every one of its series' titles: that placeholder would then overwrite
    /// a good stored title on the very next fetch, since `persist_discovered` writes
    /// `DiscoveredSeries::title` unconditionally.
    #[tokio::test]
    async fn discover_fails_when_a_titles_cache_is_empty() {
        let mock = MockSource::start().await;
        // No .with_table_titles(...): titles are exactly as they'd be in a fresh process whose
        // refresh_reference_data hasn't run (or failed) yet.
        let e = BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
        assert!(e.to_string().contains("title not yet known"), "{e}");
        // The empty cache fails discovery before it ever asks BEA for anything.
        assert!(mock.received_requests().await.is_empty());
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
            s.dataset,
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
        assert_eq!(&ca.dataset, &regional_dimensions("SAGDP2N", "1", "06000"));
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

    /// A table whose title isn't in the in-process cache (BEA stopped listing it, or this
    /// process hasn't refreshed titles since its series were discovered) still fetches real
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
        assert_eq!(a.refresh_reference_data(&ctx).await.unwrap_err(), expected);
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

    /// `refresh_table_titles` fetches `GetParameterValues(TableName)`, merges the titles into
    /// the dataset's `table_name` dimension, caches the `ETag`, and fills the in-process cache
    /// `table_titles` then reads straight from (no second DB round trip needed in the same
    /// process). A second call that gets a `304` reloads the same titles from the database
    /// instead of trusting memory, so a fresh process picks up another instance's last merge.
    #[tokio::test]
    async fn refresh_table_titles_merges_labels_caches_etag_and_fills_in_process_cache() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_refresh_table_titles",
        )
        .await;
        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog
            .insert(
                SourceId::Bea,
                &[NIPA_DATASET, REGIONAL_DATASET],
                crate::dataset::parse_dataset_file(
                    &std::fs::read_to_string(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("data/datasets/bea.toml"),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        crate::persist::sync_datasets(&db.pool, &catalog)
            .await
            .unwrap();

        // `mount_expect`'s drop-time check only covers the last mount still standing when the
        // mock server drops: `reset()` below clears it along with everything mounted before, so
        // only the final (Regional) `mount_expect` actually gets verified that way. The NIPA 200
        // and 304 legs are still proven correct, just by their own assertions: a stray extra
        // request to the wrong fixture would 404 (the route wouldn't match) and `unwrap()` would
        // fail, and the 304 leg's title can only come from the database reload, never the mock.
        let mock = MockSource::start().await;
        mock.mount_expect(
            &table_titles_route("NIPA"),
            Reply::json_str(NIPA_TABLE_TITLES).header("ETag", "\"v1\""),
            1,
        )
        .await;
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let adapter = BeaAdapter::new(mock.base_url());
        let nipa_list = table_titles_code_list(&adapter, BeaDataset::Nipa);
        adapter
            .refresh_table_titles(&ctx, &nipa_list, BeaDataset::Nipa)
            .await
            .unwrap();

        assert_eq!(
            adapter.table_titles(BeaDataset::Nipa).await.get("T10105"),
            Some(&"Gross Domestic Product".to_string())
        );
        let cache_key = table_titles_cache_key(BeaDataset::Nipa);
        assert_eq!(
            persist::reference_file_etag(&db.pool, SourceId::Bea, cache_key)
                .await
                .unwrap(),
            Some("\"v1\"".to_string())
        );
        let labels =
            persist::dataset_dimension_labels(&db.pool, SourceId::Bea, NIPA_DATASET, "table_name")
                .await
                .unwrap();
        assert_eq!(
            labels.get("T10105"),
            Some(&"Gross Domestic Product".to_string())
        );

        // A fresh adapter instance (no in-process cache), a `304` this time: it reloads the
        // titles already merged into the database rather than ending up with nothing. `reset`
        // first: `mount` matches in mount order, so without it the still-mounted 200 fixture
        // above would answer this request too and the 304 path would go untested.
        mock.reset().await;
        mock.mount_expect(
            &table_titles_route("NIPA"),
            Reply::status(304).header("ETag", "\"v1\""),
            1,
        )
        .await;
        let fresh = BeaAdapter::new(mock.base_url());
        let fresh_nipa_list = table_titles_code_list(&fresh, BeaDataset::Nipa);
        fresh
            .refresh_table_titles(&ctx, &fresh_nipa_list, BeaDataset::Nipa)
            .await
            .unwrap();
        assert_eq!(
            fresh.table_titles(BeaDataset::Nipa).await.get("T10105"),
            Some(&"Gross Domestic Product".to_string())
        );
        // Conditional on the ETag cached from the first fetch: proves the 304 above was actually
        // exercising the conditional-GET path, not just a mock that would have answered 304
        // regardless of what was asked.
        assert_eq!(
            mock.received_requests()
                .await
                .last()
                .unwrap()
                .headers
                .get("If-None-Match")
                .unwrap(),
            "\"v1\"",
        );

        // Regional works the same, with its own cache key and dimension row.
        mock.reset().await;
        mock.mount_expect(
            &table_titles_route("Regional"),
            Reply::json_str(REGIONAL_TABLE_TITLES),
            1,
        )
        .await;
        let regional_list = table_titles_code_list(&adapter, BeaDataset::Regional);
        adapter
            .refresh_table_titles(&ctx, &regional_list, BeaDataset::Regional)
            .await
            .unwrap();
        assert_eq!(
            adapter
                .table_titles(BeaDataset::Regional)
                .await
                .get("SAGDP2N"),
            Some(&"SAGDP2N Gross domestic product (GDP) by state".to_string())
        );

        db.drop().await;
    }

    /// A BEA error envelope for the title fetch classifies like any other BEA error and doesn't
    /// cache an `ETag` (there's nothing to treat as "unchanged" next time), so the next refresh
    /// retries rather than silently keeping an empty title cache.
    #[tokio::test]
    async fn refresh_table_titles_fails_and_does_not_cache_etag_on_bea_error() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_refresh_table_titles_error",
        )
        .await;

        let mock = MockSource::start().await;
        mock.mount(
            &table_titles_route("NIPA"),
            Reply::json_str(
                r#"{"BEAAPI":{"Results":{"Error":{"APIErrorCode":"1","APIErrorDescription":"Unknown error."}}}}"#,
            ),
        )
        .await;
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let adapter = BeaAdapter::new(mock.base_url());
        let nipa_list = table_titles_code_list(&adapter, BeaDataset::Nipa);
        let e = adapter
            .refresh_table_titles(&ctx, &nipa_list, BeaDataset::Nipa)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");

        assert_eq!(
            persist::reference_file_etag(
                &db.pool,
                SourceId::Bea,
                table_titles_cache_key(BeaDataset::Nipa)
            )
            .await
            .unwrap(),
            None
        );
        assert!(adapter.table_titles(BeaDataset::Nipa).await.is_empty());

        db.drop().await;
    }

    /// A refresh that fails outright (BEA down, rate limited, a bad body) still reloads whatever
    /// titles an earlier successful merge already put in the database into the in-process cache,
    /// rather than leaving it empty: a worker that starts while BEA is having a bad day should
    /// still be able to `discover` using titles a previous process (or an earlier refresh in this
    /// one) already stored, not fail every table with "title not yet known".
    #[tokio::test]
    async fn refresh_table_titles_fails_but_still_loads_titles_already_in_the_database() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_refresh_table_titles_error_with_stored_titles",
        )
        .await;
        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog
            .insert(
                SourceId::Bea,
                &[NIPA_DATASET, REGIONAL_DATASET],
                crate::dataset::parse_dataset_file(
                    &std::fs::read_to_string(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("data/datasets/bea.toml"),
                    )
                    .unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        crate::persist::sync_datasets(&db.pool, &catalog)
            .await
            .unwrap();
        // A prior successful merge, as if an earlier process (or an earlier refresh in this one)
        // had already stored titles.
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Bea,
            NIPA_DATASET,
            "table_name",
            &[("T10105".to_string(), "Gross Domestic Product".to_string())],
        )
        .await
        .unwrap();

        let mock = MockSource::start().await;
        mock.mount(
            &table_titles_route("NIPA"),
            Reply::json_str(
                r#"{"BEAAPI":{"Results":{"Error":{"APIErrorCode":"1","APIErrorDescription":"Unknown error."}}}}"#,
            ),
        )
        .await;
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let adapter = BeaAdapter::new(mock.base_url());
        let nipa_list = table_titles_code_list(&adapter, BeaDataset::Nipa);
        let e = adapter
            .refresh_table_titles(&ctx, &nipa_list, BeaDataset::Nipa)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
        assert_eq!(
            adapter.table_titles(BeaDataset::Nipa).await.get("T10105"),
            Some(&"Gross Domestic Product".to_string()),
            "a failed refresh should still have loaded the titles already stored in the database"
        );

        db.drop().await;
    }

    #[test]
    fn strip_table_number_prefix_removes_the_table_number_but_leaves_other_descriptions_alone() {
        assert_eq!(
            strip_table_number_prefix("Table 1.1.5. Gross Domestic Product"),
            "Gross Domestic Product"
        );
        assert_eq!(
            strip_table_number_prefix("Table 2.8.4. Price Indexes for PCE"),
            "Price Indexes for PCE"
        );
        // No table-number prefix (Regional's descriptions, or an unexpected NIPA shape): left
        // exactly as BEA sent it.
        assert_eq!(
            strip_table_number_prefix("SAGDP2N Gross domestic product (GDP) by state"),
            "SAGDP2N Gross domestic product (GDP) by state"
        );
        assert_eq!(
            strip_table_number_prefix("GDP, current dollars"),
            "GDP, current dollars"
        );
        // "Table " followed by something that isn't a dotted table number: left alone too.
        assert_eq!(
            strip_table_number_prefix("Table of contents"),
            "Table of contents"
        );
        // A trailing letter on the table number (BEA really has these: 6.1D, 7.2.5A/B) is part
        // of the number, not the title.
        assert_eq!(
            strip_table_number_prefix(
                "Table 6.1D. National Income Without Capital Consumption Adjustment by Industry"
            ),
            "National Income Without Capital Consumption Adjustment by Industry"
        );
        assert_eq!(
            strip_table_number_prefix("Table 7.2.5A. Auto Output"),
            "Auto Output"
        );
        // Nothing but a table number: strips to empty (the caller filters this out the same as
        // an empty description).
        assert_eq!(strip_table_number_prefix("Table 1.1.5."), "");
    }

    #[test]
    fn parse_table_titles_drops_empty_rows_and_unwraps_a_one_element_results_array() {
        let titles = parse_table_titles(
            r#"{"BEAAPI":{"Results":[{"ParamValue":[
                {"TableName": "T10105", "Description": "GDP, current dollars"},
                {"TableName": "", "Description": "no table name"},
                {"TableName": "T99999", "Description": ""}
            ]}]}}"#,
        )
        .unwrap();
        assert_eq!(
            titles,
            vec![("T10105".to_string(), "GDP, current dollars".to_string())]
        );
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
