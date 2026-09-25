// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! Per-source politeness and retry policy.

use std::time::Duration;

use crate::source::SourceId;

/// How the crawler may talk to one source: request rate, concurrency and retry behaviour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourcePolicy {
    /// Sustained request rate (token-bucket refill rate). May be below 1.0 (e.g. 25/min).
    pub requests_per_second: f64,
    /// Token-bucket capacity: requests that may be issued back-to-back before the rate applies.
    pub burst: u32,
    /// Maximum in-flight requests to this source.
    pub max_concurrency: usize,
    /// Maximum attempts for a queue job before it is marked failed.
    pub max_retries: u32,
    /// Delay before the first queue-level retry; doubles each attempt (see [`SourcePolicy::backoff`]).
    pub base_backoff: Duration,
    /// Upper bound for the queue-level retry delay.
    pub max_backoff: Duration,
    /// Whether the source refuses requests without an API key.
    pub needs_api_key: bool,
}

impl SourcePolicy {
    /// The built-in policy for `source`.
    ///
    /// Rates for FRED (120/min), BLS (25/min), BEA (30/min) and Census (40/min) come from the
    /// legacy `enhanced_crawler_scheduler`; SEC uses 8/s (under SEC's 10/s fair-access limit,
    /// matching the SEC crawler default); every other source gets 1 req/s with concurrency 2.
    pub fn default_for(source: SourceId) -> Self {
        let base = SourcePolicy {
            requests_per_second: 1.0,
            burst: 1,
            max_concurrency: 2,
            max_retries: 3,
            base_backoff: Duration::from_secs(30),
            max_backoff: Duration::from_secs(30 * 60),
            needs_api_key: false,
        };
        match source {
            SourceId::Fred => SourcePolicy {
                requests_per_second: 120.0 / 60.0,
                burst: 4,
                max_concurrency: 4,
                needs_api_key: true,
                ..base
            },
            SourceId::Bls => SourcePolicy {
                requests_per_second: 25.0 / 60.0,
                max_concurrency: 1,
                ..base
            },
            SourceId::Bea => SourcePolicy {
                requests_per_second: 30.0 / 60.0,
                max_concurrency: 1,
                needs_api_key: true,
                ..base
            },
            SourceId::Census => SourcePolicy {
                requests_per_second: 40.0 / 60.0,
                ..base
            },
            SourceId::Sec => SourcePolicy {
                requests_per_second: 8.0,
                burst: 8,
                max_concurrency: 4,
                ..base
            },
            _ => base,
        }
    }

    /// Queue-level retry delay after `attempt` failed attempts (0-based):
    /// `base_backoff * 2^attempt`, capped at `max_backoff`. No jitter.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt);
        self.base_backoff
            .checked_mul(factor)
            .map_or(self.max_backoff, |d| d.min(self.max_backoff))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn per_minute(p: &SourcePolicy) -> f64 {
        (p.requests_per_second * 60.0).round()
    }

    #[test]
    fn legacy_scheduler_rates() {
        assert_eq!(
            per_minute(&SourcePolicy::default_for(SourceId::Fred)),
            120.0
        );
        assert_eq!(per_minute(&SourcePolicy::default_for(SourceId::Bls)), 25.0);
        assert_eq!(per_minute(&SourcePolicy::default_for(SourceId::Bea)), 30.0);
        assert_eq!(
            per_minute(&SourcePolicy::default_for(SourceId::Census)),
            40.0
        );
    }

    #[test]
    fn sec_is_eight_per_second() {
        let p = SourcePolicy::default_for(SourceId::Sec);
        assert_eq!(p.requests_per_second, 8.0);
        assert!(!p.needs_api_key);
    }

    #[test]
    fn other_sources_get_conservative_default() {
        for id in [
            SourceId::WorldBank,
            SourceId::Imf,
            SourceId::Ecb,
            SourceId::Fhfa,
        ] {
            let p = SourcePolicy::default_for(id);
            assert_eq!(p.requests_per_second, 1.0);
            assert_eq!(p.max_concurrency, 2);
            assert!(!p.needs_api_key);
        }
    }

    #[test]
    fn all_defaults_are_valid() {
        for id in SourceId::ALL {
            let p = SourcePolicy::default_for(id);
            assert!(p.requests_per_second > 0.0, "{id}");
            assert!(p.burst >= 1, "{id}");
            assert!(p.max_concurrency >= 1, "{id}");
            assert!(p.base_backoff <= p.max_backoff, "{id}");
        }
    }

    #[test]
    fn api_key_requirements() {
        assert!(SourcePolicy::default_for(SourceId::Fred).needs_api_key);
        assert!(SourcePolicy::default_for(SourceId::Bea).needs_api_key);
    }

    #[test]
    fn backoff_doubles_and_caps() {
        let p = SourcePolicy::default_for(SourceId::Fred);
        assert_eq!(p.backoff(0), p.base_backoff);
        assert_eq!(p.backoff(1), p.base_backoff * 2);
        assert_eq!(p.backoff(2), p.base_backoff * 4);
        assert_eq!(p.backoff(30), p.max_backoff);
        assert_eq!(p.backoff(u32::MAX), p.max_backoff);
    }
}
