// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! HTTP client for the daily aggregates of Massive, a market data vendor.
//!
//! # Requests
//!
//! Every request is a `GET` under the API root ([`DEFAULT_BASE_URL`]), always with
//! `adjusted=false`: the API defaults to split-adjusted prices, and the crawler stores prices as
//! traded.
//!
//! - Per-ticker aggregates, `/v2/aggs/ticker/{ticker}/range/1/day/{from}/{to}` with
//!   `sort=asc&limit=50000`: one ticker's daily bars over a date range (both ends inclusive).
//!   A response with more bars than `limit` carries a `next_url`, which is followed until it is
//!   absent, and only while it stays on the API's own host, so the key goes nowhere else.
//! - Grouped daily, `/v2/aggs/grouped/locale/us/market/stocks/{date}`: every US stock's bar for
//!   one day, in one response.
//!
//! The key is sent as `Authorization: Bearer <key>`, never in the URL, so no logged or stored URL
//! carries it, and [`HttpFetcher::get_text_bearer`] scrubs it from error messages too. Requests go
//! through `ctx.http`, so they keep to the source's rate limit and concurrency
//! ([`SourcePolicy`](crate::policy::SourcePolicy)).
//!
//! # Responses
//!
//! `{"status": "OK", "adjusted": false, "results": [{"T", "o", "h", "l", "c", "v", "vw", "t", "n"}]}`.
//! `T` (the ticker) is only in grouped results; a per-ticker response names it once, at the top.
//! Prices and volume are read from the response text digit for digit
//! ([`parse_observation_value`]), never through a float, so `184.25` stays `184.25`, a fractional
//! volume such as `25933.6` is kept, and `7.0790813e+07` is exactly `70790813`. `o`, `h`, `l`, `c`
//! and `v` are required; `vw` may be missing. A day with no trades has no bar: a market holiday's
//! grouped response, and a ticker with no data, have no `results`, which is an empty list, not an
//! error. `status` `OK` and `DELAYED` (a plan with delayed data) are success; anything else, or
//! `adjusted: true`, is a parse error, since storing split-adjusted prices as unadjusted ones
//! would be silently wrong.
//!
//! A per-ticker bar's `t` is the Unix time in milliseconds of the start of its window, midnight in
//! New York, which is 04:00 or 05:00 UTC on the same calendar date, so the bar's date is `t`'s UTC
//! date. A grouped bar's `t` is the end of its window, so its date is the requested date instead.
//!
//! # Errors
//!
//! `401` (an unknown key), and `403` from a plan that isn't entitled to the data, are
//! [`ClientError::AuthRejected`], and each increments
//! `crawler_source_auth_rejections_total{source="MASSIVE"}`, which the `MassiveApiKeyRejected`
//! alert watches. A `403` saying the plan doesn't include the requested timeframe is
//! [`ClientError::OutsidePlanWindow`] instead: the key works, but the date is older than the
//! plan's history, so it is not counted as a rejection. `429` is [`ClientError::RateLimited`],
//! with the server's `Retry-After` when it sends one; the paid plans set no request limit, so a
//! `429` there is a fault worth seeing. Other statuses map as for every source
//! ([`CrawlError::from_status`]).

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate};
use serde::Deserialize;
use serde_json::value::RawValue;

use crate::adapter::CrawlCtx;
use crate::error::CrawlError;
use crate::http::HttpFetcher;
use crate::source::SourceId;
use crate::value::parse_observation_value;

pub use super::dataset::DailyBar;

/// The real Massive REST API root.
pub const DEFAULT_BASE_URL: &str = "https://api.massive.com";

/// Most bars one per-ticker request asks for: the API's maximum. 50,000 trading days is about
/// two centuries, so a range is in practice one page.
pub const RANGE_LIMIT: u32 = 50_000;

/// Most `next_url` pages followed for one range, against a server that never stops paging.
const MAX_PAGES: usize = 100;

/// The error when `MASSIVE_API_KEY` is missing.
pub const MISSING_KEY: &str =
    "MASSIVE_API_KEY not set: Massive refuses requests without an API key";

/// Words of the `403` message Massive sends for a date older than the plan's history ("Your
/// plan doesn't include this data timeframe"), compared case-insensitively.
const TIMEFRAME_MARKER: &str = "timeframe";

/// Why a Massive request failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    /// No API key is configured. No request was made.
    #[error("{MISSING_KEY}")]
    MissingKey,
    /// HTTP 401, or 403 for data the plan isn't entitled to. The subscription may have lapsed.
    #[error("Massive rejected the API key: {0}")]
    AuthRejected(String),
    /// HTTP 403 saying the plan doesn't include this data timeframe: `date` is older than the
    /// plan's history. The key itself works.
    #[error("Massive: {date} is outside the plan's history: {message}")]
    OutsidePlanWindow {
        /// The requested day (a range's start).
        date: NaiveDate,
        /// The server's message, with the key scrubbed.
        message: String,
    },
    /// HTTP 429: too many requests.
    #[error("Massive rate limited the request{}", .retry_after.map(|d| format!(" (retry after {}s)", d.as_secs())).unwrap_or_default())]
    RateLimited {
        /// The server's `Retry-After`, if it sent one.
        retry_after: Option<Duration>,
    },
    /// The response body was not the expected shape.
    #[error("Massive response: {0}")]
    Parse(String),
    /// Any other failure (transport, 5xx, 404, ...), as the HTTP layer classified it.
    #[error(transparent)]
    Other(CrawlError),
}

impl From<ClientError> for CrawlError {
    fn from(e: ClientError) -> Self {
        match e {
            ClientError::MissingKey => CrawlError::Auth(MISSING_KEY.into()),
            ClientError::AuthRejected(m) => CrawlError::Auth(m),
            // Retrying can't help: the plan's history ends after this date.
            e @ ClientError::OutsidePlanWindow { .. } => CrawlError::Permanent(e.to_string()),
            ClientError::RateLimited { retry_after } => CrawlError::RateLimited { retry_after },
            ClientError::Parse(m) => CrawlError::Parse(m),
            ClientError::Other(e) => e,
        }
    }
}

/// Daily bars from Massive. [`MassiveClient`] is the real one; tests (the adapter's and the
/// backfill's) fake it.
#[async_trait]
pub trait DailyBars: Send + Sync {
    /// `ticker`'s unadjusted daily bars from `from` to `to`, both inclusive, oldest first. Empty
    /// when the ticker has no bars in the range.
    async fn ticker_range(
        &self,
        ticker: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DailyBar>, ClientError>;

    /// Every US stock's unadjusted bar for `date`. Empty on a day the market was closed.
    async fn grouped(&self, date: NaiveDate) -> Result<Vec<DailyBar>, ClientError>;
}

/// The Massive REST client. Cheap to clone. `Debug` leaves the key out.
#[derive(Clone)]
pub struct MassiveClient {
    http: HttpFetcher,
    base_url: String,
    api_key: String,
}

impl std::fmt::Debug for MassiveClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MassiveClient")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl MassiveClient {
    /// A client for the API at `base_url` (e.g. [`DEFAULT_BASE_URL`]), sending `api_key`, making
    /// its requests through `http` under `SourceId::Massive`'s policy.
    pub fn new(http: HttpFetcher, base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        }
    }

    /// A client using `ctx`'s HTTP fetcher and Massive key. [`ClientError::MissingKey`] when
    /// `MASSIVE_API_KEY` is not set.
    pub fn from_ctx(ctx: &CrawlCtx, base_url: impl Into<String>) -> Result<Self, ClientError> {
        let key = ctx
            .keys
            .get(SourceId::Massive)
            .ok_or(ClientError::MissingKey)?;
        Ok(Self::new(ctx.http.clone(), base_url, key))
    }

    /// GETs `url` with `query` and the key, and parses the body as an aggregates response.
    /// `date` is the requested day (a range's start), for [`ClientError::OutsidePlanWindow`].
    async fn get(
        &self,
        what: &str,
        url: &str,
        query: &[(&str, &str)],
        date: NaiveDate,
    ) -> Result<AggsResponse, ClientError> {
        let body = self
            .http
            .get_text_bearer(SourceId::Massive, url, query, &self.api_key)
            .await
            .map_err(|e| classify(e, date))?;
        // The HTTP layer scrubs the key from its own errors, but this body didn't pass through
        // that scrubbing, and a parse error quotes it.
        parse_response(what, &body).map_err(|e| match e {
            ClientError::Parse(m) => {
                ClientError::Parse(crate::http::scrub(&m, std::slice::from_ref(&self.api_key)))
            }
            e => e,
        })
    }

    /// `next` if it is on the API's own host (scheme, host and port), so the key is never sent
    /// anywhere else.
    fn checked_next_url<'a>(&self, next: &'a str) -> Result<&'a str, ClientError> {
        let origin = |u: &str| url::Url::parse(u).ok().map(|u| u.origin());
        match (origin(next), origin(&self.base_url)) {
            (Some(a), Some(b)) if a == b => Ok(next),
            _ => Err(ClientError::Parse(
                "next_url points outside the API's host; not following it".into(),
            )),
        }
    }
}

#[async_trait]
impl DailyBars for MassiveClient {
    async fn ticker_range(
        &self,
        ticker: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DailyBar>, ClientError> {
        let what = format!("{ticker} {from}..{to}");
        let limit = RANGE_LIMIT.to_string();
        let first = format!(
            "{}/v2/aggs/ticker/{}/range/1/day/{from}/{to}",
            self.base_url,
            encode_path_segment(ticker)
        );
        let query = [("adjusted", "false"), ("sort", "asc"), ("limit", &limit)];
        let mut response = self.get(&what, &first, &query, from).await?;
        let mut bars = response.bars(&what, Some(ticker), None)?;
        let mut pages = 1;
        while let Some(next) = response.next_url.take() {
            if pages >= MAX_PAGES {
                return Err(ClientError::Parse(format!(
                    "{what}: still paging after {MAX_PAGES} pages"
                )));
            }
            let next = self.checked_next_url(&next)?.to_string();
            // `next_url` carries the original query (adjusted=false included) and a cursor.
            response = self.get(&what, &next, &[], from).await?;
            bars.extend(response.bars(&what, Some(ticker), None)?);
            pages += 1;
        }
        Ok(bars)
    }

    async fn grouped(&self, date: NaiveDate) -> Result<Vec<DailyBar>, ClientError> {
        let what = format!("grouped {date}");
        let url = format!(
            "{}/v2/aggs/grouped/locale/us/market/stocks/{date}",
            self.base_url
        );
        let response = self
            .get(&what, &url, &[("adjusted", "false")], date)
            .await?;
        response.bars(&what, None, Some(date))
    }
}

/// `ticker` as one URL path segment. Class shares such as `BRK.B` need no escaping.
fn encode_path_segment(ticker: &str) -> String {
    url::form_urlencoded::byte_serialize(ticker.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

/// The HTTP layer's error as a [`ClientError`]. 401 and 403 (`Auth`) are key rejections, and
/// counted, except a 403 for a timeframe outside the plan. `date` is the requested day.
fn classify(e: CrawlError, date: NaiveDate) -> ClientError {
    match e {
        CrawlError::Auth(m)
            if m.starts_with("HTTP 403") && m.to_ascii_lowercase().contains(TIMEFRAME_MARKER) =>
        {
            ClientError::OutsidePlanWindow { date, message: m }
        }
        CrawlError::Auth(m) => {
            record_auth_rejection();
            tracing::warn!(source = %SourceId::Massive, error = %m,
                "Massive rejected the API key (401 or 403); the subscription may have lapsed");
            ClientError::AuthRejected(m)
        }
        CrawlError::RateLimited { retry_after } => ClientError::RateLimited { retry_after },
        CrawlError::Parse(m) => ClientError::Parse(m),
        e => ClientError::Other(e),
    }
}

/// Increments `crawler_source_auth_rejections_total{source="MASSIVE"}`.
fn record_auth_rejection() {
    econ_graph_metrics::crawler::CRAWLER_SOURCE_HEALTH_METRICS
        .record_auth_rejection(SourceId::Massive.as_str());
}

/// An aggregates response (per-ticker or grouped), with the values kept as written.
#[derive(Debug, Deserialize)]
struct AggsResponse {
    status: String,
    #[serde(default)]
    adjusted: Option<bool>,
    #[serde(default)]
    ticker: Option<String>,
    #[serde(default)]
    results: Option<Vec<RawBar>>,
    #[serde(default)]
    next_url: Option<String>,
}

/// One bar, its numbers still as response text.
#[derive(Debug, Deserialize)]
struct RawBar {
    #[serde(rename = "T", default)]
    ticker: Option<String>,
    o: Box<RawValue>,
    h: Box<RawValue>,
    l: Box<RawValue>,
    c: Box<RawValue>,
    v: Box<RawValue>,
    #[serde(default)]
    vw: Option<Box<RawValue>>,
    #[serde(default)]
    t: Option<i64>,
}

/// Parses an aggregates body, checking its status and that it is unadjusted.
fn parse_response(what: &str, body: &str) -> Result<AggsResponse, ClientError> {
    let response: AggsResponse = serde_json::from_str(body).map_err(|e| {
        ClientError::Parse(format!(
            "{what}: {e}: {}",
            body.chars().take(200).collect::<String>()
        ))
    })?;
    if !matches!(response.status.as_str(), "OK" | "DELAYED") {
        return Err(ClientError::Parse(format!(
            "{what}: status {:?}",
            response.status
        )));
    }
    if response.adjusted == Some(true) {
        return Err(ClientError::Parse(format!(
            "{what}: the response is split-adjusted, but adjusted=false was asked for"
        )));
    }
    Ok(response)
}

impl AggsResponse {
    /// The bars, for `ticker` (a per-ticker response) or each result's own `T` (grouped), dated
    /// `date` when given (grouped) or else by `t`.
    fn bars(
        &self,
        what: &str,
        ticker: Option<&str>,
        date: Option<NaiveDate>,
    ) -> Result<Vec<DailyBar>, ClientError> {
        if let (Some(asked), Some(got)) = (ticker, self.ticker.as_deref())
            && asked != got
        {
            return Err(ClientError::Parse(format!(
                "{what}: the response is for ticker {got}"
            )));
        }
        let Some(results) = &self.results else {
            return Ok(Vec::new());
        };
        results
            .iter()
            .map(|raw| {
                let ticker = match (ticker, raw.ticker.as_deref()) {
                    (Some(t), _) | (None, Some(t)) => t.to_string(),
                    (None, None) => {
                        return Err(ClientError::Parse(format!(
                            "{what}: a bar has no ticker (T)"
                        )))
                    }
                };
                // A grouped bar's `t` is its window's end: the day is the one requested.
                let bar_date = match date {
                    Some(day) => day,
                    None => raw
                        .t
                        .and_then(DateTime::from_timestamp_millis)
                        .map(|t| t.date_naive())
                        .ok_or_else(|| {
                            ClientError::Parse(format!(
                                "{what}: {ticker}: bad or missing timestamp (t)"
                            ))
                        })?,
                };
                let num = |field: &str, v: &RawValue| {
                    parse_observation_value(v.get()).map_err(|e| {
                        ClientError::Parse(format!("{what}: {ticker} {bar_date} {field}: {e}"))
                    })
                };
                Ok(DailyBar {
                    open: num("o", &raw.o)?,
                    high: num("h", &raw.h)?,
                    low: num("l", &raw.l)?,
                    close: num("c", &raw.c)?,
                    volume: num("v", &raw.v)?,
                    vwap: raw.vw.as_deref().map(|v| num("vw", v)).transpose()?,
                    date: bar_date,
                    ticker,
                })
            })
            .collect()
    }
}
