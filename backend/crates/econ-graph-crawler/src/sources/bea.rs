// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! BEA (Bureau of Economic Analysis) adapter: NIPA and Regional table families selected from
//! BEA's current upstream catalog, one series per table line and area/frequency.
//!
//! `catalog_selection/bea.toml` supplies dataset, table-prefix/title rules and frequencies.
//! A renamed or newly published table in a selected family needs no curated-id update. Selection
//! has explicit table and expanded-series limits; exceeding either fails the whole discovery
//! before any series are persisted or enqueued.
//!
//! `GetParameterValues(TableName)` is refreshed before discovery. Exact membership is stored in
//! immutable, body-hash keyed shared snapshots, so conditional responses and other worker
//! processes cannot reuse a stale in-memory catalog. The accumulated dataset dimension labels
//! retain the last known titles of tables the source removed; they never decide membership.
//!
//! # Datasets and ids
//!
//! Two datasets, defined in `datasets/bea.toml` (see [`crate::dataset`]):
//!
//! - [`NIPA_DATASET`] (`bea_nipa`), dimensions `table_name`, `line_number` and `frequency` (BEA's
//!   `A`, `Q` or `M`): `bea_nipa/T10105.1.Q` is quarterly GDP in current dollars.
//! - [`REGIONAL_DATASET`] (`bea_regional`), dimensions `table_name`, `line_code` and `geo_fips`
//!   (BEA's five-digit area code: `00000` for the United States, `SS000` for a state, `9R000` for
//!   a BEA region): `bea_regional/SAGDP2.1.06000` is California's GDP.
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
//! Discovery is all or nothing: failed/empty catalog refreshes, failed table requests or empty
//! table/frequency responses fail the discovery. Only a complete successful selected catalog can
//! supply lifecycle absence evidence. Missing series keep their stored history and titles; the
//! shared lifecycle reconciliation requires repeated absence and guards mass disappearance.
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
//! history is still one modest request. A requested series with no rows in a successful
//! response is `Absent`, not `NotFound`: omitting a line or area is weaker evidence than an
//! explicit "does not exist". That includes an empty response: a Regional request is filtered
//! to the batch's areas, so it is empty when every requested area is gone (legacy counties),
//! and BEA answers a NIPA table it no longer publishes at that frequency with no rows. A
//! briefly empty table during a BEA update cannot retire anything: six misses about ten
//! refresh intervals apart are needed, and listed series are not counted.
//! It isn't retried (each retry would request the whole table again); the scheduler tries again
//! after the same long backoff as NotFound. For a series the current accepted catalog lists,
//! `Absent` is not counted at all: catalog discovery alone decides whether it was removed. For
//! an unlisted or legacy series, six consecutive `Absent` results retire it from fetching (see
//! [`lifecycle`](crate::lifecycle)), and it is then probed on a slow schedule.
//!
//! `DataValue` has thousands separators and is scaled by `10^UNIT_MULT`, so values are in base
//! units (dollars, not millions of dollars). Units come from `CL_UNIT`, or `METRIC_NAME` when
//! `CL_UNIT` is `Level`, with a `Thousands of` / `Millions of` / `Billions of` prefix matching
//! `UNIT_MULT` removed. BEA's markers for a missing value (`(D)` suppressed, `(NA)`, `(NM)`,
//! `---` and the like: anything without a digit) become points with no value. `TimePeriod` is
//! `2024`, `2024Q3` or `2024M07`; the point date is the period's first day.
//!
//! A table whose title isn't known (BEA no longer lists it in
//! `GetParameterValues(TableName)`, or `refresh_table_titles` hasn't synced it this process) is
//! never discovered with a placeholder title that would overwrite every one of its series' stored
//! titles: discovery skips it or fails (see Discovery above). A fetch for such a table (discovery
//! ran earlier, successfully, before the title went stale) still returns its points but with no
//! metadata, so [`persist_series`](crate::persist::persist_series)'s COALESCE keeps the series'
//! already-stored title instead of overwriting it with the table name.
//!
//! `NoteRef` is dropped: there is no place for observation attributes, and the notes are table
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

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, Utc};
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::adapter::{
    ApiKeys, BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries,
    NewSeriesMetadataLite, SourceAdapter,
};
use crate::catalog_selection::{bea_rules, BeaSelection};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::http::scrub_error;
use crate::persist;
#[cfg(test)]
use crate::reference_file::{download_seed_entries, seed_migration_sql};
use crate::reference_file::{labels_only, CodeList};
use crate::source::SourceId;
use crate::units::{Scale, StructuredUnit, UnitsContext};
use crate::value::{check_observation_value, parse_observation_value};
use econ_graph_core::error::AppError;

/// The real BEA API root.
pub const DEFAULT_BASE_URL: &str = "https://apps.bea.gov/api/data";

/// Dataset code of NIPA table lines.
pub const NIPA_DATASET: &str = "bea_nipa";

/// Dataset code of Regional table lines by area.
pub const REGIONAL_DATASET: &str = "bea_regional";

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

    /// The dataset whose [`code`](Self::code) is `code`.
    pub fn from_code(code: &str) -> Option<Self> {
        [BeaDataset::Nipa, BeaDataset::Regional]
            .into_iter()
            .find(|d| d.code() == code)
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

/// A table selected from the current upstream catalog and its configured frequencies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeaTable {
    pub dataset: BeaDataset,
    pub table_name: String,
    pub frequencies: Vec<Frequency>,
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

/// What discovery's error says when BEA serves no rows (NIPA) or no line codes or areas
/// (Regional) for a table it lists.
const EMPTY_TABLE: &str = "BEA serves no data for this listed table";

/// Whether `error` is discovery finding a listed table empty (see [`EMPTY_TABLE`]).
pub(crate) fn is_empty_table(error: &CrawlError) -> bool {
    matches!(error, CrawlError::Permanent(message) if message.contains(EMPTY_TABLE))
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
    /// cached (see the module docs); `discover` instead skips such a table or fails (see
    /// [`listed`](Self::listed)), rather than persisting the bare table name as a real title.
    /// Labels merged into the database accumulate, so a table BEA has dropped can keep its last
    /// known title here.
    titles: tokio::sync::RwLock<HashMap<BeaDataset, HashMap<String, String>>>,
    /// Exact names from each dataset's most recent successful current-catalog refresh. Never
    /// populated from additive historical labels; cleared when its refresh fails.
    listed: tokio::sync::RwLock<HashMap<BeaDataset, HashSet<String>>>,
    selection: Option<BeaSelection>,
    /// Series-id prefixes of tables the last successful discovery saw listed upstream but
    /// did not select: their tracked series are still published (see
    /// [`CatalogLifecycle::unselected_prefixes`](crate::lifecycle::CatalogLifecycle::unselected_prefixes)).
    unselected_prefixes: std::sync::Mutex<Vec<String>>,
}

impl BeaAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            current_year: None,
            titles: tokio::sync::RwLock::new(HashMap::new()),
            listed: tokio::sync::RwLock::new(HashMap::new()),
            selection: None,
            unselected_prefixes: std::sync::Mutex::default(),
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

    /// Marks `dataset`'s cached titles as exactly what a successful
    /// [`refresh_table_titles`](Self::refresh_table_titles) listed, bypassing BEA and the
    /// database.
    #[cfg(test)]
    fn with_refreshed_titles(self, dataset: BeaDataset) -> Self {
        let current: HashSet<String> = self
            .titles
            .try_read()
            .unwrap()
            .get(&dataset)
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default();
        let mut listed = self.listed.into_inner();
        listed.insert(dataset, current);
        Self {
            listed: tokio::sync::RwLock::new(listed),
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
        // BEA's in-body error text and serde's quoted values come from the response body: scrub
        // the key from them in case BEA echoes it.
        let secret = [key.to_owned()];
        let results = decode_results(body, method).map_err(|e| scrub_error(e, &secret))?;
        serde_json::from_value(results).map_err(|e| {
            scrub_error(
                CrawlError::Parse(format!("BEA {method}: unexpected Results: {e}")),
                &secret,
            )
        })
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

    /// Refresh exact current membership through the versioned shared catalog snapshot. Historic
    /// dimension labels stay additive, so old tables retain titles without staying selected.
    async fn refresh_table_titles(
        &self,
        ctx: &CrawlCtx,
        list: &CodeList,
        dataset: BeaDataset,
    ) -> Result<(), CrawlError> {
        let refreshed = crate::catalog_reference::current_codes(ctx, SourceId::Bea, list).await;
        match &refreshed {
            Ok(codes) => {
                self.listed
                    .write()
                    .await
                    .insert(dataset, codes.iter().map(|c| c.code.clone()).collect());
            }
            Err(error) => {
                self.listed.write().await.remove(&dataset);
                if error.kind() == "parse" {
                    ctx.http
                        .record_response_error(SourceId::Bea, list.request_url(), error);
                }
            }
        }
        // Fetches can keep serving stored history even when current catalog discovery failed.
        let from_db = persist::dataset_dimension_labels(
            &ctx.pool,
            SourceId::Bea,
            dataset.code(),
            "table_name",
        )
        .await
        .map_err(db_err)?;
        if !from_db.is_empty() {
            self.titles.write().await.insert(dataset, from_db);
        }
        refreshed?;
        Ok(())
    }

    /// The selection rules: the ones a test gave with `with_selection`, else
    /// `catalog_selection/bea.toml`.
    pub(crate) fn selection(&self) -> Result<&BeaSelection, CrawlError> {
        match self.selection.as_ref() {
            Some(selection) => Ok(selection),
            None => bea_rules(),
        }
    }

    /// Uses `selection` instead of `catalog_selection/bea.toml`.
    #[cfg(test)]
    pub(crate) fn with_selection(mut self, selection: BeaSelection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// The `TableName`s BEA listed in `dataset`'s last successful refresh, or `None` when the
    /// last refresh failed or none has run (see [`listed`](Self::listed)).
    async fn listed_tables(&self, dataset: BeaDataset) -> Option<HashSet<String>> {
        self.listed.read().await.get(&dataset).cloned()
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

    /// Positively enroll existing histories matching the current configured selection scope.
    /// This also finds a table renamed before the first catalog-driven crawl. It never treats
    /// every BEA series as selected, and a title predicate requires a retained upstream title.
    async fn stored_scope_members(&self, ctx: &CrawlCtx) -> Result<Vec<String>, CrawlError> {
        use diesel::sql_types::{Array, Text, Uuid as SqlUuid};
        use diesel_async::RunQueryDsl;
        use futures::TryStreamExt;

        #[derive(diesel::QueryableByName)]
        struct StoredId {
            #[diesel(sql_type = Text)]
            external_id: String,
        }

        let rules = self.selection()?;
        let mut titles = HashMap::new();
        for dataset in [BeaDataset::Nipa, BeaDataset::Regional] {
            titles.insert(
                dataset,
                persist::dataset_dimension_labels(
                    &ctx.pool,
                    SourceId::Bea,
                    dataset.code(),
                    "table_name",
                )
                .await
                .map_err(db_err)?,
            );
        }
        let source_id = persist::data_source_id(&ctx.pool, SourceId::Bea)
            .await
            .map_err(db_err)?;
        // Prefixes are validated alphanumeric, so these SQL LIKE patterns contain only the
        // intended suffix wildcard. Apply them before loading histories from either store.
        let patterns: Vec<String> = rules
            .rule
            .iter()
            .map(|rule| {
                let code = if rule.dataset == "NIPA" {
                    NIPA_DATASET
                } else {
                    REGIONAL_DATASET
                };
                format!("{}/{}%", code.replace('_', "\\_"), rule.prefix)
            })
            .collect();
        let mut conn = ctx
            .pool
            .get()
            .await
            .map_err(|e| CrawlError::Transient(format!("BEA scope membership: {e}")))?;
        let query = diesel::sql_query(
            "SELECT external_id FROM economic_series WHERE source_id = $1 AND external_id LIKE ANY($2) \
             UNION SELECT external_id FROM series_metadata WHERE source_id = $1 AND external_id LIKE ANY($2) \
             ORDER BY external_id"
        ).bind::<SqlUuid, _>(source_id).bind::<Array<Text>, _>(patterns);
        let mut rows = query
            .load_stream::<StoredId>(&mut conn)
            .await
            .map_err(|e| CrawlError::Transient(format!("BEA scope membership: {e}")))?;
        let mut members = Vec::new();
        while let Some(row) = rows
            .try_next()
            .await
            .map_err(|e| CrawlError::Transient(format!("BEA scope membership: {e}")))?
        {
            let Some(key) = SeriesKey::parse(&row.external_id) else {
                continue;
            };
            let (dataset, table, frequency) = match &key {
                SeriesKey::Nipa {
                    table, frequency, ..
                } => (BeaDataset::Nipa, table, *frequency),
                SeriesKey::Regional { table, geo, .. } if is_state_level_geo(geo) => {
                    (BeaDataset::Regional, table, Frequency::Annual)
                }
                SeriesKey::Regional { .. } => continue,
            };
            let title = titles[&dataset].get(table).map(String::as_str);
            if rules.includes_stored_series(dataset, table, frequency, title) {
                rules.check_series_count(members.len().saturating_add(1))?;
                members.push(row.external_id);
            }
        }
        Ok(members)
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
                    "BEA NIPA {} ({}): {EMPTY_TABLE}; check the upstream catalog and catalog_selection/bea.toml",
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
                self.selection()?
                    .check_series_count(found.len().saturating_add(1))?;
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
                "BEA Regional {}: {EMPTY_TABLE}; check the upstream catalog and catalog_selection/bea.toml",
                table.table_name
            )));
        }
        let count = lines.len().saturating_mul(geos.len());
        self.selection()?.check_series_count(count)?;
        let mut found = Vec::with_capacity(count);
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

    /// The series one selected table publishes now: `GetData` per frequency for a NIPA table,
    /// its line codes and areas for a Regional one (see [Discovery](self#discovery)). `title`
    /// is the table's current upstream title. Reads and writes no database.
    pub(crate) async fn discover_table(
        &self,
        ctx: &CrawlCtx,
        table: &BeaTable,
        title: &str,
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let key = self.api_key(ctx)?;
        let code = table.dataset.code();
        let def = crate::reference::datasets(SourceId::Bea)?
            .iter()
            .find(|d| d.code == code)
            .ok_or_else(|| {
                CrawlError::Permanent(format!("BEA dataset {code} has no definition"))
            })?;
        match table.dataset {
            BeaDataset::Nipa => self.discover_nipa(ctx, key, def, table, title).await,
            BeaDataset::Regional => self.discover_regional(ctx, key, def, table, title).await,
        }
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
                // Weaker than NotFound (see the module docs).
                None => Err(CrawlError::Absent(format!(
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
                // Weaker than NotFound (see the module docs).
                None => Err(CrawlError::Absent(format!(
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
        self.api_key(ctx)?;
        let rules = self.selection()?;
        let mut current_catalogs = HashMap::new();
        for dataset in [BeaDataset::Nipa, BeaDataset::Regional] {
            let listed = self.listed_tables(dataset).await.ok_or_else(|| CrawlError::Transient(format!(
                "BEA {}: current table catalog unavailable; title not yet known; refresh_reference_data must succeed", dataset.api_name()
            )))?;
            let titles = self.table_titles(dataset).await;
            let mut current = HashMap::new();
            for name in listed {
                let title = titles.get(&name).ok_or_else(|| {
                    CrawlError::Transient(format!(
                        "BEA {} {name}: title not yet known",
                        dataset.api_name()
                    ))
                })?;
                current.insert(name, title.clone());
            }
            current_catalogs.insert(dataset, current);
        }
        let tables = rules.select(&current_catalogs)?;
        let selected: HashSet<(BeaDataset, &str)> = tables
            .iter()
            .map(|t| (t.dataset, t.table_name.as_str()))
            .collect();
        let mut unselected_prefixes: Vec<String> = current_catalogs
            .iter()
            .flat_map(|(dataset, catalog)| {
                catalog
                    .keys()
                    .filter(|name| !selected.contains(&(*dataset, name.as_str())))
                    .map(|name| format!("{}/{name}.", dataset.code()))
            })
            .collect();
        unselected_prefixes.sort_unstable();
        *self
            .unselected_prefixes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = unselected_prefixes;
        let mut found = Vec::new();
        for table in &tables {
            let title = &current_catalogs[&table.dataset][&table.table_name];
            let series = self.discover_table(ctx, table, title).await?;
            rules.check_series_count(found.len().saturating_add(series.len()))?;
            found.extend(series);
        }
        tracing::info!(
            tables = tables.len(),
            series = found.len(),
            "BEA catalog-driven discovery finished"
        );
        Ok(found)
    }

    /// Successful discovery is a complete listing of the configured selection scope.
    fn discovery_is_complete(&self) -> bool {
        true
    }

    fn tracks_catalog_lifecycle(&self) -> bool {
        true
    }

    fn catalog_scope(&self) -> Option<String> {
        self.selection().ok().map(BeaSelection::fingerprint)
    }

    fn catalog_lifecycle(&self) -> crate::lifecycle::CatalogLifecycle {
        crate::lifecycle::CatalogLifecycle {
            unselected_prefixes: self
                .unselected_prefixes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            ..Default::default()
        }
    }

    async fn catalog_scope_members(&self, ctx: &CrawlCtx) -> Result<Vec<String>, CrawlError> {
        self.stored_scope_members(ctx).await
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
    /// each dataset's additive `table_name` dimension (see
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
            let dataset = BeaDataset::from_code(list.dataset).unwrap_or(BeaDataset::Regional);
            if let Err(e) = self.refresh_table_titles(ctx, list, dataset).await {
                errors.push(format!("{}: {e}", dataset.api_name()));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(CrawlError::Transient(format!(
                "BEA table catalog refresh: {}",
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

/// `[SAGDP2] All industry total` -> `All industry total`.
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
/// gets its line: `Goods, line 17 (GDP, current dollars, quarterly)`. A line that only restates
/// its table's title leaves the title out: `Gross domestic product (quarterly)`, not
/// `Gross domestic product (Gross Domestic Product, quarterly)`.
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
    let line_desc = line_desc.trim();
    let frequency = frequency.label().to_lowercase();
    if line_desc.eq_ignore_ascii_case(title.trim()) {
        format!("{line_desc}{line} ({frequency})")
    } else {
        format!("{line_desc}{line} ({title}, {frequency})")
    }
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
        units_raw: unit_text(row.metric_name.as_deref(), row.cl_unit.as_deref()).map(Into::into),
        structured_unit: Some(Box::new(scaled_values())),
        units_context: UnitsContext::for_source(SourceId::Bea),
        frequency: Some(frequency.label().into()),
        seasonal_adjustment: nipa_seasonal_adjustment(frequency, row),
        ..NewSeriesMetadataLite::default()
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
        units_raw: unit_text(None, first.cl_unit.as_deref()).map(Into::into),
        structured_unit: Some(Box::new(scaled_values())),
        units_context: UnitsContext::for_source(SourceId::Bea),
        frequency: Some(Frequency::Annual.label().into()),
        seasonal_adjustment: None,
        ..NewSeriesMetadataLite::default()
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
            attributes: None,
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
    let value = parse_observation_value(&v.replace(',', ""))
        .map_err(|e| CrawlError::Parse(format!("BEA: invalid DataValue {raw:?}: {e}")))?;
    // Exact: a BigDecimal product keeps every digit of both factors.
    let scaled = value * BigDecimal::from(10u64.pow(mult));
    check_observation_value(&scaled).map_err(|e| {
        CrawlError::Parse(format!("BEA: DataValue {raw:?} scaled by 10^{mult}: {e}"))
    })?;
    Ok(Some(scaled))
}

/// BEA's units text as given: `CL_UNIT`, or `METRIC_NAME` when `CL_UNIT` is `Level` or absent.
fn unit_text<'a>(metric_name: Option<&'a str>, cl_unit: Option<&'a str>) -> Option<&'a str> {
    match cl_unit.map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) if !u.eq_ignore_ascii_case("level") => Some(u),
        _ => metric_name.map(str::trim).filter(|m| !m.is_empty()),
    }
}

/// What BEA's structured fields say about a stored series: [`parse_value`] multiplies every
/// value by `10^UNIT_MULT`, so the values are in base units (dollars, not millions of dollars),
/// whatever the text says.
fn scaled_values() -> StructuredUnit {
    StructuredUnit {
        scale: Some(Scale::Units),
        ..StructuredUnit::default()
    }
}

/// The units of a scaled value: `CL_UNIT`, or `METRIC_NAME` when `CL_UNIT` is `Level`, with a
/// `Thousands of` / `Millions of` / `Billions of` prefix that `mult` already applied removed.
fn units(metric_name: Option<&str>, cl_unit: Option<&str>, mult: u32) -> Option<String> {
    let base = unit_text(metric_name, cl_unit)?;
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
/// `regional_sagdp2_linecodes.json`'s real `LineCode` response) — accept either name for either
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
    use std::str::FromStr;

    use super::*;
    use crate::testkit::contract::assert_discover_ok;
    use crate::testkit::{test_ctx, MockSource, Reply, Route, TEST_API_KEY};

    const BAD_KEY: &str = include_str!("../../tests/fixtures/bea/error_invalid_userid.json");
    const GDP_Q: &str = include_str!("../../tests/fixtures/bea/nipa_t10105_q.json");
    const REGIONAL_LINE1: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2_line1.json");
    const REGIONAL_LINES: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2_linecodes.json");
    const REGIONAL_GEOS: &str =
        include_str!("../../tests/fixtures/bea/regional_sagdp2_geofips.json");
    const NIPA_TABLE_TITLES: &str = include_str!("../../tests/fixtures/bea/nipa_table_titles.json");
    const REGIONAL_TABLE_TITLES: &str =
        include_str!("../../tests/fixtures/bea/regional_table_titles.json");

    /// Every default NIPA table family and frequency, with its fixture.
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

    /// The curated NIPA tables' titles.
    const NIPA_TEST_TITLES: &[(&str, &str)] = &[
        ("T10101", "Real GDP, percent change from preceding period"),
        ("T10105", "GDP, current dollars"),
        ("T10106", "Real GDP, chained dollars"),
        ("T20100", "Personal income and its disposition"),
        ("T20600", "Personal income and its disposition"),
        ("T20804", "PCE price indexes"),
    ];

    /// Curated tables' titles, as if [`BeaAdapter::refresh_table_titles`] had already run
    /// (`discover`/`fetch_*` read only the in-process cache, never the network, for a title).
    fn adapter(mock: &MockSource) -> BeaAdapter {
        BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
            .with_table_titles(
                BeaDataset::Regional,
                &[("SAGDP2", "Gross domestic product (GDP) by state")],
            )
            .with_refreshed_titles(BeaDataset::Nipa)
            .with_refreshed_titles(BeaDataset::Regional)
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
            .query("TableName", "SAGDP2")
            .query("TargetParameter", target)
    }

    fn regional_data() -> Route {
        get_data()
            .query("DatasetName", "Regional")
            .query("TableName", "SAGDP2")
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
            a.batch_key("bea_regional/SAGDP2.1.06000").as_deref(),
            Some("bea_regional/SAGDP2/1")
        );
        for bad in [
            "NIPA_GDP_TOTAL",
            "REG_GDP_TOTAL",
            "bea_nipa/T10105.1",
            "bea_nipa/T10105.1.X",
            "bea_nipa/T10105.1.Q.2",
            "bea_nipa/.1.Q",
            "bea_other/T10105.1.Q",
            "bea_regional/SAGDP2.1.",
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
            line_description("[SAGDP2] All industry total"),
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
        assert!(a.curated_missing().is_empty());
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
        let ca = by_id("bea_regional/SAGDP2.1.06000");
        assert_eq!(
            ca.title,
            "All industry total, California (Gross domestic product (GDP) by state)"
        );
        assert_eq!(ca.frequency.as_deref(), Some("Annual"));
        assert_eq!(
            by_id("bea_regional/SAGDP2.86.02000").title,
            "State and local, Alaska (Gross domestic product (GDP) by state)"
        );
        assert!(found
            .iter()
            .any(|s| s.external_id == "bea_regional/SAGDP2.1.00000"));
        assert!(found
            .iter()
            .any(|s| s.external_id == "bea_regional/SAGDP2.1.98000"));
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

    /// Regional titles from a successful refresh that no longer list the curated `SAGDP2`; the
    /// one table still listed matches no selection rule.
    const REGIONAL_WITHOUT_SAGDP2: &[(&str, &str)] = &[(
        "SAINC4",
        "SAINC4 Personal income and employment by major component",
    )];

    /// After a successful title refresh, a table BEA no longer lists is not selected while every
    /// other table is still discovered. BEA has no curated list, so `curated_missing` stays
    /// empty; the rule that selected the table reports zero matches through
    /// `crawler_catalog_rule_matches` (see `catalog_selection::record_rule_matches`).
    #[tokio::test]
    async fn discover_skips_a_curated_table_missing_from_a_successful_refresh() {
        let mock = MockSource::start().await;
        mount_discovery(&mock).await;
        let a = BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
            .with_refreshed_titles(BeaDataset::Nipa)
            .with_table_titles(BeaDataset::Regional, REGIONAL_WITHOUT_SAGDP2)
            .with_refreshed_titles(BeaDataset::Regional);
        let found = a.discover(&test_ctx()).await.unwrap();
        assert_eq!(found.len(), 2 * (26 + 26 + 27 + 35) + 35 + 29);
        assert!(found.iter().all(|s| s.external_id.starts_with("bea_nipa/")));
        assert!(a.curated_missing().is_empty());
        // Only the NIPA tables were requested: nothing for the skipped Regional table.
        assert_eq!(mock.received_requests().await.len(), NIPA_FIXTURES.len());
    }

    /// The same missing title without a successful refresh behind it (none yet, or the last one
    /// failed) is `Transient`: BEA may just be unreachable, so discovery retries instead of
    /// skipping the table.
    #[tokio::test]
    async fn discover_fails_transient_on_a_missing_title_without_a_successful_refresh() {
        let mock = MockSource::start().await;
        mount_discovery(&mock).await;
        let a = BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
            .with_refreshed_titles(BeaDataset::Nipa)
            .with_table_titles(BeaDataset::Regional, REGIONAL_WITHOUT_SAGDP2);
        let e = a.discover(&test_ctx()).await.unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
        assert!(e.to_string().contains("Regional"), "{e}");
        assert!(a.curated_missing().is_empty());
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

    /// The `Data` rows of a recorded `GetData` body.
    fn data_rows<R: serde::de::DeserializeOwned>(body: &str) -> Vec<R> {
        let v: serde_json::Value = serde_json::from_str(body).unwrap();
        serde_json::from_value(v["BEAAPI"]["Results"]["Data"].clone()).unwrap()
    }

    /// Every recorded row's stored units text parses, and agrees with the scale of its values:
    /// `units` drops the scale `UNIT_MULT` applied, so the text describes base units.
    /// `units_raw` keeps BEA's own wording.
    #[test]
    fn stored_units_agree_with_the_scaled_values() {
        use crate::units::{check_text, TextCheck, UnitsContext};

        let mut metas = Vec::new();
        for (table, frequency, body) in NIPA_FIXTURES {
            let frequency = match *frequency {
                "A" => Frequency::Annual,
                "Q" => Frequency::Quarterly,
                _ => Frequency::Monthly,
            };
            for row in data_rows::<NipaRow>(body) {
                metas.push(nipa_metadata(table, "t", frequency, &row, false).unwrap());
            }
        }
        let regional = data_rows::<RegionalRow>(REGIONAL_LINE1);
        let refs: Vec<&RegionalRow> = regional.iter().take(1).collect();
        let fetched = regional_series(
            "SAGDP2",
            "t",
            "All industry total",
            "1",
            "06000",
            &refs,
            regional_dimensions("SAGDP2", "1", "06000"),
        )
        .unwrap();
        metas.push(fetched.metadata.unwrap());
        assert!(metas.len() > 100, "{}", metas.len());

        for meta in &metas {
            let text = meta.units.as_deref().unwrap();
            let structured = meta.structured_unit.as_deref().unwrap();
            assert_eq!(structured, &scaled_values());
            assert!(
                matches!(
                    check_text(text, Some(structured), &UnitsContext::default()),
                    TextCheck::Agrees(_)
                ),
                "{text:?}"
            );
            assert!(meta.units_raw.is_some(), "{text:?}");
        }
        let regional = metas.last().unwrap();
        assert_eq!(regional.units.as_deref(), Some("Current dollars"));
        assert_eq!(
            regional.units_raw.as_deref(),
            Some("Millions of current dollars")
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
                "bea_regional/SAGDP2.1.06000",
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
        // A line the response omits is weaker evidence than NotFound.
        assert_eq!(
            out["bea_nipa/T10105.99.Q"].as_ref().unwrap_err().kind(),
            "absent"
        );
        mock.server().verify().await;
    }

    /// An empty response (every requested Regional area gone, or a NIPA table BEA no longer
    /// publishes at that frequency) is `Absent` for each requested id, which lifecycle counts
    /// only for series the accepted catalog does not list.
    #[tokio::test]
    async fn fetch_batch_marks_every_id_of_an_empty_response_absent() {
        const EMPTY: &str = r#"{"BEAAPI":{"Results":{"Data":[]}}}"#;
        let mock = MockSource::start().await;
        mock.mount(&nipa_route("T10105", "Q"), Reply::json_str(EMPTY))
            .await;
        mock.mount(&regional_data(), Reply::json_str(EMPTY)).await;
        mock.mount(
            &regional_params("LineCode"),
            Reply::json_str(REGIONAL_LINES),
        )
        .await;
        for ids in [
            vec!["bea_nipa/T10105.1.Q", "bea_nipa/T10105.2.Q"],
            vec!["bea_regional/SAGDP2.1.09001", "bea_regional/SAGDP2.1.09003"],
        ] {
            let ids: Vec<String> = ids.into_iter().map(String::from).collect();
            let out = adapter(&mock)
                .fetch_batch(&test_ctx(), &ids, None)
                .await
                .unwrap();
            assert_eq!(out.len(), ids.len());
            for id in &ids {
                assert_eq!(out[id].as_ref().unwrap_err().kind(), "absent", "{id}");
            }
        }
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
            "bea_regional/SAGDP2.1.06000",
            "bea_regional/SAGDP2.1.00000",
            "bea_regional/SAGDP2.1.56000",
            "bea_regional/SAGDP2.1.01000",
            "bea_regional/SAGDP2.1.72000",
        ]
        .map(String::from)
        .to_vec();
        let out = adapter(&mock)
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap();
        let ca = out["bea_regional/SAGDP2.1.06000"].as_ref().unwrap();
        let meta = ca.metadata.as_ref().unwrap();
        assert_eq!(
            meta.title,
            "All industry total, California (Gross domestic product (GDP) by state)"
        );
        assert_eq!(meta.units.as_deref(), Some("Current dollars"));
        assert_eq!(meta.frequency.as_deref(), Some("Annual"));
        assert_eq!(ca.points.len(), 3);
        assert_eq!(ca.points[0].date, d("2021-01-01"));
        assert_eq!(ca.points[0].value, dec("3598103000000"));
        assert_eq!(&ca.dataset, &regional_dimensions("SAGDP2", "1", "06000"));
        // `(D)` (suppressed) is a point without a value.
        let wy = out["bea_regional/SAGDP2.1.56000"].as_ref().unwrap();
        assert_eq!(wy.points.len(), 1);
        assert_eq!(wy.points[0].value, None);
        // An area the response omits is weaker evidence than NotFound.
        assert_eq!(
            out["bea_regional/SAGDP2.1.72000"]
                .as_ref()
                .unwrap_err()
                .kind(),
            "absent"
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
                .query("TableName", "SAGDP2")
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
            .fetch_series(&test_ctx(), "bea_regional/SAGDP2.99.06000", None)
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
        assert!(e.to_string().contains("SAGDP2"), "{e}");
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

    #[tokio::test]
    async fn an_in_body_error_echoing_an_encoded_key_is_scrubbed() {
        let key = "k+y/1=2&3%4 5";
        let form: String = url::form_urlencoded::byte_serialize(key.as_bytes()).collect();
        let mut ctx = test_ctx();
        ctx.keys.bea = Some(key.to_owned());
        for echoed in [key.to_owned(), form.clone(), form.to_lowercase()] {
            let mock = MockSource::start().await;
            let body = serde_json::json!({"BEAAPI": {"Error": {
                "APIErrorCode": "3", "APIErrorDescription": format!("Invalid API UserId {echoed}.")}}});
            mock.mount(&Route::get("/"), Reply::json(body)).await;
            let e = adapter(&mock)
                .fetch_series(&ctx, "bea_nipa/T10105.1.Q", None)
                .await
                .unwrap_err();
            let msg = e.to_string();
            assert_eq!(e.kind(), "auth", "{msg}");
            assert!(msg.contains("Invalid API UserId REDACTED."), "{msg}");
            for leaked in [key, form.as_str(), "k%2b", "k+y"] {
                assert!(!msg.contains(leaked), "{leaked} leaked: {msg}");
            }
        }
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
    /// process). A second call, conditional on the cached `ETag`, that gets a `304` reloads the
    /// same titles from the database instead of trusting memory.
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

        // A second refresh, a `304` this time: it reloads the titles already merged into the
        // database. `reset` first: `mount` matches in mount order, so without it the
        // still-mounted 200 fixture above would answer this request too and the 304 path would
        // go untested.
        mock.reset().await;
        mock.mount_expect(
            &table_titles_route("NIPA"),
            Reply::status(304).header("ETag", "\"v1\""),
            1,
        )
        .await;
        adapter
            .refresh_table_titles(&ctx, &nipa_list, BeaDataset::Nipa)
            .await
            .unwrap();
        assert_eq!(
            adapter.table_titles(BeaDataset::Nipa).await.get("T10105"),
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
                .get("SAGDP2"),
            Some(&"SAGDP2 Gross domestic product (GDP) by state".to_string())
        );

        db.drop().await;
    }

    /// The first catalog-driven crawl can already be missing an old table. Enroll only
    /// positively matched retained histories from either store; don't adopt the whole source.
    #[tokio::test]
    async fn initial_scope_enrolls_matching_retained_series_from_both_stores_and_bounds_them() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_initial_rule_scope",
        )
        .await;
        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog.load_adapter(&BeaAdapter::default()).unwrap();
        persist::sync_datasets(&db.pool, &catalog).await.unwrap();
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Bea,
            REGIONAL_DATASET,
            "table_name",
            &[
                ("SAGDP2N".into(), "Historic GDP by state".into()),
                ("SAGDP2EXCLUDED".into(), "GDP excluded".into()),
                ("SAGDP2BADTITLE".into(), "Personal income".into()),
            ],
        )
        .await
        .unwrap();
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Bea,
            NIPA_DATASET,
            "table_name",
            &[("T10105".into(), "Gross Domestic Product".into())],
        )
        .await
        .unwrap();
        let metadata = |id: &str| {
            let key = SeriesKey::parse(id).unwrap();
            DiscoveredSeries {
                external_id: id.into(),
                title: "Stored history".into(),
                description: None,
                units: None,
                frequency: Some("Annual".into()),
                data_url: None,
                dataset: key.dataset(),
            }
        };
        let ids = [
            "bea_regional/SAGDP2N.1.06000", // a table removed before rollout
            "bea_regional/SAINC1.1.06000",  // unselected table family
            "bea_regional/SAGDP2UNTITLED.1.06000", // cannot establish title predicate
            "bea_regional/SAGDP2EXCLUDED.1.06000",
            "bea_regional/SAGDP2BADTITLE.1.06000",
            "bea_regional/SAGDP2N.1.06037", // counties are outside state-level discovery
            "bea_nipa/T10105.1.A",          // unselected frequency
            "bea_nipa/T10105.1.Q",
        ];
        persist::persist_discovered(
            &db.pool,
            SourceId::Bea,
            &ids.iter().map(|id| metadata(id)).collect::<Vec<_>>(),
        )
        .await
        .unwrap();
        // A history may exist without a metadata row; a duplicated id in both stores is one
        // membership entry. Neither case may lose a user's retained history on first rollout.
        for id in [
            "bea_regional/SAGDP2N.1.06000",
            "bea_regional/SAGDP2N.2.00000",
        ] {
            persist::persist_series(
                &db.pool,
                SourceId::Bea,
                id,
                &FetchedSeries {
                    metadata: Some(NewSeriesMetadataLite {
                        title: "Stored history".into(),
                        frequency: Some("Annual".into()),
                        ..Default::default()
                    }),
                    points: Vec::new(),
                    dataset: SeriesKey::parse(id).unwrap().dataset(),
                    validators: None,
                },
            )
            .await
            .unwrap();
        }
        let config = "max_tables=8\nmax_series=3\nexclude=['SAGDP2EXCLUDED']\n[[rule]]\ndataset='Regional'\nprefix='SAGDP2'\ntitle_contains='GDP'\nfrequencies=['A']\n[[rule]]\ndataset='NIPA'\nprefix='T10105'\ntitle_contains='Gross Domestic Product'\nfrequencies=['Q']";
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let adapter = BeaAdapter::default().with_selection(BeaSelection::parse(config).unwrap());
        let members = adapter.catalog_scope_members(&ctx).await.unwrap();
        assert_eq!(
            members,
            [
                "bea_nipa/T10105.1.Q",
                "bea_regional/SAGDP2N.1.06000",
                "bea_regional/SAGDP2N.2.00000"
            ]
        );
        // The same rules are still bounded when enrolling pre-existing histories.
        let adapter = BeaAdapter::default().with_selection(
            BeaSelection::parse(&config.replace("max_series=3", "max_series=2")).unwrap(),
        );
        assert!(adapter
            .catalog_scope_members(&ctx)
            .await
            .unwrap_err()
            .to_string()
            .contains("volume guard"));
        db.drop().await;
    }

    /// One worker may update the shared HTTP validator while another still holds old names.
    /// A 304 must load the exact shared version, while historical dimension labels stay additive.
    #[tokio::test]
    async fn another_worker_updated_catalog_is_loaded_on_304_and_old_labels_are_preserved() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_shared_membership",
        )
        .await;
        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog.load_adapter(&BeaAdapter::default()).unwrap();
        persist::sync_datasets(&db.pool, &catalog).await.unwrap();
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let mock = MockSource::start().await;
        let a = BeaAdapter::new(mock.base_url());
        let b = BeaAdapter::new(mock.base_url());
        let list = table_titles_code_list(&a, BeaDataset::Regional);
        let body = |name: &str| serde_json::json!({"BEAAPI":{"Results":{"ParamValue":[{"Key":name,"Desc":format!("{name} GDP by state")} ]}}});
        mock.mount(
            &table_titles_route("Regional"),
            Reply::json(body("SAGDP2N")).header("ETag", "\"v1\""),
        )
        .await;
        a.refresh_table_titles(&ctx, &list, BeaDataset::Regional)
            .await
            .unwrap();
        // An unchanged response in the second worker reads the shared parsed snapshot.
        mock.reset().await;
        mock.mount(
            &table_titles_route("Regional"),
            Reply::status(304).header("ETag", "\"v1\""),
        )
        .await;
        b.refresh_table_titles(&ctx, &list, BeaDataset::Regional)
            .await
            .unwrap();
        assert!(b
            .listed_tables(BeaDataset::Regional)
            .await
            .unwrap()
            .contains("SAGDP2N"));
        // The first worker sees a rename and updates the shared snapshot and validators.
        mock.reset().await;
        mock.mount(
            &table_titles_route("Regional"),
            Reply::json(body("SAGDP2")).header("ETag", "\"v2\""),
        )
        .await;
        a.refresh_table_titles(&ctx, &list, BeaDataset::Regional)
            .await
            .unwrap();
        mock.reset().await;
        mock.mount(
            &table_titles_route("Regional"),
            Reply::status(304).header("ETag", "\"v2\""),
        )
        .await;
        b.refresh_table_titles(&ctx, &list, BeaDataset::Regional)
            .await
            .unwrap();
        assert_eq!(
            b.listed_tables(BeaDataset::Regional).await.unwrap(),
            HashSet::from(["SAGDP2".to_string()])
        );
        let labels = persist::dataset_dimension_labels(
            &db.pool,
            SourceId::Bea,
            REGIONAL_DATASET,
            "table_name",
        )
        .await
        .unwrap();
        assert!(
            labels.contains_key("SAGDP2N"),
            "upstream removal must never erase an old title"
        );
        assert!(labels.contains_key("SAGDP2"));
        assert_eq!(
            mock.received_requests().await[0]
                .headers
                .get("If-None-Match")
                .unwrap(),
            "\"v2\""
        );
        db.drop().await;
    }

    /// A rename is picked up by the unchanged family rule and flows through real discovery,
    /// with BEA's new name as the id. The previous name does not survive via cached titles.
    #[tokio::test]
    async fn discover_follows_an_upstream_table_family_rename_without_config_changes() {
        let mock = MockSource::start().await;
        let rules = BeaSelection::parse("max_tables=2\nmax_series=10\n[[rule]]\ndataset='Regional'\nprefix='SAGDP2'\nfrequencies=['A']").unwrap();
        for table in ["SAGDP2N", "SAGDP2"] {
            mock.reset().await;
            for (target, value) in [("LineCode", "1"), ("GeoFips", "06000")] {
                mock.mount(&Route::get("/").query("method", "GetParameterValuesFiltered").query("TableName", table).query("TargetParameter", target),
                    Reply::json(serde_json::json!({"BEAAPI":{"Results":{"ParamValue":[{"Key":value,"Desc":"GDP"}]}}}))).await;
            }
            let a = BeaAdapter::new(mock.base_url())
                .with_selection(rules.clone())
                .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
                .with_refreshed_titles(BeaDataset::Nipa)
                // A retained title for the old name must not cause it to be discovered.
                .with_table_titles(BeaDataset::Regional, &[(table, "GDP by state")])
                .with_refreshed_titles(BeaDataset::Regional);
            let found = a.discover(&test_ctx()).await.unwrap();
            assert_eq!(found.len(), 1);
            assert_eq!(
                found[0].external_id,
                format!("bea_regional/{table}.1.06000")
            );
        }
    }

    #[tokio::test]
    async fn table_volume_guard_stops_before_any_table_requests() {
        let mock = MockSource::start().await;
        let rules = BeaSelection::parse("max_tables=1\nmax_series=10\n[[rule]]\ndataset='Regional'\nprefix='SAGDP2'\nfrequencies=['A']").unwrap();
        let a = BeaAdapter::new(mock.base_url())
            .with_selection(rules)
            .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
            .with_refreshed_titles(BeaDataset::Nipa)
            .with_table_titles(
                BeaDataset::Regional,
                &[("SAGDP2N", "GDP"), ("SAGDP2", "GDP")],
            )
            .with_refreshed_titles(BeaDataset::Regional);
        let error = a.discover(&test_ctx()).await.unwrap_err();
        assert!(error.to_string().contains("volume guard"), "{error}");
        assert!(mock.received_requests().await.is_empty());
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

    /// Through the real refresh. A refresh that fails (a BEA error, or a response with no titles)
    /// leaves a curated table with no known title `Transient`. A successful Regional refresh that
    /// no longer lists the curated `SAGDP2` skips it and still discovers NIPA, even though
    /// SAGDP2's title from an earlier refresh is still stored (stored labels only accumulate).
    #[tokio::test]
    async fn discover_skips_a_missing_table_only_after_a_successful_refresh() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_bea_discover_skips_missing_table",
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
        let mut ctx = test_ctx();
        ctx.pool = db.pool.clone();
        let mock = MockSource::start().await;
        let a = BeaAdapter::new(mock.base_url())
            .with_current_year(2024)
            .with_table_titles(BeaDataset::Nipa, NIPA_TEST_TITLES)
            .with_refreshed_titles(BeaDataset::Nipa);
        let regional_list = table_titles_code_list(&a, BeaDataset::Regional);

        for failed in [
            serde_json::json!({"BEAAPI": {"Results": {"Error": {
                "APIErrorCode": "1", "APIErrorDescription": "Unknown error."}}}}),
            serde_json::json!({"BEAAPI": {"Results": {"ParamValue": []}}}),
        ] {
            mock.reset().await;
            mock.mount(&table_titles_route("Regional"), Reply::json(failed))
                .await;
            mount_discovery(&mock).await;
            a.refresh_table_titles(&ctx, &regional_list, BeaDataset::Regional)
                .await
                .unwrap_err();
            let e = a.discover(&ctx).await.unwrap_err();
            assert_eq!(e.kind(), "transient", "{e}");
            assert!(e.to_string().contains("Regional"), "{e}");
        }

        // An earlier refresh stored SAGDP2's title; it stays after BEA stops listing the table.
        persist::merge_dataset_dimension_codes(
            &db.pool,
            SourceId::Bea,
            REGIONAL_DATASET,
            "table_name",
            &[(
                "SAGDP2".to_string(),
                "SAGDP2 Gross domestic product (GDP) by state".to_string(),
            )],
        )
        .await
        .unwrap();
        mock.reset().await;
        mock.mount(
            &table_titles_route("Regional"),
            Reply::json(serde_json::json!({"BEAAPI": {"Results": {"ParamValue": [
                {"Key": "SAINC4", "Desc": "SAINC4 Personal income and employment by major component"}
            ]}}}))
            .header("ETag", "\"r1\""),
        )
        .await;
        mount_discovery(&mock).await;
        a.refresh_table_titles(&ctx, &regional_list, BeaDataset::Regional)
            .await
            .unwrap();
        assert!(a
            .table_titles(BeaDataset::Regional)
            .await
            .contains_key("SAGDP2"));
        let found = a.discover(&ctx).await.unwrap();
        assert!(!found.is_empty());
        assert!(found.iter().all(|s| s.external_id.starts_with("bea_nipa/")));
        assert!(a.curated_missing().is_empty());
        assert!(!mock.received_requests().await.iter().any(|r| r
            .url
            .query()
            .unwrap_or_default()
            .contains("TableName=SAGDP2")));

        // A `304` keeps the list this process last parsed, so SAGDP2 stays skipped.
        mock.reset().await;
        mock.mount(
            &table_titles_route("Regional"),
            Reply::status(304).header("ETag", "\"r1\""),
        )
        .await;
        mount_discovery(&mock).await;
        a.refresh_table_titles(&ctx, &regional_list, BeaDataset::Regional)
            .await
            .unwrap();
        a.discover(&ctx).await.unwrap();
        assert!(a.curated_missing().is_empty());

        db.drop().await;
    }

    // ECO-415: T10105's line 1 restates the table's title, which read "Gross domestic product
    // (Gross Domestic Product, quarterly)".
    #[test]
    fn nipa_title_leaves_out_a_table_title_the_line_repeats() {
        assert_eq!(
            nipa_title(
                "Gross Domestic Product",
                Frequency::Quarterly,
                "Gross domestic product",
                "1",
                false
            ),
            "Gross domestic product (quarterly)"
        );
        assert_eq!(
            nipa_title(
                "Gross Domestic Product",
                Frequency::Annual,
                "Personal consumption expenditures",
                "2",
                false
            ),
            "Personal consumption expenditures (Gross Domestic Product, annual)"
        );
        assert_eq!(
            nipa_title(
                "GDP, current dollars",
                Frequency::Quarterly,
                "Goods",
                "17",
                true
            ),
            "Goods, line 17 (GDP, current dollars, quarterly)"
        );
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
            strip_table_number_prefix("SAGDP2 Gross domestic product (GDP) by state"),
            "SAGDP2 Gross domestic product (GDP) by state"
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
