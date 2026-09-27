// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! World Bank Indicators API (v2) adapter: a curated set of World Development Indicators for
//! every country and aggregate. Every request is a `GET` with `format=json`; the API needs no key.
//!
//! # Series and ids
//!
//! The indicators are the rows of `wdi_indicators.csv` in the crawler's data directory
//! ([`crate::reference::wdi_indicators`]), which also gives each one's unit. There is one series
//! per indicator and area, in dataset `wdi` (`datasets/world_bank.toml`) with dimensions
//! `indicator` (the WDI code) and `area` (the `key` of `econ-graph-core`'s `countries.csv`: ISO
//! alpha-3 for countries, the World Bank code for aggregates such as `WLD` and `EMU`). The
//! external id is the dataset's canonical id, `wdi/{indicator}.{area}` (for example
//! `wdi/NY.GDP.PCAP.CD.USA`).
//!
//! # Requests
//!
//! Discovery and fetching both use one request per indicator for every area:
//! `{base}/country/all/indicator/{id}?format=json&per_page=20000&page=N`. One page holds the
//! whole indicator today (about 266 areas times 65 years); further pages are followed when the
//! response's `pages` says so, up to [`MAX_PAGES`].
//!
//! - **Discovery** makes that request for each indicator and lists the areas with at least one
//!   value. A series with no values is never created. `Auth` and `RateLimited` abort discovery;
//!   another failure skips that indicator with a warning, unless every indicator failed.
//! - **Fetching** is batched by indicator ([`SourceAdapter::batch_key`]): one request serves every
//!   area of an indicator, up to [`MAX_BATCH`] series. A lone `fetch_series` makes the same request
//!   and keeps its own area.
//!
//! # Rows
//!
//! Each row names its area by `countryiso3code` (ISO alpha-3, or the World Bank's code for an
//! aggregate) and `country.id` (ISO alpha-2, or a two-character aggregate id). The area is looked
//! up in the shared country table ([`econ_graph_core::reference::areas`]) by World Bank code, then
//! ISO alpha-3, then (for a row without `countryiso3code`) ISO alpha-2. Rows for an area the table does not list (regional aggregates
//! outside it, the Channel Islands) are skipped, with one warning per request naming them.
//!
//! Dates are years (`2023`), or `2023Q2` / `2023M05` for the rare quarterly or monthly indicator,
//! stored as the period's first day. A `null` value is no observation and is dropped.
//!
//! Every point's `revision_date` is the response's `lastupdated` (the date the World Bank last
//! updated the database), so each database update is stored as a new vintage of the whole series;
//! `is_original_release` is `true`, as for the other sources without vintage history. `since` is
//! ignored: the full history comes in the same single request, and returning all of it keeps
//! revisions to old years. The row's `obs_status` and `decimal` are dropped: `data_points` has no
//! attribute columns in train 1 (the dataset still declares them, for the schema). WDI leaves
//! `obs_status` empty for nearly every row.
//!
//! # Errors
//!
//! Successful responses are a two-element array `[meta, rows]` (`rows` may be `null`). Errors
//! come back as HTTP 200 with a one-element array `[{"message": [{"id", "key", "value"}]}]`;
//! [`classify_world_bank_message`] maps "Invalid value" / unknown-indicator messages (ids 120 and
//! 175) to `NotFound` and anything else to `Permanent`. A response without `lastupdated` is a
//! `Parse` error. An id that is not a `wdi` id of a listed indicator is `NotFound` without a
//! request.

use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::reference::{areas, Area};
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{
    BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite,
    SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::reference::{self, WdiIndicator};
use crate::source::SourceId;

/// The real World Bank API root.
pub const DEFAULT_BASE_URL: &str = "https://api.worldbank.org/v2";

/// The dataset every series belongs to.
pub const DATASET: &str = "wdi";

/// Human-facing indicator page, used for `data_url` (never requested).
const WEB_INDICATOR_URL: &str = "https://data.worldbank.org/indicator";

/// Rows per page. One page holds a whole indicator for every area.
const PER_PAGE: &str = "20000";

/// Upper bound on pages per indicator (a response claiming more is cut off with a warning).
pub const MAX_PAGES: u64 = 10;

/// Most series fetched per request: every area of one indicator (the country table has about 260
/// rows), with room to spare.
pub const MAX_BATCH: usize = 400;

/// World Bank adapter. See the module docs for endpoints, ids and error mapping.
#[derive(Debug, Clone)]
pub struct WorldBankAdapter {
    base_url: String,
}

impl WorldBankAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    /// Every row of `indicator` for every area, following pages.
    async fn fetch_indicator(
        &self,
        ctx: &CrawlCtx,
        indicator: &str,
    ) -> Result<IndicatorData, CrawlError> {
        let path = format!("/country/all/indicator/{indicator}");
        let url = format!("{}{path}", self.base_url);
        let mut rows = Vec::new();
        let mut last_updated = None;
        let mut page = 1u64;
        loop {
            let page_s = page.to_string();
            let body: Value = ctx
                .http
                .get_json(
                    SourceId::WorldBank,
                    &url,
                    &[
                        ("format", "json"),
                        ("per_page", PER_PAGE),
                        ("page", &page_s),
                    ],
                )
                .await?;
            let (meta, items) = parse_list(&path, body)?;
            if page == 1 {
                last_updated = meta.last_updated;
            }
            let got = items.len();
            rows.extend(parse_rows(&path, items)?);
            let pages = meta.pages.unwrap_or(1);
            if got == 0 || page >= pages {
                break;
            }
            if page >= MAX_PAGES {
                tracing::warn!(
                    indicator,
                    pages,
                    "World Bank indicator has more pages than MAX_PAGES; the rest are skipped"
                );
                break;
            }
            page += 1;
        }
        let last_updated = last_updated.ok_or_else(|| {
            CrawlError::Parse(format!("World Bank {path}: response has no lastupdated"))
        })?;
        Ok(IndicatorData::group(indicator, last_updated, rows))
    }
}

impl Default for WorldBankAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for WorldBankAdapter {
    fn id(&self) -> SourceId {
        SourceId::WorldBank
    }

    fn policy(&self) -> SourcePolicy {
        SourcePolicy {
            max_batch: MAX_BATCH,
            ..SourcePolicy::default_for(SourceId::WorldBank)
        }
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// One request per indicator; see the module docs. Series come out indicator by indicator
    /// (in file order), areas sorted by key, so the queue holds each indicator's series together
    /// for batching.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let indicators = reference::wdi_indicators()?;
        let def = wdi_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut found = Vec::new();
        let (mut any_ok, mut last_err) = (false, None);
        for indicator in indicators {
            match self.fetch_indicator(ctx, &indicator.id).await {
                Ok(data) => {
                    any_ok = true;
                    for (area, rows) in data.resolve(areas) {
                        let (external_id, dataset) = series_id(def, &indicator.id, &area.key)?;
                        let meta = metadata(indicator, area, &rows);
                        found.push(DiscoveredSeries {
                            external_id,
                            title: meta.title,
                            description: meta.description,
                            units: meta.units,
                            frequency: meta.frequency,
                            data_url: Some(data_url(&indicator.id, &rows)),
                            dataset: Some(dataset),
                        });
                    }
                }
                Err(e @ (CrawlError::Auth(_) | CrawlError::RateLimited { .. })) => return Err(e),
                Err(e) => {
                    tracing::warn!(indicator = %indicator.id, error = %e, "World Bank indicator failed; skipping");
                    last_err = Some(e);
                }
            }
        }
        match (any_ok, last_err) {
            (false, Some(e)) => Err(e),
            _ => {
                tracing::info!(series = found.len(), "World Bank discovery finished");
                Ok(found)
            }
        }
    }

    /// The same request as a batch: every area of the indicator, keeping `external_id`'s.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        let mut out = self
            .fetch_batch(ctx, &[external_id.to_string()], since)
            .await?;
        out.remove(external_id).unwrap_or_else(|| {
            Err(CrawlError::NotFound(format!(
                "World Bank {external_id}: no values for this area"
            )))
        })
    }

    /// The indicator: every area of an indicator comes from one request.
    fn batch_key(&self, external_id: &str) -> Option<String> {
        parse_id(external_id).map(|(indicator, _)| indicator.to_string())
    }

    /// One request per distinct indicator among `external_ids` (one, when the worker batches by
    /// [`batch_key`](Self::batch_key)). An id whose area has no values is left out of the map
    /// (so it fails as `NotFound`); an id that is not a listed indicator's is `NotFound` without a
    /// request. A failed request fails that indicator's ids.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        _since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let indicators = reference::wdi_indicators()?;
        let def = wdi_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut out = BatchFetch::with_capacity(external_ids.len());
        // Requested areas per indicator, in first-requested order.
        let mut wanted: Vec<(&WdiIndicator, BTreeMap<&str, &str>)> = Vec::new();
        for id in external_ids {
            let listed = parse_id(id)
                .and_then(|(ind, area)| Some((indicators.iter().find(|i| i.id == ind)?, area)));
            let Some((indicator, area)) = listed else {
                out.insert(
                    id.clone(),
                    Err(CrawlError::NotFound(format!(
                        "World Bank {id}: not a {DATASET} id of a listed indicator"
                    ))),
                );
                continue;
            };
            match wanted.iter_mut().find(|(i, _)| i.id == indicator.id) {
                Some((_, ids)) => {
                    ids.insert(area, id);
                }
                None => wanted.push((indicator, BTreeMap::from([(area, id.as_str())]))),
            }
        }
        for (indicator, ids) in wanted {
            let data = match self.fetch_indicator(ctx, &indicator.id).await {
                Ok(data) => data,
                Err(e) => {
                    for id in ids.values() {
                        out.insert((*id).to_string(), Err(e.clone()));
                    }
                    continue;
                }
            };
            for (area, rows) in data.resolve(areas) {
                let Some(id) = ids.get(area.key.as_str()) else {
                    continue;
                };
                let series = series_id(def, &indicator.id, &area.key).map(|(_, dataset)| {
                    FetchedSeries {
                        metadata: Some(metadata(indicator, area, &rows)),
                        points: rows
                            .iter()
                            .map(|r| FetchedPoint {
                                date: r.date,
                                value: Some(r.value.clone()),
                                revision_date: data.last_updated,
                                is_original_release: true,
                            })
                            .collect(),
                        dataset: Some(dataset),
                    }
                });
                out.insert((*id).to_string(), series);
            }
        }
        Ok(out)
    }
}

/// The `wdi` definition from `datasets/world_bank.toml` (cached by [`reference::datasets`]).
fn wdi_def() -> Result<&'static DatasetDef, CrawlError> {
    reference::datasets(SourceId::WorldBank)?
        .iter()
        .find(|d| d.code == DATASET)
        .ok_or_else(|| {
            CrawlError::Permanent(format!(
                "{} defines no {DATASET} dataset",
                reference::datasets_file(SourceId::WorldBank).display()
            ))
        })
}

/// The canonical id and dataset of `indicator` for `area`.
fn series_id(
    def: &DatasetDef,
    indicator: &str,
    area: &str,
) -> Result<(String, SeriesDataset), CrawlError> {
    def.series([("indicator", indicator), ("area", area)])
}

/// `(indicator, area)` from `wdi/{indicator}.{area}`. Indicator codes contain dots and area keys
/// never do, so the area is everything after the last dot.
fn parse_id(external_id: &str) -> Option<(&str, &str)> {
    let rest = external_id.strip_prefix(DATASET)?.strip_prefix('/')?;
    let (indicator, area) = rest.rsplit_once('.')?;
    (!indicator.is_empty() && !area.is_empty()).then_some((indicator, area))
}

fn metadata(indicator: &WdiIndicator, area: &Area, rows: &[Row]) -> NewSeriesMetadataLite {
    NewSeriesMetadataLite {
        title: format!("{}: {}", indicator.name, area.name),
        description: Some(format!(
            "World Development Indicators {} for {}",
            indicator.id, area.name
        )),
        units: Some(indicator.unit.clone()),
        frequency: Some(
            rows.first()
                .map_or(Frequency::Annual, |r| r.frequency)
                .label()
                .to_string(),
        ),
        seasonal_adjustment: None,
    }
}

/// The indicator's page on data.worldbank.org, filtered to the row's area.
fn data_url(indicator: &str, rows: &[Row]) -> String {
    match rows.first() {
        Some(r) if !r.wb_id.is_empty() => {
            format!("{WEB_INDICATOR_URL}/{indicator}?locations={}", r.wb_id)
        }
        _ => format!("{WEB_INDICATOR_URL}/{indicator}"),
    }
}

/// Response metadata we use.
#[derive(Debug, Default, PartialEq)]
struct Meta {
    pages: Option<u64>,
    last_updated: Option<NaiveDate>,
}

/// Splits a `[meta, items]` response into its metadata and items; a `[{"message": ..}]` body
/// becomes an error via [`classify_world_bank_message`].
fn parse_list(what: &str, body: Value) -> Result<(Meta, Vec<Value>), CrawlError> {
    let Value::Array(mut parts) = body else {
        return Err(CrawlError::Parse(format!(
            "World Bank {what}: response is not an array"
        )));
    };
    if let Some(messages) = parts.first().and_then(|m| m.get("message")) {
        return Err(classify_world_bank_message(what, messages));
    }
    if parts.len() < 2 {
        return Err(CrawlError::Parse(format!(
            "World Bank {what}: expected [metadata, rows]"
        )));
    }
    let items = match parts.swap_remove(1) {
        Value::Null => Vec::new(),
        Value::Array(items) => items,
        other => {
            return Err(CrawlError::Parse(format!(
                "World Bank {what}: expected a row array, got {}",
                type_name(&other)
            )))
        }
    };
    let meta = parts.first();
    let pages = meta.and_then(|m| m.get("pages")).and_then(|p| {
        p.as_u64()
            .or_else(|| p.as_str().and_then(|s| s.trim().parse().ok()))
    });
    let last_updated = match meta.and_then(|m| m.get("lastupdated")) {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_str()
                .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
                .ok_or_else(|| {
                    CrawlError::Parse(format!("World Bank {what}: bad lastupdated {v}"))
                })?,
        ),
    };
    Ok((
        Meta {
            pages,
            last_updated,
        },
        items,
    ))
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Maps the World Bank's in-body error (`[{"message":[{"id":"120","key":"Invalid value","value":"..."}]}]`,
/// served with HTTP 200): invalid-value / unknown-indicator messages (ids `120`, `175`) are
/// `NotFound`, anything else `Permanent`.
pub fn classify_world_bank_message(what: &str, messages: &Value) -> CrawlError {
    let first = messages.as_array().and_then(|m| m.first());
    let field = |k: &str| {
        first
            .and_then(|m| m.get(k))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let (id, key, value) = (field("id"), field("key"), field("value"));
    let msg = format!("World Bank {what}: error {id} {key}: {value}");
    if matches!(id.as_str(), "120" | "175") || key.eq_ignore_ascii_case("invalid value") {
        CrawlError::NotFound(msg)
    } else {
        CrawlError::Permanent(msg)
    }
}

/// Observation frequency, from the form of the row's date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frequency {
    Annual,
    Quarterly,
    Monthly,
}

impl Frequency {
    fn label(self) -> &'static str {
        match self {
            Frequency::Annual => "Annual",
            Frequency::Quarterly => "Quarterly",
            Frequency::Monthly => "Monthly",
        }
    }
}

/// `2023` -> 2023-01-01, `2023Q2` -> 2023-04-01, `2023M05` -> 2023-05-01.
fn parse_period(s: &str) -> Option<(NaiveDate, Frequency)> {
    let s = s.trim();
    let year: i32 = s.get(..4)?.parse().ok()?;
    let rest = &s[4..];
    let (month, freq) = if rest.is_empty() {
        (1, Frequency::Annual)
    } else if let Some(q) = rest.strip_prefix('Q') {
        let q: u32 = q.parse().ok().filter(|q| (1..=4).contains(q))?;
        (3 * q - 2, Frequency::Quarterly)
    } else {
        let m = rest.strip_prefix('M')?;
        (m.parse().ok()?, Frequency::Monthly)
    };
    Some((NaiveDate::from_ymd_opt(year, month, 1)?, freq))
}

/// One observation with a value, and the area as the row names it.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// `countryiso3code` (ISO alpha-3, or the World Bank code of an aggregate); may be empty.
    iso3: String,
    /// `country.id` (ISO alpha-2, or a two-character aggregate id).
    wb_id: String,
    /// `country.value`, for warnings.
    name: String,
    date: NaiveDate,
    frequency: Frequency,
    value: BigDecimal,
}

/// Wire format of a data row (only the fields we use).
#[derive(Debug, Deserialize)]
struct WbRow {
    country: IdValue,
    #[serde(default, rename = "countryiso3code")]
    iso3: Option<String>,
    date: String,
    value: Option<serde_json::Number>,
}

#[derive(Debug, Deserialize)]
struct IdValue {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    value: Option<String>,
}

/// The rows with a value. A row whose date or value cannot be parsed is a `Parse` error.
fn parse_rows(what: &str, items: Vec<Value>) -> Result<Vec<Row>, CrawlError> {
    let raw: Vec<WbRow> = serde_json::from_value(Value::Array(items))
        .map_err(|e| CrawlError::Parse(format!("World Bank {what}: bad data rows: {e}")))?;
    let mut rows = Vec::with_capacity(raw.len());
    for r in raw {
        let Some(value) = r.value else { continue };
        let (date, frequency) = parse_period(&r.date).ok_or_else(|| {
            CrawlError::Parse(format!("World Bank {what}: bad date {:?}", r.date))
        })?;
        let value = BigDecimal::from_str(&value.to_string()).map_err(|e| {
            CrawlError::Parse(format!("World Bank {what}: bad value {value}: {e}"))
        })?;
        rows.push(Row {
            iso3: r.iso3.unwrap_or_default().trim().to_string(),
            wb_id: r.country.id.unwrap_or_default().trim().to_string(),
            name: r.country.value.unwrap_or_default(),
            date,
            frequency,
            value,
        });
    }
    Ok(rows)
}

/// One indicator's rows with values, grouped by the area code the rows use.
struct IndicatorData {
    indicator: String,
    last_updated: NaiveDate,
    /// `(iso3, wb_id)` -> rows, sorted by date.
    by_code: BTreeMap<(String, String), Vec<Row>>,
}

impl IndicatorData {
    fn group(indicator: &str, last_updated: NaiveDate, rows: Vec<Row>) -> Self {
        let mut by_code: BTreeMap<(String, String), Vec<Row>> = BTreeMap::new();
        for r in rows {
            by_code
                .entry((r.iso3.clone(), r.wb_id.clone()))
                .or_default()
                .push(r);
        }
        for rows in by_code.values_mut() {
            rows.sort_by_key(|r| r.date);
            // A repeated date keeps its last row.
            rows.reverse();
            rows.dedup_by_key(|r| r.date);
            rows.reverse();
        }
        Self {
            indicator: indicator.to_string(),
            last_updated,
            by_code,
        }
    }

    /// Rows per area of the country table, sorted by area key. Codes the table does not know are
    /// logged once and dropped. Two raw codes can resolve to the same area (a country whose
    /// `countryiso3code` is empty on some rows and set on others), so rows are merged rather than
    /// keeping only the first-seen code, with a repeated date keeping its last-merged row.
    fn resolve<'a>(&self, areas: &'a econ_graph_core::reference::Areas) -> Vec<(&'a Area, Vec<Row>)> {
        let mut out: BTreeMap<&str, (&Area, Vec<Row>)> = BTreeMap::new();
        let mut unknown = BTreeSet::new();
        let mut merged = BTreeSet::new();
        for ((iso3, wb_id), rows) in &self.by_code {
            let area = lookup(areas, iso3, wb_id);
            match area {
                Some(area) => match out.entry(area.key.as_str()) {
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert((area, rows.clone()));
                    }
                    std::collections::btree_map::Entry::Occupied(mut e) => {
                        merged.insert(area.key.as_str());
                        let combined = &mut e.get_mut().1;
                        combined.extend(rows.iter().cloned());
                        combined.sort_by_key(|r| r.date);
                        // A date shared between the merged codes keeps its later-merged row (the
                        // same "last wins" rule `group` uses within one raw code).
                        combined.reverse();
                        combined.dedup_by_key(|r| r.date);
                        combined.reverse();
                    }
                },
                None => {
                    let name = rows.first().map_or("", |r| r.name.as_str());
                    unknown.insert(format!("{iso3}/{wb_id} {name}"));
                }
            }
        }
        if !unknown.is_empty() {
            tracing::warn!(
                indicator = %self.indicator,
                count = unknown.len(),
                areas = ?unknown,
                "World Bank rows for areas not in the country table skipped"
            );
        }
        if !merged.is_empty() {
            tracing::info!(
                indicator = %self.indicator,
                areas = ?merged,
                "World Bank rows for these areas used more than one raw code (e.g. countryiso3code \
                 empty on some rows); merged into one series"
            );
        }
        out.into_values().collect()
    }
}

/// The area a row names: `countryiso3code` as a World Bank code or ISO alpha-3, or, only when a
/// row has no `countryiso3code`, `country.id` as ISO alpha-2 (aggregate ids are not ISO codes).
fn lookup<'a>(
    areas: &'a econ_graph_core::reference::Areas,
    iso3: &str,
    wb_id: &str,
) -> Option<&'a Area> {
    let by_iso3 = (!iso3.is_empty())
        .then(|| areas.by_wb_code(iso3).or_else(|| areas.by_iso3(iso3)))
        .flatten();
    by_iso3.or_else(|| {
        (iso3.is_empty() && wb_id.len() == 2)
            .then(|| areas.by_iso2(wb_id))
            .flatten()
    })
}

#[cfg(test)]
mod tests;
