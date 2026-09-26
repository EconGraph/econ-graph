// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Adapter contract test kit: a mock upstream server, a ready-made [`CrawlCtx`], and the
//! standard assertions every [`SourceAdapter`](crate::SourceAdapter) must pass.
//!
//! Available to this crate's own tests automatically (`cfg(test)`); other crates enable it with
//! `econ-graph-crawler = { path = "...", features = ["testkit"] }` under `[dev-dependencies]`.
//!
//! # Adapter constructor convention
//!
//! Every adapter must be pointable at the mock server, so every adapter follows this shape:
//!
//! ```rust,ignore
//! pub const DEFAULT_BASE_URL: &str = "https://api.stlouisfed.org/fred";
//!
//! pub struct FredAdapter { base_url: String }
//!
//! impl FredAdapter {
//!     /// Talks to `base_url` (no trailing slash) instead of the real API. Used by tests.
//!     pub fn new(base_url: impl Into<String>) -> Self {
//!         Self { base_url: base_url.into().trim_end_matches('/').to_string() }
//!     }
//! }
//!
//! impl Default for FredAdapter {
//!     fn default() -> Self { Self::new(DEFAULT_BASE_URL) }
//! }
//! ```
//!
//! - `new(base_url)` takes the API root; every request URL is `format!("{}{path}", self.base_url)`.
//!   Adapters never hard-code the host anywhere else.
//! - `Default` uses the real URL (the `DEFAULT_BASE_URL` constant). Production code registers
//!   `Arc::new(FredAdapter::default())`.
//! - If a source uses more than one host (e.g. an API host and a bulk-download host), the adapter
//!   takes one `base_url` per host in `new(..)` in a fixed order, documents it, and tests pass
//!   `mock.base_url()` for each.
//! - All HTTP goes through `ctx.http` (never a private `reqwest::Client`); API keys come from
//!   `ctx.keys`. [`test_ctx`] fills every key with [`TEST_API_KEY`].
//!
//! # Writing an adapter's tests
//!
//! Put recorded upstream responses under `crates/econ-graph-crawler/tests/fixtures/<source>/`
//! (trimmed to a few observations) and load them with `include_str!`. Then:
//!
//! ```rust,ignore
//! #[cfg(test)]
//! mod contract {
//!     use super::*;
//!     use crate::testkit::{Reply, Route};
//!
//!     crate::adapter_contract_tests! {
//!         adapter: |base_url: String| FredAdapter::new(base_url),
//!         external_id: "GDP",
//!         route: Route::get("/series/observations").query("series_id", "GDP"),
//!         ok_reply: Reply::json_str(include_str!("../../tests/fixtures/fred/gdp_observations.json")),
//!         expect_points: 5,
//!         // Optional; omit the whole `discover` block for sources without discovery.
//!         discover: {
//!             route: Route::get("/category/series"),
//!             reply: Reply::json_str(include_str!("../../tests/fixtures/fred/category_series.json")),
//!             min_series: 2,
//!         },
//!     }
//! }
//! ```
//!
//! That expands to `#[tokio::test]`s for: success with the expected point count, 404 ->
//! `NotFound`, 429 -> `RateLimited`, 500 -> `Transient`, malformed body -> `Parse`, and (if given)
//! discovery. If an adapter needs a fetch to make several requests (e.g. metadata + observations),
//! mount the extra routes in `setup: |mock| { mock.mount(..).await; }` or write the test by hand with
//! [`MockSource`] and the functions in [`contract`]. Source-specific behaviour (missing-value
//! markers, date formats, pagination) gets ordinary hand-written tests next to these.

pub mod contract;

#[cfg(test)]
mod echo;

use std::collections::HashMap;
use std::time::Duration;

use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;
use econ_graph_core::DatabasePool;
use serde::Serialize;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use crate::adapter::{ApiKeys, CrawlCtx};
use crate::http::{HttpConfig, HttpFetcher};
use crate::policy::SourcePolicy;
use crate::source::SourceId;

pub use wiremock;

/// The value [`test_ctx`] puts in every [`ApiKeys`] field.
pub const TEST_API_KEY: &str = "testkit-api-key";

/// Per-request timeout of the [`HttpFetcher`] built by [`test_ctx`].
pub const TEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Connection string used for the lazy pool when `DATABASE_URL` is unset. Nothing listens there.
const NO_DATABASE_URL: &str = "postgres://testkit:testkit@127.0.0.1:1/testkit_no_database";

/// A mock upstream API (a [`wiremock::MockServer`] on a random local port).
///
/// Each test starts its own; it shuts down when dropped.
pub struct MockSource {
    server: MockServer,
}

impl MockSource {
    /// Starts a fresh server with nothing mounted (unmatched requests get 404).
    pub async fn start() -> Self {
        Self {
            server: MockServer::start().await,
        }
    }

    /// Root URL, e.g. `http://127.0.0.1:41234` (no trailing slash). Pass it to the adapter's `new`.
    pub fn base_url(&self) -> String {
        self.server.uri()
    }

    /// `base_url()` + `path` (which should start with `/`).
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server.uri())
    }

    /// Serves `reply` to every request matching `route`.
    ///
    /// When several mounted fixtures match, the first mounted wins.
    pub async fn mount(&self, route: &Route, reply: Reply) {
        route.to_mock(reply).mount(&self.server).await;
    }

    /// Like [`mount`](Self::mount), but the server panics on drop unless `route` was hit exactly
    /// `times` times.
    pub async fn mount_expect(&self, route: &Route, reply: Reply, times: u64) {
        route
            .to_mock(reply)
            .expect(times)
            .named(route.to_string())
            .mount(&self.server)
            .await;
    }

    /// Removes every mounted fixture and forgets recorded requests.
    pub async fn reset(&self) {
        self.server.reset().await;
    }

    /// Every request the server has received so far.
    pub async fn received_requests(&self) -> Vec<Request> {
        self.server.received_requests().await.unwrap_or_default()
    }

    /// The underlying wiremock server, for matchers this kit does not wrap.
    pub fn server(&self) -> &MockServer {
        &self.server
    }
}

/// Which requests a fixture answers: method + exact path, plus any query parameters and (for
/// POST) a JSON subset the body must contain. Parameters not listed are ignored.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    method: &'static str,
    path: String,
    query: Vec<(String, String)>,
    body_partial_json: Option<serde_json::Value>,
}

impl Route {
    /// `GET path` (path starts with `/`, no query string).
    pub fn get(path: impl Into<String>) -> Self {
        Self::new("GET", path)
    }

    /// `POST path`.
    pub fn post(path: impl Into<String>) -> Self {
        Self::new("POST", path)
    }

    fn new(method: &'static str, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            query: Vec::new(),
            body_partial_json: None,
        }
    }

    /// Also require query parameter `key=value`.
    #[must_use]
    pub fn query(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.push((key.into(), value.into()));
        self
    }

    /// Also require the JSON request body to contain `subset` (wiremock `body_partial_json`).
    #[must_use]
    pub fn body_contains(mut self, subset: impl Serialize) -> Self {
        self.body_partial_json =
            Some(serde_json::to_value(subset).expect("Route::body_contains: not serializable"));
        self
    }

    fn to_mock(&self, reply: Reply) -> Mock {
        let mut builder = Mock::given(method(self.method)).and(path(self.path.as_str()));
        for (k, v) in &self.query {
            builder = builder.and(query_param(k.as_str(), v.as_str()));
        }
        if let Some(body) = &self.body_partial_json {
            builder = builder.and(body_partial_json(body.clone()));
        }
        builder.respond_with(reply.into_template())
    }
}

impl std::fmt::Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method, self.path)?;
        for (i, (k, v)) in self.query.iter().enumerate() {
            write!(f, "{}{k}={v}", if i == 0 { '?' } else { '&' })?;
        }
        Ok(())
    }
}

/// What a fixture answers with: status, body, headers and an optional delay.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    status: u16,
    body: Option<(Vec<u8>, &'static str)>,
    headers: Vec<(String, String)>,
    delay: Option<Duration>,
}

impl Reply {
    /// 200 with no body.
    pub fn ok() -> Self {
        Self::status(200)
    }

    /// `code` with no body.
    pub fn status(code: u16) -> Self {
        Self {
            status: code,
            body: None,
            headers: Vec::new(),
            delay: None,
        }
    }

    /// 200 with `value` serialized as JSON (`Content-Type: application/json`).
    pub fn json(value: impl Serialize) -> Self {
        let body = serde_json::to_vec(&value).expect("Reply::json: not serializable");
        Self::ok().with_body(body, "application/json")
    }

    /// 200 with `raw` as the body, labelled `application/json` (need not be valid JSON:
    /// use it for recorded fixtures and for malformed bodies).
    pub fn json_str(raw: impl Into<String>) -> Self {
        Self::ok().with_body(raw.into().into_bytes(), "application/json")
    }

    /// 200 with `text` as a `text/plain` body (CSV, TSV, ...).
    pub fn text(text: impl Into<String>) -> Self {
        Self::ok().with_body(text.into().into_bytes(), "text/plain")
    }

    /// 200 with `xml` as an `application/xml` body (SDMX, ...).
    pub fn xml(xml: impl Into<String>) -> Self {
        Self::ok().with_body(xml.into().into_bytes(), "application/xml")
    }

    fn with_body(mut self, body: Vec<u8>, content_type: &'static str) -> Self {
        self.body = Some((body, content_type));
        self
    }

    /// Replaces the status code, keeping the body.
    #[must_use]
    pub fn with_status(mut self, code: u16) -> Self {
        self.status = code;
        self
    }

    /// Adds `Retry-After: <secs>`.
    #[must_use]
    pub fn retry_after(self, secs: u64) -> Self {
        self.header("Retry-After", secs.to_string())
    }

    /// Adds a response header.
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Waits `delay` before responding (exceed [`TEST_TIMEOUT`] to provoke a timeout).
    #[must_use]
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    fn into_template(self) -> ResponseTemplate {
        let mut t = ResponseTemplate::new(self.status);
        if let Some((body, content_type)) = self.body {
            t = t.set_body_raw(body, content_type);
        }
        for (k, v) in self.headers {
            t = t.insert_header(k.as_str(), v.as_str());
        }
        if let Some(d) = self.delay {
            t = t.set_delay(d);
        }
        t
    }
}

/// `SourcePolicy::default_for(source)` made fast for tests: effectively unlimited rate and
/// concurrency, millisecond backoffs.
pub fn fast_policy(source: SourceId) -> SourcePolicy {
    SourcePolicy {
        requests_per_second: 1000.0,
        burst: 1000,
        max_concurrency: 16,
        base_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(50),
        ..SourcePolicy::default_for(source)
    }
}

/// HTTP settings for tests: [`TEST_TIMEOUT`] and a testkit User-Agent.
pub fn test_http_config() -> HttpConfig {
    HttpConfig {
        timeout: TEST_TIMEOUT,
        user_agent: format!("EconGraph-testkit/{}", env!("CARGO_PKG_VERSION")),
    }
}

/// A database pool that never connects until something calls `pool.get()`.
///
/// Uses `DATABASE_URL` if set, otherwise an address where nothing listens (so an adapter that
/// unexpectedly touches the database fails fast with a connection error instead of hanging).
/// Built with bb8's `build_unchecked` and no reaper, so it needs no database and no runtime.
pub fn lazy_pool() -> DatabasePool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| NO_DATABASE_URL.to_string());
    let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(url);
    Pool::builder()
        .max_size(2)
        .min_idle(None)
        .max_lifetime(None)
        .idle_timeout(None)
        .connection_timeout(Duration::from_secs(2))
        .build_unchecked(manager)
}

/// Every API key set to [`TEST_API_KEY`].
pub fn test_keys() -> ApiKeys {
    ApiKeys {
        fred: Some(TEST_API_KEY.into()),
        bls: Some(TEST_API_KEY.into()),
        bea: Some(TEST_API_KEY.into()),
        census: Some(TEST_API_KEY.into()),
    }
}

/// A [`CrawlCtx`] for adapter tests: [`fast_policy`] for every source, [`test_http_config`],
/// [`lazy_pool`] and [`test_keys`]. Fields are public, so tweak (e.g. `ctx.keys.fred = None`) as needed.
pub fn test_ctx() -> CrawlCtx {
    test_ctx_with(HashMap::new())
}

/// Like [`test_ctx`], with `overrides` replacing the fast policy for the given sources.
pub fn test_ctx_with(overrides: HashMap<SourceId, SourcePolicy>) -> CrawlCtx {
    let mut policies: HashMap<SourceId, SourcePolicy> = SourceId::ALL
        .into_iter()
        .map(|id| (id, fast_policy(id)))
        .collect();
    policies.extend(overrides);
    let http = HttpFetcher::new(test_http_config(), policies)
        .expect("testkit: building HttpFetcher failed");
    CrawlCtx {
        http,
        pool: lazy_pool(),
        keys: test_keys(),
    }
}

// Shared setup for this crate's own DB-backed unit tests (worker, scheduler, status, cli). They
// all empty and assert on `crawl_queue`, and every module compiles into the same test binary, so
// they serialise on one crate-wide lock rather than one per module. With it they are safe with or
// without `--test-threads=1`.

#[cfg(test)]
static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[cfg(test)]
static MIGRATED: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

/// `DATABASE_URL` and the crate-wide DB test lock, after running migrations once per test binary.
/// `None` (the test should skip) when `DATABASE_URL` is unset; `what` names the test group in the
/// skip message. Hold the guard for the whole test.
#[cfg(test)]
pub(crate) async fn lock_test_db(
    what: &str,
) -> Option<(String, tokio::sync::MutexGuard<'static, ()>)> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        eprintln!("DATABASE_URL not set; skipping DB-backed {what} test");
        return None;
    };
    let guard = DB_LOCK.lock().await;
    MIGRATED
        .get_or_init(|| async {
            econ_graph_core::run_migrations(&url)
                .await
                .expect("running migrations");
        })
        .await;
    Some((url, guard))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(url: &str) -> reqwest::Response {
        reqwest::Client::new().get(url).send().await.unwrap()
    }

    #[tokio::test]
    async fn json_fixture_is_served_on_matching_route() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/series/GDP").query("units", "lin"),
            Reply::json(serde_json::json!({"points": [1, 2]})),
        )
        .await;

        let res = get(&mock.url("/series/GDP?units=lin&extra=1")).await;
        assert_eq!(res.status(), 200);
        assert_eq!(
            res.headers()["content-type"].to_str().unwrap(),
            "application/json"
        );
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["points"][1], 2);

        // Wrong query value and wrong path don't match.
        assert_eq!(get(&mock.url("/series/GDP?units=chg")).await.status(), 404);
        assert_eq!(get(&mock.url("/series/CPI?units=lin")).await.status(), 404);
        assert_eq!(mock.received_requests().await.len(), 3);
    }

    #[tokio::test]
    async fn text_status_and_retry_after() {
        let mock = MockSource::start().await;
        mock.mount(&Route::get("/csv"), Reply::text("a,b\n1,2\n"))
            .await;
        mock.mount(&Route::get("/busy"), Reply::status(429).retry_after(3))
            .await;
        mock.mount(
            &Route::get("/broken"),
            Reply::json_str("{ nope").with_status(500),
        )
        .await;

        let res = get(&mock.url("/csv")).await;
        assert_eq!(res.headers()["content-type"], "text/plain");
        assert_eq!(res.text().await.unwrap(), "a,b\n1,2\n");

        let res = get(&mock.url("/busy")).await;
        assert_eq!(res.status(), 429);
        assert_eq!(res.headers()["retry-after"], "3");

        let res = get(&mock.url("/broken")).await;
        assert_eq!(res.status(), 500);
        assert_eq!(res.text().await.unwrap(), "{ nope");
    }

    #[tokio::test]
    async fn post_body_matcher() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::post("/timeseries/data").body_contains(serde_json::json!({"seriesid": ["X"]})),
            Reply::json(serde_json::json!({"status": "REQUEST_SUCCEEDED"})),
        )
        .await;
        let client = reqwest::Client::new();
        let hit = client
            .post(mock.url("/timeseries/data"))
            .json(&serde_json::json!({"seriesid": ["X"], "startyear": "2020"}))
            .send()
            .await
            .unwrap();
        assert_eq!(hit.status(), 200);
        let miss = client
            .post(mock.url("/timeseries/data"))
            .json(&serde_json::json!({"seriesid": ["Y"]}))
            .send()
            .await
            .unwrap();
        assert_eq!(miss.status(), 404);
    }

    #[tokio::test]
    async fn delay_is_applied() {
        let mock = MockSource::start().await;
        mock.mount(
            &Route::get("/slow"),
            Reply::ok().delay(Duration::from_millis(300)),
        )
        .await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let err = client.get(mock.url("/slow")).send().await.unwrap_err();
        assert!(err.is_timeout());
    }

    #[tokio::test]
    async fn mount_expect_and_reset() {
        let mock = MockSource::start().await;
        mock.mount_expect(&Route::get("/once"), Reply::ok(), 1)
            .await;
        assert_eq!(get(&mock.url("/once")).await.status(), 200);
        mock.server().verify().await;
        mock.reset().await;
        assert_eq!(get(&mock.url("/once")).await.status(), 404);
    }

    #[test]
    fn route_display() {
        let r = Route::get("/a").query("x", "1").query("y", "2");
        assert_eq!(r.to_string(), "GET /a?x=1&y=2");
    }

    #[test]
    fn lazy_pool_builds_without_database_or_runtime() {
        let pool = lazy_pool();
        assert_eq!(pool.state().connections, 0);
    }

    #[tokio::test]
    async fn lazy_pool_fails_fast_without_database() {
        if std::env::var("DATABASE_URL").is_ok() {
            return; // a real database may be reachable
        }
        let started = std::time::Instant::now();
        assert!(lazy_pool().get().await.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn test_ctx_is_fast_and_keyed() {
        let ctx = test_ctx();
        assert_eq!(ctx.http.policy(SourceId::Bls), fast_policy(SourceId::Bls));
        assert_eq!(ctx.keys.get(SourceId::Fred), Some(TEST_API_KEY));

        let slow = SourcePolicy::default_for(SourceId::Sec);
        let ctx = test_ctx_with(HashMap::from([(SourceId::Sec, slow)]));
        assert_eq!(ctx.http.policy(SourceId::Sec), slow);
        assert_eq!(ctx.http.policy(SourceId::Fred), fast_policy(SourceId::Fred));
    }
}
