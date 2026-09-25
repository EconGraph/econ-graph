// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! World Bank Indicators API (v2) adapter — discovery only.
//!
//! Ported from `econ-graph-services/src/services/series_discovery/world_bank.rs`. Every request is
//! a `GET` with `format=json`; the API needs no key.
//!
//! # Discovery
//!
//! Same strategies and order as the old code, de-duplicated by indicator id in first-seen order
//! (the old sort + `dedup_by` kept the first-inserted entry as well):
//! 1. `{base}/topic/{3,7,11}/indicator?per_page=1000&page=N` (Economy & Growth, Financial Sector,
//!    Trade), at most [`MAX_TOPIC_PAGES`] pages per topic.
//! 2. `{base}/indicator/{id}` for each of [`KEY_INDICATORS`].
//! 3. `{base}/country/{c}/indicator/{id}?per_page=1` for [`MAJOR_COUNTRIES`] x [`COUNTRY_SAMPLE_INDICATORS`]:
//!    an indicator with data yields a synthetic entry (only kept if 1–2 did not already find it).
//! 4. `{base}/indicator?per_page=100&page=N`, pages 1..=[`MAX_SEARCH_PAGES`], filtered by
//!    [`is_economic_indicator`].
//!
//! Pagination follows the `pages` field of the response metadata and never exceeds the caps, so
//! discovery makes at most `3 * 5 + 10 + 75 + 10 = 110` requests. The old fixed 100 ms sleeps are
//! gone: the per-source rate limiter paces requests.
//!
//! `Auth` and `RateLimited` abort discovery; every other per-request failure is logged and
//! skipped (a failed page ends that walk, keeping earlier pages), unless every request failed.
//!
//! # Response format and errors
//!
//! Successful responses are a two-element array `[meta, items]` (`items` may be `null`). Errors
//! come back as HTTP 200 with a one-element array `[{"message": [{"id", "key", "value"}]}]`;
//! [`classify_world_bank_message`] maps "Invalid value" / unknown-indicator messages (ids 120 and
//! 175) to `NotFound` and anything else to `Permanent`.
//!
//! Note: the old code deserialized `items` as `{"indicator": [...]}`, which does not match the
//! API (the second element is a plain array), so its topic/paginated strategies always failed.
//! This adapter parses the array.
//!
//! # Fetching
//!
//! Not implemented: the old code never parsed indicator *values* (it only checked that the
//! country/indicator endpoint returned a non-empty array), and a discovered indicator id is not
//! tied to a country. [`SourceAdapter::fetch_series`] returns `Permanent`.

use std::collections::HashSet;

use async_trait::async_trait;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;
use crate::source::SourceId;

/// The real World Bank API root.
pub const DEFAULT_BASE_URL: &str = "https://api.worldbank.org/v2";

/// Human-facing indicator page, used for [`DiscoveredSeries::data_url`] (never requested).
const WEB_INDICATOR_URL: &str = "https://data.worldbank.org/indicator";

/// Topics walked first: Economy & Growth, Financial Sector, Trade.
const TOPICS: &[&str] = &["3", "7", "11"];
/// Page size and page cap for topic listings.
const TOPIC_PAGE_SIZE: usize = 1000;
const MAX_TOPIC_PAGES: usize = 5;

/// Key economic indicators fetched by direct lookup.
const KEY_INDICATORS: &[&str] = &[
    "NY.GDP.MKTP.CD",
    "NY.GDP.MKTP.KD.ZG",
    "FP.CPI.TOTL.ZG",
    "SL.UEM.TOTL.ZS",
    "FR.INR.RINR",
    "NE.TRD.GNFS.ZS",
    "GC.DOD.TOTL.GD.ZS",
    "GC.REV.XGRT.GD.ZS",
    "GC.XPN.TOTL.GD.ZS",
    "BN.CAB.XOKA.GD.ZS",
];

/// Countries probed for [`COUNTRY_SAMPLE_INDICATORS`].
const MAJOR_COUNTRIES: &[&str] = &[
    "US", "CN", "DE", "JP", "GB", "FR", "IT", "CA", "AU", "BR", "IN", "RU", "ZA", "MX", "KR",
];

/// Indicators probed per country.
const COUNTRY_SAMPLE_INDICATORS: &[&str] = &[
    "NY.GDP.MKTP.CD",
    "FP.CPI.TOTL.ZG",
    "SL.UEM.TOTL.ZS",
    "NE.TRD.GNFS.ZS",
    "GC.DOD.TOTL.GD.ZS",
];

/// Page size and page cap for the full indicator listing.
const SEARCH_PAGE_SIZE: usize = 100;
const MAX_SEARCH_PAGES: usize = 10;

/// World Bank data is overwhelmingly annual; the indicator endpoint carries no frequency.
const DEFAULT_FREQUENCY: &str = "Annual";

/// World Bank adapter. See the module docs for endpoints and error mapping.
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

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    /// One `GET` returning `(pages, items)`.
    async fn get_list(
        &self,
        ctx: &CrawlCtx,
        path: &str,
        extra: &[(&str, &str)],
    ) -> Result<(Option<u64>, Vec<Value>), CrawlError> {
        let mut query = vec![("format", "json")];
        query.extend_from_slice(extra);
        let body: Value = ctx
            .http
            .get_json(SourceId::WorldBank, &self.url(path), &query)
            .await?;
        parse_list(path, body)
    }

    /// Walks `path` page by page (`per_page`, `page=1..`) until the last page, an empty page, or
    /// `max_pages`. Page 1 failing is an error; a later page failing ends the walk (aborting
    /// errors still propagate).
    async fn walk_pages(
        &self,
        ctx: &CrawlCtx,
        path: &str,
        per_page: usize,
        max_pages: usize,
    ) -> Result<Vec<WbIndicator>, CrawlError> {
        let per_page = per_page.to_string();
        let mut out = Vec::new();
        for page in 1..=max_pages {
            let page_s = page.to_string();
            let result = self
                .get_list(ctx, path, &[("per_page", &per_page), ("page", &page_s)])
                .await
                .and_then(|(pages, items)| Ok((pages, parse_indicators(path, items)?)));
            let (pages, items) = match result {
                Ok(r) => r,
                Err(e) if page == 1 => return Err(e),
                Err(e) => {
                    skip_or_abort(e, path)?;
                    break;
                }
            };
            let got = items.len();
            out.extend(items);
            if got == 0 || pages.is_none_or(|p| page as u64 >= p) {
                break;
            }
        }
        Ok(out)
    }

    async fn single_indicator(&self, ctx: &CrawlCtx, id: &str) -> Result<WbIndicator, CrawlError> {
        let path = format!("/indicator/{id}");
        let (_, items) = self.get_list(ctx, &path, &[]).await?;
        parse_indicators(&path, items)?
            .into_iter()
            .next()
            .ok_or_else(|| CrawlError::NotFound(format!("World Bank indicator {id}: no result")))
    }

    /// Whether `country` has any data for `indicator` (one observation requested).
    async fn country_has_data(
        &self,
        ctx: &CrawlCtx,
        country: &str,
        indicator: &str,
    ) -> Result<bool, CrawlError> {
        let path = format!("/country/{country}/indicator/{indicator}");
        let (_, items) = self.get_list(ctx, &path, &[("per_page", "1")]).await?;
        Ok(!items.is_empty())
    }
}

impl Default for WorldBankAdapter {
    fn default() -> Self {
        Self::new(DEFAULT_BASE_URL)
    }
}

/// Collects discovery results: first-seen de-duplication plus the skip/abort bookkeeping.
#[derive(Default)]
struct Collector {
    seen: HashSet<String>,
    found: Vec<DiscoveredSeries>,
    any_ok: bool,
    last_err: Option<CrawlError>,
}

impl Collector {
    fn add(&mut self, indicators: impl IntoIterator<Item = WbIndicator>) {
        self.any_ok = true;
        for i in indicators {
            if self.seen.insert(i.id.clone()) {
                self.found.push(to_discovered(i));
            }
        }
    }

    fn fail(&mut self, err: CrawlError, what: &str) -> Result<(), CrawlError> {
        self.last_err = Some(skip_or_abort(err, what)?);
        Ok(())
    }

    fn finish(self) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        match (self.any_ok, self.last_err) {
            (false, Some(e)) => Err(e),
            _ => {
                tracing::info!(series = self.found.len(), "World Bank discovery finished");
                Ok(self.found)
            }
        }
    }
}

#[async_trait]
impl SourceAdapter for WorldBankAdapter {
    fn id(&self) -> SourceId {
        SourceId::WorldBank
    }

    /// See the module docs for the four strategies, caps and error handling.
    async fn discover(&self, ctx: &CrawlCtx) -> Result<Vec<DiscoveredSeries>, CrawlError> {
        let mut c = Collector::default();

        for topic in TOPICS {
            let path = format!("/topic/{topic}/indicator");
            match self
                .walk_pages(ctx, &path, TOPIC_PAGE_SIZE, MAX_TOPIC_PAGES)
                .await
            {
                Ok(found) => c.add(found),
                Err(e) => c.fail(e, &path)?,
            }
        }

        for id in KEY_INDICATORS {
            match self.single_indicator(ctx, id).await {
                Ok(found) => c.add([found]),
                Err(e) => c.fail(e, id)?,
            }
        }

        for country in MAJOR_COUNTRIES {
            for indicator in COUNTRY_SAMPLE_INDICATORS {
                match self.country_has_data(ctx, country, indicator).await {
                    Ok(true) => c.add([country_sample(country, indicator)]),
                    Ok(false) => c.any_ok = true,
                    Err(e) => c.fail(e, &format!("{country}/{indicator}"))?,
                }
            }
        }

        match self
            .walk_pages(ctx, "/indicator", SEARCH_PAGE_SIZE, MAX_SEARCH_PAGES)
            .await
        {
            Ok(found) => c.add(found.into_iter().filter(is_economic_indicator)),
            Err(e) => c.fail(e, "/indicator")?,
        }

        c.finish()
    }

    /// Not implemented; see the module docs.
    async fn fetch_series(
        &self,
        _ctx: &CrawlCtx,
        _external_id: &str,
        _since: Option<NaiveDate>,
    ) -> Result<FetchedSeries, CrawlError> {
        Err(CrawlError::Permanent(
            "WORLD_BANK fetch_series not implemented yet".into(),
        ))
    }
}

/// Returns `err` for aborting errors (auth, rate limiting); otherwise logs it and hands it back
/// to be remembered as the last failure.
fn skip_or_abort(err: CrawlError, what: &str) -> Result<CrawlError, CrawlError> {
    match err {
        CrawlError::Auth(_) | CrawlError::RateLimited { .. } => Err(err),
        e => {
            tracing::warn!(query = what, error = %e, "World Bank request failed; skipping");
            Ok(e)
        }
    }
}

/// Splits a `[meta, items]` response into `(pages, items)`; a `[{"message": ..}]` body becomes
/// an error via [`classify_world_bank_message`].
fn parse_list(what: &str, body: Value) -> Result<(Option<u64>, Vec<Value>), CrawlError> {
    let Value::Array(mut parts) = body else {
        return Err(CrawlError::Parse(format!(
            "World Bank {what}: response is not an array"
        )));
    };
    if let Some(messages) = parts.first().and_then(|m| m.get("message")) {
        return Err(classify_world_bank_message(what, messages));
    }
    if parts.len() < 2 {
        return Ok((None, Vec::new()));
    }
    let items = match parts.swap_remove(1) {
        Value::Null => Vec::new(),
        Value::Array(items) => items,
        other => {
            return Err(CrawlError::Parse(format!(
                "World Bank {what}: expected an item array, got {}",
                type_name(&other)
            )))
        }
    };
    let pages = parts
        .first()
        .and_then(|meta| meta.get("pages"))
        .and_then(|p| {
            p.as_u64()
                .or_else(|| p.as_str().and_then(|s| s.trim().parse().ok()))
        });
    Ok((pages, items))
}

fn parse_indicators(what: &str, items: Vec<Value>) -> Result<Vec<WbIndicator>, CrawlError> {
    serde_json::from_value(Value::Array(items))
        .map_err(|e| CrawlError::Parse(format!("World Bank {what}: bad indicator list: {e}")))
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

/// Same keyword / id-prefix test as the old `is_economic_indicator`.
pub fn is_economic_indicator(indicator: &WbIndicator) -> bool {
    const KEYWORDS: &[&str] = &[
        "gdp",
        "gross domestic product",
        "inflation",
        "unemployment",
        "interest rate",
        "exchange rate",
        "trade",
        "debt",
        "revenue",
        "expenditure",
        "current account",
        "balance of payments",
        "economic",
        "financial",
        "monetary",
        "fiscal",
        "price",
        "wage",
        "income",
        "consumption",
        "investment",
        "savings",
        "export",
        "import",
        "balance",
        "surplus",
        "deficit",
        "budget",
    ];
    const ID_PATTERNS: &[&str] = &[
        "ny.gdp", "fp.cpi", "sl.uem", "fr.inr", "ne.trd", "gc.rev", "gc.xpn", "bn.cab", "dt.dod",
        "ic.tax", "ic.bus", "ic.reg", "ic.gov", "ic.lgl", "ie.tic", "ie.tra", "ie.tec", "ie.eng",
        "ie.ene", "ie.env", "ie.hea", "ie.edu", "ie.agr", "ie.fin", "ie.inf", "ie.urb", "ie.rur",
        "ie.gen",
    ];
    let name = indicator.name.to_lowercase();
    let id = indicator.id.to_lowercase();
    KEYWORDS.iter().any(|k| name.contains(k)) || ID_PATTERNS.iter().any(|p| id.contains(p))
}

/// The synthetic entry the old code created for an indicator seen via a country probe.
fn country_sample(country: &str, indicator: &str) -> WbIndicator {
    WbIndicator {
        id: indicator.to_string(),
        name: format!("{indicator} for {country}"),
        unit: None,
        source_note: Some(format!("Available for country: {country}")),
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn to_discovered(i: WbIndicator) -> DiscoveredSeries {
    DiscoveredSeries {
        data_url: Some(format!("{WEB_INDICATOR_URL}/{}", i.id)),
        external_id: i.id,
        title: i.name,
        description: non_empty(i.source_note),
        units: non_empty(i.unit),
        frequency: Some(DEFAULT_FREQUENCY.to_string()),
    }
}

// ---- Wire format (only the fields we use) ----

/// One entry of an indicator list (`/indicator`, `/topic/{id}/indicator`, `/indicator/{id}`).
#[derive(Debug, Clone, Deserialize)]
pub struct WbIndicator {
    /// Indicator code, e.g. `NY.GDP.MKTP.CD`.
    pub id: String,
    /// Indicator name.
    pub name: String,
    /// Unit, usually empty.
    #[serde(default)]
    pub unit: Option<String>,
    /// Long description.
    #[serde(default, rename = "sourceNote")]
    pub source_note: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{test_ctx, MockSource, Reply, Route};

    const TOPIC: &str = include_str!("../../tests/fixtures/world_bank/topic_indicators.json");
    const SINGLE: &str = include_str!("../../tests/fixtures/world_bank/indicator_single.json");
    const PAGE1: &str = include_str!("../../tests/fixtures/world_bank/indicators_page1.json");
    const PAGE2: &str = include_str!("../../tests/fixtures/world_bank/indicators_page2.json");
    const COUNTRY: &str = include_str!("../../tests/fixtures/world_bank/country_indicator.json");
    const INVALID: &str = include_str!("../../tests/fixtures/world_bank/error_invalid_value.json");

    fn ids(found: &[DiscoveredSeries]) -> Vec<&str> {
        found.iter().map(|s| s.external_id.as_str()).collect()
    }

    fn count(reqs: &[wiremock::Request], path: &str) -> usize {
        reqs.iter().filter(|r| r.url.path() == path).count()
    }

    #[test]
    fn constructor_convention() {
        assert_eq!(WorldBankAdapter::default().base_url, DEFAULT_BASE_URL);
        assert_eq!(WorldBankAdapter::new("http://x/").base_url, "http://x");
        assert_eq!(WorldBankAdapter::default().id(), SourceId::WorldBank);
    }

    #[tokio::test]
    async fn fetch_series_is_not_implemented() {
        let mock = MockSource::start().await;
        let e = WorldBankAdapter::new(mock.base_url())
            .fetch_series(&test_ctx(), "NY.GDP.MKTP.CD", None)
            .await
            .unwrap_err();
        assert_eq!(
            e,
            CrawlError::Permanent("WORLD_BANK fetch_series not implemented yet".into())
        );
        assert!(mock.received_requests().await.is_empty());
    }

    #[tokio::test]
    async fn discover_runs_all_strategies_and_dedupes() {
        let mock = MockSource::start().await;
        // Topic 3 has two indicators; 7 is empty; 11 fails (skipped).
        mock.mount_expect(
            &Route::get("/topic/3/indicator")
                .query("format", "json")
                .query("per_page", "1000")
                .query("page", "1"),
            Reply::json_str(TOPIC),
            1,
        )
        .await;
        mock.mount(
            &Route::get("/topic/11/indicator"),
            Reply::text("boom").with_status(500),
        )
        .await;
        mock.mount(
            &Route::get("/topic/7/indicator"),
            Reply::json(serde_json::json!([{"page": 1, "pages": 0, "total": 0}, null])),
        )
        .await;
        // Key lookups: FR.INR.RINR found; NY.GDP.MKTP.CD duplicates topic 3; the rest invalid.
        mock.mount(
            &Route::get("/indicator/FR.INR.RINR"),
            Reply::json_str(SINGLE),
        )
        .await;
        mock.mount(
            &Route::get("/indicator/NY.GDP.MKTP.CD"),
            Reply::json_str(TOPIC),
        )
        .await;
        // Country probe: only JP has GC.DOD.TOTL.GD.ZS (not found by any other strategy).
        mock.mount(
            &Route::get("/country/JP/indicator/GC.DOD.TOTL.GD.ZS").query("per_page", "1"),
            Reply::json_str(COUNTRY),
        )
        .await;
        mock.mount(
            &Route::get("/country/US/indicator/NY.GDP.MKTP.CD"),
            Reply::json_str(COUNTRY),
        )
        .await;
        // Two pages of the full listing; non-economic entries are filtered out.
        mock.mount_expect(
            &Route::get("/indicator")
                .query("per_page", "100")
                .query("page", "1"),
            Reply::json_str(PAGE1),
            1,
        )
        .await;
        mock.mount_expect(
            &Route::get("/indicator")
                .query("per_page", "100")
                .query("page", "2"),
            Reply::json_str(PAGE2),
            1,
        )
        .await;
        // Everything else: the in-body "Invalid value" error.
        mock.server()
            .register(
                wiremock::Mock::given(wiremock::matchers::method("GET")).respond_with(
                    wiremock::ResponseTemplate::new(200)
                        .set_body_raw(INVALID.as_bytes().to_vec(), "application/json"),
                ),
            )
            .await;

        let found = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        assert_eq!(
            ids(&found),
            [
                "NY.GDP.MKTP.CD",
                "NY.GDP.MKTP.KD.ZG",
                "FR.INR.RINR",
                "GC.DOD.TOTL.GD.ZS",
                "BN.CAB.XOKA.GD.ZS",
                "DT.DOD.DECT.CD",
            ]
        );
        let gdp = &found[0];
        assert_eq!(gdp.title, "GDP (current US$)");
        assert!(gdp
            .description
            .as_deref()
            .unwrap()
            .starts_with("GDP at purchaser"));
        assert_eq!(gdp.units, None, "empty unit -> None");
        assert_eq!(gdp.frequency.as_deref(), Some("Annual"));
        assert_eq!(
            gdp.data_url.as_deref(),
            Some("https://data.worldbank.org/indicator/NY.GDP.MKTP.CD")
        );
        assert_eq!(found[3].title, "GC.DOD.TOTL.GD.ZS for JP");

        let reqs = mock.received_requests().await;
        assert!(reqs.iter().all(|r| r
            .url
            .query_pairs()
            .any(|(k, v)| k == "format" && v == "json")));
        // 1 + 1 + (3 in-process attempts on 500) topic requests, 10 key lookups,
        // 15 * 5 country probes, 2 listing pages.
        assert_eq!(count(&reqs, "/topic/11/indicator"), 3);
        assert_eq!(
            reqs.len(),
            2 + 3
                + KEY_INDICATORS.len()
                + MAJOR_COUNTRIES.len() * COUNTRY_SAMPLE_INDICATORS.len()
                + 2
        );
        mock.server().verify().await;
    }

    #[tokio::test]
    async fn discover_page_count_is_bounded() {
        let mock = MockSource::start().await;
        // Claims a huge page count, forever.
        let endless = PAGE1.replace("\"pages\": 2", "\"pages\": 1000000");
        assert_ne!(endless, PAGE1);
        mock.mount(&Route::get("/indicator"), Reply::json_str(endless.clone()))
            .await;
        for topic in TOPICS {
            mock.mount(
                &Route::get(format!("/topic/{topic}/indicator")),
                Reply::json_str(endless.clone()),
            )
            .await;
        }
        let found = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap();
        assert!(ids(&found).contains(&"BN.CAB.XOKA.GD.ZS"));
        let reqs = mock.received_requests().await;
        assert_eq!(count(&reqs, "/indicator"), MAX_SEARCH_PAGES);
        for topic in TOPICS {
            assert_eq!(
                count(&reqs, &format!("/topic/{topic}/indicator")),
                MAX_TOPIC_PAGES
            );
        }
    }

    #[tokio::test]
    async fn discover_aborts_on_rate_limit_and_auth() {
        for (reply, kind) in [
            (Reply::status(429).retry_after(60), "rate_limited"),
            (Reply::status(403), "auth"),
        ] {
            let mock = MockSource::start().await;
            mock.mount(&Route::get("/topic/3/indicator"), reply).await;
            let e = WorldBankAdapter::new(mock.base_url())
                .discover(&test_ctx())
                .await
                .unwrap_err();
            assert_eq!(e.kind(), kind);
            assert_eq!(mock.received_requests().await.len(), 1);
        }
    }

    #[tokio::test]
    async fn discover_fails_when_everything_fails() {
        let mock = MockSource::start().await;
        let e = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        // Nothing mounted: every request 404s.
        assert_eq!(e.kind(), "not_found");
    }

    #[test]
    fn parse_list_shapes() {
        let (pages, items) = parse_list("x", serde_json::from_str(PAGE1).unwrap()).unwrap();
        assert_eq!(pages, Some(2));
        assert_eq!(items.len(), 2);
        // `pages` as a string, and `null` items.
        let (pages, items) = parse_list("x", serde_json::json!([{"pages": "3"}, null])).unwrap();
        assert_eq!((pages, items.len()), (Some(3), 0));
        assert_eq!(
            parse_list("x", serde_json::json!({"a": 1}))
                .unwrap_err()
                .kind(),
            "parse"
        );
        let e = parse_list("x", serde_json::from_str(INVALID).unwrap()).unwrap_err();
        assert_eq!(e.kind(), "not_found", "{e}");
        let other = classify_world_bank_message(
            "x",
            &serde_json::json!([{"id": "999", "key": "Something", "value": "else"}]),
        );
        assert_eq!(other.kind(), "permanent");
    }

    #[test]
    fn economic_filter() {
        let ind = |id: &str, name: &str| WbIndicator {
            id: id.into(),
            name: name.into(),
            unit: None,
            source_note: None,
        };
        assert!(is_economic_indicator(&ind(
            "NY.GDP.MKTP.CD",
            "GDP (current US$)"
        )));
        assert!(is_economic_indicator(&ind("DT.DOD.X", "Something")));
        assert!(!is_economic_indicator(&ind(
            "SH.STA.ACSN",
            "Improved sanitation facilities (% of population with access)"
        )));
    }
}

/// Discovery contract (fetch_series is not implemented, so the fetch half of
/// `adapter_contract_tests!` does not apply; these use the same testkit assertions).
#[cfg(test)]
mod contract {
    use super::WorldBankAdapter;
    use crate::testkit::contract::{assert_discover_ok, MALFORMED_JSON};
    use crate::testkit::{test_ctx, MockSource, Reply, Route};
    use crate::SourceAdapter;

    #[tokio::test]
    async fn contract_discover_ok() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/indicator"),
            Reply::json_str(include_str!(
                "../../tests/fixtures/world_bank/indicators_page1.json"
            )),
        )
        .await;
        let adapter = WorldBankAdapter::new(mock.base_url());
        assert_discover_ok(&adapter, &test_ctx(), &mock, 1).await;
    }

    /// Every request answers `status` with `body`.
    async fn mount_everything(mock: &MockSource, status: u16, body: &str) {
        mock.server()
            .register(
                wiremock::Mock::given(wiremock::matchers::method("GET"))
                    .respond_with(wiremock::ResponseTemplate::new(status).set_body_string(body)),
            )
            .await;
    }

    #[tokio::test]
    async fn contract_discover_429_rate_limited() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/topic/3/indicator"),
            Reply::status(429).retry_after(1),
        )
        .await;
        let e = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "rate_limited", "{e}");
    }

    #[tokio::test]
    async fn contract_discover_500_transient() {
        // Every request fails; the listing (the last request) with a 500, the rest 404 (an
        // all-500 mock would spend ~1 s of in-process backoff per request). Discovery reports
        // the last failure.
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/indicator"),
            Reply::text("internal error").with_status(500),
        )
        .await;
        let e = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "transient", "{e}");
    }

    #[tokio::test]
    async fn contract_discover_malformed_is_parse_error() {
        let mock = MockSource::start().await;
        mount_everything(&mock, 200, MALFORMED_JSON).await;
        let e = WorldBankAdapter::new(mock.base_url())
            .discover(&test_ctx())
            .await
            .unwrap_err();
        assert_eq!(e.kind(), "parse", "{e}");
    }
}
