// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Bank of Canada Valet adapter.
//!
//! [Valet](https://www.bankofcanada.ca/valet/docs) is the Bank of Canada's public JSON API. It
//! needs no key. Endpoints used (all `GET`):
//!
//! - `{base}/series/{name}/json`: one series' details (`seriesDetails`: `name`, `label`,
//!   `description`, `dateFormat` and a `metadata` object that may carry `unit` and `frequency`).
//!   Used by discovery, one request per curated name.
//! - `{base}/observations/{name}/json[?start_date=YYYY-MM-DD]`: the series' details again
//!   (`seriesDetail.{name}`) and its observations, oldest first, each
//!   `{"d": "YYYY-MM-DD", "{name}": {"v": "1.3985"}}`. Used by fetching.
//!
//! # Series
//!
//! Discovery looks up each name in [`crate::reference::boc_series`] (a short curated list read
//! from `data/boc_series.csv`). Only the names are curated: the title is Valet's `label`, the
//! description its `description`, the units `metadata.unit` and the frequency
//! `metadata.frequency`. Valet leaves `metadata` empty for some series (the monetary
//! aggregates); those get no units, and their frequency is Valet's `dateFormat` (`monthly`) with
//! its first letter capitalised. No metadata is typed in by hand.
//!
//! Every series belongs to the one dimensionless dataset [`DATASET`] (`data/datasets/boc.toml`)
//! and keeps its Valet name as its external id. A name with characters outside
//! [`is_valet_series_name`] is `NotFound` without a request, so an id can't change the request
//! path.
//!
//! # Observations
//!
//! `since` becomes `start_date`, so a refresh asks only for the scheduler's revision window
//! (`revision_lookback`, 365 days). Each observation's `d` is its date; daily, weekly and monthly
//! series all use `YYYY-MM-DD` (monthly ones the first of the month), and any other date form is
//! a `Parse` error. Values are strings parsed digit for digit
//! ([`crate::value::parse_observation_value`]). An observation with no entry for the series, or
//! an empty `v`, is stored as a missing value.
//!
//! Valet publishes no vintages: points get `revision_date = date` and `is_original_release =
//! true`, as for BLS and FHFA, so a revised value overwrites the stored one. Valet sends no
//! `ETag` or `Last-Modified`, so every refresh is a full request for the window; the requests
//! are small (one series, at most a year of daily values).
//!
//! # Errors
//!
//! An unknown name is HTTP 404 with `{"message": "Series NAME not found.", ...}`, mapped to
//! `NotFound` by the shared fetcher, as are 429 (`RateLimited`) and 5xx (`Transient`). A 200
//! body without the requested series' details, or whose details name another series, is a
//! `Parse` error.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{
    CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite, SourceAdapter,
};
use crate::dataset::SeriesDataset;
use crate::error::CrawlError;
use crate::reference::{boc_series, is_valet_series_name};
use crate::source::SourceId;
use crate::value::parse_observation_value;

/// The real Valet API root.
pub const DEFAULT_BASE_URL: &str = "https://www.bankofcanada.ca/valet";

/// Code of the dataset every Valet series belongs to. It has no dimensions: a Valet series name
/// is an opaque key.
pub const DATASET: &str = "valet";

/// Valet's own link for a series (the `link` in its series list), used for
/// [`DiscoveredSeries::data_url`]. Never requested.
const VALET_SERIES_LINK: &str = "https://www.bankofcanada.ca/valet/series";

/// Bank of Canada Valet adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct BocAdapter {
    base_url: String,
    /// The curated names the last successful discovery skipped because their lookup failed, for
    /// [`curated_missing`](SourceAdapter::curated_missing).
    skipped_ids: Arc<Mutex<Vec<String>>>,
}

impl BocAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            skipped_ids: Arc::default(),
        }
    }

    /// Valet's details for `name`, as discovery stores them.
    async fn lookup(&self, ctx: &CrawlCtx, name: &str) -> Result<DiscoveredSeries, CrawlError> {
        check_name(name)?;
        let url = format!("{}/series/{name}/json", self.base_url);
        let body: SeriesResponse = ctx.http.get_json(SourceId::Boc, &url, &[]).await?;
        let details = body.series_details;
        if details.name.as_deref() != Some(name) {
            return Err(CrawlError::Parse(format!(
                "BOC {name}: series details name another series ({:?})",
                details.name
            )));
        }
        let meta = details.detail.metadata(name)?;
        Ok(DiscoveredSeries {
            external_id: name.to_string(),
            title: meta.title,
            description: meta.description,
            units: meta.units,
            frequency: meta.frequency,
            data_url: Some(format!("{VALET_SERIES_LINK}/{name}")),
            dataset: valet_dataset(),
        })
    }

    /// Looks up each of `names`. A name whose lookup fails with `NotFound`, `Parse` or `Permanent`
    /// is skipped and reported by [`curated_missing`](SourceAdapter::curated_missing); `Auth` and
    /// `RateLimited` abort at once. A `Transient` failure, or every name failing, fails the
    /// discovery, so a partial catalog is never taken as complete.
    async fn discover_names(
        &self,
        ctx: &CrawlCtx,
        names: &[String],
    ) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let mut found = Vec::with_capacity(names.len());
        let mut skipped = Vec::new();
        let mut last_err = None;
        let mut first_transient = None;
        for name in names {
            match self.lookup(ctx, name).await {
                Ok(series) => found.push(series),
                Err(e @ (CrawlError::Auth(_) | CrawlError::RateLimited { .. })) => return Err(e),
                Err(e) => {
                    tracing::warn!(series_id = %name, error = %e, "BOC series lookup failed; skipping");
                    if first_transient.is_none() && e.is_retryable() {
                        first_transient = Some(e.clone());
                    }
                    skipped.push(name.clone());
                    last_err = Some(e);
                }
            }
        }
        if let Some(e) = first_transient {
            return Err(e);
        }
        if let (true, Some(e)) = (found.is_empty(), last_err) {
            return Err(e);
        }
        tracing::info!(
            series = found.len(),
            skipped = skipped.len(),
            "BOC discovery finished"
        );
        *self
            .skipped_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = skipped;
        Ok(found)
    }
}

impl Default for BocAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for BocAdapter {
    fn id(&self) -> SourceId {
        SourceId::Boc
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// Looks up every name in the curated list. See the module docs.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        self.discover_names(ctx, boc_series()?).await
    }

    /// The curated list is the whole catalog: a series it no longer names is retired.
    fn discovery_is_complete(&self) -> bool {
        true
    }

    /// The curated names the last successful discovery skipped because their lookup failed.
    fn curated_missing(&self) -> Vec<String> {
        self.skipped_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// `/observations/{name}/json`, from `since` when given. See the module docs.
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        check_name(external_id)?;
        let url = format!("{}/observations/{external_id}/json", self.base_url);
        let start = since.map(|d| d.format("%Y-%m-%d").to_string());
        let query: Vec<(&str, &str)> = start
            .as_deref()
            .map(|s| ("start_date", s))
            .into_iter()
            .collect();
        let body: ObservationsResponse = ctx.http.get_json(SourceId::Boc, &url, &query).await?;
        parse_observations(external_id, body)
    }
}

/// `NotFound` for a name that can't be a Valet series name (see [`is_valet_series_name`]).
fn check_name(name: &str) -> Result<(), CrawlError> {
    if is_valet_series_name(name) {
        Ok(())
    } else {
        Err(CrawlError::NotFound(format!(
            "BOC: {name:?} is not a Valet series name"
        )))
    }
}

fn valet_dataset() -> SeriesDataset {
    SeriesDataset {
        code: DATASET.into(),
        ..SeriesDataset::default()
    }
}

/// `/series/{name}/json`.
#[derive(Debug, Deserialize)]
struct SeriesResponse {
    #[serde(rename = "seriesDetails")]
    series_details: NamedDetail,
}

#[derive(Debug, Deserialize)]
struct NamedDetail {
    #[serde(default)]
    name: Option<String>,
    #[serde(flatten)]
    detail: Detail,
}

/// A series' details, as both endpoints give them.
#[derive(Debug, Deserialize)]
struct Detail {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "dateFormat", default)]
    date_format: Option<String>,
    #[serde(default)]
    metadata: DetailMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct DetailMetadata {
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    frequency: Option<String>,
}

impl Detail {
    /// The series' metadata (see [Series](self#series)). A series without a label is a `Parse`
    /// error: every Valet series has one.
    fn metadata(self, name: &str) -> Result<NewSeriesMetadataLite, CrawlError> {
        let title = non_empty(self.label)
            .ok_or_else(|| CrawlError::Parse(format!("BOC {name}: series has no label")))?;
        let frequency = non_empty(self.metadata.frequency)
            .or_else(|| non_empty(self.date_format).map(|f| capitalise(&f)));
        Ok(NewSeriesMetadataLite {
            title,
            description: non_empty(self.description),
            units: non_empty(self.metadata.unit),
            frequency,
            seasonal_adjustment: None,
        })
    }
}

/// `/observations/{name}/json`.
#[derive(Debug, Deserialize)]
struct ObservationsResponse {
    #[serde(rename = "seriesDetail")]
    series_detail: HashMap<String, Detail>,
    observations: Vec<serde_json::Map<String, Value>>,
}

/// Turns an observations response for `name` into the fetched series.
fn parse_observations(name: &str, body: ObservationsResponse) -> Result<FetchedSeries, CrawlError> {
    let ObservationsResponse {
        mut series_detail,
        observations,
    } = body;
    let detail = series_detail.remove(name).ok_or_else(|| {
        CrawlError::Parse(format!(
            "BOC {name}: response has no details for the series"
        ))
    })?;
    let metadata = detail.metadata(name)?;
    let points = observations
        .iter()
        .map(|obs| parse_point(name, obs))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FetchedSeries {
        metadata: Some(metadata),
        points,
        dataset: valet_dataset(),
        validators: None,
    })
}

/// One observation: `{"d": "2026-10-29", "{name}": {"v": "1.3985"}}`.
fn parse_point(
    name: &str,
    obs: &serde_json::Map<String, Value>,
) -> Result<FetchedPoint, CrawlError> {
    let d = obs.get("d").and_then(Value::as_str).ok_or_else(|| {
        CrawlError::Parse(format!("BOC {name}: observation without a date: {obs:?}"))
    })?;
    let date = NaiveDate::parse_from_str(d, "%Y-%m-%d")
        .map_err(|e| CrawlError::Parse(format!("BOC {name}: invalid date {d:?}: {e}")))?;
    let value = match obs.get(name).map(|cell| cell.get("v")) {
        None | Some(None) | Some(Some(Value::Null)) => None,
        Some(Some(Value::String(s))) if s.trim().is_empty() => None,
        Some(Some(Value::String(s))) => Some(parse_value(name, d, s)?),
        Some(Some(other)) => {
            return Err(CrawlError::Parse(format!(
                "BOC {name} {d}: value is not a string: {other}"
            )))
        }
    };
    Ok(FetchedPoint {
        date,
        value,
        revision_date: date,
        is_original_release: true,
    })
}

fn parse_value(name: &str, date: &str, s: &str) -> Result<BigDecimal, CrawlError> {
    parse_observation_value(s)
        .map_err(|e| CrawlError::Parse(format!("BOC {name} {date}: value {s:?}: {e}")))
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// `monthly` -> `Monthly`.
fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contract {
    use super::BocAdapter;
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| BocAdapter::new(base_url),
        external_id: "FXUSDCAD",
        route: Route::get("/observations/FXUSDCAD/json"),
        ok_reply: Reply::json_str(include_str!("../../tests/fixtures/boc/observations_FXUSDCAD.json")),
        expect_points: 9,
    }
}
