// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Per-source rate limiting and concurrency control.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::source::SourceId;

/// Token-bucket rate limiter (governor) plus a concurrency semaphore for every [`SourceId`].
///
/// Cheap to clone; clones share state.
#[derive(Clone)]
pub struct SourceRateLimiter {
    buckets: Arc<HashMap<SourceId, Bucket>>,
}

struct Bucket {
    limiter: DefaultDirectRateLimiter,
    concurrency: Arc<Semaphore>,
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
    /// Fails with [`CrawlError::Permanent`] if a policy has a non-positive/non-finite rate.
    /// `burst` and `max_concurrency` of 0 are treated as 1.
    pub fn new(policies: &HashMap<SourceId, SourcePolicy>) -> Result<Self, CrawlError> {
        let mut buckets = HashMap::with_capacity(SourceId::ALL.len());
        for id in SourceId::ALL {
            let policy = policies
                .get(&id)
                .copied()
                .unwrap_or_else(|| SourcePolicy::default_for(id));
            let rps = policy.requests_per_second;
            if !rps.is_finite() || rps <= 0.0 {
                return Err(CrawlError::Permanent(format!(
                    "invalid requests_per_second {rps} for {id}"
                )));
            }
            let quota = Quota::with_period(Duration::from_secs_f64(1.0 / rps))
                .ok_or_else(|| {
                    CrawlError::Permanent(format!("requests_per_second {rps} too high for {id}"))
                })?
                .allow_burst(NonZeroU32::new(policy.burst.max(1)).unwrap_or(NonZeroU32::MIN));
            buckets.insert(
                id,
                Bucket {
                    limiter: RateLimiter::direct(quota),
                    concurrency: Arc::new(Semaphore::new(policy.max_concurrency.max(1))),
                },
            );
        }
        Ok(Self {
            buckets: Arc::new(buckets),
        })
    }

    /// Waits for a concurrency slot and a rate-limit token for `source`, in that order,
    /// awaiting governor readiness (no polling). Hold the returned permit until the
    /// request (including reading the body) completes.
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
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
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

    // governor's default clock is a real monotonic clock (quanta), not tokio's, so these
    // tests use real time with tolerances.
    #[tokio::test]
    async fn enforces_rate() {
        let l = limiter_for(SourceId::Fred, 5.0, 1, 10);
        let start = std::time::Instant::now();
        for _ in 0..6 {
            drop(l.acquire(SourceId::Fred).await);
        }
        let elapsed = start.elapsed();
        // First token is immediate, the next five arrive every 200ms.
        assert!(elapsed >= Duration::from_millis(950), "{elapsed:?}");
        assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    }

    #[tokio::test]
    async fn burst_is_immediate() {
        let l = limiter_for(SourceId::Fred, 1.0, 5, 10);
        let start = std::time::Instant::now();
        for _ in 0..5 {
            drop(l.acquire(SourceId::Fred).await);
        }
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn sub_one_per_second_rate() {
        // 25/min: one token per 2.4s. The first is immediate, the second must wait.
        let l = limiter_for(SourceId::Bls, 25.0 / 60.0, 1, 1);
        drop(l.acquire(SourceId::Bls).await);
        let second = tokio::time::timeout(Duration::from_millis(500), l.acquire(SourceId::Bls));
        assert!(
            second.await.is_err(),
            "second BLS token should not be ready yet"
        );
    }

    #[tokio::test]
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
        let permit = tokio::time::timeout(Duration::from_millis(200), l.acquire(SourceId::Bls))
            .await
            .expect("independent bucket");
        drop(permit);
    }

    #[tokio::test]
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
