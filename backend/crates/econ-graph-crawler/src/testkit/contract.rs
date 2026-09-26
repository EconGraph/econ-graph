// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Assertions every [`SourceAdapter`] must satisfy, and [`adapter_contract_tests!`](crate::adapter_contract_tests)
//! which turns them into a standard set of `#[tokio::test]`s.
//!
//! Each function takes the adapter (already pointed at `mock.base_url()`), a context from
//! [`test_ctx`](super::test_ctx) and the [`MockSource`]. Success assertions expect the caller to
//! have mounted the fixtures; error assertions mount their own reply on `route`.

use std::collections::HashSet;

use crate::adapter::{CrawlCtx, DiscoveredSeries, FetchedSeries, SourceAdapter};
use crate::error::CrawlError;

use super::{MockSource, Reply, Route};

/// A body that no JSON adapter can parse. Used by the macro when `malformed_reply` is omitted.
pub const MALFORMED_JSON: &str = "{ \"this is\": not json";

/// `fetch_series(external_id, None)` succeeds with exactly `expect_points` points, no duplicate
/// `(date, revision_date)` pairs, and the request actually reached the mock. Returns the series
/// for further, source-specific checks.
pub async fn assert_fetch_ok(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    external_id: &str,
    expect_points: usize,
) -> FetchedSeries {
    let series = adapter
        .fetch_series(ctx, external_id, None)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "{:?} fetch_series({external_id:?}) failed: {e}",
                adapter.id()
            )
        });
    assert_hit_mock(mock).await;
    assert_eq!(
        series.points.len(),
        expect_points,
        "{:?} fetch_series({external_id:?}): wrong number of points",
        adapter.id()
    );
    let mut seen = HashSet::new();
    for p in &series.points {
        assert!(
            seen.insert((p.date, p.revision_date)),
            "{:?} fetch_series({external_id:?}): duplicate point for {} (revision {})",
            adapter.id(),
            p.date,
            p.revision_date
        );
    }
    series
}

/// Mounts `reply` on `route`, calls `fetch_series(external_id, None)` and asserts it fails with an
/// error whose [`CrawlError::kind`] is `expected_kind`. Returns the error.
pub async fn assert_fetch_error(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    route: &Route,
    reply: Reply,
    external_id: &str,
    expected_kind: &str,
) -> CrawlError {
    mock.mount(route, reply).await;
    let result = adapter.fetch_series(ctx, external_id, None).await;
    assert_hit_mock(mock).await;
    match result {
        Ok(s) => panic!(
            "{:?} fetch_series({external_id:?}) on {route}: expected a {expected_kind} error, got Ok with {} points",
            adapter.id(),
            s.points.len()
        ),
        Err(e) => {
            assert_eq!(
                e.kind(),
                expected_kind,
                "{:?} fetch_series({external_id:?}) on {route}: wrong error: {e}",
                adapter.id()
            );
            e
        }
    }
}

/// HTTP 404 on `route` -> [`CrawlError::NotFound`].
pub async fn assert_fetch_maps_404_to_not_found(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    route: &Route,
    external_id: &str,
) -> CrawlError {
    let reply = Reply::json_str("{\"error\":\"not found\"}").with_status(404);
    assert_fetch_error(adapter, ctx, mock, route, reply, external_id, "not_found").await
}

/// HTTP 429 (with `Retry-After: 1`) on `route` -> [`CrawlError::RateLimited`].
pub async fn assert_fetch_maps_429_to_rate_limited(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    route: &Route,
    external_id: &str,
) -> CrawlError {
    let reply = Reply::status(429).retry_after(1);
    assert_fetch_error(
        adapter,
        ctx,
        mock,
        route,
        reply,
        external_id,
        "rate_limited",
    )
    .await
}

/// HTTP 500 on `route` -> [`CrawlError::Transient`].
pub async fn assert_fetch_maps_500_to_transient(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    route: &Route,
    external_id: &str,
) -> CrawlError {
    let reply = Reply::text("internal error").with_status(500);
    assert_fetch_error(adapter, ctx, mock, route, reply, external_id, "transient").await
}

/// A 200 with `malformed` as the body on `route` -> [`CrawlError::Parse`].
/// Use [`MALFORMED_JSON`] for JSON sources; text/XML sources pass something their parser rejects.
pub async fn assert_fetch_malformed_body_is_parse_error(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    route: &Route,
    malformed: Reply,
    external_id: &str,
) -> CrawlError {
    assert_fetch_error(adapter, ctx, mock, route, malformed, external_id, "parse").await
}

/// `discover()` succeeds with at least `min_series` entries, each with a non-empty
/// `external_id` and `title`, and the request reached the mock. Returns the entries.
pub async fn assert_discover_ok(
    adapter: &dyn SourceAdapter,
    ctx: &CrawlCtx,
    mock: &MockSource,
    min_series: usize,
) -> Vec<DiscoveredSeries> {
    let found = adapter
        .discover(ctx)
        .await
        .unwrap_or_else(|e| panic!("{:?} discover() failed: {e}", adapter.id()));
    assert_hit_mock(mock).await;
    assert!(
        found.len() >= min_series,
        "{:?} discover(): {} series, expected at least {min_series}",
        adapter.id(),
        found.len()
    );
    for s in &found {
        assert!(
            !s.external_id.trim().is_empty(),
            "{:?} discover(): empty external_id in {s:?}",
            adapter.id()
        );
        assert!(
            !s.title.trim().is_empty(),
            "{:?} discover(): empty title for {}",
            adapter.id(),
            s.external_id
        );
    }
    found
}

async fn assert_hit_mock(mock: &MockSource) {
    assert!(
        !mock.received_requests().await.is_empty(),
        "the adapter never called the mock server; does it use the base_url passed to new()?"
    );
}

/// Expands to the standard adapter contract `#[tokio::test]`s. Invoke it inside a dedicated
/// test module (the generated function names are fixed: `contract_fetch_ok`, `contract_fetch_404_not_found`,
/// `contract_fetch_429_rate_limited`, `contract_fetch_500_transient`,
/// `contract_fetch_malformed_is_parse_error`, and `contract_discover_ok` when `discover` is given).
///
/// ```rust,ignore
/// crate::adapter_contract_tests! {
///     // Optional attributes applied to every generated test, e.g. #[ignore = "reason"].
///     adapter: |base_url: String| MyAdapter::new(base_url),   // closure or fn: String -> adapter
///     external_id: "GDP",
///     route: Route::get("/series/GDP"),        // the request fetch_series(external_id) makes
///     ok_reply: Reply::json_str(include_str!("...")),
///     expect_points: 3,
///     // Optional, in this order:
///     malformed_reply: Reply::text("garbage"),  // default: Reply::json_str(MALFORMED_JSON)
///     setup: |mock| { mock.mount(&Route::get("/meta"), Reply::json(..)).await; },
///     discover: { route: Route::get("/catalog"), reply: Reply::json(..), min_series: 2 },
/// }
/// ```
///
/// `setup` runs before every generated test, after the mock starts and before the test's own
/// fixture is mounted; use it for secondary requests (metadata, auth tokens) the fetch also makes.
/// `route`, `ok_reply` etc. are re-evaluated in each test, so they may be any expressions.
#[macro_export]
macro_rules! adapter_contract_tests {
    // With discovery: the fetch tests, plus `contract_discover_ok`.
    (
        $(#[$meta:meta])*
        adapter: $adapter:expr,
        external_id: $id:expr,
        route: $route:expr,
        ok_reply: $ok:expr,
        expect_points: $points:expr
        $(, malformed_reply: $malformed:expr)?
        $(, setup: |$m:ident| $setup:block)?,
        discover: {
            route: $droute:expr,
            reply: $dreply:expr,
            min_series: $dmin:expr $(,)?
        }
        $(,)?
    ) => {
        $crate::adapter_contract_tests! {
            $(#[$meta])*
            adapter: $adapter,
            external_id: $id,
            route: $route,
            ok_reply: $ok,
            expect_points: $points
            $(, malformed_reply: $malformed)?
            $(, setup: |$m| $setup)?
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_discover_ok() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            mock.mount(&$droute, $dreply).await;
            $crate::testkit::contract::assert_discover_ok(&*adapter, &ctx, &mock, $dmin).await;
        }
    };

    // Fetch tests only.
    (
        $(#[$meta:meta])*
        adapter: $adapter:expr,
        external_id: $id:expr,
        route: $route:expr,
        ok_reply: $ok:expr,
        expect_points: $points:expr
        $(, malformed_reply: $malformed:expr)?
        $(, setup: |$m:ident| $setup:block)?
        $(,)?
    ) => {
        async fn __contract_prepare() -> (
            $crate::testkit::MockSource,
            $crate::CrawlCtx,
            ::std::boxed::Box<dyn $crate::SourceAdapter>,
        ) {
            let mock = $crate::testkit::MockSource::start().await;
            $( {
                let $m: &$crate::testkit::MockSource = &mock;
                $setup
            } )?
            let adapter = ($adapter)(mock.base_url());
            (
                mock,
                $crate::testkit::test_ctx(),
                ::std::boxed::Box::new(adapter),
            )
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_fetch_ok() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            mock.mount(&$route, $ok).await;
            $crate::testkit::contract::assert_fetch_ok(&*adapter, &ctx, &mock, $id, $points).await;
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_fetch_404_not_found() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            $crate::testkit::contract::assert_fetch_maps_404_to_not_found(
                &*adapter, &ctx, &mock, &$route, $id,
            )
            .await;
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_fetch_429_rate_limited() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            $crate::testkit::contract::assert_fetch_maps_429_to_rate_limited(
                &*adapter, &ctx, &mock, &$route, $id,
            )
            .await;
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_fetch_500_transient() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            $crate::testkit::contract::assert_fetch_maps_500_to_transient(
                &*adapter, &ctx, &mock, &$route, $id,
            )
            .await;
        }

        $(#[$meta])*
        #[::tokio::test]
        async fn contract_fetch_malformed_is_parse_error() {
            let (mock, ctx, adapter) = __contract_prepare().await;
            #[allow(unused_mut, unused_assignments)]
            let mut malformed =
                $crate::testkit::Reply::json_str($crate::testkit::contract::MALFORMED_JSON);
            $( malformed = $malformed; )?
            $crate::testkit::contract::assert_fetch_malformed_body_is_parse_error(
                &*adapter, &ctx, &mock, &$route, malformed, $id,
            )
            .await;
        }
    };
}
