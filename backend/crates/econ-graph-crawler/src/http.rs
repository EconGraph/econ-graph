// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The single HTTP client every adapter uses.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use econ_graph_metrics::crawler::CRAWLER_METRICS;
use rand::RngExt;
use reqwest::header::{
    HeaderMap, ACCEPT, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED,
    RETRY_AFTER,
};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::rate_limit::SourceRateLimiter;
use crate::source::SourceId;

/// Extra in-process attempts after the first one for retryable errors.
const EXTRA_ATTEMPTS: u32 = 2;
/// First in-process retry delay; multiplied by 4 per attempt (250ms, 1s), ±25% jitter.
const RETRY_BASE_DELAY: Duration = Duration::from_millis(250);
/// A `Retry-After` longer than this is not slept in-process: the error is returned so the
/// queue can reschedule the job instead of holding a worker.
const MAX_IN_PROCESS_RETRY_AFTER: Duration = Duration::from_secs(5);
/// Maximum characters of a response body quoted in a [`CrawlError::Parse`] message.
const SNIPPET_CHARS: usize = 200;
/// Query parameters whose values are replaced by [`REDACTED`] in logs and errors
/// (compared case-insensitively).
const SECRET_PARAMS: &[&str] = &[
    "api_key",
    "apikey",
    "registrationkey",
    "userid",
    "key",
    "token",
];
const REDACTED: &str = "REDACTED";

/// Client-wide HTTP settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpConfig {
    /// Total per-request timeout (connect + body).
    pub timeout: Duration,
    /// `User-Agent` header sent on every request. SEC requires a contact address in it.
    pub user_agent: String,
}

impl Default for HttpConfig {
    /// 30s timeout; `User-Agent: EconGraph/<crate version> (+https://github.com/jmalicki/econ-graph; jmalicki+econgraph@gmail.com)`.
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            user_agent: format!(
                "EconGraph/{} (+https://github.com/jmalicki/econ-graph; jmalicki+econgraph@gmail.com)",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }
}

/// A successful (or `304`) response: the status, the response's `ETag` and `Last-Modified` if it
/// sent them, and the body (empty for `304`).
struct RawResponse {
    status: StatusCode,
    etag: Option<String>,
    last_modified: Option<String>,
    body: String,
}

/// What a later fetch needs to tell whether a source resource changed since it was last read.
///
/// `etag` and `last_modified` are the response headers, sent back as `If-None-Match` and
/// `If-Modified-Since`. `content_sha256` is the hex SHA-256 of the body, which catches an
/// unchanged re-download from a server that honours neither header. `version` is a
/// version the adapter sets itself (FRED's `last_updated`, the World Bank's `lastupdated`, or the
/// adapter's own parser version); the fetcher never compares it, and keeps `known`'s on an
/// [`IfChanged::Unchanged`] result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_sha256: Option<String>,
    pub version: Option<String>,
}

/// Outcome of [`HttpFetcher::get_text_if_changed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IfChanged {
    /// The resource is the one `known` described: the server answered `304`, or re-sent a body
    /// with the same SHA-256. `validators` are the ones to store for next time (the server may
    /// have sent a new `ETag` or `Last-Modified` with an identical body), with `known`'s `version`.
    Unchanged { validators: Validators },
    /// A new body, with its validators (`version` is `None`: the caller sets it).
    Changed {
        body: String,
        validators: Validators,
    },
}

/// Outcome of [`HttpFetcher::get_text_conditional`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionalText {
    /// The server confirmed the cached copy is still current; the caller keeps using it.
    NotModified,
    /// A fresh body, with the `ETag` to send next time (`None` if the server sent none, in
    /// which case the caller has no validator to use for a future conditional GET).
    Modified { body: String, etag: Option<String> },
}

/// Rate-limited, retrying HTTP client shared by all adapters. Cheap to clone.
///
/// Every request method:
/// - waits on the source's rate limiter and concurrency semaphore ([`SourceRateLimiter::acquire`])
///   before each attempt, holding the permit until the body has been read;
/// - retries [retryable](CrawlError::is_retryable) failures in-process up to 2 extra times with
///   jittered exponential backoff (250ms, 1s, ±25%), or after the server's `Retry-After` if one
///   was sent. A `Retry-After` above 5s is not slept: the `RateLimited` error is returned at once
///   so the queue reschedules the job;
/// - maps non-success statuses with [`CrawlError::from_status`] and body decode failures to
///   [`CrawlError::Parse`] (never retried);
/// - records `CRAWLER_METRICS` (crawler type = lowercase source id, source = URL host), and
///   redacts the values of `api_key`, `apikey`, `registrationkey`, `userid`, `key` and `token`
///   query parameters (case-insensitive) from anything it logs or puts in an error.
#[derive(Clone, Debug)]
pub struct HttpFetcher {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    client: reqwest::Client,
    config: HttpConfig,
    /// Complete: contains an entry for every `SourceId`.
    policies: HashMap<SourceId, SourcePolicy>,
    limiter: SourceRateLimiter,
}

/// One logical request, prepared once and replayed for each attempt.
struct Target {
    source: SourceId,
    method: Method,
    url: Url,
    body: Option<Vec<u8>>,
    accept_json: bool,
    /// `If-None-Match` value to send, for a conditional GET ([`HttpFetcher::get_text_if_changed`]).
    if_none_match: Option<String>,
    /// `If-Modified-Since` value to send, for a conditional GET.
    if_modified_since: Option<String>,
    /// The URL with secret query values replaced; the only form that may be logged.
    redacted: String,
    /// Secret values sent in the query or a JSON body, scrubbed from quoted response bodies too.
    secrets: Vec<String>,
    /// Metrics labels.
    crawler_type: String,
    host: String,
    endpoint: String,
}

impl Target {
    fn new(
        source: SourceId,
        method: Method,
        url: &str,
        query: &[(&str, &str)],
        body: Option<Vec<u8>>,
        accept_json: bool,
    ) -> Result<Self, CrawlError> {
        let mut url = Url::parse(url)
            .map_err(|e| CrawlError::Permanent(format!("{source}: invalid URL: {e}")))?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut secrets: Vec<String> = url
            .query_pairs()
            .filter(|(k, v)| is_secret_param(k) && !v.is_empty())
            .map(|(_, v)| v.into_owned())
            .collect();
        secrets.extend(body.as_deref().map(body_secrets).unwrap_or_default());
        Ok(Self {
            source,
            method,
            redacted: redact_url(&url),
            secrets,
            crawler_type: source.as_str().to_ascii_lowercase(),
            host: url.host_str().unwrap_or("unknown").to_owned(),
            endpoint: endpoint_label(&url),
            url,
            body,
            accept_json,
            if_none_match: None,
            if_modified_since: None,
        })
    }

    /// `"<SOURCE> <METHOD> <redacted url>"`, used as error context.
    fn context(&self) -> String {
        format!("{} {} {}", self.source, self.method, self.redacted)
    }

    fn record_error(&self, err: &CrawlError) {
        CRAWLER_METRICS.record_error(&self.crawler_type, &self.host, err.kind());
    }
}

impl HttpFetcher {
    /// Builds the shared client. Sources absent from `policies` use [`SourcePolicy::default_for`].
    ///
    /// Fails with [`CrawlError::Permanent`] if the client cannot be built (e.g. an invalid
    /// `user_agent`) or a policy is invalid.
    pub fn new(
        config: HttpConfig,
        policies: HashMap<SourceId, SourcePolicy>,
    ) -> Result<Self, CrawlError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .user_agent(config.user_agent.clone())
            .build()
            .map_err(|e| CrawlError::Permanent(format!("building HTTP client: {e}")))?;
        let policies: HashMap<SourceId, SourcePolicy> = SourceId::ALL
            .into_iter()
            .map(|id| {
                let p = policies
                    .get(&id)
                    .copied()
                    .unwrap_or_else(|| SourcePolicy::default_for(id));
                (id, p)
            })
            .collect();
        let limiter = SourceRateLimiter::new(&policies)?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                config,
                policies,
                limiter,
            }),
        })
    }

    /// The effective policy for `source`.
    pub fn policy(&self, source: SourceId) -> SourcePolicy {
        self.inner
            .policies
            .get(&source)
            .copied()
            .unwrap_or_else(|| SourcePolicy::default_for(source))
    }

    /// GET `url` with `query` appended and return the body as text.
    pub async fn get_text(
        &self,
        source: SourceId,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<String, CrawlError> {
        let target = Target::new(source, Method::GET, url, query, None, false)?;
        Ok(self.execute(&target).await?.body)
    }

    /// Conditional GET of `url`: sends `If-None-Match: etag` when `etag` is given, and returns
    /// [`ConditionalText::NotModified`] on a `304` response instead of fetching the body again.
    /// On any other successful status, returns the body and the response's own `ETag` (`None` if
    /// the server didn't send one), for the caller to store and pass back next time.
    pub async fn get_text_conditional(
        &self,
        source: SourceId,
        url: &str,
        etag: Option<&str>,
    ) -> Result<ConditionalText, CrawlError> {
        let known = etag.map(|e| Validators {
            etag: Some(e.to_owned()),
            ..Validators::default()
        });
        Ok(
            match self
                .get_text_if_changed(source, url, &[], known.as_ref())
                .await?
            {
                IfChanged::Unchanged { .. } => ConditionalText::NotModified,
                IfChanged::Changed { body, validators } => ConditionalText::Modified {
                    body,
                    etag: validators.etag,
                },
            },
        )
    }

    /// GET `url` with `query` appended, skipping the body when it hasn't changed since `known`.
    ///
    /// Sends `If-None-Match` and `If-Modified-Since` from `known`'s `etag` and `last_modified`.
    /// A `304` is [`IfChanged::Unchanged`], keeping `known`'s validators except where the `304`
    /// sent new ones. A `200` whose body hashes to `known.content_sha256` is also `Unchanged`
    /// (the server ignored or doesn't support the conditional headers), with the response's
    /// validators. Any other success is [`IfChanged::Changed`]. With `known = None` the request is
    /// a plain GET that returns `Changed` with the validators to store; a `304` to a request that
    /// sent no conditional header is a `Transient` error, never `Unchanged`.
    pub async fn get_text_if_changed(
        &self,
        source: SourceId,
        url: &str,
        query: &[(&str, &str)],
        known: Option<&Validators>,
    ) -> Result<IfChanged, CrawlError> {
        let mut target = Target::new(source, Method::GET, url, query, None, false)?;
        if let Some(k) = known {
            target.if_none_match = k.etag.clone();
            target.if_modified_since = k.last_modified.clone();
        }
        let resp = self.execute(&target).await?;
        if resp.status == StatusCode::NOT_MODIFIED {
            if target.if_none_match.is_none() && target.if_modified_since.is_none() {
                return Err(CrawlError::Transient(format!(
                    "{}: 304 Not Modified to an unconditional request",
                    target.context()
                )));
            }
            let mut validators = known.cloned().unwrap_or_default();
            if resp.etag.is_some() {
                validators.etag = resp.etag;
            }
            if resp.last_modified.is_some() {
                validators.last_modified = resp.last_modified;
            }
            return Ok(IfChanged::Unchanged { validators });
        }
        let mut validators = Validators {
            etag: resp.etag,
            last_modified: resp.last_modified,
            content_sha256: Some(sha256_hex(resp.body.as_bytes())),
            version: None,
        };
        if known.and_then(|k| k.content_sha256.as_deref()) == validators.content_sha256.as_deref() {
            validators.version = known.and_then(|k| k.version.clone());
            return Ok(IfChanged::Unchanged { validators });
        }
        Ok(IfChanged::Changed {
            body: resp.body,
            validators,
        })
    }

    /// Records `err` in `CRAWLER_METRICS`, for a caller that fetched a successful (HTTP 200)
    /// response with [`get_text`](Self::get_text) and then classified it as an error itself
    /// (e.g. a 200 page whose body means the request was rejected). `get_json`'s [`decode`]
    /// records this on the caller's behalf; a caller reading the raw text to decide the
    /// [`CrawlError`] kind must call this itself so the failure isn't silently uncounted.
    /// `crawler_type` and `host` are derived the same way [`Target::new`] derives them.
    pub fn record_response_error(&self, source: SourceId, url: &str, err: &CrawlError) {
        let crawler_type = source.as_str().to_ascii_lowercase();
        let host = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".to_owned());
        CRAWLER_METRICS.record_error(&crawler_type, &host, err.kind());
    }

    /// GET `url` with `query` appended and deserialize the JSON body.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        source: SourceId,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<T, CrawlError> {
        let target = Target::new(source, Method::GET, url, query, None, true)?;
        let text = self.execute(&target).await?.body;
        decode(&target, &text)
    }

    /// POST `body` as JSON to `url` and deserialize the JSON response.
    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        source: SourceId,
        url: &str,
        body: &B,
    ) -> Result<T, CrawlError> {
        let bytes = serde_json::to_vec(body).map_err(|e| {
            CrawlError::Permanent(format!("{source}: serializing request body: {e}"))
        })?;
        let target = Target::new(source, Method::POST, url, &[], Some(bytes), true)?;
        let text = self.execute(&target).await?.body;
        decode(&target, &text)
    }

    /// Runs the attempt loop: first attempt plus up to [`EXTRA_ATTEMPTS`] retries.
    async fn execute(&self, target: &Target) -> Result<RawResponse, CrawlError> {
        let mut attempt: u32 = 0;
        loop {
            let err = match self.attempt(target).await {
                Ok(resp) => return Ok(resp),
                Err(e) => e,
            };
            if !err.is_retryable() {
                tracing::debug!(source = %target.source, url = %target.redacted, error = %err, "request failed");
                return Err(err);
            }
            if attempt >= EXTRA_ATTEMPTS {
                tracing::warn!(
                    source = %target.source,
                    url = %target.redacted,
                    attempts = attempt + 1,
                    error = %err,
                    "request failed after in-process retries"
                );
                return Err(err);
            }
            let delay = match err.retry_after() {
                Some(d) if d > MAX_IN_PROCESS_RETRY_AFTER => {
                    tracing::debug!(
                        source = %target.source,
                        url = %target.redacted,
                        retry_after_secs = d.as_secs(),
                        "Retry-After too long to wait in-process; returning to queue"
                    );
                    return Err(err);
                }
                Some(d) => d,
                None => backoff_delay(attempt),
            };
            CRAWLER_METRICS.record_retry(&target.crawler_type, &target.host, err.kind());
            tracing::debug!(
                source = %target.source,
                url = %target.redacted,
                attempt = attempt + 1,
                delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
                error = %err,
                "retrying request"
            );
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    /// One rate-limited request, including reading the body. A `304` response (only possible
    /// when [`Target::if_none_match`] was sent) is treated as success, with an empty body.
    async fn attempt(&self, target: &Target) -> Result<RawResponse, CrawlError> {
        let _permit = self.inner.limiter.acquire(target.source).await;

        let mut request = self
            .inner
            .client
            .request(target.method.clone(), target.url.clone());
        if target.accept_json {
            request = request.header(ACCEPT, "application/json");
        }
        if let Some(etag) = &target.if_none_match {
            request = request.header(IF_NONE_MATCH, etag.clone());
        }
        if let Some(date) = &target.if_modified_since {
            request = request.header(IF_MODIFIED_SINCE, date.clone());
        }
        if let Some(body) = &target.body {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone());
        }

        let start = Instant::now();
        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => return Err(self.transport_error(target, e, start)),
        };

        let status = response.status();
        if status == StatusCode::NOT_MODIFIED {
            self.record_request(target, status.as_str(), start);
            return Ok(RawResponse {
                status,
                etag: header_string(response.headers(), ETAG),
                last_modified: header_string(response.headers(), LAST_MODIFIED),
                body: String::new(),
            });
        }
        if !status.is_success() {
            let retry_after = parse_retry_after(response.headers());
            // Include a scrubbed snippet of the body: many APIs (FRED, BLS) explain the
            // failure there, and adapters classify on it (e.g. "series does not exist").
            let body = response.text().await.unwrap_or_default();
            let context = if body.trim().is_empty() {
                target.context()
            } else {
                format!("{}: {}", target.context(), snippet(&body, &target.secrets))
            };
            let err = CrawlError::from_status(status, retry_after, context);
            self.record_request(target, status.as_str(), start);
            if status == StatusCode::TOO_MANY_REQUESTS
                || matches!(err, CrawlError::RateLimited { .. })
            {
                CRAWLER_METRICS.record_rate_limit_hit(&target.crawler_type, &target.host);
            }
            target.record_error(&err);
            return Err(err);
        }

        let etag = header_string(response.headers(), ETAG);
        let last_modified = header_string(response.headers(), LAST_MODIFIED);
        match response.text().await {
            Ok(body) => {
                self.record_request(target, status.as_str(), start);
                CRAWLER_METRICS.record_bytes_downloaded(
                    &target.crawler_type,
                    &target.host,
                    body.len() as u64,
                );
                Ok(RawResponse {
                    status,
                    etag,
                    last_modified,
                    body,
                })
            }
            Err(e) => Err(self.transport_error(target, e, start)),
        }
    }

    fn record_request(&self, target: &Target, status: &str, start: Instant) {
        CRAWLER_METRICS.record_request(
            &target.crawler_type,
            &target.host,
            &target.endpoint,
            status,
            start.elapsed().as_secs_f64(),
        );
    }

    /// Maps a reqwest failure (no HTTP status) to a `CrawlError` without leaking the URL.
    fn transport_error(&self, target: &Target, e: reqwest::Error, start: Instant) -> CrawlError {
        let is_timeout = e.is_timeout();
        let permanent = e.is_builder() || e.is_redirect();
        // reqwest's Display includes the full URL (query string and all); strip it.
        let detail = error_chain(&e.without_url());
        let err = if is_timeout {
            CrawlError::Transient(format!(
                "{}: timed out after {:?}: {detail}",
                target.context(),
                self.inner.config.timeout
            ))
        } else if permanent {
            CrawlError::Permanent(format!("{}: {detail}", target.context()))
        } else {
            // connect, send and body-read failures
            CrawlError::Transient(format!("{}: {detail}", target.context()))
        };
        self.record_request(target, if is_timeout { "timeout" } else { "error" }, start);
        if is_timeout {
            CRAWLER_METRICS.record_timeout(&target.crawler_type, &target.host);
        }
        target.record_error(&err);
        err
    }
}

/// Deserializes a JSON body, mapping failure to `Parse` with a short, scrubbed body snippet.
fn decode<T: DeserializeOwned>(target: &Target, text: &str) -> Result<T, CrawlError> {
    serde_json::from_str(text).map_err(|e| {
        let err = CrawlError::Parse(format!(
            "{}: invalid JSON ({e}); body starts: {:?}",
            target.context(),
            snippet(text, &target.secrets)
        ));
        target.record_error(&err);
        err
    })
}

/// `text` with any `secrets` scrubbed, then cut to its first [`SNIPPET_CHARS`] characters.
///
/// Scrubbing happens on the whole text *before* truncating: truncating first would leave the
/// prefix of a secret that straddles the cut unmatched (and so leaked).
fn snippet(text: &str, secrets: &[String]) -> String {
    let mut scrubbed = text.to_owned();
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        scrubbed = scrubbed.replace(secret.as_str(), REDACTED);
    }
    match scrubbed.char_indices().nth(SNIPPET_CHARS) {
        Some((cut, _)) => {
            scrubbed.truncate(cut);
            scrubbed.push('…');
            scrubbed
        }
        None => scrubbed,
    }
}

/// `e` and its source chain, joined by `": "`.
fn error_chain(e: &(dyn std::error::Error + 'static)) -> String {
    let mut out = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        out.push_str(": ");
        out.push_str(&s.to_string());
        source = s.source();
    }
    out
}

fn is_secret_param(name: &str) -> bool {
    SECRET_PARAMS.iter().any(|p| p.eq_ignore_ascii_case(name))
}

/// Non-empty string values of secret keys (e.g. BLS's `registrationkey`) in the top-level
/// object of a JSON request body.
fn body_secrets(body: &[u8]) -> Vec<String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_slice(body) else {
        return Vec::new();
    };
    map.into_iter()
        .filter(|(k, _)| is_secret_param(k))
        .filter_map(|(_, v)| match v {
            serde_json::Value::String(s) if !s.is_empty() => Some(s),
            _ => None,
        })
        .collect()
}

/// `url` with userinfo removed and secret query parameter values replaced by `REDACTED`.
fn redact_url(url: &Url) -> String {
    let mut u = url.clone();
    // Both only fail for URLs that cannot have credentials, which then have none to strip.
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.set_fragment(None);
    if u.query().is_some() {
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| {
                let v = if is_secret_param(&k) {
                    REDACTED.to_owned()
                } else {
                    v.into_owned()
                };
                (k.into_owned(), v)
            })
            .collect();
        u.query_pairs_mut().clear().extend_pairs(pairs);
    }
    u.to_string()
}

/// Low-cardinality metrics label for the URL path: at most three segments, with id-like
/// segments (longer than 3 characters and containing a digit) replaced by `:id`.
fn endpoint_label(url: &Url) -> String {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segs| {
            segs.filter(|s| !s.is_empty())
                .take(3)
                .map(|s| {
                    if s.len() > 3 && s.chars().any(|c| c.is_ascii_digit()) {
                        ":id"
                    } else {
                        s
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    format!("/{}", segments.join("/"))
}

/// `name`'s value, if present and visible ASCII.
fn header_string(headers: &HeaderMap, name: reqwest::header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Lowercase hex SHA-256 of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `Retry-After` as delta-seconds or an HTTP-date (a date in the past means "now").
fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let when = httpdate::parse_http_date(value).ok()?;
    Some(
        when.duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}

/// In-process retry delay before retry `attempt + 1`: 250ms * 4^attempt, ±25% jitter.
fn backoff_delay(attempt: u32) -> Duration {
    let base = RETRY_BASE_DELAY.saturating_mul(4u32.saturating_pow(attempt));
    base.mul_f64(rand::rng().random_range(0.75..=1.25))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn default_config() {
        let c = HttpConfig::default();
        assert_eq!(c.timeout, Duration::from_secs(30));
        assert!(c.user_agent.starts_with("EconGraph/"));
        assert!(c.user_agent.contains('@'), "SEC requires a contact address");
    }

    #[test]
    fn policy_overrides_and_defaults() {
        let mut custom = SourcePolicy::default_for(SourceId::Fred);
        custom.max_concurrency = 9;
        let f = HttpFetcher::new(
            HttpConfig::default(),
            HashMap::from([(SourceId::Fred, custom)]),
        )
        .unwrap();
        assert_eq!(f.policy(SourceId::Fred), custom);
        assert_eq!(
            f.policy(SourceId::Bls),
            SourcePolicy::default_for(SourceId::Bls)
        );
    }

    #[test]
    fn invalid_user_agent_is_rejected() {
        let config = HttpConfig {
            user_agent: "bad\nagent".into(),
            ..HttpConfig::default()
        };
        assert!(matches!(
            HttpFetcher::new(config, HashMap::new()),
            Err(CrawlError::Permanent(_))
        ));
    }

    #[test]
    fn redacts_secret_query_params() {
        let url = Url::parse(
            "https://user:pw@api.example.com/x?series_id=GDP&api_key=s1&ApiKey=s2&registrationKey=s3\
             &UserID=s4&key=s5&TOKEN=s6&keyword=keep&file_type=json#frag",
        )
        .unwrap();
        let r = redact_url(&url);
        for secret in ["s1", "s2", "s3", "s4", "s5", "s6", "pw", "user", "frag"] {
            assert!(!r.contains(secret), "{secret} leaked in {r}");
        }
        assert!(r.contains("series_id=GDP"), "{r}");
        assert!(r.contains("keyword=keep"), "{r}");
        assert!(r.contains("api_key=REDACTED"), "{r}");
        assert!(r.contains("UserID=REDACTED"), "{r}");
    }

    #[test]
    fn endpoint_labels_are_low_cardinality() {
        let l = |u: &str| endpoint_label(&Url::parse(u).unwrap());
        assert_eq!(
            l("https://api.stlouisfed.org/fred/series/observations?x=1"),
            "/fred/series/observations"
        );
        assert_eq!(
            l("https://data.sec.gov/submissions/CIK0000320193.json"),
            "/submissions/:id"
        );
        assert_eq!(
            l("https://api.worldbank.org/v2/country/all/indicator/NY"),
            "/v2/country/all"
        );
        assert_eq!(l("https://example.com"), "/");
    }

    #[test]
    fn retry_after_parsing() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(RETRY_AFTER, v.parse().unwrap());
            parse_retry_after(&m)
        };
        assert_eq!(h("120"), Some(Duration::from_secs(120)));
        assert_eq!(h(" 3 "), Some(Duration::from_secs(3)));
        assert_eq!(h("Wed, 21 Oct 2015 07:28:00 GMT"), Some(Duration::ZERO));
        let future = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(90));
        let d = h(&future).unwrap();
        assert!(
            d > Duration::from_secs(80) && d <= Duration::from_secs(90),
            "{d:?}"
        );
        assert_eq!(h("soon"), None);
        assert_eq!(parse_retry_after(&HeaderMap::new()), None);
    }

    #[test]
    fn backoff_is_jittered_exponential() {
        for _ in 0..50 {
            let d0 = backoff_delay(0);
            let d1 = backoff_delay(1);
            assert!(d0 >= Duration::from_millis(187) && d0 <= Duration::from_millis(313));
            assert!(d1 >= Duration::from_millis(750) && d1 <= Duration::from_millis(1250));
        }
    }

    #[test]
    fn snippet_truncates_and_scrubs() {
        let long = "x".repeat(500);
        let s = snippet(&long, &[]);
        assert_eq!(s.chars().count(), SNIPPET_CHARS + 1);
        assert_eq!(
            snippet("key is abc123", &["abc123".into()]),
            "key is REDACTED"
        );
        // Empty secrets are ignored rather than matching everywhere.
        assert_eq!(snippet("abc", &[String::new()]), "abc");
    }

    #[test]
    fn target_collects_secrets_from_query_and_json_body() {
        let body = serde_json::to_vec(&serde_json::json!({
            "seriesid": ["CUUR0000SA0"],
            "registrationkey": "bodykey123",
            "catalog": false,
        }))
        .unwrap();
        let t = Target::new(
            SourceId::Bls,
            Method::POST,
            "https://api.example.test/data",
            &[("api_key", "querykey456"), ("file_type", "json")],
            Some(body),
            true,
        )
        .unwrap();
        assert_eq!(t.secrets, vec!["querykey456", "bodykey123"]);
        assert_eq!(
            snippet(
                "The key:bodykey123 provided by the User is invalid.",
                &t.secrets
            ),
            "The key:REDACTED provided by the User is invalid."
        );
        // Non-JSON bodies and non-string or empty values contribute nothing.
        assert!(body_secrets(b"not json").is_empty());
        assert!(body_secrets(br#"{"registrationkey":"","api_key":7}"#).is_empty());
    }

    #[test]
    fn snippet_scrubs_secret_straddling_the_cut() {
        let secret = "SuperSecretKey1234567890";
        // The secret starts 10 chars before the cut, so truncating first would leak
        // "SuperSecre" into the snippet.
        for (pad, ch) in [(SNIPPET_CHARS - 10, 'x'), (SNIPPET_CHARS - 1, 'é')] {
            let text = format!("{}{secret}{}", ch.to_string().repeat(pad), "y".repeat(50));
            let s = snippet(&text, &[secret.to_owned()]);
            assert!(!s.contains("SuperS"), "secret prefix leaked: {s}");
            assert!(!s.contains('S'), "any part of the secret leaked: {s}");
            assert_eq!(s.chars().count(), SNIPPET_CHARS + 1, "{s}");
            assert!(s.ends_with('…'));
            assert!(
                s.contains('R'),
                "redaction marker starts before the cut: {s}"
            );
        }
    }

    fn fetcher_with(timeout: Duration, concurrency: usize) -> HttpFetcher {
        let mut p = SourcePolicy::default_for(SourceId::Fred);
        p.requests_per_second = 1000.0;
        p.burst = 100;
        p.max_concurrency = concurrency;
        HttpFetcher::new(
            HttpConfig {
                timeout,
                ..HttpConfig::default()
            },
            HashMap::from([(SourceId::Fred, p)]),
        )
        .unwrap()
    }

    fn fetcher() -> HttpFetcher {
        fetcher_with(Duration::from_secs(5), 4)
    }

    async fn hits(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    /// A caller that fetched a 200 response with `get_text` and classified it as an error
    /// itself (like Census's "Invalid Key" page) must still get it counted, exactly as
    /// `get_json`'s `decode` counts a parse failure. A unique host avoids colliding with any
    /// other test's crawler-error counters, since `CRAWLER_METRICS` is process-global.
    #[test]
    fn record_response_error_counts_it() {
        let host = "record-response-error-test.example";
        let url = format!("https://{host}/x");
        let before = CRAWLER_METRICS
            .crawler_errors_total
            .with_label_values(&["census", host, "auth"])
            .get();
        fetcher().record_response_error(
            SourceId::Census,
            &url,
            &CrawlError::Auth("Invalid Key".into()),
        );
        let after = CRAWLER_METRICS
            .crawler_errors_total
            .with_label_values(&["census", host, "auth"])
            .get();
        assert_eq!(after - before, 1);
    }

    #[derive(Debug, serde::Deserialize, PartialEq)]
    struct Obs {
        value: u32,
    }

    #[tokio::test]
    async fn get_json_ok_with_query_and_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/obs"))
            .and(query_param("series_id", "GDP"))
            .and(query_param("existing", "1"))
            .and(header("accept", "application/json"))
            .and(header(
                "user-agent",
                HttpConfig::default().user_agent.as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"value": 7})))
            .expect(1)
            .mount(&server)
            .await;
        let obs: Obs = fetcher()
            .get_json(
                SourceId::Fred,
                &format!("{}/obs?existing=1", server.uri()),
                &[("series_id", "GDP")],
            )
            .await
            .unwrap();
        assert_eq!(obs, Obs { value: 7 });
    }

    #[tokio::test]
    async fn get_text_ok() {
        let server = MockServer::start().await;
        Mock::given(path("/t"))
            .respond_with(ResponseTemplate::new(200).set_body_string("a,b\n1,2\n"))
            .mount(&server)
            .await;
        let body = fetcher()
            .get_text(SourceId::Fred, &format!("{}/t", server.uri()), &[])
            .await
            .unwrap();
        assert_eq!(body, "a,b\n1,2\n");
    }

    #[tokio::test]
    async fn post_json_sends_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/p"))
            .and(header("content-type", "application/json"))
            .and(body_json(json!({"seriesid": ["A"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"value": 1})))
            .expect(1)
            .mount(&server)
            .await;
        let obs: Obs = fetcher()
            .post_json(
                SourceId::Fred,
                &format!("{}/p", server.uri()),
                &json!({"seriesid": ["A"]}),
            )
            .await
            .unwrap();
        assert_eq!(obs.value, 1);
    }

    #[tokio::test]
    async fn not_found_is_not_retried() {
        let server = MockServer::start().await;
        Mock::given(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let err = fetcher()
            .get_json::<Obs>(SourceId::Fred, &format!("{}/missing", server.uri()), &[])
            .await
            .unwrap_err();
        assert!(matches!(err, CrawlError::NotFound(_)), "{err:?}");
        assert_eq!(hits(&server).await, 1);
    }

    #[tokio::test]
    async fn server_errors_then_success() {
        let server = MockServer::start().await;
        Mock::given(path("/flaky"))
            .respond_with(ResponseTemplate::new(500))
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(path("/flaky"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"value": 3})))
            .mount(&server)
            .await;
        let obs: Obs = fetcher()
            .get_json(SourceId::Fred, &format!("{}/flaky", server.uri()), &[])
            .await
            .unwrap();
        assert_eq!(obs.value, 3);
        assert_eq!(hits(&server).await, 3);
    }

    #[tokio::test]
    async fn persistent_server_error_gives_up_after_three_attempts() {
        let server = MockServer::start().await;
        Mock::given(path("/down"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let err = fetcher()
            .get_json::<Obs>(SourceId::Fred, &format!("{}/down", server.uri()), &[])
            .await
            .unwrap_err();
        assert!(matches!(err, CrawlError::Transient(_)), "{err:?}");
        assert_eq!(hits(&server).await, 3);
    }

    #[tokio::test]
    async fn short_retry_after_is_honoured_in_process() {
        let server = MockServer::start().await;
        Mock::given(path("/throttled"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(path("/throttled"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"value": 4})))
            .mount(&server)
            .await;
        let start = Instant::now();
        let obs: Obs = fetcher()
            .get_json(SourceId::Fred, &format!("{}/throttled", server.uri()), &[])
            .await
            .unwrap();
        assert_eq!(obs.value, 4);
        assert!(start.elapsed() >= Duration::from_millis(950));
        assert_eq!(hits(&server).await, 2);
    }

    #[tokio::test]
    async fn long_retry_after_returns_rate_limited_immediately() {
        let server = MockServer::start().await;
        Mock::given(path("/throttled"))
            .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "120"))
            .mount(&server)
            .await;
        let fetcher = fetcher();
        let start = Instant::now();
        let err = fetcher
            .get_json::<Obs>(SourceId::Fred, &format!("{}/throttled", server.uri()), &[])
            .await
            .unwrap_err();
        assert_eq!(
            err,
            CrawlError::RateLimited {
                retry_after: Some(Duration::from_secs(120))
            }
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(hits(&server).await, 1);
    }

    #[tokio::test]
    async fn timeout_is_transient() {
        let server = MockServer::start().await;
        Mock::given(path("/slow"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"value": 1}))
                    .set_delay(Duration::from_secs(2)),
            )
            .mount(&server)
            .await;
        let err = fetcher_with(Duration::from_millis(200), 4)
            .get_json::<Obs>(SourceId::Fred, &format!("{}/slow", server.uri()), &[])
            .await
            .unwrap_err();
        assert!(
            matches!(&err, CrawlError::Transient(m) if m.contains("timed out")),
            "{err:?}"
        );
        assert_eq!(hits(&server).await, 3);
    }

    #[tokio::test]
    async fn malformed_json_is_parse_error() {
        let server = MockServer::start().await;
        Mock::given(path("/bad"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html>oops</html>"))
            .mount(&server)
            .await;
        let err = fetcher()
            .get_json::<Obs>(SourceId::Fred, &format!("{}/bad", server.uri()), &[])
            .await
            .unwrap_err();
        assert!(
            matches!(&err, CrawlError::Parse(m) if m.contains("<html>oops")),
            "{err:?}"
        );
        assert_eq!(hits(&server).await, 1);
    }

    #[tokio::test]
    async fn api_key_is_redacted_from_status_errors() {
        let server = MockServer::start().await;
        Mock::given(path("/secret"))
            .respond_with(ResponseTemplate::new(400).set_body_string("echo SUPERSECRET"))
            .mount(&server)
            .await;
        let err = fetcher()
            .get_json::<Obs>(
                SourceId::Fred,
                &format!("{}/secret", server.uri()),
                &[("series_id", "GDP"), ("api_key", "SUPERSECRET")],
            )
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, CrawlError::Permanent(_)), "{err:?}");
        assert!(!msg.contains("SUPERSECRET"), "{msg}");
        assert!(msg.contains("api_key=REDACTED"), "{msg}");
        assert!(msg.contains("series_id=GDP"), "{msg}");
    }

    #[tokio::test]
    async fn api_key_is_redacted_from_parse_errors() {
        let server = MockServer::start().await;
        Mock::given(path("/echo"))
            .respond_with(ResponseTemplate::new(200).set_body_string("bad key SUPERSECRET"))
            .mount(&server)
            .await;
        let err = fetcher()
            .get_json::<Obs>(
                SourceId::Fred,
                &format!("{}/echo", server.uri()),
                &[("UserID", "SUPERSECRET")],
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CrawlError::Parse(_)), "{err:?}");
        assert!(!err.to_string().contains("SUPERSECRET"), "{err}");
    }

    #[tokio::test]
    async fn api_key_is_redacted_from_transport_errors() {
        // Nothing listens on port 1: connection refused.
        let err = fetcher()
            .get_text(
                SourceId::Fred,
                "http://127.0.0.1:1/x",
                &[("registrationkey", "SUPERSECRET")],
            )
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, CrawlError::Transient(_)), "{err:?}");
        assert!(!msg.contains("SUPERSECRET"), "{msg}");
    }

    #[tokio::test]
    async fn invalid_url_is_permanent() {
        let err = fetcher()
            .get_text(SourceId::Fred, "not a url", &[])
            .await
            .unwrap_err();
        assert!(matches!(err, CrawlError::Permanent(_)), "{err:?}");
    }

    #[tokio::test]
    async fn concurrency_is_limited_per_source() {
        let server = MockServer::start().await;
        Mock::given(path("/c"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("ok")
                    .set_delay(Duration::from_millis(500)),
            )
            .mount(&server)
            .await;
        let f = fetcher_with(Duration::from_secs(5), 2);
        let url = format!("{}/c", server.uri());
        let start = Instant::now();
        let (a, b, c) = tokio::join!(
            f.get_text(SourceId::Fred, &url, &[]),
            f.get_text(SourceId::Fred, &url, &[]),
            f.get_text(SourceId::Fred, &url, &[]),
        );
        a.unwrap();
        b.unwrap();
        c.unwrap();
        // Two run together; the third waits for a slot, so two 500ms rounds.
        assert!(
            start.elapsed() >= Duration::from_millis(950),
            "{:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn get_text_conditional_returns_body_and_etag_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("a,b\n1,2\n")
                    .insert_header("ETag", "\"v1\""),
            )
            .mount(&server)
            .await;
        let got = fetcher()
            .get_text_conditional(SourceId::Fred, &format!("{}/r", server.uri()), None)
            .await
            .unwrap();
        assert_eq!(
            got,
            ConditionalText::Modified {
                body: "a,b\n1,2\n".to_string(),
                etag: Some("\"v1\"".to_string()),
            }
        );
    }

    #[tokio::test]
    async fn get_text_conditional_sends_if_none_match_and_handles_304() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .and(header("if-none-match", "\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let got = fetcher()
            .get_text_conditional(
                SourceId::Fred,
                &format!("{}/r", server.uri()),
                Some("\"v1\""),
            )
            .await
            .unwrap();
        assert_eq!(got, ConditionalText::NotModified);
    }

    #[tokio::test]
    async fn get_text_conditional_with_no_response_etag_is_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(ResponseTemplate::new(200).set_body_string("x"))
            .mount(&server)
            .await;
        let got = fetcher()
            .get_text_conditional(SourceId::Fred, &format!("{}/r", server.uri()), None)
            .await
            .unwrap();
        assert_eq!(
            got,
            ConditionalText::Modified {
                body: "x".to_string(),
                etag: None,
            }
        );
    }

    #[tokio::test]
    async fn get_text_if_changed_sends_both_validators_and_keeps_known_on_304() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .and(header("if-none-match", "\"v1\""))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let known = Validators {
            etag: Some("\"v1\"".into()),
            last_modified: Some("Tue, 01 Sep 2026 00:00:00 GMT".into()),
            content_sha256: Some("abc".into()),
            version: Some("ignored".into()),
        };
        let got = fetcher()
            .get_text_if_changed(
                SourceId::Fred,
                &format!("{}/r", server.uri()),
                &[],
                Some(&known),
            )
            .await
            .unwrap();
        assert_eq!(got, IfChanged::Unchanged { validators: known });
        // (wiremock's `header` matcher splits values on commas, so an HTTP-date is checked here.)
        let sent = server.received_requests().await.unwrap();
        assert_eq!(
            sent[0].headers.get("if-modified-since").unwrap(),
            "Tue, 01 Sep 2026 00:00:00 GMT"
        );
    }

    #[tokio::test]
    async fn get_text_if_changed_takes_new_headers_from_a_304() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(ResponseTemplate::new(304).insert_header("ETag", "\"v2\""))
            .mount(&server)
            .await;
        let known = Validators {
            etag: Some("\"v1\"".into()),
            ..Validators::default()
        };
        let got = fetcher()
            .get_text_if_changed(
                SourceId::Fred,
                &format!("{}/r", server.uri()),
                &[],
                Some(&known),
            )
            .await
            .unwrap();
        assert_eq!(
            got,
            IfChanged::Unchanged {
                validators: Validators {
                    etag: Some("\"v2\"".into()),
                    ..Validators::default()
                }
            }
        );
    }

    #[tokio::test]
    async fn get_text_if_changed_detects_an_identical_body_without_header_support() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("same")
                    .insert_header("Last-Modified", "Wed, 02 Sep 2026 00:00:00 GMT"),
            )
            .mount(&server)
            .await;
        let url = format!("{}/r", server.uri());
        let IfChanged::Changed { body, validators } = fetcher()
            .get_text_if_changed(SourceId::Fred, &url, &[], None)
            .await
            .unwrap()
        else {
            panic!("a first fetch is always Changed")
        };
        assert_eq!(body, "same");
        assert_eq!(
            validators,
            Validators {
                etag: None,
                last_modified: Some("Wed, 02 Sep 2026 00:00:00 GMT".into()),
                // sha256("same")
                content_sha256: Some(
                    "0967115f2813a3541eaef77de9d9d5773f1c0c04314b0bbfe4ff3b3b1c55b5d5".into()
                ),
                version: None,
            }
        );
        let again = fetcher()
            .get_text_if_changed(SourceId::Fred, &url, &[], Some(&validators))
            .await
            .unwrap();
        assert_eq!(again, IfChanged::Unchanged { validators });
    }

    #[tokio::test]
    async fn get_text_if_changed_returns_a_different_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(ResponseTemplate::new(200).set_body_string("new"))
            .mount(&server)
            .await;
        let known = Validators {
            content_sha256: Some(sha256_hex(b"old")),
            ..Validators::default()
        };
        let got = fetcher()
            .get_text_if_changed(
                SourceId::Fred,
                &format!("{}/r", server.uri()),
                &[],
                Some(&known),
            )
            .await
            .unwrap();
        assert!(
            matches!(got, IfChanged::Changed { ref body, .. } if body == "new"),
            "{got:?}"
        );
    }

    #[tokio::test]
    async fn a_304_to_an_unconditional_request_is_an_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/r"))
            .respond_with(ResponseTemplate::new(304))
            .mount(&server)
            .await;
        let url = format!("{}/r", server.uri());
        let only_hash = Validators {
            content_sha256: Some("ff".into()),
            ..Validators::default()
        };
        for known in [None, Some(&only_hash)] {
            let err = fetcher()
                .get_text_if_changed(SourceId::Fred, &url, &[], known)
                .await
                .unwrap_err();
            assert_eq!(err.kind(), "transient", "{err}");
        }
    }
}
