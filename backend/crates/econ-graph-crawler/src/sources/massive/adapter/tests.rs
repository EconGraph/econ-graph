//! Massive adapter and client tests, against the hand-shaped responses in
//! `tests/fixtures/massive/` served by a mock upstream.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use chrono::TimeZone;
use wiremock::matchers::{header, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::adapter::ApiKeys;
use crate::testkit::{test_ctx, TEST_API_KEY};

const GROUPED_DAY: &str =
    include_str!("../../../../tests/fixtures/massive/grouped_2024-01-03.json");
const GROUPED_HOLIDAY: &str =
    include_str!("../../../../tests/fixtures/massive/grouped_2024-01-01_holiday.json");
const AAPL_RANGE: &str =
    include_str!("../../../../tests/fixtures/massive/ticker_aapl_2024-01-02_2024-01-03.json");
const NO_DATA: &str = include_str!("../../../../tests/fixtures/massive/ticker_no_data.json");
const ERROR_401: &str = include_str!("../../../../tests/fixtures/massive/error_401.json");
const ERROR_403: &str =
    include_str!("../../../../tests/fixtures/massive/error_403_not_entitled.json");
const ERROR_403_TIMEFRAME: &str =
    include_str!("../../../../tests/fixtures/massive/error_403_timeframe.json");
const ERROR_429: &str = include_str!("../../../../tests/fixtures/massive/error_429.json");
const MALFORMED: &str = include_str!("../../../../tests/fixtures/massive/malformed.txt");

const AAPL: &str = "BBG001S5N8V8";
const MSFT: &str = "BBG001S5TD05";
const GOOGL: &str = "BBG009S39JY5";
const GOOG: &str = "BBG009S3NB21";
const NODATA: &str = "BBG000NODATA";

/// Thursday 2024-01-04, 18:00 UTC: the newest settled trading day is Wednesday 2024-01-03.
fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2024, 1, 4, 18, 0, 0).unwrap()
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

fn dec(s: &str) -> BigDecimal {
    BigDecimal::from_str(s).unwrap()
}

fn symbols_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/massive/symbols.csv")
}

fn adapter(server: &MockServer) -> MassiveAdapter {
    MassiveAdapter::new(server.uri())
        .with_symbols_path(symbols_path())
        .with_now(now())
}

fn ids(figis: &[&str]) -> Vec<String> {
    figis
        .iter()
        .flat_map(|f| ds::PriceField::ALL.map(|field| ds::external_id(f, field)))
        .collect()
}

fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "application/json")
}

fn status(code: u16, body: &str) -> ResponseTemplate {
    ResponseTemplate::new(code).set_body_raw(body.as_bytes().to_vec(), "application/json")
}

/// Grouped responses: `GROUPED_DAY` for 2024-01-03, a closed market for any other day.
async fn mount_grouped(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v2/aggs/grouped/locale/us/market/stocks/2024-01-03"))
        .and(query_param("adjusted", "false"))
        .respond_with(json(GROUPED_DAY))
        .with_priority(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path_regex(
            r"^/v2/aggs/grouped/locale/us/market/stocks/[0-9-]+$",
        ))
        .respond_with(json(GROUPED_HOLIDAY))
        .with_priority(5)
        .mount(server)
        .await;
}

fn value_on(fetched: &FetchedSeries, date: &str) -> Option<BigDecimal> {
    fetched
        .points
        .iter()
        .find(|p| p.date == day(date))
        .and_then(|p| p.value.clone())
}

#[test]
fn plan_picks_the_fewer_requests() {
    // The daily refresh: three trading days for every S&P 500 security.
    assert_eq!(plan_requests(3, 503), Plan::Grouped);
    // A single series' first fetch: a month of days, one ticker.
    assert_eq!(plan_requests(21, 1), Plan::PerTicker);
    assert_eq!(plan_requests(4, 4), Plan::Grouped);
}

#[test]
fn batch_key_is_shared_by_every_massive_series() {
    let a = MassiveAdapter::default();
    assert_eq!(
        a.batch_key(&ds::external_id(AAPL, ds::PriceField::Close)),
        Some("daily".into())
    );
    assert_eq!(
        a.batch_key(&ds::external_id(MSFT, ds::PriceField::Vwap)),
        Some("daily".into())
    );
    assert_eq!(a.batch_key("not-a-massive-id"), None);
    assert!(a.policy().max_batch >= 500 * 6);
}

/// A normal day: grouped requests with the key in the header only, unadjusted, exact decimals,
/// share classes apart, unknown tickers ignored, and the fetch date as the revision date.
#[tokio::test]
async fn normal_day_uses_grouped_daily() {
    let server = MockServer::start().await;
    mount_grouped(&server).await;
    let figis = [AAPL, MSFT, GOOGL, GOOG];
    let out = adapter(&server)
        .fetch_batch(&test_ctx(), &ids(&figis), Some(day("2024-01-03")))
        .await
        .unwrap();
    assert_eq!(out.len(), 24);

    let get = |figi: &str, field| out[&ds::external_id(figi, field)].clone().unwrap();
    let close = get(AAPL, ds::PriceField::Close);
    assert_eq!(value_on(&close, "2024-01-03"), Some(dec("184.25")));
    assert_eq!(
        value_on(&close, "2024-01-03").unwrap().to_string(),
        "184.25"
    );
    assert_eq!(
        value_on(&get(AAPL, ds::PriceField::Volume), "2024-01-03"),
        Some(dec("58414460"))
    );
    assert_eq!(
        value_on(&get(MSFT, ds::PriceField::Volume), "2024-01-03"),
        Some(dec("25933.6"))
    );
    assert_eq!(
        value_on(&get(MSFT, ds::PriceField::Close), "2024-01-03")
            .unwrap()
            .to_string(),
        "370.60"
    );
    assert!(
        get(MSFT, ds::PriceField::Vwap).points.is_empty(),
        "MSFT has no vw"
    );
    assert_eq!(
        value_on(&get(GOOGL, ds::PriceField::Close), "2024-01-03"),
        Some(dec("138.92"))
    );
    assert_eq!(
        value_on(&get(GOOG, ds::PriceField::Close), "2024-01-03"),
        Some(dec("140.36"))
    );
    for p in &close.points {
        assert_eq!(p.revision_date, day("2024-01-04"));
    }
    assert_eq!(close.dataset.code, DATASET_CODE);

    let requests = server.received_requests().await.unwrap();
    // One request per trading day in the window, never one per ticker.
    assert!(!requests.is_empty() && requests.len() <= 3, "{requests:?}");
    for r in &requests {
        assert!(r.url.path().starts_with("/v2/aggs/grouped/"), "{}", r.url);
        assert!(!r.url.as_str().contains(TEST_API_KEY), "{}", r.url);
        assert_eq!(
            r.headers.get("authorization").unwrap().to_str().unwrap(),
            format!("Bearer {TEST_API_KEY}")
        );
        assert!(r
            .url
            .query_pairs()
            .any(|(k, v)| k == "adjusted" && v == "false"));
    }
}

/// A second batch of the same refresh reuses the grouped responses.
#[tokio::test]
async fn later_batches_reuse_grouped_responses() {
    let server = MockServer::start().await;
    mount_grouped(&server).await;
    let a = adapter(&server);
    let ctx = test_ctx();
    a.fetch_batch(&ctx, &ids(&[AAPL, MSFT, GOOGL]), Some(day("2024-01-03")))
        .await
        .unwrap();
    let first = server.received_requests().await.unwrap().len();
    // A lone security would go per ticker, but its days are cached: no request at all.
    let out = a
        .fetch_batch(&ctx, &ids(&[GOOG]), Some(day("2024-01-03")))
        .await
        .unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), first);
    assert_eq!(
        value_on(
            out[&ds::external_id(GOOG, ds::PriceField::Close)]
                .as_ref()
                .unwrap(),
            "2024-01-03"
        ),
        Some(dec("140.36"))
    );
}

/// A holiday: no bars at all, and no failure.
#[tokio::test]
async fn holiday_is_not_a_failure() {
    let server = MockServer::start().await;
    Mock::given(path_regex(r"^/v2/aggs/grouped/"))
        .respond_with(json(GROUPED_HOLIDAY))
        .mount(&server)
        .await;
    let out = adapter(&server)
        .fetch_batch(
            &test_ctx(),
            &ids(&[AAPL, MSFT, GOOGL]),
            Some(day("2024-01-03")),
        )
        .await
        .unwrap();
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .all(|r| r.url.path().starts_with("/v2/aggs/grouped/")));
    for (id, result) in out {
        let fetched = result.unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(fetched.points.is_empty(), "{id}");
    }
}

/// A ticker missing from an open day's grouped response is retried per ticker once; with no
/// data there either it fails as `Absent`, and the others are unaffected.
#[tokio::test]
async fn ticker_with_no_data_fails_only_its_series() {
    let server = MockServer::start().await;
    mount_grouped(&server).await;
    Mock::given(path_regex(r"^/v2/aggs/ticker/NODATA/range/1/day/"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(json(NO_DATA))
        .expect(1)
        .mount(&server)
        .await;
    let out = adapter(&server)
        .fetch_batch(
            &test_ctx(),
            &ids(&[AAPL, MSFT, NODATA]),
            Some(day("2024-01-03")),
        )
        .await
        .unwrap();
    for field in ds::PriceField::ALL {
        let id = ds::external_id(NODATA, field);
        assert!(
            matches!(&out[&id], Err(CrawlError::Absent(_))),
            "{id}: {:?}",
            out[&id]
        );
        assert!(out[&ds::external_id(AAPL, field)].is_ok());
    }
}

/// A lone series over a long window goes per ticker, dated by each bar's `t`.
#[tokio::test]
async fn lone_series_is_fetched_per_ticker() {
    let server = MockServer::start().await;
    Mock::given(path_regex(
        r"^/v2/aggs/ticker/AAPL/range/1/day/[0-9-]+/2024-01-03$",
    ))
    .and(query_param("adjusted", "false"))
    .and(query_param("limit", "50000"))
    .respond_with(json(AAPL_RANGE))
    .expect(1)
    .mount(&server)
    .await;
    let id = ds::external_id(AAPL, ds::PriceField::Close);
    let fetched = adapter(&server)
        .fetch_series(&test_ctx(), &id, None)
        .await
        .unwrap();
    assert_eq!(value_on(&fetched, "2024-01-02"), Some(dec("185.64")));
    assert_eq!(value_on(&fetched, "2024-01-03"), Some(dec("184.25")));
}

/// 401, 403 (not entitled) and 403 (outside the plan's history): only the first two are key
/// rejections and counted. Run in one test so the counter's deltas are this test's own.
#[tokio::test]
async fn auth_errors() {
    let counter = || {
        econ_graph_metrics::crawler::CRAWLER_SOURCE_HEALTH_METRICS
            .auth_rejections_total
            .with_label_values(&["MASSIVE"])
            .get()
    };
    let ctx = test_ctx();
    for (body, code, rejected) in [
        (ERROR_401, 401, true),
        (ERROR_403, 403, true),
        (ERROR_403_TIMEFRAME, 403, false),
    ] {
        let server = MockServer::start().await;
        Mock::given(path_regex(r"^/v2/aggs/"))
            .respond_with(status(code, body))
            .mount(&server)
            .await;
        let client = MassiveClient::from_ctx(&ctx, server.uri()).unwrap();
        let before = counter();
        let err = client.grouped(day("2024-01-03")).await.unwrap_err();
        assert!(!err.to_string().contains(TEST_API_KEY), "{err}");
        if rejected {
            assert!(
                matches!(err, ClientError::AuthRejected(_)),
                "{code}: {err:?}"
            );
            assert_eq!(counter(), before + 1, "{code}");
            // The whole batch fails as Auth.
            let batch = adapter(&server)
                .fetch_batch(&ctx, &ids(&[AAPL, MSFT]), Some(day("2024-01-03")))
                .await;
            assert!(matches!(batch, Err(CrawlError::Auth(_))), "{batch:?}");
        } else {
            assert_eq!(
                err,
                ClientError::OutsidePlanWindow {
                    date: day("2024-01-03"),
                    message: match &err {
                        ClientError::OutsidePlanWindow { message, .. } => message.clone(),
                        _ => unreachable!(),
                    },
                }
            );
            assert_eq!(counter(), before, "a timeframe 403 is not a key rejection");
            assert!(matches!(CrawlError::from(err), CrawlError::Permanent(_)));
        }
    }
}

#[tokio::test]
async fn rate_limit_fails_the_batch_as_rate_limited() {
    let server = MockServer::start().await;
    Mock::given(path_regex(r"^/v2/aggs/"))
        .respond_with(status(429, ERROR_429).insert_header("Retry-After", "60"))
        .mount(&server)
        .await;
    let ctx = test_ctx();
    let client = MassiveClient::from_ctx(&ctx, server.uri()).unwrap();
    let err = client.grouped(day("2024-01-03")).await.unwrap_err();
    assert!(matches!(err, ClientError::RateLimited { .. }), "{err:?}");
    let batch = adapter(&server)
        .fetch_batch(&ctx, &ids(&[AAPL, MSFT]), Some(day("2024-01-03")))
        .await;
    assert!(
        matches!(batch, Err(CrawlError::RateLimited { .. })),
        "{batch:?}"
    );
}

#[tokio::test]
async fn malformed_body_is_a_parse_error() {
    let server = MockServer::start().await;
    Mock::given(path_regex(r"^/v2/aggs/"))
        .respond_with(json(MALFORMED))
        .mount(&server)
        .await;
    let ctx = test_ctx();
    let client = MassiveClient::from_ctx(&ctx, server.uri()).unwrap();
    let err = client.grouped(day("2024-01-03")).await.unwrap_err();
    assert!(matches!(err, ClientError::Parse(_)), "{err:?}");
    let err = client
        .ticker_range("AAPL", day("2024-01-02"), day("2024-01-03"))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Parse(_)), "{err:?}");
}

#[tokio::test]
async fn adjusted_or_failed_status_is_refused() {
    let server = MockServer::start().await;
    Mock::given(path("/v2/aggs/grouped/locale/us/market/stocks/2024-01-03"))
        .respond_with(json(
            &GROUPED_DAY.replace(r#""adjusted":false"#, r#""adjusted":true"#),
        ))
        .mount(&server)
        .await;
    Mock::given(path("/v2/aggs/grouped/locale/us/market/stocks/2024-01-02"))
        .respond_with(json(&GROUPED_HOLIDAY.replace(r#""OK""#, r#""ERROR""#)))
        .mount(&server)
        .await;
    let client = MassiveClient::from_ctx(&test_ctx(), server.uri()).unwrap();
    for d in ["2024-01-03", "2024-01-02"] {
        let err = client.grouped(day(d)).await.unwrap_err();
        assert!(matches!(err, ClientError::Parse(_)), "{d}: {err:?}");
    }
}

/// `next_url` is followed on the API's host and refused elsewhere, so the key goes nowhere else.
#[tokio::test]
async fn next_url_is_followed_on_the_api_host_only() {
    let server = MockServer::start().await;
    let page1 = AAPL_RANGE.replacen(
        r#""status":"OK""#,
        &format!(
            r#""status":"OK","next_url":"{}/v2/aggs/next/cursor1""#,
            server.uri()
        ),
        1,
    );
    Mock::given(path_regex(r"^/v2/aggs/ticker/AAPL/"))
        .respond_with(json(&page1))
        .mount(&server)
        .await;
    Mock::given(path("/v2/aggs/next/cursor1"))
        .and(header(
            "authorization",
            format!("Bearer {TEST_API_KEY}").as_str(),
        ))
        .respond_with(json(NO_DATA.replace("NODATA", "AAPL").as_str()))
        .expect(1)
        .mount(&server)
        .await;
    let client = MassiveClient::from_ctx(&test_ctx(), server.uri()).unwrap();
    let bars = client
        .ticker_range("AAPL", day("2024-01-02"), day("2024-01-03"))
        .await
        .unwrap();
    assert_eq!(bars.len(), 2);

    let elsewhere = AAPL_RANGE.replacen(
        r#""status":"OK""#,
        r#""status":"OK","next_url":"https://evil.example/v2/aggs/next""#,
        1,
    );
    server.reset().await;
    Mock::given(path_regex(r"^/v2/aggs/ticker/AAPL/"))
        .respond_with(json(&elsewhere))
        .mount(&server)
        .await;
    let err = client
        .ticker_range("AAPL", day("2024-01-02"), day("2024-01-03"))
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Parse(_)), "{err:?}");
}

#[tokio::test]
async fn discovery_lists_six_series_per_listed_security_without_a_request() {
    let server = MockServer::start().await;
    let found = adapter(&server).discover(&test_ctx()).await.unwrap();
    assert_eq!(found.len(), 5 * 6);
    assert!(found
        .iter()
        .any(|s| s.external_id == ds::external_id(GOOG, ds::PriceField::Close)));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn without_a_key_nothing_is_requested() {
    let server = MockServer::start().await;
    let mut ctx = test_ctx();
    ctx.keys.massive = None;
    let a = adapter(&server);
    assert!(matches!(a.discover(&ctx).await, Err(CrawlError::Auth(_))));
    let batch = a.fetch_batch(&ctx, &ids(&[AAPL]), None).await;
    assert!(matches!(batch, Err(CrawlError::Auth(m)) if m.contains("MASSIVE_API_KEY")));
    assert!(server.received_requests().await.unwrap().is_empty());
}

/// Without `MASSIVE_API_KEY` the worker's registry leaves Massive out, logging the skip once,
/// and the scheduler enqueues nothing for it.
#[tokio::test]
async fn without_a_key_the_worker_skips_massive_once_and_enqueues_nothing() {
    let keys = ApiKeys {
        massive: None,
        ..crate::testkit::test_keys()
    };
    let logs = LogCapture::default();
    let registry = tracing::subscriber::with_default(logs.subscriber(), || {
        crate::sources::registry_for_keys(&keys)
    });
    assert!(registry.get(SourceId::Massive).is_none());
    assert!(registry.get(SourceId::Fred).is_some());
    let text = logs.text();
    assert_eq!(text.matches("MASSIVE_API_KEY").count(), 1, "{text}");
    assert_eq!(
        text.lines()
            .filter(|l| l.contains("source skipped"))
            .count(),
        // WTO's key is set in test_keys, so Massive is the only skip.
        1,
        "{text}"
    );

    // The worker's registry also passes the licence gate; the scheduler is built from it.
    let registry = crate::licence::ungated_registry(registry);
    assert_eq!(
        massive_jobs_after_a_tick(registry.ids()).await,
        db_set().then_some(0)
    );
}

/// With `MASSIVE_API_KEY` set but the `crawl_massive` licence flag off (as in every profile),
/// the worker's registry leaves Massive out, and even a scheduler handed Massive enqueues
/// nothing for it: a key alone never starts crawling.
#[tokio::test]
async fn with_a_key_but_the_licence_flag_off_massive_is_not_crawled() {
    assert!(
        !cfg!(flag_crawl_massive),
        "crawl_massive is off in every profile"
    );
    let keys = crate::testkit::test_keys();
    assert!(keys.massive.is_some());
    let keyed = crate::sources::registry_for_keys(&keys);
    assert!(keyed.get(SourceId::Massive).is_some());
    let worker = crate::licence::ungated_registry(keyed.clone());
    assert!(worker.get(SourceId::Massive).is_none());
    assert!(worker.get(SourceId::Fred).is_some());
    // The scheduler applies the gate itself too.
    assert_eq!(
        massive_jobs_after_a_tick(keyed.ids()).await,
        db_set().then_some(0)
    );
}

/// With the key set and the licence flag on, the worker's registry has Massive; the flag
/// without the key is not enough.
#[test]
fn with_a_key_and_the_licence_flag_on_massive_is_registered() {
    let _unlocked = crate::licence::testing::unlock(SourceId::Massive);
    let worker = crate::licence::ungated_registry(crate::sources::registry_for_keys(
        &crate::testkit::test_keys(),
    ));
    assert!(worker.get(SourceId::Massive).is_some());
    let keyless = crate::licence::ungated_registry(crate::sources::registry_for_keys(&ApiKeys {
        massive: None,
        ..crate::testkit::test_keys()
    }));
    assert!(keyless.get(SourceId::Massive).is_none());
}

fn db_set() -> bool {
    std::env::var_os("DATABASE_URL").is_some()
}

/// `MASSIVE` jobs in an emptied queue after one scheduler tick over `sources`. `None` without a
/// database (`DATABASE_URL` unset).
async fn massive_jobs_after_a_tick(sources: Vec<SourceId>) -> Option<i64> {
    use diesel_async::RunQueryDsl;
    let (url, _guard) = crate::testkit::lock_test_db("massive-gate").await?;
    let pool = econ_graph_core::create_pool(&url).await.expect("pool");
    {
        let mut conn = pool.get().await.unwrap();
        diesel::sql_query("DELETE FROM crawl_queue")
            .execute(&mut conn)
            .await
            .unwrap();
    }
    let scheduler = crate::scheduler::RefreshScheduler::new(
        pool.clone(),
        sources,
        crate::scheduler::SchedulerConfig::default(),
    );
    scheduler.tick().await.unwrap();
    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    let mut conn = pool.get().await.unwrap();
    let massive: Count =
        diesel::sql_query("SELECT count(*) AS n FROM crawl_queue WHERE source = 'MASSIVE'")
            .get_result(&mut conn)
            .await
            .unwrap();
    Some(massive.n)
}

/// Captures formatted log lines for one closure.
#[derive(Clone, Default)]
struct LogCapture(Arc<Mutex<Vec<u8>>>);

impl LogCapture {
    fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync {
        tracing_subscriber::fmt()
            .with_writer(self.clone())
            .with_ansi(false)
            .finish()
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl std::io::Write for LogCapture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = LogCapture;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
