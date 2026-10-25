// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Fetching series with more vintage dates than one FRED request may cover, against a fake
//! ALFRED ([`FakeAlfred`]) that answers `/series/vintagedates` and `/series/observations` from
//! a known revision history the way FRED does: real-time periods clamped to the requested
//! window, `limit`/`offset` paging, and an HTTP 400 for a window covering too many vintage
//! dates. Each test compares the fetched points with the ones the whole history gives in one
//! request, so windowing must neither lose, duplicate nor re-mark a single revision.

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use wiremock::{Mock, Request, Respond, ResponseTemplate};

use super::*;
use crate::policy::SourcePolicy;
use crate::testkit::{fast_policy, test_ctx, test_ctx_with, MockSource, Reply, Route};

const SERIES_GDP: &str = include_str!("../../../tests/fixtures/fred/series_gdp.json");
const ERR_BAD_VARIABLE: &str = include_str!("../../../tests/fixtures/fred/error_bad_variable.json");
const TOO_MANY_VINTAGES: &str = "{\"error_code\":400,\"error_message\":\"Bad Request.  \
     This exceeds the maximum number of vintage dates allowed (2000).\"}";

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// One real-time period of one observation: `value` was the published value of `date` from
/// `start` through `end`.
#[derive(Debug, Clone)]
struct Period {
    date: NaiveDate,
    start: NaiveDate,
    end: NaiveDate,
    value: String,
}

/// A daily series' revision history: vintage `i` is day `i` from 2010-01-01; it first
/// publishes the observation of the day before it (value `i`) and, per `revisions`, revises
/// older ones. `revisions` are `(observation index, vintage index, value)`.
struct History {
    vintages: Vec<NaiveDate>,
    periods: Vec<Period>,
}

impl History {
    fn daily(n: usize, revisions: &[(usize, usize, &str)]) -> Self {
        let vintages: Vec<NaiveDate> = (0..n)
            .map(|i| d("2010-01-01") + chrono::Duration::days(i as i64))
            .collect();
        let date = |i: usize| vintages[i].pred_opt().unwrap();
        let mut by_date: BTreeMap<NaiveDate, BTreeMap<usize, String>> = BTreeMap::new();
        for i in 0..n {
            by_date.entry(date(i)).or_default().insert(i, i.to_string());
        }
        for &(obs, vintage, value) in revisions {
            assert!(obs < vintage && vintage < n, "revision ({obs}, {vintage})");
            by_date
                .entry(date(obs))
                .or_default()
                .insert(vintage, value.to_string());
        }
        let mut periods = Vec::new();
        for (date, values) in by_date {
            let values: Vec<_> = values.into_iter().collect();
            for (j, (vintage, value)) in values.iter().enumerate() {
                let end = values
                    .get(j + 1)
                    .map(|(next, _)| vintages[*next].pred_opt().unwrap())
                    .unwrap_or_else(latest_realtime_end);
                periods.push(Period {
                    date,
                    start: vintages[*vintage],
                    end,
                    value: value.clone(),
                });
            }
        }
        Self { vintages, periods }
    }

    /// The points one request for the whole history would give, from `known_vintage` on.
    fn expected(&self, known_vintage: Option<NaiveDate>) -> Vec<FetchedPoint> {
        let mut first: HashMap<NaiveDate, NaiveDate> = HashMap::new();
        for p in &self.periods {
            let e = first.entry(p.date).or_insert(p.start);
            *e = (*e).min(p.start);
        }
        let mut points: Vec<FetchedPoint> = self
            .periods
            .iter()
            .filter(|p| known_vintage.is_none_or(|k| p.start >= k))
            .map(|p| FetchedPoint {
                date: p.date,
                value: (p.value != ".").then(|| BigDecimal::from_str(&p.value).unwrap()),
                revision_date: p.start,
                is_original_release: first[&p.date] == p.start,
            })
            .collect();
        points.sort_by_key(|p| (p.date, p.revision_date));
        points
    }
}

/// One request the fake answered.
#[derive(Debug, Clone)]
struct Logged {
    path: String,
    realtime_start: NaiveDate,
    realtime_end: NaiveDate,
    offset: usize,
    status: u16,
    /// Vintage dates inside the requested window.
    vintages: usize,
    at: Instant,
}

/// FRED's `/series/vintagedates` and `/series/observations` over a [`History`].
#[derive(Clone)]
struct FakeAlfred {
    history: Arc<History>,
    /// Most vintage dates one observations request may cover.
    vintage_limit: usize,
    /// Most rows per observations page, whatever `limit` asks (to force paging).
    page_rows: usize,
    /// Most dates per vintagedates page, whatever `limit` asks (to force paging).
    vintage_page: usize,
    log: Arc<Mutex<Vec<Logged>>>,
}

impl FakeAlfred {
    fn new(history: History) -> Self {
        Self {
            history: Arc::new(history),
            vintage_limit: FRED_DOCUMENTED_VINTAGE_DATE_LIMIT,
            page_rows: OBSERVATIONS_PAGE_SIZE,
            vintage_page: VINTAGE_DATES_PAGE_SIZE,
            log: Arc::default(),
        }
    }

    async fn mount(&self, mock: &MockSource) {
        mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
            .await;
        Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path_regex(
                "^/series/(vintagedates|observations)$",
            ))
            .respond_with(self.clone())
            .mount(mock.server())
            .await;
    }

    fn log(&self) -> Vec<Logged> {
        self.log.lock().unwrap().clone()
    }

    fn observation_requests(&self) -> Vec<Logged> {
        self.log()
            .into_iter()
            .filter(|l| l.path == "/series/observations")
            .collect()
    }
}

impl Respond for FakeAlfred {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let q: HashMap<String, String> = req.url.query_pairs().into_owned().collect();
        let date_param = |k: &str, default: NaiveDate| q.get(k).map_or(default, |s| d(s));
        let start = date_param("realtime_start", earliest_realtime_start());
        let end = date_param("realtime_end", latest_realtime_end());
        let offset: usize = q.get("offset").map_or(0, |s| s.parse().unwrap());
        let limit: usize = q.get("limit").map_or(usize::MAX, |s| s.parse().unwrap());
        let vintages: Vec<NaiveDate> = self
            .history
            .vintages
            .iter()
            .copied()
            .filter(|v| (start..=end).contains(v))
            .collect();
        let path = req.url.path().to_string();
        let (status, body) = if path.ends_with("/vintagedates") {
            let page: Vec<String> = vintages
                .iter()
                .skip(offset)
                .take(limit.min(self.vintage_page))
                .map(|v| v.to_string())
                .collect();
            let body = serde_json::json!({
                "count": vintages.len(), "offset": offset, "limit": limit, "vintage_dates": page,
            });
            (200, body.to_string())
        } else if vintages.len() > self.vintage_limit {
            (400, TOO_MANY_VINTAGES.to_string())
        } else {
            let observation_start = q.get("observation_start").map(|s| d(s));
            let rows: Vec<serde_json::Value> = self
                .history
                .periods
                .iter()
                .filter(|p| p.start <= end && p.end >= start)
                .filter(|p| observation_start.is_none_or(|o| p.date >= o))
                .map(|p| {
                    serde_json::json!({
                        "realtime_start": p.start.max(start).to_string(),
                        "realtime_end": p.end.min(end).to_string(),
                        "date": p.date.to_string(),
                        "value": p.value,
                    })
                })
                .collect();
            let page: Vec<_> = rows
                .iter()
                .skip(offset)
                .take(limit.min(self.page_rows))
                .collect();
            let body = serde_json::json!({
                "count": rows.len(), "offset": offset, "limit": limit, "observations": page,
            });
            (200, body.to_string())
        };
        self.log.lock().unwrap().push(Logged {
            path,
            realtime_start: start,
            realtime_end: end,
            offset,
            status,
            vintages: vintages.len(),
            at: Instant::now(),
        });
        ResponseTemplate::new(status).set_body_raw(body, "application/json")
    }
}

/// 2,500 daily vintages (DFF has about 5,150): three windows at the default of 1,000. Besides
/// the routine revisions, some sit on the window boundaries (vintage 999 is the first window's
/// last, 1000 the second's first), and some revise across them.
fn large_history() -> History {
    let mut revisions: Vec<(usize, usize, String)> = (0..2490)
        .filter(|i| i % 97 == 3)
        .map(|i| (i, i + 5, format!("{i}.5")))
        .collect();
    revisions.extend(
        [
            (990, 999, "990.25"),   // revised on the first window's last vintage
            (995, 1000, "995.25"),  // revised on the second window's first vintage
            (998, 1000, "998.25"),  // first published in window 1, revised on the boundary
            (999, 1001, "999.25"),  // first published on window 2's first vintage, revised
            (10, 1500, "10.25"),    // in effect across a whole window, then revised
            (10, 2400, "10.5"),     // and again in the last window
            (1998, 2000, "1998.5"), // on the third window's first vintage
            (20, 30, "."),          // revised to missing
        ]
        .map(|(o, v, s)| (o, v, s.to_string())),
    );
    let revisions: Vec<(usize, usize, &str)> = revisions
        .iter()
        .map(|(o, v, s)| (*o, *v, s.as_str()))
        .collect();
    History::daily(2500, &revisions)
}

/// Fetches through the full path (metadata, vintage dates, observations) with `ctx`.
async fn fetch(
    ctx: &CrawlCtx,
    mock: &MockSource,
    known_vintage: Option<NaiveDate>,
) -> Result<FetchedSeries, CrawlError> {
    FredAdapter::new(mock.base_url())
        .fetch_series_incremental(ctx, "DFF", None, known_vintage)
        .await
}

/// Normal operation: a series with more vintages than FRED allows is planned in windows of at
/// most the default from the start, so FRED never rejects a request, and the points are those
/// of the whole history in one request.
#[tokio::test]
async fn a_large_series_is_requested_in_windows_below_the_limit() {
    let mock = MockSource::start().await;
    let fake = FakeAlfred::new(large_history());
    fake.mount(&mock).await;

    let s = fetch(&test_ctx(), &mock, None).await.unwrap();
    assert_eq!(s.points, fake.history.expected(None));

    let log = fake.log();
    assert!(log.iter().all(|l| l.status == 200), "{log:#?}");
    assert_eq!(
        log[0].path, "/series/vintagedates",
        "planned before any observations"
    );
    let obs = fake.observation_requests();
    let windows: Vec<_> = obs
        .iter()
        .map(|l| (l.realtime_start, l.realtime_end))
        .collect();
    let v = &fake.history.vintages;
    assert_eq!(
        windows,
        vec![
            (earliest_realtime_start(), v[999]),
            (v[999], v[1999]),
            (v[1999], latest_realtime_end()),
        ]
    );
    // The default, plus the previous window's last vintage on which each later window starts.
    for l in &obs {
        assert!(
            l.vintages <= FRED_MAX_VINTAGE_DATES_PER_REQUEST + 1,
            "{l:?}"
        );
    }

    // The boundary revisions, spelled out.
    let at = |obs: usize| v[obs].pred_opt().unwrap();
    let of = |obs: usize| -> Vec<(NaiveDate, String, bool)> {
        s.points
            .iter()
            .filter(|p| p.date == at(obs))
            .map(|p| {
                let value = p.value.as_ref().map_or(".".into(), |x| x.to_string());
                (p.revision_date, value, p.is_original_release)
            })
            .collect()
    };
    assert_eq!(
        of(995),
        vec![
            (v[995], "995".into(), true),
            (v[1000], "995.25".into(), false)
        ]
    );
    assert_eq!(
        of(990),
        vec![
            (v[990], "990".into(), true),
            (v[999], "990.25".into(), false)
        ]
    );
    assert_eq!(
        of(999),
        vec![
            (v[999], "999".into(), true),
            (v[1001], "999.25".into(), false)
        ]
    );
    assert_eq!(
        of(10),
        vec![
            (v[10], "10".into(), true),
            (v[1500], "10.25".into(), false),
            (v[2400], "10.5".into(), false),
        ]
    );
    assert_eq!(
        of(20),
        vec![(v[20], "20".into(), true), (v[30], ".".into(), false)]
    );
}

/// A series under the default is one observations request, the same one as before windowing.
#[tokio::test]
async fn a_small_series_is_one_request_for_the_whole_window() {
    let mock = MockSource::start().await;
    let fake = FakeAlfred::new(History::daily(300, &[(5, 200, "5.5")]));
    fake.mount(&mock).await;

    let s = fetch(&test_ctx(), &mock, None).await.unwrap();
    assert_eq!(s.points, fake.history.expected(None));
    let obs = fake.observation_requests();
    assert_eq!(obs.len(), 1);
    assert_eq!(
        (obs[0].realtime_start, obs[0].realtime_end),
        (earliest_realtime_start(), latest_realtime_end())
    );
}

/// Regression for the deployed failure (DFF, DGS10, EFFR): FRED rejects a window with more
/// vintage dates than it allows. Here its limit (300) is below the default, as if FRED had
/// lowered it: the rejected window is split in half until FRED accepts, every follow-up request
/// is smaller than the one rejected, and every revision still arrives exactly once.
#[tokio::test]
async fn a_rejected_window_is_split_until_fred_accepts_it() {
    let mock = MockSource::start().await;
    let mut fake = FakeAlfred::new(large_history());
    fake.vintage_limit = 300;
    fake.mount(&mock).await;

    let s = fetch(&test_ctx(), &mock, None).await.unwrap();
    assert_eq!(s.points, fake.history.expected(None));

    let obs = fake.observation_requests();
    // The first planned window (1,000 vintages) is rejected, then halved: 500 (rejected),
    // 250 (accepted).
    assert_eq!(obs[0].status, 400);
    assert_eq!(obs[0].vintages, FRED_MAX_VINTAGE_DATES_PER_REQUEST);
    assert_eq!(
        (obs[1].status, obs[1].vintages, obs[1].realtime_start),
        (400, 500, earliest_realtime_start())
    );
    assert_eq!((obs[2].status, obs[2].vintages), (200, 250));
    let rejected: Vec<_> = obs.iter().filter(|l| l.status == 400).collect();
    let accepted: Vec<_> = obs.iter().filter(|l| l.status == 200).collect();
    assert!(accepted.iter().all(|l| l.vintages <= 300), "{accepted:#?}");
    // The two planned windows of 1,000 are rejected, then each of their halves (501 with the
    // boundary vintage), and their quarters accepted; the last window of 500 (501) is rejected
    // once and its halves accepted. No window is requested twice.
    assert_eq!((rejected.len(), accepted.len()), (7, 10));
    let mut windows: Vec<_> = obs
        .iter()
        .map(|l| (l.realtime_start, l.realtime_end))
        .collect();
    windows.sort();
    windows.dedup();
    assert_eq!(windows.len(), obs.len());
    // The vintage dates were listed once, up front.
    let listings = fake
        .log()
        .iter()
        .filter(|l| l.path == "/series/vintagedates")
        .count();
    assert_eq!(listings, 1);
}

/// The deployed request shape: one window for everything, rejected. With FRED's real limit and
/// a series of 5,151 vintages, a window FRED rejects is split rather than failing the series.
#[tokio::test]
async fn the_full_window_rejection_is_answered_by_smaller_requests() {
    let mock = MockSource::start().await;
    let fake = FakeAlfred::new(History::daily(5151, &[(100, 3000, "100.5")]));
    fake.mount(&mock).await;
    let ctx = test_ctx();
    let adapter = FredAdapter::new(mock.base_url());
    let whole = RealtimeWindow {
        start: earliest_realtime_start(),
        end: latest_realtime_end(),
        keep_from: None,
        vintages: None,
    };
    // What the crawler used to send, and FRED's answer to it.
    let err = adapter
        .fetch_window(&ctx, TEST_KEY, "DFF", None, &whole)
        .await
        .unwrap_err();
    assert!(is_too_many_vintage_dates(&err), "{err}");
    let parts = adapter
        .split_rejected(&ctx, TEST_KEY, "DFF", &whole)
        .await
        .unwrap();
    assert_eq!(parts.len(), 6);
    let mut rows = Vec::new();
    for part in &parts {
        rows.extend(
            adapter
                .fetch_window(&ctx, TEST_KEY, "DFF", None, part)
                .await
                .unwrap(),
        );
    }
    assert_eq!(to_points(rows), fake.history.expected(None));
    let obs = fake.observation_requests();
    assert_eq!(obs[0].vintages, 5151);
    assert!(obs[1..]
        .iter()
        .all(|l| l.status == 200 && l.vintages <= 1001));
}

/// Row paging (`limit`/`offset`) still runs inside each window.
#[tokio::test]
async fn each_window_is_paged() {
    let mock = MockSource::start().await;
    let mut fake = FakeAlfred::new(large_history());
    fake.page_rows = 400;
    fake.vintage_page = 700; // the vintage-date listing is paged too
    fake.mount(&mock).await;

    let s = fetch(&test_ctx(), &mock, None).await.unwrap();
    assert_eq!(s.points, fake.history.expected(None));
    let obs = fake.observation_requests();
    let mut pages_per_window: BTreeMap<NaiveDate, Vec<usize>> = BTreeMap::new();
    for l in &obs {
        pages_per_window
            .entry(l.realtime_start)
            .or_default()
            .push(l.offset);
    }
    assert_eq!(pages_per_window.len(), 3);
    for offsets in pages_per_window.values() {
        assert!(offsets.len() >= 3, "{offsets:?}");
        assert!(
            offsets.windows(2).all(|w| w[1] == w[0] + 400),
            "{offsets:?}"
        );
    }
    let listings: Vec<usize> = fake
        .log()
        .iter()
        .filter(|l| l.path == "/series/vintagedates")
        .map(|l| l.offset)
        .collect();
    assert_eq!(listings, [0, 700, 1400, 2100]);
}

/// An incremental fetch lists the vintage dates from the day before the known vintage and
/// windows them the same way; rows from before it are not returned as points, and the dates
/// first published before it have no original release among the new points.
#[tokio::test]
async fn an_incremental_fetch_of_many_new_vintages_is_windowed_too() {
    let mock = MockSource::start().await;
    let fake = FakeAlfred::new(large_history());
    fake.mount(&mock).await;
    let v = fake.history.vintages.clone();
    let known = v[200];

    let s = fetch(&test_ctx(), &mock, Some(known)).await.unwrap();
    assert_eq!(s.points, fake.history.expected(Some(known)));
    assert!(s.points.iter().all(|p| p.revision_date >= known));

    let listing = &fake.log()[0];
    assert_eq!(listing.path, "/series/vintagedates");
    assert_eq!(listing.realtime_start, v[199]);
    let obs = fake.observation_requests();
    let windows: Vec<_> = obs
        .iter()
        .map(|l| (l.realtime_start, l.realtime_end))
        .collect();
    // 2,301 vintages from v[199]: windows of 1,000, 1,000 and 301.
    assert_eq!(
        windows,
        vec![
            (v[199], v[1198]),
            (v[1198], v[2198]),
            (v[2198], latest_realtime_end()),
        ]
    );
    assert!(obs.iter().all(|l| l.status == 200));

    // A refresh with only a few new vintages is one observations request.
    let fake_small = FakeAlfred::new(large_history());
    let mock = MockSource::start().await;
    fake_small.mount(&mock).await;
    let known = v[2490];
    let s = fetch(&test_ctx(), &mock, Some(known)).await.unwrap();
    assert_eq!(s.points, fake_small.history.expected(Some(known)));
    let obs = fake_small.observation_requests();
    assert_eq!(obs.len(), 1);
    assert_eq!(obs[0].realtime_start, v[2489]);
}

/// Any other 400 on a window is permanent, and isn't retried or split.
#[tokio::test]
async fn an_unrelated_400_is_not_split() {
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
        .await;
    mock.mount(
        &Route::get("/series/vintagedates"),
        Reply::json(serde_json::json!({"count": 3, "vintage_dates":
            ["2020-01-01", "2020-01-02", "2020-01-03"]})),
    )
    .await;
    mock.mount_expect(
        &Route::get("/series/observations"),
        Reply::json_str(ERR_BAD_VARIABLE).with_status(400),
        1,
    )
    .await;
    let e = fetch(&test_ctx(), &mock, None).await.unwrap_err();
    assert_eq!(e.kind(), "permanent", "{e}");
    mock.server().verify().await;
    let vintage_listings = mock
        .received_requests()
        .await
        .iter()
        .filter(|r| r.url.path() == "/series/vintagedates")
        .count();
    assert_eq!(vintage_listings, 1, "no re-listing to split");

    // An unknown series on the vintage-date listing is NotFound, as on the metadata.
    let mock = MockSource::start().await;
    mock.mount(&Route::get("/series"), Reply::json_str(SERIES_GDP))
        .await;
    mock.mount(
        &Route::get("/series/vintagedates"),
        Reply::json_str(
            "{\"error_code\":400,\"error_message\":\"Bad Request.  The series does not exist.\"}",
        )
        .with_status(400),
    )
    .await;
    let e = fetch(&test_ctx(), &mock, None).await.unwrap_err();
    assert_eq!(e.kind(), "not_found", "{e}");
}

/// A window FRED rejects even at a single vintage can't be split further: the fetch fails
/// permanently, after a bounded number of requests, rather than looping.
#[tokio::test]
async fn splitting_stops_at_a_single_vintage() {
    let mock = MockSource::start().await;
    let mut fake = FakeAlfred::new(History::daily(8, &[]));
    fake.vintage_limit = 0;
    fake.mount(&mock).await;
    let e = fetch(&test_ctx(), &mock, None).await.unwrap_err();
    assert!(is_too_many_vintage_dates(&e), "{e}");
    // 8 vintages: 8, then 4, 2 and 1, giving up at the first window of one.
    let obs: Vec<usize> = fake
        .observation_requests()
        .iter()
        .map(|l| l.vintages)
        .collect();
    assert_eq!(obs, [8, 4, 2, 1]);
}

/// Windows are requested one after another through the shared HTTP client, so FRED's rate limit
/// spaces them out rather than letting a windowed series burst.
#[tokio::test]
async fn windows_are_rate_limited() {
    let mock = MockSource::start().await;
    let mut fake = FakeAlfred::new(large_history());
    fake.vintage_limit = 300; // more requests: rejected windows count too
    fake.mount(&mock).await;
    let rate = 50.0;
    let ctx = test_ctx_with(HashMap::from([(
        SourceId::Fred,
        SourcePolicy {
            requests_per_second: rate,
            burst: 1,
            max_concurrency: 4,
            ..fast_policy(SourceId::Fred)
        },
    )]));
    let started = Instant::now();
    fetch(&ctx, &mock, None).await.unwrap();
    let elapsed = started.elapsed();

    // Every request: metadata, the listing and 17 observation windows.
    let total = mock.received_requests().await.len();
    assert_eq!(total, 1 + 1 + 17);
    let interval = Duration::from_secs_f64(1.0 / rate);
    let floor = interval * (total as u32 - 1);
    assert!(
        elapsed >= floor,
        "{total} requests in {elapsed:?}, under {floor:?}"
    );
    let log = fake.log();
    let span = log.last().unwrap().at - log[0].at;
    let fake_floor = interval * (log.len() as u32 - 1);
    // Arrival times jitter by the local round trip; allow 10% of the whole span for it.
    assert!(
        span >= fake_floor.mul_f64(0.9),
        "{} requests reached FRED within {span:?}",
        log.len()
    );
}

const TEST_KEY: &str = crate::testkit::TEST_API_KEY;
