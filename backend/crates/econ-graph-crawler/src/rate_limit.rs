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
    #[allow(dead_code)] // used by `acquire` (A3)
    limiter: DefaultDirectRateLimiter,
    #[allow(dead_code)] // used by `acquire` (A3)
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
        let _ = source;
        todo!("A3")
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
