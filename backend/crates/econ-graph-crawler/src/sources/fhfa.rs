// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! FHFA (Federal Housing Finance Agency) House Price Index adapter.
//!
//! FHFA publishes every HPI series in one file, the HPI master CSV
//! (`GET {base}`[`MASTER_CSV_PATH`]), with one row per series and period:
//!
//! ```text
//! hpi_type,hpi_flavor,frequency,level,place_name,place_id,yr,period,index_nsa,index_sa
//! traditional,purchase-only,monthly,USA or Census Division,United States,USA,2024,3,420.1,418.7
//! ```
//!
//! Columns are found by header name, so their order does not matter. The layout comes from FHFA's
//! public documentation; the fixture is built from it, not recorded (see
//! `tests/fixtures/fhfa/README.md`).
//!
//! # Scope (train 1)
//!
//! Rows with `hpi_type` `traditional`, `hpi_flavor` `purchase-only` or `all-transactions`,
//! `frequency` `monthly` or `quarterly`, and `level` `USA or Census Division` or `State`: the
//! United States, the nine census divisions and the states and DC. Every other row (metros,
//! expanded-data, non-metro, distress-free, ...) is skipped without being parsed.
//!
//! # Series and ids
//!
//! Each series is in dataset [`DATASET`] (`data/datasets/fhfa.toml`). Its dimensions are the file's
//! `hpi_type`, `hpi_flavor`, `frequency`, `level` and `place_id` columns, plus
//! `seasonal_adjustment` (`nsa` for the `index_nsa` column, `sa` for `index_sa`): train 1 stores
//! one value per observation, so the two value columns are separate series. Ids are canonical
//! dataset ids, for example `fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa`.
//! `level` is FHFA's value lowercased with spaces replaced by `-` (one way: ids are never parsed
//! back). A series exists only when its column has at least one value (`index_sa` is empty for
//! many series).
//!
//! # Discovery and fetching
//!
//! Discovery downloads the file, and so does each fetch batch. Every series shares one
//! [`batch_key`](SourceAdapter::batch_key) and the policy's `max_batch` ([`MAX_BATCH`]) covers
//! every train 1 series, so fetching them all is normally one download (discovery downloads the
//! file separately). Discovery also labels every `place_id` dimension code (states and census
//! divisions alike) from the file's own `place_name` column, reusing that same download; the
//! dataset definition (`data/datasets/fhfa.toml`) ships no hand-typed place labels.
//!
//! Points are dated the first day of their month or quarter, with `revision_date = date` and
//! `is_original_release = true`. FHFA re-estimates the whole history at every release and the file
//! has no vintage column, so a fetch returns every observation (ignoring `since`) and each refresh
//! overwrites the stored values with the latest estimate.
//!
//! # Concurrency
//!
//! Discovery and every fetch batch download the same master file, so two workers can end up
//! downloading it at once even though [`MAX_BATCH`] normally covers the whole catalog in one
//! fetch. A process-local in-flight reservation, keyed by the master URL, makes the second
//! download a no-op instead: it makes no request and fails with a retryable [`CrawlError::Busy`]
//! (exponential backoff with jitter), which the worker reschedules without counting a failed
//! attempt (see [`worker`](crate::worker)). The reservation is one process's in-memory state: it
//! does nothing for two separate `crawler-worker` processes downloading at once. Recovering a
//! reservation left behind by a crawler crash is
//! [ECO-257](https://linear.app/econgraph/issue/ECO-257/recover-shared-download-reservations-after-crawler-crashes);
//! today a crash simply loses the in-memory reservation along with the rest of the process.
//!
//! # Errors
//!
//! - Ids outside the dataset are `NotFound` without a request; ids not in the file are `NotFound`.
//! - HTTP errors use the fetcher's status mapping and apply to the whole batch, as does a response
//!   that is not the master CSV (a missing column, or bytes that aren't CSV).
//! - A bad year, period or value, or two rows for the same period with different values, fails
//!   only the series it belongs to (`Parse`).
//! - An in-scope row without a usable `place_id` (empty, or containing `.` or `/`, which ids can't
//!   hold) is skipped with one warning per file; a series only on such rows is then `NotFound`.
//!   A row with too few fields reads its missing cells as empty.
//! - A dataset definition whose dimensions don't match the adapter's fails the whole batch
//!   (`Permanent`).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use rand::RngExt;

use crate::adapter::{
    BatchFetch, CrawlCtx, DiscoveredSeries, FetchedPoint, FetchedSeries, NewSeriesMetadataLite,
    SourceAdapter,
};
use crate::dataset::{DatasetDef, SeriesDataset};
use crate::error::CrawlError;
use crate::persist;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// FHFA's website root.
pub const DEFAULT_BASE_URL: &str = "https://www.fhfa.gov";

/// Path of the HPI master CSV below the base URL.
pub const MASTER_CSV_PATH: &str = "/hpi/download/monthly/hpi_master.csv";

/// The dataset every FHFA series belongs to.
pub const DATASET: &str = "fhfa_hpi";

/// Most series fetched per download: more than train 1's roughly 200, so one download fetches
/// them all.
pub const MAX_BATCH: usize = 500;

/// Every series comes from the same file, so they all share one batch key.
const BATCH_KEY: &str = "hpi_master";

const HPI_TYPES: &[&str] = &["traditional"];
const FLAVORS: &[&str] = &["purchase-only", "all-transactions"];
const FREQUENCIES: &[&str] = &["monthly", "quarterly"];
const LEVELS: &[&str] = &["USA or Census Division", "State"];

/// Value columns: `(column, seasonal_adjustment code, seasonal adjustment as stored in metadata)`.
const MEASURES: [(&str, &str, &str); 2] = [
    ("index_nsa", "nsa", "Not Seasonally Adjusted"),
    ("index_sa", "sa", "Seasonally Adjusted"),
];

/// Column positions in the file, found by header name.
struct Columns {
    hpi_type: usize,
    hpi_flavor: usize,
    frequency: usize,
    level: usize,
    place_name: usize,
    place_id: usize,
    yr: usize,
    period: usize,
    /// In [`MEASURES`] order.
    values: [usize; 2],
}

impl Columns {
    fn find(headers: &csv::StringRecord) -> Result<Self, CrawlError> {
        let find = |name: &str| {
            headers.iter().position(|h| h == name).ok_or_else(|| {
                CrawlError::Parse(format!(
                    "FHFA: response is not the HPI master CSV: no {name:?} column (header: {:?})",
                    headers.iter().take(12).collect::<Vec<_>>()
                ))
            })
        };
        Ok(Self {
            hpi_type: find("hpi_type")?,
            hpi_flavor: find("hpi_flavor")?,
            frequency: find("frequency")?,
            level: find("level")?,
            place_name: find("place_name")?,
            place_id: find("place_id")?,
            yr: find("yr")?,
            period: find("period")?,
            values: [find(MEASURES[0].0)?, find(MEASURES[1].0)?],
        })
    }
}

/// One series read from the file.
#[derive(Debug, Clone, PartialEq)]
struct HpiSeries {
    dataset: SeriesDataset,
    place_name: String,
    flavor: String,
    frequency: String,
    seasonal_adjustment: &'static str,
    points: BTreeMap<NaiveDate, BigDecimal>,
    /// First problem found in one of the series' rows; the series fails with it.
    error: Option<String>,
}

impl HpiSeries {
    fn title(&self) -> String {
        format!(
            "{} House Price Index: {}, {}, {}",
            self.place_name,
            title_case(&self.flavor),
            title_case(&self.frequency),
            self.seasonal_adjustment
        )
    }

    fn description(&self) -> String {
        format!(
            "FHFA {} house price index for {}, {}, {}. From FHFA's HPI master file; FHFA \
             re-estimates the whole history at every release.",
            self.flavor,
            self.place_name,
            self.frequency,
            self.seasonal_adjustment.to_lowercase()
        )
    }

    /// The index's base period, as FHFA documents it.
    fn units(&self) -> &'static str {
        match (self.flavor.as_str(), self.frequency.as_str()) {
            ("purchase-only", "monthly") => "Index (January 1991 = 100)",
            ("purchase-only", _) => "Index (1991Q1 = 100)",
            _ => "Index (1980Q1 = 100)",
        }
    }

    fn metadata(&self) -> NewSeriesMetadataLite {
        NewSeriesMetadataLite {
            title: self.title(),
            description: Some(self.description()),
            units: Some(self.units().into()),
            frequency: Some(title_case(&self.frequency)),
            seasonal_adjustment: Some(self.seasonal_adjustment.into()),
        }
    }

    fn fetched(&self, id: &str) -> Result<FetchedSeries, CrawlError> {
        if let Some(e) = &self.error {
            return Err(CrawlError::Parse(format!("FHFA {id}: {e}")));
        }
        Ok(FetchedSeries {
            metadata: Some(self.metadata()),
            points: self
                .points
                .iter()
                .map(|(date, value)| FetchedPoint {
                    date: *date,
                    value: Some(value.clone()),
                    revision_date: *date,
                    is_original_release: true,
                })
                .collect(),
            dataset: self.dataset.clone(),
        })
    }
}

/// `purchase-only` -> `Purchase-Only`, `monthly` -> `Monthly`.
fn title_case(s: &str) -> String {
    s.split('-')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// The `level` dimension value for a `level` cell: `USA or Census Division` ->
/// `usa-or-census-division`.
fn level_code(level: &str) -> String {
    level.to_ascii_lowercase().replace(' ', "-")
}

/// First day of the period: `period` is the month (1-12) for monthly rows and the quarter (1-4)
/// for quarterly rows.
fn period_start(frequency: &str, yr: &str, period: &str) -> Result<NaiveDate, String> {
    let year: i32 = yr.parse().map_err(|_| format!("invalid yr {yr:?}"))?;
    let p: u32 = period
        .parse()
        .map_err(|_| format!("invalid period {period:?}"))?;
    let month = match (frequency, p) {
        ("monthly", 1..=12) => p,
        ("quarterly", 1..=4) => p * 3 - 2,
        _ => return Err(format!("invalid {frequency} period {p} in {year}")),
    };
    NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| format!("invalid yr {year}"))
}

/// A row's series in the file: `(hpi_type, hpi_flavor, frequency, level, place_id)`. Each has
/// up to one series per value column.
type RowKey = (String, String, String, String, String);

/// Parses the master CSV into the train 1 series, keyed by external id (see the module docs).
fn parse_master(text: &str, def: &DatasetDef) -> Result<BTreeMap<String, HpiSeries>, CrawlError> {
    let file_error = |e: csv::Error| CrawlError::Parse(format!("FHFA HPI master file: {e}"));
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(text.as_bytes());
    let col = Columns::find(reader.headers().map_err(file_error)?)?;

    // The dataset and id are built once per series.
    let mut series: BTreeMap<RowKey, [Option<(String, HpiSeries)>; 2]> = BTreeMap::new();
    let mut bad_places = BTreeSet::new();
    let mut record = csv::StringRecord::new();
    while reader.read_record(&mut record).map_err(file_error)? {
        let get = |i: usize| record.get(i).unwrap_or("");
        let (hpi_type, flavor) = (get(col.hpi_type), get(col.hpi_flavor));
        let (frequency, level) = (get(col.frequency), get(col.level));
        if !HPI_TYPES.contains(&hpi_type)
            || !FLAVORS.contains(&flavor)
            || !FREQUENCIES.contains(&frequency)
            || !LEVELS.contains(&level)
        {
            continue;
        }
        let place_id = get(col.place_id);
        if place_id.is_empty() || place_id.contains(['.', '/']) {
            bad_places.insert(place_id.to_string());
            continue;
        }
        let line = record.position().map_or(0, csv::Position::line);
        if col.values.iter().all(|&i| get(i).is_empty()) {
            continue;
        }
        let date = period_start(frequency, get(col.yr), get(col.period));
        let slots = series
            .entry((
                hpi_type.to_string(),
                flavor.to_string(),
                frequency.to_string(),
                level.to_string(),
                place_id.to_string(),
            ))
            .or_default();
        for (m, (column, sa_code, sa_label)) in MEASURES.iter().enumerate() {
            let raw = get(col.values[m]);
            // An empty cell is no observation; a bad date only matters when there is a value.
            if raw.is_empty() {
                continue;
            }
            let (_, s) = match &mut slots[m] {
                Some(slot) => slot,
                slot @ None => {
                    let (id, dataset) = def.series([
                        ("hpi_type", hpi_type.to_string()),
                        ("hpi_flavor", flavor.to_string()),
                        ("frequency", frequency.to_string()),
                        ("level", level_code(level)),
                        ("place_id", place_id.to_string()),
                        ("seasonal_adjustment", (*sa_code).to_string()),
                    ])?;
                    slot.insert((
                        id,
                        HpiSeries {
                            dataset,
                            place_name: get(col.place_name).to_string(),
                            flavor: flavor.to_string(),
                            frequency: frequency.to_string(),
                            seasonal_adjustment: sa_label,
                            points: BTreeMap::new(),
                            error: None,
                        },
                    ))
                }
            };
            if s.error.is_some() {
                continue;
            }
            let point = date.clone().and_then(|date| {
                let value = BigDecimal::from_str(raw)
                    .map_err(|_| format!("invalid {column} {raw:?} on {date}"))?;
                Ok((date, value))
            });
            match point {
                Ok((date, value)) => match s.points.get(&date) {
                    // The first value is kept; an equal one (e.g. `100` and `100.0`) is fine.
                    Some(prev) if *prev != value => {
                        s.error = Some(format!(
                            "two rows for {date} with different {column} ({prev} and {value})"
                        ));
                    }
                    Some(_) => {}
                    None => {
                        s.points.insert(date, value);
                    }
                },
                Err(e) => s.error = Some(format!("line {line}: {e}")),
            }
        }
    }
    if !bad_places.is_empty() {
        tracing::warn!(
            place_ids = ?bad_places,
            "FHFA: skipped in-scope rows whose place_id is empty or can't be part of an id"
        );
    }
    Ok(series.into_values().flatten().flatten().collect())
}

/// First contention backoff; doubles per contender waiting on the same download, ±25% jitter,
/// capped at [`CONTENTION_MAX_BACKOFF`]. Mirrors [`crate::http`]'s in-process retry backoff.
const CONTENTION_BASE_BACKOFF: Duration = Duration::from_millis(250);
const CONTENTION_MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Downloads in flight, by URL, with how many callers have found each one busy (see the module
/// docs, "Concurrency"). One process only.
static IN_FLIGHT: LazyLock<Mutex<HashMap<String, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Holds a URL's in-flight reservation until dropped, which releases it whether `master` finishes
/// normally, fails, or its future is cancelled (an `async fn`'s locals, this one included, drop on
/// every exit path).
struct DownloadGuard(String);

impl DownloadGuard {
    /// Reserves `url`, or fails with a retryable [`CrawlError::Busy`] if another crawl already
    /// holds it. Never blocks and never makes a request.
    fn try_acquire(url: &str) -> Result<Self, CrawlError> {
        let mut in_flight = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        match in_flight.get_mut(url) {
            None => {
                in_flight.insert(url.to_string(), 0);
                Ok(Self(url.to_string()))
            }
            Some(contenders) => {
                *contenders += 1;
                Err(busy(url, *contenders))
            }
        }
    }
}

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        IN_FLIGHT
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// `CrawlError::Busy` for the `n`th contender waiting on `url`: `CONTENTION_BASE_BACKOFF * 2^(n
/// - 1)`, jittered, capped at `CONTENTION_MAX_BACKOFF`.
fn busy(url: &str, n: u32) -> CrawlError {
    let base = CONTENTION_BASE_BACKOFF.saturating_mul(1u32.checked_shl(n - 1).unwrap_or(u32::MAX));
    let jittered = base.mul_f64(rand::rng().random_range(0.75..=1.25));
    CrawlError::Busy {
        retry_after: jittered.min(CONTENTION_MAX_BACKOFF),
        message: format!("FHFA: another crawl is already downloading {url}"),
    }
}

/// FHFA adapter. See the module docs.
#[derive(Debug, Clone)]
pub struct FhfaAdapter {
    base_url: String,
}

impl FhfaAdapter {
    /// Downloads the master file from `base_url` (the site root, e.g. [`DEFAULT_BASE_URL`]).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn master_url(&self) -> String {
        format!("{}{MASTER_CSV_PATH}", self.base_url)
    }

    /// Downloads and parses the master file.
    async fn master(&self, ctx: &CrawlCtx) -> Result<BTreeMap<String, HpiSeries>, CrawlError> {
        let url = self.master_url();
        let _reservation = DownloadGuard::try_acquire(&url)?;
        let def = crate::reference::dataset(SourceId::Fhfa, DATASET)?;
        let text = ctx.http.get_text(SourceId::Fhfa, &url, &[]).await?;
        parse_master(&text, def)
    }

    /// Labels every `place_id` found in `master` (states and census divisions alike) from the
    /// file's own `place_name` column, into the `place_id` dimension of [`DATASET`]. Reuses
    /// `master`'s already-downloaded rows rather than fetching the file again: unlike
    /// [`SourceAdapter::refresh_reference_data`] (always run before `discover`, so it can't share
    /// `discover`'s download), this runs from inside `discover` itself. A failure here is logged
    /// and does not fail discovery; a stale or missing label is cosmetic, not a crawl error.
    async fn refresh_place_labels(&self, ctx: &CrawlCtx, master: &BTreeMap<String, HpiSeries>) {
        let mut labels: BTreeMap<String, String> = BTreeMap::new();
        for s in master.values() {
            if let Some(place_id) = s.dataset.dimensions.0.get("place_id") {
                labels
                    .entry(place_id.clone())
                    .or_insert_with(|| s.place_name.clone());
            }
        }
        let labels: Vec<(String, String)> = labels.into_iter().collect();
        if let Err(error) = persist::merge_dataset_dimension_codes(
            &ctx.pool,
            SourceId::Fhfa,
            DATASET,
            "place_id",
            &labels,
        )
        .await
        {
            tracing::warn!(%error, "FHFA: failed to refresh place_id labels");
        }
    }
}

impl Default for FhfaAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

fn is_hpi_id(external_id: &str) -> bool {
    external_id
        .strip_prefix(DATASET)
        .is_some_and(|rest| rest.starts_with('/'))
}

fn not_in_file(external_id: &str) -> CrawlError {
    CrawlError::NotFound(format!(
        "FHFA: series {external_id:?} is not in the HPI master file"
    ))
}

#[async_trait]
impl SourceAdapter for FhfaAdapter {
    fn id(&self) -> SourceId {
        SourceId::Fhfa
    }

    fn policy(&self) -> SourcePolicy {
        SourcePolicy {
            max_batch: MAX_BATCH,
            ..SourcePolicy::default_for(SourceId::Fhfa)
        }
    }

    fn datasets(&self) -> &[&str] {
        &[DATASET]
    }

    /// Discovery reads every in-scope row of the master file (see the module docs): a series it
    /// no longer lists, including the retired `{CODE}HPI` ids, has been retired by FHFA.
    fn discovery_is_complete(&self) -> bool {
        true
    }

    /// Every train 1 series in the master file (see the module docs).
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let url = self.master_url();
        let master = self.master(ctx).await?;
        self.refresh_place_labels(ctx, &master).await;
        Ok(master
            .into_iter()
            .map(|(external_id, s)| {
                let m = s.metadata();
                DiscoveredSeries {
                    title: m.title,
                    description: m.description,
                    units: m.units,
                    frequency: m.frequency,
                    data_url: Some(url.clone()),
                    dataset: s.dataset,
                    external_id,
                }
            })
            .collect())
    }

    /// Every observation of the series; `since` is ignored (see the module docs).
    async fn fetch_series(
        &self,
        ctx: &CrawlCtx,
        external_id: &str,
        since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        if !is_hpi_id(external_id) {
            return Err(CrawlError::NotFound(format!(
                "FHFA: unknown series {external_id:?}"
            )));
        }
        let ids = [external_id.to_string()];
        self.fetch_batch(ctx, &ids, since)
            .await?
            .remove(external_id)
            .unwrap_or_else(|| Err(not_in_file(external_id)))
    }

    fn batch_key(&self, external_id: &str) -> Option<String> {
        is_hpi_id(external_id).then(|| BATCH_KEY.to_string())
    }

    /// One download of the master file for the whole batch.
    async fn fetch_batch(
        &self,
        ctx: &CrawlCtx,
        external_ids: &[String],
        _since: Option<NaiveDate>,
    ) -> Result<BatchFetch, CrawlError> {
        let master = self.master(ctx).await?;
        Ok(external_ids
            .iter()
            .map(|id| {
                let fetched = master
                    .get(id)
                    .map_or_else(|| Err(not_in_file(id)), |s| s.fetched(id));
                (id.clone(), fetched)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route};

    const MASTER: &str = include_str!("../../tests/fixtures/fhfa/hpi_master.csv");
    const HEADER: &str =
        "hpi_type,hpi_flavor,frequency,level,place_name,place_id,yr,period,index_nsa,index_sa\n";

    const US_PO_MONTHLY_SA: &str =
        "fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa";
    const CA_AT_NSA: &str = "fhfa_hpi/traditional.all-transactions.quarterly.state.CA.nsa";

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn def() -> &'static DatasetDef {
        crate::reference::dataset(SourceId::Fhfa, DATASET).unwrap()
    }

    fn parse(text: &str) -> BTreeMap<String, HpiSeries> {
        parse_master(text, def()).unwrap()
    }

    /// Id of an all-transactions series of state `place`.
    fn at_state(frequency: &str, place: &str, sa: &str) -> String {
        format!("fhfa_hpi/traditional.all-transactions.{frequency}.state.{place}.{sa}")
    }

    async fn serving(body: &str) -> MockSource {
        let mock = MockSource::start().await;
        mock.mount(&Route::get(MASTER_CSV_PATH), Reply::text(body))
            .await;
        mock
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(FhfaAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(FhfaAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(
            FhfaAdapter::new("http://x/").master_url(),
            "http://x/hpi/download/monthly/hpi_master.csv"
        );
        assert_eq!(FhfaAdapter::default().id(), SourceId::Fhfa);
        assert_eq!(FhfaAdapter::default().datasets(), [DATASET]);
        assert!(FhfaAdapter::default().discovery_is_complete());
        assert_eq!(FhfaAdapter::default().retirement_scope_prefix(), None);
    }

    #[test]
    fn policy_batches_every_series_in_one_download() {
        let p = FhfaAdapter::default().policy();
        assert_eq!(p.max_batch, MAX_BATCH);
        assert!(MAX_BATCH > parse(MASTER).len());
        assert_eq!(
            SourcePolicy { max_batch: 1, ..p },
            SourcePolicy::default_for(SourceId::Fhfa)
        );
    }

    #[test]
    fn fixture_scope_and_ids() {
        let series = parse(MASTER);
        // Purchase-only monthly: US + 9 divisions, NSA and SA. Purchase-only quarterly: US + 9
        // divisions + 51 states, NSA and SA. All-transactions quarterly: the same places, NSA only.
        assert_eq!(series.len(), 10 * 2 + 61 * 2 + 61);
        for (id, s) in &series {
            assert_eq!(def().external_id(&s.dataset.dimensions).unwrap(), *id);
            assert_eq!(s.dataset.code, DATASET);
            assert!(s.error.is_none(), "{id}");
            assert!(!s.points.is_empty(), "{id}");
            let dims = &s.dataset.dimensions.0;
            assert_eq!(dims["hpi_type"], "traditional");
            assert!(
                ["usa-or-census-division", "state"].contains(&dims["level"].as_str()),
                "{id}: metros and other levels are skipped"
            );
        }
        // No SA series where the column is empty.
        assert!(!series.contains_key(&at_state("quarterly", "CA", "sa")));
        // LA is Louisiana: places are FHFA's own ids, and states and divisions can't collide.
        let la = &series["fhfa_hpi/traditional.purchase-only.quarterly.state.LA.nsa"];
        assert_eq!(la.place_name, "Louisiana");
    }

    #[test]
    fn values_dates_and_metadata() {
        let series = parse(MASTER);
        let f = series[US_PO_MONTHLY_SA].fetched(US_PO_MONTHLY_SA).unwrap();
        let got: Vec<(NaiveDate, String)> = f
            .points
            .iter()
            .map(|p| (p.date, p.value.as_ref().unwrap().to_string()))
            .collect();
        assert_eq!(got.len(), 6);
        assert_eq!(got[0], (d("2024-10-01"), "203.78".to_string()));
        assert_eq!(got[5].0, d("2025-03-01"));
        assert!(f
            .points
            .iter()
            .all(|p| p.revision_date == p.date && p.is_original_release));
        let m = f.metadata.unwrap();
        assert_eq!(
            m.title,
            "United States House Price Index: Purchase-Only, Monthly, Seasonally Adjusted"
        );
        assert_eq!(m.frequency.as_deref(), Some("Monthly"));
        assert_eq!(m.units.as_deref(), Some("Index (January 1991 = 100)"));
        assert_eq!(
            m.seasonal_adjustment.as_deref(),
            Some("Seasonally Adjusted")
        );
        let dims = &f.dataset.dimensions.0;
        assert_eq!(dims["level"], "usa-or-census-division");
        assert_eq!(dims["seasonal_adjustment"], "sa");

        let ca = series[CA_AT_NSA].fetched(CA_AT_NSA).unwrap();
        let dates: Vec<NaiveDate> = ca.points.iter().map(|p| p.date).collect();
        assert_eq!(dates, [d("2024-07-01"), d("2024-10-01"), d("2025-01-01")]);
        assert_eq!(
            ca.metadata.unwrap().units.as_deref(),
            Some("Index (1980Q1 = 100)")
        );
        let po_q = "fhfa_hpi/traditional.purchase-only.quarterly.state.CA.nsa";
        assert_eq!(series[po_q].units(), "Index (1991Q1 = 100)");
    }

    #[test]
    fn columns_are_found_by_name() {
        let text = "index_sa,index_nsa,period,yr,place_id,place_name,level,frequency,hpi_flavor,hpi_type,extra\n\
                    ,101.5,2,2024,TX,Texas,State,quarterly,all-transactions,traditional,x\n";
        let series = parse(text);
        let tx = &series[&at_state("quarterly", "TX", "nsa")];
        assert_eq!(tx.points[&d("2024-04-01")].to_string(), "101.5");
        assert_eq!(series.len(), 1);
    }

    #[test]
    fn bom_crlf_and_quoted_names() {
        let text = format!(
            "\u{feff}{}traditional,all-transactions,quarterly,State,\"Washington, D.C.\",DC,2024,1,100,\r\n",
            HEADER.replace('\n', "\r\n")
        );
        let series = parse(&text);
        let dc = &series[&at_state("quarterly", "DC", "nsa")];
        assert_eq!(dc.place_name, "Washington, D.C.");
        assert_eq!(dc.points[&d("2024-01-01")].to_string(), "100");
    }

    #[test]
    fn file_level_errors() {
        let missing =
            parse_master("hpi_type,hpi_flavor\ntraditional,purchase-only\n", def()).unwrap_err();
        assert_eq!(missing.kind(), "parse");
        assert!(
            missing.to_string().contains("not the HPI master CSV"),
            "{missing}"
        );
        assert!(missing.to_string().contains("frequency"), "{missing}");
        let html = parse_master("<html><body>Maintenance</body></html>", def()).unwrap_err();
        assert_eq!(html.kind(), "parse");
    }

    #[test]
    fn short_rows_do_not_fail_the_file() {
        let text = format!(
            "{HEADER}\
             traditional,all-transactions,quarterly,MSA,\"Abilene, TX\"\n\
             traditional,all-transactions,quarterly,State,Texas,TX,2024,1,100\n\
             Source: FHFA\n"
        );
        let series = parse(&text);
        assert_eq!(series.len(), 1);
        assert_eq!(series[&at_state("quarterly", "TX", "nsa")].points.len(), 1);
    }

    #[test]
    fn unusable_place_ids_are_skipped() {
        let text = format!(
            "{HEADER}\
             traditional,all-transactions,quarterly,State,Nowhere,,2024,1,100,\n\
             traditional,all-transactions,quarterly,State,Dotted,A.B,2024,1,100,\n\
             traditional,all-transactions,quarterly,State,Slashed,A/B,2024,1,100,\n\
             traditional,all-transactions,quarterly,State,Texas,TX,2024,1,100,\n"
        );
        let series = parse(&text);
        assert_eq!(
            series.keys().collect::<Vec<_>>(),
            [&at_state("quarterly", "TX", "nsa")]
        );
    }

    #[test]
    fn row_errors_fail_only_their_series() {
        let text = format!(
            "{HEADER}\
             traditional,all-transactions,quarterly,State,Texas,TX,2024,5,100,\n\
             traditional,all-transactions,quarterly,State,Ohio,OH,2024,1,abc,\n\
             traditional,all-transactions,quarterly,State,Ohio,OH,2024,1,100,x\n\
             traditional,all-transactions,quarterly,State,Iowa,IA,2024,1,100,\n\
             traditional,all-transactions,quarterly,State,Iowa,IA,2024,1,101,\n\
             traditional,all-transactions,quarterly,State,Utah,UT,2024,1,100,\n\
             traditional,all-transactions,quarterly,State,Utah,UT,2024,1,100.0,\n\
             traditional,all-transactions,monthly,State,Maine,ME,2024,12,99,\n"
        );
        let series = parse(&text);
        let id =
            |p: &str, sa: &str| at_state(if p == "ME" { "monthly" } else { "quarterly" }, p, sa);
        let err = |p: &str, sa: &str| series[&id(p, sa)].fetched(&id(p, sa)).unwrap_err();
        assert!(
            err("TX", "nsa").to_string().contains("line 2:"),
            "{}",
            err("TX", "nsa")
        );
        assert!(err("TX", "nsa").to_string().contains("period 5"));
        assert!(err("OH", "nsa").to_string().contains("abc"));
        assert!(err("OH", "sa").to_string().contains("\"x\""));
        assert!(err("IA", "nsa").to_string().contains("two rows"));
        assert_eq!(err("IA", "nsa").kind(), "parse");
        // The same value twice is fine, and the first one is kept.
        let ut = &series[&id("UT", "nsa")].points;
        assert_eq!(ut.len(), 1);
        assert_eq!(ut[&d("2024-01-01")].to_string(), "100");
        assert_eq!(
            series[&id("ME", "nsa")].points[&d("2024-12-01")].to_string(),
            "99"
        );
    }

    #[test]
    fn period_rules() {
        assert_eq!(
            period_start("monthly", "2024", "12").unwrap(),
            d("2024-12-01")
        );
        assert_eq!(
            period_start("quarterly", "2024", "4").unwrap(),
            d("2024-10-01")
        );
        assert!(period_start("monthly", "2024", "13").is_err());
        assert!(period_start("quarterly", "2024", "0").is_err());
        assert!(period_start("quarterly", "x", "1").is_err());
    }

    #[test]
    fn batch_key_only_for_dataset_ids() {
        let a = FhfaAdapter::default();
        assert_eq!(a.batch_key(CA_AT_NSA).as_deref(), Some(BATCH_KEY));
        assert_eq!(a.batch_key(US_PO_MONTHLY_SA), a.batch_key(CA_AT_NSA));
        assert_eq!(a.batch_key("USHPI"), None);
        assert_eq!(a.batch_key("fhfa_hpix/a"), None);
    }

    #[tokio::test]
    async fn discover_reads_places_from_the_file() {
        let mock = serving(MASTER).await;
        let found = FhfaAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        assert_eq!(mock.received_requests().await.len(), 1);
        assert_eq!(found.len(), 203);
        let ids: HashSet<&str> = found.iter().map(|s| s.external_id.as_str()).collect();
        assert_eq!(ids.len(), found.len(), "ids are unique");
        let titles: HashSet<&str> = found.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles.len(), found.len(), "titles are unique");
        let us = found
            .iter()
            .find(|s| s.external_id == US_PO_MONTHLY_SA)
            .unwrap();
        assert_eq!(us.frequency.as_deref(), Some("Monthly"));
        assert_eq!(us.units.as_deref(), Some("Index (January 1991 = 100)"));
        assert_eq!(
            us.data_url,
            Some(format!("{}{MASTER_CSV_PATH}", mock.base_url()))
        );
        crate::testkit::contract::assert_series_datasets(
            &FhfaAdapter::default(),
            found.iter().map(|s| (s.external_id.as_str(), &s.dataset)),
        );
    }

    #[tokio::test]
    async fn one_download_serves_a_batch() {
        let mock = serving(MASTER).await;
        let adapter = FhfaAdapter::new(mock.base_url());
        let missing = at_state("quarterly", "ZZ", "nsa");
        let ids: Vec<String> = vec![
            US_PO_MONTHLY_SA.to_string(),
            CA_AT_NSA.to_string(),
            missing.clone(),
        ];
        let out = adapter
            .fetch_batch(&test_ctx(), &ids, Some(d("2025-01-01")))
            .await
            .unwrap();
        assert_eq!(mock.received_requests().await.len(), 1);
        assert_eq!(out.len(), 3);
        // `since` is ignored: FHFA revises the whole history.
        assert_eq!(out[US_PO_MONTHLY_SA].as_ref().unwrap().points.len(), 6);
        assert_eq!(out[CA_AT_NSA].as_ref().unwrap().points.len(), 3);
        assert_eq!(out[&missing].as_ref().unwrap_err().kind(), "not_found");
    }

    #[tokio::test]
    async fn every_discovered_series_fetches_in_one_batch() {
        let mock = serving(MASTER).await;
        let adapter = FhfaAdapter::new(mock.base_url());
        let ctx = test_ctx();
        let ids: Vec<String> = adapter
            .discover(&ctx)
            .await
            .unwrap()
            .into_iter()
            .map(|s| s.external_id)
            .collect();
        assert!(ids.len() <= MAX_BATCH);
        let keys: HashSet<_> = ids.iter().map(|id| adapter.batch_key(id)).collect();
        assert_eq!(keys.len(), 1);
        let out = adapter.fetch_batch(&ctx, &ids, None).await.unwrap();
        assert!(out.values().all(Result::is_ok));
        // One download for discovery, one for the batch.
        assert_eq!(mock.received_requests().await.len(), 2);
    }

    #[tokio::test]
    async fn unknown_ids_are_not_found() {
        let mock = serving(MASTER).await;
        let adapter = FhfaAdapter::new(mock.base_url());
        for id in ["USHPI", "CAHPI", "LAHPI"] {
            let e = adapter
                .fetch_series(&test_ctx(), id, None)
                .await
                .unwrap_err();
            assert_eq!(e.kind(), "not_found", "{id}");
        }
        assert!(mock.received_requests().await.is_empty());
        let e = adapter
            .fetch_series(
                &test_ctx(),
                "fhfa_hpi/traditional.purchase-only.monthly.state.CA.sa",
                None,
            )
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "not_found");
        assert_eq!(mock.received_requests().await.len(), 1);
    }

    #[tokio::test]
    async fn http_errors_fail_the_whole_batch() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get(MASTER_CSV_PATH),
            Reply::status(429).retry_after(60),
        )
        .await;
        let ids = vec![US_PO_MONTHLY_SA.to_string(), CA_AT_NSA.to_string()];
        let e = FhfaAdapter::new(mock.base_url())
            .fetch_batch(&test_ctx(), &ids, None)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "rate_limited");
    }

    fn contention_delay(n: u32) -> Duration {
        match busy("u", n) {
            CrawlError::Busy { retry_after, .. } => retry_after,
            e => panic!("expected Busy, got {e:?}"),
        }
    }

    #[test]
    fn contention_backoff_is_jittered_exponential_and_capped() {
        for _ in 0..50 {
            let d1 = contention_delay(1);
            let d2 = contention_delay(2);
            assert!(
                d1 >= Duration::from_millis(187) && d1 <= Duration::from_millis(313),
                "{d1:?}"
            );
            assert!(
                d2 >= Duration::from_millis(375) && d2 <= Duration::from_millis(625),
                "{d2:?}"
            );
        }
        assert_eq!(contention_delay(u32::MAX), CONTENTION_MAX_BACKOFF);
    }

    /// Discovery and a fetch batch share one reservation keyed by the master URL: a second caller
    /// while the first is still downloading makes no request and fails with a retryable `Busy`,
    /// and a caller after the first finishes makes its own request normally.
    #[tokio::test]
    async fn concurrent_downloads_are_deduped() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get(MASTER_CSV_PATH),
            Reply::text(MASTER).delay(Duration::from_millis(200)),
        )
        .await;
        let adapter = FhfaAdapter::new(mock.base_url());

        let first = tokio::spawn({
            let adapter = adapter.clone();
            async move { adapter.discover(&test_ctx()).await }
        });
        // Wait for the first request to reach the mock: the reservation is acquired before it,
        // so this means the guard is held. Polls rather than a fixed sleep, so this isn't flaky
        // under CPU contention from other tests running in parallel.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while mock.received_requests().await.is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "first request never reached the mock"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let busy = adapter
            .fetch_series(&test_ctx(), US_PO_MONTHLY_SA, None)
            .await
            .unwrap_err();
        assert_eq!(busy.kind(), "busy");
        assert!(busy.is_retryable());
        assert_eq!(
            mock.received_requests().await.len(),
            1,
            "no second request while the first is in flight"
        );

        let discovered = first.await.unwrap().unwrap();
        assert!(!discovered.is_empty());

        // Released: a normal fetch now makes its own request.
        let ok = adapter
            .fetch_series(&test_ctx(), US_PO_MONTHLY_SA, None)
            .await
            .unwrap();
        assert!(!ok.points.is_empty());
        assert_eq!(mock.received_requests().await.len(), 2);
    }

    /// `refresh_place_labels` merges a label for every `place_id` in the file (states and census
    /// divisions alike) into `fhfa.toml`'s `place_id` dimension, which ships with no codes of its
    /// own. The conditional-GET mechanics are covered at the `HttpFetcher` level (see BLS's
    /// `refresh_code_file_merges_labels_and_caches_etag`); this checks FHFA's own wiring: reading
    /// `place_name` out of the already-parsed file and merging it through `persist`.
    #[tokio::test]
    async fn refresh_place_labels_merges_states_and_divisions() {
        let Some(admin_url) = crate::persist::stable_id_tests::database_url() else {
            return;
        };
        let db = crate::persist::stable_id_tests::FreshDb::create(
            &admin_url,
            "econgraph_fhfa_refresh_place_labels",
        )
        .await;
        let mut catalog = crate::dataset::DatasetCatalog::empty();
        catalog
            .insert(
                SourceId::Fhfa,
                &[DATASET],
                crate::dataset::parse_dataset_file(
                    &std::fs::read_to_string(
                        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("data/datasets/fhfa.toml"),
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
        let master = parse(MASTER);
        FhfaAdapter::default()
            .refresh_place_labels(&ctx, &master)
            .await;

        let codes = {
            use diesel::prelude::*;
            use diesel_async::RunQueryDsl;
            use econ_graph_core::schema::datasets::dsl;
            let mut conn = db.pool.get().await.unwrap();
            let dims: econ_graph_core::models::DatasetComponents = dsl::datasets
                .filter(dsl::code.eq(DATASET))
                .select(dsl::dimensions)
                .first(&mut conn)
                .await
                .unwrap();
            dims.0
                .into_iter()
                .find(|d| d.name == "place_id")
                .unwrap()
                .codes
                .unwrap()
        };
        // US + 9 census divisions + 51 states (including DC): every in-scope place is labelled,
        // not just the divisions a hand-typed list used to cover.
        assert_eq!(codes.len(), 61);
        assert!(codes
            .iter()
            .any(|c| c.code == "USA" && c.label == "United States"));
        assert!(codes
            .iter()
            .any(|c| c.code == "DV_ENC" && c.label == "East North Central Division"));
        assert!(codes.iter().any(|c| c.code == "LA" && c.label == "Louisiana"));

        db.drop().await;
    }
}

#[cfg(test)]
mod contract {
    use super::{FhfaAdapter, MASTER_CSV_PATH};
    use crate::testkit::{Reply, Route};

    crate::adapter_contract_tests! {
        adapter: |base_url: String| FhfaAdapter::new(base_url),
        external_id: "fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa",
        route: Route::get(MASTER_CSV_PATH),
        ok_reply: Reply::text(include_str!("../../tests/fixtures/fhfa/hpi_master.csv")),
        expect_points: 6,
        discover: {
            route: Route::get(MASTER_CSV_PATH),
            reply: Reply::text(include_str!("../../tests/fixtures/fhfa/hpi_master.csv")),
            min_series: 200,
        },
    }
}
