// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Per-source rate limiting and concurrency control.
//!
//! The rate limiter is a small in-house GCRA (generic cell rate algorithm, the "virtual
//! scheduling" form of a token bucket) on tokio's clock, so tests can drive it with
//! `tokio::time::pause`. It replaces `governor` 0.6, which granted `burst + 1` back-to-back
//! requests after an idle period.
//!
//! # Guarantee
//!
//! With emission interval `T = ceil_ns(1 / requests_per_second)` and burst `b`, the grant times
//! `g_1 <= g_2 <= ...` of one source satisfy, for any `i <= j`,
//!
//! ```text
//! (j - i + 1) <= b + (g_j - g_i) / T <= b + requests_per_second * (g_j - g_i)
//! ```
//!
//! i.e. at most `burst + rate * window` requests in any window. Proof: the state is the
//! theoretical arrival time `tat`. A request at `g_k` is granted only if
//! `g_k >= tat_{k-1} - (b - 1) T`, and then `tat_k = max(tat_{k-1}, g_k) + T`. Hence
//! `tat_i >= g_i + T` and `tat_m >= tat_{m-1} + T`, so `tat_{j-1} >= g_i + (j - i) T`, and
//! `g_j >= tat_{j-1} - (b - 1) T >= g_i + (j - i + 1 - b) T`. Rounding `T` up to whole
//! nanoseconds only makes it stricter. A fresh or fully idle bucket (`tat <= now`) therefore
//! grants exactly `b` requests at once, never `b + 1`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// Longest emission interval accepted (slowest rate): one request per ~year.
const MAX_INTERVAL: Duration = Duration::from_secs(366 * 24 * 3600);

/// Rate limiter (GCRA, see the module docs) plus a concurrency semaphore for every [`SourceId`].
///
/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct SourceRateLimiter {
    buckets: Arc<HashMap<SourceId, Bucket>>,
}

struct Bucket {
    limiter: Gcra,
    concurrency: Arc<Semaphore>,
}

/// Strict GCRA limiter on tokio's clock.
#[derive(Debug)]
struct Gcra {
    /// Emission interval `T` (time per request at the sustained rate).
    interval: Duration,
    /// `(burst - 1) * T`: how far past `now` the theoretical arrival time may run.
    tolerance: Duration,
    /// Theoretical arrival time; `None` until the first grant. Waiters queue on this lock
    /// (tokio's mutex is FIFO) and hold it while sleeping, so grants are fair and a cancelled
    /// waiter consumes nothing.
    tat: Mutex<Option<Instant>>,
}

impl Gcra {
    fn new(rps: f64, burst: u32) -> Result<Self, String> {
        if !rps.is_finite() || rps <= 0.0 {
            return Err(format!("invalid requests_per_second {rps}"));
        }
        // Round up so the effective rate never exceeds the policy.
        let nanos = (1e9 / rps).ceil().max(1.0);
        if nanos > MAX_INTERVAL.as_nanos() as f64 {
            return Err(format!("requests_per_second {rps} too low"));
        }
        let interval = Duration::from_nanos(nanos as u64);
        let tolerance = interval
            .checked_mul(burst.max(1) - 1)
            .filter(|t| *t <= MAX_INTERVAL.saturating_mul(16))
            .ok_or_else(|| format!("burst {burst} too large for requests_per_second {rps}"))?;
        Ok(Self {
            interval,
            tolerance,
            tat: Mutex::new(None),
        })
    }

    /// Waits until one more request conforms to the policy, then records it.
    async fn until_ready(&self) {
        let mut tat = self.tat.lock().await;
        if let Some(t) = *tat {
            // Earliest conforming time: tat - (burst - 1) * T.
            if let Some(allowed_at) = t.checked_sub(self.tolerance) {
                if allowed_at > Instant::now() {
                    tokio::time::sleep_until(allowed_at).await;
                }
            }
        }
        let now = Instant::now();
        let base = match *tat {
            Some(t) if t > now => t,
            _ => now,
        };
        *tat = Some(base + self.interval);
    }
}

/// Held for the duration of one request; dropping it frees the concurrency slot.
#[must_use = "the concurrency slot is released when the permit is dropped"]
#[derive(Debug)]
pub struct SourcePermit {
    _slot: OwnedSemaphorePermit,
}

impl std::fmt::Debug for SourceRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sources: Vec<_> = self.buckets.keys().collect();
        sources.sort();
        f.debug_struct("SourceRateLimiter")
            .field("sources", &sources)
            .finish()
    }
}

impl SourceRateLimiter {
    /// Builds a limiter for every source. Sources absent from `policies` use
    /// [`SourcePolicy::default_for`].
    ///
    /// Fails with [`CrawlError::Permanent`] if a policy has a non-positive/non-finite rate, a
    /// rate slower than one request per year, or a burst whose window would overflow.
    /// `burst` and `max_concurrency` of 0 are treated as 1.
    pub fn new(policies: &HashMap<SourceId, SourcePolicy>) -> Result<Self, CrawlError> {
        let mut buckets = HashMap::with_capacity(SourceId::ALL.len());
        for id in SourceId::ALL {
            let policy = policies
                .get(&id)
                .copied()
                .unwrap_or_else(|| SourcePolicy::default_for(id));
            let limiter = Gcra::new(policy.requests_per_second, policy.burst)
                .map_err(|e| CrawlError::Permanent(format!("{e} for {id}")))?;
            buckets.insert(
                id,
                Bucket {
                    limiter,
                    concurrency: Arc::new(Semaphore::new(policy.max_concurrency.max(1))),
                },
            );
        }
        Ok(Self {
            buckets: Arc::new(buckets),
        })
    }

    /// Waits for a concurrency slot and a rate-limit grant for `source`, in that order (no
    /// polling). Hold the returned permit until the request (including reading the body)
    /// completes. Dropping the future before it resolves consumes no rate budget.
    pub async fn acquire(&self, source: SourceId) -> SourcePermit {
        let bucket = self
            .buckets
            .get(&source)
            .expect("SourceRateLimiter::new builds a bucket for every SourceId");
        let slot = Arc::clone(&bucket.concurrency)
            .acquire_owned()
            .await
            .expect("source semaphores are never closed");
        bucket.limiter.until_ready().await;
        SourcePermit { _slot: slot }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_for_all_sources_with_defaults() {
        let l = SourceRateLimiter::new(&HashMap::new()).unwrap();
        assert_eq!(l.buckets.len(), SourceId::ALL.len());
    }

    #[test]
    fn rejects_invalid_rate() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-12] {
            let mut p = HashMap::new();
            let mut policy = SourcePolicy::default_for(SourceId::Fred);
            policy.requests_per_second = bad;
            p.insert(SourceId::Fred, policy);
            assert!(matches!(
                SourceRateLimiter::new(&p),
                Err(CrawlError::Permanent(_))
            ));
        }
    }

    #[test]
    fn rejects_overflowing_burst() {
        let mut policy = SourcePolicy::default_for(SourceId::Fred);
        policy.requests_per_second = 1e-7; // ~116 days per request
        policy.burst = u32::MAX;
        assert!(matches!(
            SourceRateLimiter::new(&HashMap::from([(SourceId::Fred, policy)])),
            Err(CrawlError::Permanent(_))
        ));
    }

    #[test]
    fn interval_rounds_up() {
        // 3/s is 333_333_333.33ns; rounding down would exceed the policy rate.
        let g = Gcra::new(3.0, 1).unwrap();
        assert_eq!(g.interval, Duration::from_nanos(333_333_334));
        let g = Gcra::new(2.0, 4).unwrap();
        assert_eq!(g.tolerance, Duration::from_millis(1500));
    }

    fn limiter_for(
        source: SourceId,
        rps: f64,
        burst: u32,
        concurrency: usize,
    ) -> SourceRateLimiter {
        let mut policy = SourcePolicy::default_for(source);
        policy.requests_per_second = rps;
        policy.burst = burst;
        policy.max_concurrency = concurrency;
        SourceRateLimiter::new(&HashMap::from([(source, policy)])).unwrap()
    }

    // The limiter runs on tokio's clock, so these tests use paused (virtual) time: they are
    // instant and the timings are exact.

    #[tokio::test(start_paused = true)]
    async fn enforces_rate() {
        let l = limiter_for(SourceId::Fred, 5.0, 1, 10);
        let start = Instant::now();
        for _ in 0..6 {
            drop(l.acquire(SourceId::Fred).await);
        }
        // First grant is immediate, the next five arrive every 200ms.
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn burst_is_immediate_then_rate_limited() {
        let l = limiter_for(SourceId::Fred, 1.0, 5, 10);
        let start = Instant::now();
        for _ in 0..5 {
            drop(l.acquire(SourceId::Fred).await);
        }
        assert_eq!(start.elapsed(), Duration::ZERO);
        drop(l.acquire(SourceId::Fred).await);
        assert_eq!(start.elapsed(), Duration::from_secs(1));
    }

    /// Regression for governor 0.6.3, which granted `burst + 1` at once after an idle period.
    #[tokio::test(start_paused = true)]
    async fn idle_bucket_grants_exactly_burst() {
        for burst in [1u32, 2, 4] {
            let l = limiter_for(SourceId::Fred, 2.0, burst, 16);
            for round in 0..3 {
                let start = Instant::now();
                let mut immediate = 0;
                for _ in 0..burst + 2 {
                    drop(l.acquire(SourceId::Fred).await);
                    if start.elapsed() == Duration::ZERO {
                        immediate += 1;
                    }
                }
                assert_eq!(immediate, burst, "burst {burst}, round {round}");
                tokio::time::sleep(Duration::from_secs(10)).await; // refill completely
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn partial_refill_grants_accrued_tokens_only() {
        let l = limiter_for(SourceId::Fred, 2.0, 4, 16);
        for _ in 0..4 {
            drop(l.acquire(SourceId::Fred).await);
        }
        // 1.1s at 2/s refills two tokens (plus a fraction).
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let start = Instant::now();
        drop(l.acquire(SourceId::Fred).await);
        drop(l.acquire(SourceId::Fred).await);
        assert_eq!(start.elapsed(), Duration::ZERO);
        drop(l.acquire(SourceId::Fred).await);
        assert_eq!(start.elapsed(), Duration::from_millis(400));
    }

    #[tokio::test(start_paused = true)]
    async fn sub_one_per_second_rate() {
        // 25/min: one grant per 2.4s. The first is immediate, the second must wait.
        let l = limiter_for(SourceId::Bls, 25.0 / 60.0, 1, 1);
        drop(l.acquire(SourceId::Bls).await);
        let second = tokio::time::timeout(Duration::from_millis(2399), l.acquire(SourceId::Bls));
        assert!(
            second.await.is_err(),
            "second BLS grant should not be ready yet"
        );
        // The cancelled wait consumed nothing: the grant comes at 2.4s (+ rounding), not 4.8s.
        let start = Instant::now();
        drop(l.acquire(SourceId::Bls).await);
        let waited = start.elapsed();
        assert!(
            waited >= Duration::from_millis(1) && waited < Duration::from_millis(2),
            "{waited:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn sources_are_independent() {
        let mut policies = HashMap::new();
        for id in [SourceId::Fred, SourceId::Bls] {
            let mut p = SourcePolicy::default_for(id);
            p.requests_per_second = 0.1;
            p.burst = 1;
            policies.insert(id, p);
        }
        let l = SourceRateLimiter::new(&policies).unwrap();
        drop(l.acquire(SourceId::Fred).await);
        // BLS has its own bucket, so FRED's exhausted one does not block it.
        let start = Instant::now();
        drop(l.acquire(SourceId::Bls).await);
        assert_eq!(start.elapsed(), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrency_limit() {
        let l = limiter_for(SourceId::Imf, 1000.0, 100, 2);
        let a = l.acquire(SourceId::Imf).await;
        let _b = l.acquire(SourceId::Imf).await;
        let third = l.clone();
        let mut waiter = tokio::spawn(async move { third.acquire(SourceId::Imf).await });
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut waiter)
                .await
                .is_err(),
            "third acquisition must wait while two permits are held"
        );
        drop(a);
        let permit = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("third acquisition proceeds once a slot frees")
            .unwrap();
        drop(permit);
    }

    /// Many concurrent tasks with bursty demand: in every window the grant count is at most
    /// `burst + window / T` (checked exactly, in integer nanoseconds).
    #[tokio::test(start_paused = true)]
    async fn grants_conform_under_concurrent_demand() {
        use rand::{RngExt, SeedableRng};
        for (rps, burst) in [(2.0, 4u32), (2.0, 1), (3.0, 3), (25.0 / 60.0, 1)] {
            let l = limiter_for(SourceId::Fred, rps, burst, 8);
            let grants = Arc::new(std::sync::Mutex::new(Vec::new()));
            let mut tasks = Vec::new();
            for seed in 0..8u64 {
                let (l, grants) = (l.clone(), grants.clone());
                tasks.push(tokio::spawn(async move {
                    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                    for _ in 0..15 {
                        let permit = l.acquire(SourceId::Fred).await;
                        grants.lock().unwrap().push(Instant::now());
                        let hold = rng.random_range(0..300);
                        tokio::time::sleep(Duration::from_millis(hold)).await;
                        drop(permit);
                        if rng.random_bool(0.2) {
                            let idle = rng.random_range(500..5000);
                            tokio::time::sleep(Duration::from_millis(idle)).await;
                        }
                    }
                }));
            }
            for t in tasks {
                t.await.unwrap();
            }
            let mut g = grants.lock().unwrap().clone();
            g.sort();
            assert_eq!(g.len(), 120);
            let t_ns = (1e9 / rps).ceil() as u128;
            let mut saw_full_burst = false;
            for i in 0..g.len() {
                for j in i..g.len() {
                    let n = (j - i + 1) as u128;
                    let dt = (g[j] - g[i]).as_nanos();
                    assert!(
                        n * t_ns <= u128::from(burst) * t_ns + dt,
                        "rps {rps} burst {burst}: {n} grants in {dt}ns"
                    );
                    saw_full_burst |= dt == 0 && n == u128::from(burst);
                }
            }
            assert!(saw_full_burst, "rps {rps} burst {burst}: burst never used");
        }
    }

    #[test]
    fn overrides_concurrency() {
        let mut p = HashMap::new();
        let mut policy = SourcePolicy::default_for(SourceId::Imf);
        policy.max_concurrency = 7;
        p.insert(SourceId::Imf, policy);
        let l = SourceRateLimiter::new(&p).unwrap();
        assert_eq!(l.buckets[&SourceId::Imf].concurrency.available_permits(), 7);
    }
}
