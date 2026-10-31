// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! WTO Timeseries API (v1) adapter: a curated set of World Trade Organization trade and tariff
//! indicators for every reporting economy.
//!
//! # Key
//!
//! The API is served through the WTO API portal (<https://apiportal.wto.org>) and refuses every
//! request without a subscription key (HTTP 401). The key is `WTO_API_KEY`
//! ([`ApiKeys::wto`](crate::adapter::ApiKeys::wto)) and is sent as the `subscription-key` query parameter, which
//! [`HttpFetcher`](crate::http::HttpFetcher) redacts from logs and errors. Without it, discovery
//! and fetching fail with `Auth` ("WTO_API_KEY not set") before any request; the crawler worker
//! does not get that far, because it leaves the source out of its registry while the key is
//! unset ([`crate::sources::registry_for_keys`]).
//!
//! # Series and ids
//!
//! The indicators are the rows of `wto_indicators.csv` in the crawler's data directory
//! ([`crate::reference::wto_indicators`]), each with the product/sector (`pc`) and partner
//! economy (`p`) that select its headline series, for example total merchandise (`TO`) traded
//! with the world (`000`). There is one series per indicator and reporting economy, in dataset
//! `timeseries` (`datasets/wto.toml`) with dimensions `indicator` (the WTO code) and `area` (the
//! `key` of `econ-graph-core`'s `countries.csv`). The external id is the dataset's canonical id,
//! `timeseries/{indicator}.{area}` (for example `timeseries/ITS_MTV_AX.USA`).
//!
//! Rows name the reporting economy by the WTO's numeric code (`ReportingEconomyCode`, UN M49
//! style). A country's code is looked up as an ISO 3166-1 numeric code in the shared country
//! table, except Serbia (`892`), Montenegro (`893`) and Sudan (`736`), whose WTO codes differ from
//! their ISO codes; the world (`000`) and the European Union (`918`) map to the table's `WLD` and
//! `EUU` aggregates. Other economies (regional and economic groups, customs territories the table
//! doesn't list, historical entities) are skipped with one warning per request naming them.
//!
//! # Requests
//!
//! Every request is a `GET` under the API root:
//!
//! - `/indicators?i={indicator}&lang=1`: the indicator's own name, unit (`unitLabel`), frequency
//!   (`frequencyLabel`) and description. Series titles, units and frequencies come from here,
//!   never from this crate. An indicator the WTO doesn't list (an empty answer or HTTP 400) is
//!   skipped by discovery with a warning and reported by [`SourceAdapter::curated_missing`].
//! - `/data?i={indicator}&r=all&ps=all&fmt=json&mode=full&head=M&lang=1&max=1000000`, plus
//!   `pc` and `p` when the list gives them: every reporting economy's full history in one
//!   response, `{"Dataset": [row, ...]}`, or an empty `204` when nothing matches. It gets a
//!   longer timeout than ordinary requests (`DATA_TIMEOUT`, see
//!   [`HttpFetcher::get_text_slow`](crate::http::HttpFetcher::get_text_slow)). A fetch for a
//!   single economy sends that economy's code as `r` instead of `all`.
//!
//! Discovery makes both requests for each listed indicator and lists every economy with at least
//! one value; it is complete ([`SourceAdapter::discovery_is_complete`]), so series of an economy
//! the WTO stops publishing for an indicator are retired. An indicator the WTO no longer lists, or
//! lists with no usable value (a `204`, an empty `Dataset`, or no row [`resolve`] keeps), is
//! skipped and reported by
//! [`SourceAdapter::curated_missing`] instead, so the worker retires none of its series (nor
//! anything else in that discovery). Fetching is batched by indicator
//! ([`SourceAdapter::batch_key`]): one pair of requests serves every economy of an indicator.
//! A parse error quoting the `/data` body has the key removed (that body bypasses the HTTP
//! layer's error scrubbing).
//! Discovery also merges the indicators' names, units and descriptions, and the WTO's names for
//! the two aggregates, into the dataset's dimension codes (best effort).
//!
//! The request rate is the source policy's ([`SourcePolicy::default_for`]): one request a second,
//! one at a time, the limit the API sets for its data endpoint.
//!
//! # Rows
//!
//! The `head=M` (machine-readable) column names are read: `ReportingEconomyCode`,
//! `ReportingEconomy`, `PartnerEconomyCode`, `ProductOrSectorCode`, `PeriodCode`, `Year` and
//! `Value` (also accepted in the camelCase the portal's published schema uses). A row whose product or partner differs from the list's is ignored; two different
//! values for one economy and period are a `Parse` error, since the list then doesn't pin the
//! series down. `PeriodCode` is `A` (annual, stored as 1 January), `Q1`–`Q4`, `M01`–`M12` or
//! `S1`/`S2`, each stored as the period's first day. A `null` value is no observation. Values are
//! read from the response text digit for digit ([`crate::value::parse_observation_value`]).
//! `ValueFlagCode` is dropped: `data_points` has no attribute columns.
//!
//! The API publishes no release dates or vintages, so each point is stored with
//! `revision_date = date` and `is_original_release = true`: a revised value overwrites the stored
//! one, as for BLS, Census and BEA. `since` is ignored; the full history is one request.
//!
//! # Errors
//!
//! HTTP statuses map as usual ([`CrawlError::from_status`]); in addition a `403` whose body
//! mentions the call quota (the portal's "Out of call volume quota") is `RateLimited`, not
//! `Auth`. An id that is not a `timeseries` id of a listed indicator is `NotFound` without a
//! request.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use econ_graph_core::models::Code;
use econ_graph_core::reference::{areas, Area, AreaKind, Areas};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use serde_json::Value;

use crate::adapter::{
    BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite,
    SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::persist;
use crate::policy::SourcePolicy;
use crate::reference::{self, WtoIndicator};
use crate::source::SourceId;
use crate::value::parse_observation_value;

/// The real WTO Timeseries API root.
pub const DEFAULT_BASE_URL: &str = "https://api.wto.org/timeseries/v1";

/// The dataset every series belongs to.
pub const DATASET: &str = "timeseries";

/// Query parameter carrying the subscription key (the portal also accepts a header).
const KEY_PARAM: &str = "subscription-key";

/// Most rows one `/data` request asks for (the API's own cap). A response that fills it may be
/// cut short, so it is a `Parse` error rather than a silently partial series.
pub const MAX_ROWS: usize = 1_000_000;

/// Per-attempt timeout of a `/data` request. Every economy's full history comes in one response,
/// which the API can take well over the crawler's ordinary 30-second timeout to produce.
const DATA_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Most series fetched per request: every economy of one indicator, with room to spare.
pub const MAX_BATCH: usize = 400;

/// The error when `WTO_API_KEY` is missing.
pub const MISSING_KEY: &str = "WTO_API_KEY not set: the WTO Timeseries API refuses requests \
     without a subscription key (free from https://apiportal.wto.org)";

/// Human-facing data portal, used for `data_url` (never requested).
const WEB_URL: &str = "https://stats.wto.org";

/// The dataset dimension indicator names are merged into.
const INDICATOR_DIMENSION: &str = "indicator";

/// The dataset dimension keyed by the shared country table.
const AREA_DIMENSION: &str = "area";

/// WTO economy codes of the aggregates the shared country table carries, and their keys there.
const AGGREGATES: [(&str, &str); 2] = [("000", "WLD"), ("918", "EUU")];

/// Current countries whose WTO economy code is not their ISO 3166-1 numeric code, and their keys
/// in the shared country table: Serbia (ISO 688), Montenegro (ISO 499) and Sudan (the WTO keeps
/// 736, the code before South Sudan's independence; ISO now has 729).
const NON_ISO_COUNTRIES: [(&str, &str); 3] = [("892", "SRB"), ("893", "MNE"), ("736", "SDN")];

/// WTO Timeseries adapter. See the module docs for requests, ids and error mapping.
#[derive(Debug, Clone)]
pub struct WtoAdapter {
    base_url: String,
    /// Listed indicators the last discovery skipped because the WTO doesn't list them (see
    /// [`SourceAdapter::curated_missing`]). Shared by every clone.
    missing: Arc<Mutex<Vec<String>>>,
}

impl WtoAdapter {
    /// Talks to `base_url` (the API root, e.g. [`DEFAULT_BASE_URL`]) instead of the real API.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            missing: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// `indicator`'s entry in `/indicators`. `NotFound` when the WTO doesn't list it.
    async fn indicator_info(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        indicator: &str,
    ) -> Result<IndicatorInfo, CrawlError> {
        let infos: Vec<IndicatorInfo> = ctx
            .http
            .get_json(
                SourceId::Wto,
                &format!("{}/indicators", self.base_url),
                &[("i", indicator), ("lang", "1"), (KEY_PARAM, key)],
            )
            .await
            .map_err(|e| match classify(e) {
                // A code the API's validation rejects is one it doesn't know.
                CrawlError::Permanent(m) if m.starts_with("HTTP 400") => CrawlError::NotFound(m),
                e => e,
            })?;
        let info = infos
            .into_iter()
            .find(|i| i.code.trim() == indicator)
            .ok_or_else(|| {
                CrawlError::NotFound(format!("WTO /indicators does not list {indicator}"))
            })?;
        if info.name.trim().is_empty() {
            return Err(CrawlError::Parse(format!(
                "WTO /indicators: {indicator} has no name"
            )));
        }
        Ok(info)
    }

    /// Every row of `indicator` for every reporting economy, with the list's product and partner.
    async fn fetch_rows(
        &self,
        ctx: &CrawlCtx,
        key: &str,
        indicator: &WtoIndicator,
        economy: Option<&str>,
    ) -> Result<Vec<Row>, CrawlError> {
        let max = MAX_ROWS.to_string();
        let mut query: Vec<(&str, &str)> = vec![
            ("i", &indicator.id),
            // One economy's code when the caller wants only that one (a lone series); otherwise
            // every economy, which one request serves as cheaply as a few of them.
            ("r", economy.unwrap_or("all")),
            ("ps", "all"),
            ("fmt", "json"),
            ("mode", "full"),
            ("head", "M"),
            ("lang", "1"),
            ("max", &max),
        ];
        if !indicator.product.is_empty() {
            query.push(("pc", &indicator.product));
        }
        if !indicator.partner.is_empty() {
            query.push(("p", &indicator.partner));
        }
        query.push((KEY_PARAM, key));
        let body = ctx
            .http
            .get_text_slow(
                SourceId::Wto,
                &format!("{}/data", self.base_url),
                &query,
                DATA_TIMEOUT,
            )
            .await
            .map_err(classify)?;
        let what = format!("/data {}", indicator.id);
        // This body did not pass through the HTTP layer's error scrubbing, and a parse error
        // quotes it: never let it carry the key into logs or stored job errors.
        let rows = parse_rows(&what, &body).map_err(|e| redact_key(e, key))?;
        if rows.len() >= MAX_ROWS {
            return Err(CrawlError::Parse(format!(
                "WTO {what}: {} rows fills the request's max={MAX_ROWS}; the series may be cut short",
                rows.len()
            )));
        }
        Ok(rows)
    }

    /// Merges indicator names, units and descriptions, and the WTO's names for the aggregates,
    /// into the dataset's dimension codes, so the API labels them. Best effort: a failure is
    /// logged, and the next discovery tries again.
    async fn merge_labels(&self, ctx: &CrawlCtx, indicators: Vec<Code>, aggregates: Vec<Code>) {
        for (dimension, codes) in [
            (INDICATOR_DIMENSION, indicators),
            (AREA_DIMENSION, aggregates),
        ] {
            if codes.is_empty() {
                continue;
            }
            if let Err(error) = persist::merge_dataset_dimension_code_entries(
                &ctx.pool,
                SourceId::Wto,
                DATASET,
                dimension,
                &codes,
            )
            .await
            {
                tracing::warn!(%error, dimension, "WTO: could not merge labels into the dataset codes");
            }
        }
    }
}

impl Default for WtoAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

#[async_trait]
impl SourceAdapter for WtoAdapter {
    fn id(&self) -> SourceId {
        SourceId::Wto
    }

    fn policy(&self) -> SourcePolicy {
        SourcePolicy {
            max_batch: MAX_BATCH,
            ..SourcePolicy::default_for(SourceId::Wto)
        }
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// Two requests per listed indicator; see the module docs. Series come out indicator by
    /// indicator (in file order), economies sorted by key, so the queue holds each indicator's
    /// series together for batching.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let key = api_key(ctx)?;
        let indicators = reference::wto_indicators()?;
        let def = dataset_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut found = Vec::new();
        let mut missing = Vec::new();
        let mut labels = Vec::new();
        let mut aggregates: BTreeMap<String, String> = BTreeMap::new();
        for indicator in indicators {
            let info = match self.indicator_info(ctx, key, &indicator.id).await {
                Ok(info) => info,
                Err(CrawlError::NotFound(message)) => {
                    tracing::warn!(indicator = %indicator.id, %message,
                        "WTO no longer lists this indicator of wto_indicators.csv; skipping it");
                    missing.push(indicator.id.clone());
                    continue;
                }
                Err(e) => return Err(e),
            };
            let rows = self.fetch_rows(ctx, key, indicator, None).await?;
            let resolved = resolve(indicator, rows, areas)?;
            if resolved.is_empty() {
                // Listed but without a value for any economy we keep (no rows at all, or only
                // rows for another product or partner, or for economies the country table
                // doesn't list): likely a passing gap upstream, or a product or partner the WTO
                // no longer publishes. Retiring every series of the indicator on that would be
                // wrong either way.
                tracing::warn!(indicator = %indicator.id,
                    "WTO lists this indicator of wto_indicators.csv but returned no usable data; skipping it");
                missing.push(indicator.id.clone());
                continue;
            }
            for (area, series) in resolved {
                let (external_id, dataset) = series_id(def, &indicator.id, &area.key)?;
                let meta = series_metadata(&info, indicator, area, &series);
                if area.kind == AreaKind::Aggregate && !series.economy_name.is_empty() {
                    aggregates.insert(area.key.clone(), series.economy_name.clone());
                }
                found.push(DiscoveredSeries {
                    external_id,
                    title: meta.title,
                    description: meta.description,
                    units: meta.units,
                    frequency: meta.frequency,
                    data_url: Some(WEB_URL.to_string()),
                    dataset,
                });
            }
            labels.push(info.code_entry(&indicator.id));
        }
        *self.missing.lock().unwrap() = missing;
        let aggregates = aggregates
            .iter()
            .map(|(key, name)| Code::new(key, name))
            .collect();
        self.merge_labels(ctx, labels, aggregates).await;
        tracing::info!(series = found.len(), "WTO discovery finished");
        Ok(found)
    }

    /// Discovery lists every economy of every listed indicator the WTO publishes.
    fn discovery_is_complete(&self) -> bool {
        true
    }

    fn curated_missing(&self) -> Vec<String> {
        self.missing.lock().unwrap().clone()
    }

    /// The same requests as a batch: every economy of the indicator, keeping `external_id`'s.
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
                "WTO {external_id}: no values for this economy"
            )))
        })
    }

    /// The indicator: every economy of an indicator comes from one request.
    fn batch_key(&self, external_id: &str) -> Option<String> {
        parse_id(external_id).map(|(indicator, _)| indicator.to_string())
    }

    /// One `/indicators` and one `/data` request per distinct indicator among `external_ids`. An
    /// id whose economy has no values is left out of the map (so it fails as `NotFound`); an id
    /// that is not a listed indicator's is `NotFound` without a request. A failed request fails
    /// that indicator's ids; a missing key fails the whole batch.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        _since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let key = api_key(ctx)?;
        let indicators = reference::wto_indicators()?;
        let def = dataset_def()?;
        let areas = areas().map_err(|e| CrawlError::Permanent(e.to_string()))?;
        let mut out = BatchFetch::with_capacity(external_ids.len());
        // Requested areas per indicator, in first-requested order.
        let mut wanted: Vec<(&WtoIndicator, BTreeMap<&str, &str>)> = Vec::new();
        for id in external_ids {
            let listed = parse_id(id)
                .and_then(|(ind, area)| Some((indicators.iter().find(|i| i.id == ind)?, area)));
            let Some((indicator, area)) = listed else {
                out.insert(
                    id.clone(),
                    Err(CrawlError::NotFound(format!(
                        "WTO {id}: not a {DATASET} id of a listed indicator"
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
            // A lone economy is asked for by its own code; several share one all-economy request.
            let single = match ids.keys().collect::<Vec<_>>()[..] {
                [area] => areas.by_key(area).and_then(wto_economy_code),
                _ => None,
            };
            let fetched = async {
                let info = self.indicator_info(ctx, key, &indicator.id).await?;
                let rows = self
                    .fetch_rows(ctx, key, indicator, single.as_deref())
                    .await?;
                Ok::<_, CrawlError>((info, resolve(indicator, rows, areas)?))
            }
            .await;
            let (info, resolved) = match fetched {
                Ok(r) => r,
                Err(e) => {
                    for id in ids.values() {
                        out.insert((*id).to_string(), Err(e.clone()));
                    }
                    continue;
                }
            };
            for (area, series) in resolved {
                let Some(id) = ids.get(area.key.as_str()) else {
                    continue;
                };
                let fetched =
                    series_id(def, &indicator.id, &area.key).map(|(_, dataset)| FetchedSeries {
                        metadata: Some(series_metadata(&info, indicator, area, &series)),
                        points: series
                            .points
                            .iter()
                            .map(|(date, value)| FetchedPoint {
                                date: *date,
                                value: Some(value.clone()),
                                revision_date: *date,
                                is_original_release: true,
                            })
                            .collect(),
                        dataset,
                        validators: None,
                    });
                out.insert((*id).to_string(), fetched);
            }
        }
        Ok(out)
    }
}

/// `ctx`'s WTO key, or `Auth` naming `WTO_API_KEY`.
fn api_key(ctx: &CrawlCtx) -> Result<&str, CrawlError> {
    ctx.keys
        .wto
        .as_deref()
        .ok_or_else(|| CrawlError::Auth(MISSING_KEY.into()))
}

/// `e` with `key` scrubbed from its message, in any form (see [`crate::http::scrub`]).
fn redact_key(e: CrawlError, key: &str) -> CrawlError {
    crate::http::scrub_error(e, &[key.to_owned()])
}

/// The API portal answers an exhausted call quota with `403` ("Out of call volume quota. Quota
/// will be replenished in ..."): that is throttling, retried later, not a bad key. The job is
/// retried once the quota is replenished, as the message says, kept between a minute and
/// [`QUOTA_RETRY_MAX`], or after [`QUOTA_RETRY_FLOOR`] when it doesn't say.
fn classify(e: CrawlError) -> CrawlError {
    match e {
        CrawlError::Auth(m) if m.to_ascii_lowercase().contains("quota") => {
            CrawlError::RateLimited {
                retry_after: Some(quota_replenished_in(&m).map_or(QUOTA_RETRY_FLOOR, |d| {
                    d.clamp(std::time::Duration::from_secs(60), QUOTA_RETRY_MAX)
                })),
            }
        }
        e => e,
    }
}

/// How long to wait after an exhausted quota whose message gives no replenishment time.
const QUOTA_RETRY_FLOOR: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The longest wait for a quota, whatever the message says: a day.
const QUOTA_RETRY_MAX: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// The wait in "Quota will be replenished in HH:MM:SS" (also `D.HH:MM:SS`, and fractional
/// seconds `HH:MM:SS.fff`, which are dropped), if the message has it and it fits a `u64` of
/// seconds.
fn quota_replenished_in(message: &str) -> Option<std::time::Duration> {
    const MARKER: &str = "replenished in ";
    let start = message.find(MARKER)? + MARKER.len();
    let text: String = message[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.')
        .collect();
    let text = text.trim_end_matches('.');
    // A leading `D.` is days; a `.` after the seconds starts their fraction.
    let (days, clock) = match text.split_once('.') {
        Some((d, rest)) if !d.contains(':') => (d.parse::<u64>().ok()?, rest),
        _ => (0, text),
    };
    let clock = clock.split('.').next()?;
    let parts: Vec<u64> = clock
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, s] = parts[..] else { return None };
    if m >= 60 || s >= 60 {
        return None;
    }
    let secs = days
        .checked_mul(24)?
        .checked_add(h)?
        .checked_mul(60)?
        .checked_add(m)?
        .checked_mul(60)?
        .checked_add(s)?;
    Some(std::time::Duration::from_secs(secs))
}

/// The `timeseries` definition from `datasets/wto.toml`.
fn dataset_def() -> Result<&'static DatasetDef, CrawlError> {
    reference::dataset(SourceId::Wto, DATASET)
}

/// The canonical id and dataset of `indicator` for `area`.
fn series_id(
    def: &DatasetDef,
    indicator: &str,
    area: &str,
) -> Result<(String, SeriesDataset), CrawlError> {
    def.series([(INDICATOR_DIMENSION, indicator), (AREA_DIMENSION, area)])
}

/// `(indicator, area)` from `timeseries/{indicator}.{area}`.
fn parse_id(external_id: &str) -> Option<(&str, &str)> {
    let rest = external_id.strip_prefix(DATASET)?.strip_prefix('/')?;
    let (indicator, area) = rest.rsplit_once('.')?;
    (!indicator.is_empty() && !area.is_empty()).then_some((indicator, area))
}

/// One indicator's entry in `/indicators` (only the fields used).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndicatorInfo {
    code: String,
    name: String,
    #[serde(default)]
    unit_label: Option<String>,
    #[serde(default)]
    frequency_label: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

impl IndicatorInfo {
    fn unit(&self) -> Option<String> {
        non_empty(self.unit_label.as_deref())
    }

    fn frequency(&self) -> Option<String> {
        non_empty(self.frequency_label.as_deref())
    }

    /// The dataset `indicator` code for this indicator: its name, unit and description.
    fn code_entry(&self, id: &str) -> Code {
        let mut code = Code::new(id, self.name.trim());
        code.unit = self.unit();
        code.description = non_empty(self.description.as_deref());
        code
    }
}

fn non_empty(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A series' title, description, unit and frequency: the indicator's own from `/indicators`, the
/// economy's name, the product and partner the row names, and the WTO as the source.
fn series_metadata(
    info: &IndicatorInfo,
    indicator: &WtoIndicator,
    area: &Area,
    series: &EconomySeries,
) -> NewSeriesMetadataLite {
    // A country by its name in the shared table; an aggregate (which the table doesn't name) by
    // the WTO's own name for it.
    let area_name = match area.kind {
        AreaKind::Country => area.name.as_str(),
        AreaKind::Aggregate if !series.economy_name.is_empty() => series.economy_name.as_str(),
        AreaKind::Aggregate => area.key.as_str(),
    };
    let name = info.name.trim();
    let mut description = non_empty(info.description.as_deref())
        .unwrap_or_else(|| name.to_string())
        .trim_end_matches('.')
        .to_string();
    description.push_str(&format!(". Reporting economy: {area_name}."));
    for (what, code, label) in [
        ("Product/sector", &indicator.product, &series.product_name),
        ("Partner", &indicator.partner, &series.partner_name),
    ] {
        if !code.is_empty() {
            let label = if label.is_empty() { code } else { label };
            description.push_str(&format!(" {what}: {label} ({code})."));
        }
    }
    description.push_str(" Source: World Trade Organization, WTO Stats.");
    NewSeriesMetadataLite {
        title: format!("{name}: {area_name}"),
        description: Some(description),
        units: info.unit(),
        frequency: info.frequency(),
        seasonal_adjustment: None,
        ..NewSeriesMetadataLite::default()
    }
}

/// One observation row with a value.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// `ReportingEconomyCode`, digits zero-padded to three (`840`, `000`).
    economy: String,
    /// `ReportingEconomy`: the WTO's name for the economy.
    economy_name: String,
    /// `ProductOrSectorCode` and `ProductOrSector`; empty for an indicator without products.
    product: String,
    product_name: String,
    /// `PartnerEconomyCode` and `PartnerEconomy`; empty for an indicator without partners.
    partner: String,
    partner_name: String,
    date: NaiveDate,
    value: BigDecimal,
}

/// Wire format of a `head=M` data row (only the fields used). Codes and years may come as JSON
/// strings or numbers. The portal's published schema writes the same fields in camelCase, so
/// that spelling is accepted too.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct WireRow<'a> {
    #[serde(default, alias = "reportingEconomyCode", deserialize_with = "text")]
    reporting_economy_code: Option<String>,
    #[serde(default, alias = "reportingEconomy", deserialize_with = "text")]
    reporting_economy: Option<String>,
    #[serde(default, alias = "partnerEconomyCode", deserialize_with = "text")]
    partner_economy_code: Option<String>,
    #[serde(default, alias = "partnerEconomy", deserialize_with = "text")]
    partner_economy: Option<String>,
    #[serde(default, alias = "productOrSectorCode", deserialize_with = "text")]
    product_or_sector_code: Option<String>,
    #[serde(default, alias = "productOrSector", deserialize_with = "text")]
    product_or_sector: Option<String>,
    #[serde(default, alias = "periodCode", deserialize_with = "text")]
    period_code: Option<String>,
    #[serde(default, alias = "year", deserialize_with = "text")]
    year: Option<String>,
    /// The value exactly as the response writes it (`null` is `None`).
    #[serde(borrow, default, alias = "value")]
    value: Option<&'a RawValue>,
}

/// A string, or a number's text; `null` is `None`. Trimmed.
fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(other) => {
            return Err(serde::de::Error::custom(format!(
                "expected a string or number, got {other}"
            )))
        }
    })
}

/// A code made only of digits, zero-padded to three (`840`, `4` -> `004`, `0` -> `000`); any
/// other code as is.
fn economy_code(code: &str) -> String {
    if !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()) {
        format!("{code:0>3}")
    } else {
        code.to_string()
    }
}

/// `Year` and `PeriodCode` as the period's first day: `A` (or none) is 1 January, `Q1`–`Q4`,
/// `M01`–`M12` and `S1`/`S2` their first month.
fn parse_period(year: &str, period: Option<&str>) -> Option<NaiveDate> {
    let year: i32 = year
        .trim()
        .parse()
        .ok()
        .filter(|y| (1000..=9999).contains(y))?;
    let period = period.map(str::trim).unwrap_or_default();
    let month = match period {
        "" | "A" => 1,
        p if p.starts_with('Q') => {
            let q: u32 = p[1..].parse().ok().filter(|q| (1..=4).contains(q))?;
            3 * q - 2
        }
        p if p.starts_with('S') => match &p[1..] {
            "1" => 1,
            "2" => 7,
            _ => return None,
        },
        p if p.starts_with('M') && p.len() == 3 => p[1..].parse().ok()?,
        _ => return None,
    };
    NaiveDate::from_ymd_opt(year, month, 1)
}

/// A row's `Value` as written: a JSON number's own text, or the text of a numeric string.
fn row_value(raw: &RawValue) -> Result<Option<BigDecimal>, String> {
    let text = raw.get().trim();
    if text == "null" {
        return Ok(None);
    }
    let text = if text.starts_with('"') {
        let s: String = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if s.trim().is_empty() {
            return Ok(None);
        }
        s
    } else {
        text.to_string()
    };
    parse_observation_value(&text).map(Some)
}

/// The `/data` response's rows with a value. An empty body (the API's `204`) is no rows. A body
/// without a `Dataset` array, or a row whose period or value cannot be read, is a `Parse` error.
fn parse_rows(what: &str, body: &str) -> Result<Vec<Row>, CrawlError> {
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    struct Envelope<'a> {
        #[serde(borrow, rename = "Dataset", alias = "dataset", default)]
        dataset: Option<Vec<&'a RawValue>>,
    }
    let bad = |e: String| CrawlError::Parse(format!("WTO {what}: {e}"));
    let envelope: Envelope =
        serde_json::from_str(body).map_err(|e| bad(format!("invalid response: {e}")))?;
    let items = envelope
        .dataset
        .ok_or_else(|| bad("response has no Dataset array".into()))?;
    let mut rows = Vec::with_capacity(items.len());
    for item in items {
        let r: WireRow =
            serde_json::from_str(item.get()).map_err(|e| bad(format!("bad data row: {e}")))?;
        let Some(raw) = r.value else { continue };
        let Some(value) =
            row_value(raw).map_err(|e| bad(format!("bad value {}: {e}", raw.get())))?
        else {
            continue;
        };
        let year = r.year.unwrap_or_default();
        let date = parse_period(&year, r.period_code.as_deref()).ok_or_else(|| {
            bad(format!(
                "bad period: Year {year:?}, PeriodCode {:?}",
                r.period_code
            ))
        })?;
        rows.push(Row {
            economy: economy_code(&r.reporting_economy_code.unwrap_or_default()),
            economy_name: r.reporting_economy.unwrap_or_default(),
            product: r.product_or_sector_code.unwrap_or_default(),
            product_name: r.product_or_sector.unwrap_or_default(),
            partner: economy_code(&r.partner_economy_code.unwrap_or_default()),
            partner_name: r.partner_economy.unwrap_or_default(),
            date,
            value,
        });
    }
    Ok(rows)
}

/// One reporting economy's observations of an indicator.
#[derive(Debug, Clone, PartialEq, Default)]
struct EconomySeries {
    economy_name: String,
    product_name: String,
    partner_name: String,
    points: BTreeMap<NaiveDate, BigDecimal>,
}

/// The WTO's own codes for these economies and their keys in the shared country table: the
/// aggregates, then the countries whose WTO code is not their ISO 3166-1 numeric code.
fn wto_codes() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    AGGREGATES.iter().chain(NON_ISO_COUNTRIES.iter())
}

/// The area of the shared country table a WTO economy code names, if any. Codes of historical
/// entities (the Federal Republic of Germany 280, Serbia and Montenegro 891, the USSR 810, the
/// Netherlands Antilles 530) name no area, so their rows are skipped.
fn lookup<'a>(areas: &'a Areas, economy: &str) -> Option<&'a Area> {
    if let Some((_, key)) = wto_codes().find(|(code, _)| *code == economy) {
        return areas.by_key(key);
    }
    economy
        .parse::<u16>()
        .ok()
        .filter(|&n| n != 0)
        .and_then(|n| areas.by_iso_numeric(n))
        .filter(|a| a.kind == AreaKind::Country)
}

/// The WTO economy code of an area of the country table (the reverse of [`lookup`]): `000` or
/// `918` for the two aggregates, the WTO's own code for the countries in [`NON_ISO_COUNTRIES`],
/// any other country's ISO numeric code written with three digits.
fn wto_economy_code(area: &Area) -> Option<String> {
    if let Some((code, _)) = wto_codes().find(|(_, key)| *key == area.key) {
        return Some((*code).to_string());
    }
    (area.kind == AreaKind::Country)
        .then_some(area.iso_numeric)
        .flatten()
        .map(|n| format!("{n:03}"))
}

/// `rows` grouped by area of the country table, sorted by area key, keeping only rows of the
/// list's product and partner. Economies the table does not know are logged once and dropped.
/// When rows for a country in [`NON_ISO_COUNTRIES`] come under both its WTO code and its ISO code,
/// only the WTO code's rows are kept, and the dropped code is logged once. Two different values
/// for one area and date are otherwise a `Parse` error.
fn resolve<'a>(
    indicator: &WtoIndicator,
    rows: Vec<Row>,
    areas: &'a Areas,
) -> Result<Vec<(&'a Area, EconomySeries)>, CrawlError> {
    let mut out: BTreeMap<&str, (&Area, EconomySeries)> = BTreeMap::new();
    let mut unknown = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    let differs = |want: &str, got: &str| !want.is_empty() && !got.is_empty() && want != got;
    let selected = |row: &Row| {
        !differs(&indicator.product, &row.product) && !differs(&indicator.partner, &row.partner)
    };
    let (rows, other): (Vec<Row>, Vec<Row>) = rows.into_iter().partition(selected);
    let other_selection = other.len();
    // The WTO codes of NON_ISO_COUNTRIES among the rows kept, by area key. Only those rows count:
    // a WTO code seen only for another product or partner must not hide the ISO code's rows.
    let preferred: BTreeMap<&str, &str> = NON_ISO_COUNTRIES
        .iter()
        .filter(|(code, _)| rows.iter().any(|r| r.economy == *code))
        .map(|(code, key)| (*key, *code))
        .collect();
    for row in rows {
        let Some(area) = lookup(areas, &row.economy) else {
            unknown.insert(format!("{} {}", row.economy, row.economy_name));
            continue;
        };
        if preferred
            .get(area.key.as_str())
            .is_some_and(|code| *code != row.economy)
        {
            duplicate.insert(format!("{} ({})", row.economy, area.key));
            continue;
        }
        let (_, series) = out.entry(area.key.as_str()).or_insert_with(|| {
            (
                area,
                EconomySeries {
                    economy_name: row.economy_name.clone(),
                    product_name: row.product_name.clone(),
                    partner_name: row.partner_name.clone(),
                    ..EconomySeries::default()
                },
            )
        });
        if let Some(previous) = series.points.insert(row.date, row.value.clone())
            && previous != row.value
        {
            return Err(CrawlError::Parse(format!(
                "WTO /data {}: two values for {} {} ({previous} and {}); give the indicator's \
                 product and partner in wto_indicators.csv",
                indicator.id, area.key, row.date, row.value
            )));
        }
    }
    if other_selection > 0 {
        tracing::debug!(indicator = %indicator.id, rows = other_selection,
            "WTO rows for another product or partner ignored");
    }
    if !unknown.is_empty() {
        tracing::warn!(
            indicator = %indicator.id,
            count = unknown.len(),
            economies = ?unknown,
            "WTO rows for economies not in the country table skipped"
        );
    }
    if !duplicate.is_empty() {
        tracing::warn!(
            indicator = %indicator.id,
            economies = ?duplicate,
            "WTO rows under a country's ISO code dropped in favour of its WTO code"
        );
    }
    Ok(out.into_values().collect())
}

#[cfg(test)]
mod tests;
