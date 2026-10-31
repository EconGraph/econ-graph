// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! **Licence gate**: sources the crawler does not download.
//!
//! A source is gated when its terms restrict our use and the maintainer has not decided to
//! download it. FRED and WTO have restrictive terms but are downloaded and stored, with their
//! catalogs hidden through Keycloak catalog roles, so they are not gated. IMF is gated because
//! the decision is not to download it; SNB, BOJ and UN_STATS have no adapter and no decision.
//!
//! Each entry of [`LICENCE_GATES`] names a source, the restriction in its terms, the Linear
//! ticket holding the full review, and the `crawl_<source>` build flag
//! (`config/flags/flags.flagd.json`) that unlocks it. The flags are off in every profile, so a
//! gated source is never crawled unless a build turns its flag on. While a gated source is
//! disabled:
//!
//! - `crawler-worker` builds its registry with [`ungated_registry`], so no adapter of the source
//!   exists for its startup reference-data refresh, dispatch, scheduler or coverage;
//! - the [worker](crate::worker) never claims its jobs (see [`claim_exclusions`]; the claim
//!   compares names case-insensitively); jobs already in the queue stay there untouched, neither
//!   failed nor deleted, and the worker logs why once. A job claimed some other way is handed
//!   back unfetched, without spending a retry;
//! - the [scheduler](crate::scheduler) neither refreshes nor re-discovers it;
//! - `crawler enqueue`, `discover`, `fetch`, `refetch` and `record-reference-seeds`, and GraphQL
//!   `triggerCrawl`, refuse it with the [`LicenceRefusal`] message;
//! - `crawler check-selection` refuses it when named, leaves it out of its default list, and
//!   reports it as skipped without a request if asked to check it any other way;
//! - `crawler sources` lists it as `disabled: licence (<ticket>)`.
//!
//! The terms register is the Linear document "Data source terms of use" in the Data sources
//! project. A source's flag is turned on only once its licence question is settled there.

use std::fmt;
use std::sync::Once;
use std::time::Duration;

use econ_graph_core::models::{CrawlQueueItem, QueueTransition};
use econ_graph_core::DatabasePool;

use crate::adapter::AdapterRegistry;
use crate::error::CrawlError;
use crate::source::SourceId;
use crate::worker::JobOutcome;

/// One licence-gated source.
#[derive(Debug)]
pub struct LicenceGate {
    /// The gated source.
    pub source: SourceId,
    /// The build flag that unlocks crawling it (`cfg(flag_<flag>)` in this crate).
    pub flag: &'static str,
    /// The Linear issue holding the licence review, e.g. `ECO-175`.
    pub ticket: &'static str,
    /// What the source's terms restrict, in one sentence.
    pub restriction: &'static str,
    /// Whether this build was compiled with [`flag`](Self::flag) on.
    compiled_in: bool,
}

/// Every licence-gated source. Sources not listed here are not gated.
pub const LICENCE_GATES: &[LicenceGate] = &[
    // IMF terms prohibit bulk download by automated technology without explicit permission, and
    // commercial reuse of IMF data needs permission: https://linear.app/econgraph/issue/ECO-175
    LicenceGate {
        source: SourceId::Imf,
        flag: "crawl_imf",
        ticket: "ECO-175",
        restriction: "the IMF prohibits bulk download by automated technology without explicit \
                      permission, and commercial reuse of IMF data needs permission",
        compiled_in: cfg!(flag_crawl_imf),
    },
    // Bank of Japan terms require the Bank's prior permission to reproduce its data for
    // commercial purposes: https://linear.app/econgraph/issue/ECO-743
    LicenceGate {
        source: SourceId::Boj,
        flag: "crawl_boj",
        ticket: "ECO-743",
        restriction: "the Bank of Japan requires its prior permission to reproduce its data for \
                      commercial purposes",
        compiled_in: cfg!(flag_crawl_boj),
    },
    // SNB terms permit use of its data for non-commercial purposes only:
    // https://linear.app/econgraph/issue/ECO-742
    LicenceGate {
        source: SourceId::Snb,
        flag: "crawl_snb",
        ticket: "ECO-742",
        restriction: "the Swiss National Bank permits use of its data for non-commercial \
                      purposes only",
        compiled_in: cfg!(flag_crawl_snb),
    },
    // No terms are stated for the UN SDG Global Database, and the generic UN terms allow
    // personal, non-commercial use only: https://linear.app/econgraph/issue/ECO-182
    LicenceGate {
        source: SourceId::UnStats,
        flag: "crawl_un_stats",
        ticket: "ECO-182",
        restriction: "the UN SDG Global Database states no terms of its own, and the generic UN \
                      terms allow personal, non-commercial use only",
        compiled_in: cfg!(flag_crawl_un_stats),
    },
];

impl LicenceGate {
    /// The Linear URL of [`ticket`](Self::ticket).
    pub fn ticket_url(&self) -> String {
        format!("https://linear.app/econgraph/issue/{}", self.ticket)
    }

    /// Whether this build may crawl the source: its flag was compiled in.
    pub fn is_enabled(&self) -> bool {
        self.compiled_in || testing::unlocked(self.source)
    }

    /// The short label `crawler sources` shows: `disabled: licence (ECO-175)`.
    pub fn label(&self) -> String {
        format!("disabled: licence ({})", self.ticket)
    }
}

/// The gate entry for `source`, whether or not this build enables it.
pub fn gate(source: SourceId) -> Option<&'static LicenceGate> {
    LICENCE_GATES.iter().find(|g| g.source == source)
}

/// The gate keeping this build from crawling `source`, if any.
pub fn blocking(source: SourceId) -> Option<&'static LicenceGate> {
    gate(source).filter(|g| !g.is_enabled())
}

/// Whether this build may crawl `source`.
pub fn allows(source: SourceId) -> bool {
    blocking(source).is_none()
}

/// `Ok` when this build may crawl `source`, else the refusal naming the restriction and ticket.
pub fn check(source: SourceId) -> Result<(), LicenceRefusal> {
    match blocking(source) {
        Some(gate) => Err(LicenceRefusal(gate)),
        None => Ok(()),
    }
}

/// Sources this build may not crawl, in [`LICENCE_GATES`] order.
pub fn disabled_sources() -> Vec<SourceId> {
    LICENCE_GATES
        .iter()
        .filter(|g| !g.is_enabled())
        .map(|g| g.source)
        .collect()
}

/// `crawl_queue.source` values the worker must not claim. The first call in a process logs, once
/// per disabled source, that its queued jobs are skipped and why.
pub fn claim_exclusions() -> Vec<String> {
    static LOGGED: Once = Once::new();
    let disabled = disabled_sources();
    LOGGED.call_once(|| {
        for source in &disabled {
            if let Some(gate) = gate(*source) {
                tracing::warn!(
                    %source,
                    ticket = gate.ticket_url(),
                    "{source} jobs are not claimed; queued ones stay in the queue: {}",
                    LicenceRefusal(gate)
                );
            }
        }
    });
    disabled.iter().map(|s| s.as_str().to_string()).collect()
}

/// `registry` without the adapters of sources this build may not crawl. The worker builds its
/// registry with this, so its startup reference-data refresh, scheduler, coverage and job
/// dispatch never reach a gated source's upstream.
pub fn ungated_registry(registry: AdapterRegistry) -> AdapterRegistry {
    let mut out = AdapterRegistry::new();
    for source in registry.ids().into_iter().filter(|&s| allows(s)) {
        if let Some(adapter) = registry.get(source) {
            out.register(adapter);
        }
    }
    out
}

/// How long a job handed back by `hand_back` waits before it is due again.
pub const HAND_BACK_DELAY: Duration = Duration::from_secs(24 * 60 * 60);

/// Returns a claimed job of a gated source to the queue unfetched: `retrying`, due again after
/// [`HAND_BACK_DELAY`], with the refusal as its error and no retry spent. For rows the claim
/// filter misses; the worker never fetches them.
pub(crate) async fn hand_back(
    pool: &DatabasePool,
    item: &CrawlQueueItem,
    gate: &'static LicenceGate,
) -> JobOutcome {
    let refusal = LicenceRefusal(gate).to_string();
    tracing::warn!(id = %item.id, source = %item.source, "job handed back unfetched: {refusal}");
    match CrawlQueueItem::retry_later(pool, item, &refusal, HAND_BACK_DELAY, false).await {
        Ok(QueueTransition::Rescheduled { at }) => JobOutcome::Retrying {
            error: CrawlError::Permanent(refusal),
            at,
        },
        Ok(QueueTransition::Failed) => JobOutcome::Failed { error: refusal },
        Ok(QueueTransition::LostLease) => JobOutcome::LeaseLost {
            error: Some(refusal),
        },
        Err(e) => {
            tracing::warn!(id = %item.id, error = %e, "handing back a licence-gated job failed");
            JobOutcome::LeaseLost {
                error: Some(refusal),
            }
        }
    }
}

/// A gated source was asked for while its licence flag is off.
#[derive(Debug, Clone, Copy)]
pub struct LicenceRefusal(pub &'static LicenceGate);

impl fmt::Display for LicenceRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let g = self.0;
        write!(
            f,
            "{} is {}: {}. Crawling it needs the `{}` build flag, which is turned on only once \
             the licence question is settled ({})",
            g.source,
            g.label(),
            g.restriction,
            g.flag,
            g.ticket_url()
        )
    }
}

impl std::error::Error for LicenceRefusal {}

/// Test-only unlocks, so the enabled path can be exercised without rebuilding with a flag on.
/// Thread-local: each test (and its single-threaded tokio runtime) sees only its own unlocks.
pub(crate) mod testing {
    use crate::source::SourceId;

    #[cfg(test)]
    thread_local! {
        static UNLOCKED: std::cell::RefCell<Vec<SourceId>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// Whether a test unlocked `source` on this thread. Always false outside unit tests.
    pub(crate) fn unlocked(source: SourceId) -> bool {
        #[cfg(test)]
        {
            UNLOCKED.with(|u| u.borrow().contains(&source))
        }
        #[cfg(not(test))]
        {
            let _ = source;
            false
        }
    }

    /// Unlocks `source` on this thread until the guard drops.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn unlock(source: SourceId) -> Unlock {
        UNLOCKED.with(|u| u.borrow_mut().push(source));
        Unlock(source)
    }

    #[cfg(test)]
    pub(crate) struct Unlock(SourceId);

    #[cfg(test)]
    impl Drop for Unlock {
        fn drop(&mut self) {
            UNLOCKED.with(|u| u.borrow_mut().retain(|s| *s != self.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sources the terms register rules out (needs permission, or unclear) and that the
    /// maintainer has not decided to crawl anyway.
    const GATED: [SourceId; 4] = [
        SourceId::Imf,
        SourceId::Boj,
        SourceId::Snb,
        SourceId::UnStats,
    ];

    #[test]
    fn exactly_the_ruled_out_sources_are_gated() {
        let gated: Vec<SourceId> = LICENCE_GATES.iter().map(|g| g.source).collect();
        assert_eq!(gated, GATED);
        for source in SourceId::ALL {
            assert_eq!(gate(source).is_some(), GATED.contains(&source), "{source}");
        }
    }

    #[test]
    fn ungated_sources_are_always_allowed() {
        for source in SourceId::ALL.into_iter().filter(|s| !GATED.contains(s)) {
            assert!(allows(source), "{source}");
            assert!(check(source).is_ok(), "{source}");
            assert!(blocking(source).is_none(), "{source}");
        }
    }

    #[test]
    fn each_gate_follows_its_own_build_flag() {
        let compiled = [
            (SourceId::Imf, "crawl_imf", cfg!(flag_crawl_imf)),
            (SourceId::Boj, "crawl_boj", cfg!(flag_crawl_boj)),
            (SourceId::Snb, "crawl_snb", cfg!(flag_crawl_snb)),
            (
                SourceId::UnStats,
                "crawl_un_stats",
                cfg!(flag_crawl_un_stats),
            ),
        ];
        for (source, flag, on) in compiled {
            let g = gate(source).unwrap();
            assert_eq!(g.flag, flag);
            assert_eq!(g.compiled_in, on, "{source}");
            assert_eq!(allows(source), on, "{source}");
        }
    }

    /// The flags are off in both the release and dev profiles (config/flags), so a test build
    /// refuses every gated source.
    #[test]
    fn gated_sources_are_refused_with_the_restriction_and_ticket() {
        let off: Vec<SourceId> = LICENCE_GATES
            .iter()
            .filter(|g| !g.compiled_in)
            .map(|g| g.source)
            .collect();
        for source in off.iter().copied() {
            let g = gate(source).unwrap();
            assert!(!allows(source), "{source}");
            let msg = check(source).unwrap_err().to_string();
            assert!(
                msg.contains(&format!("disabled: licence ({})", g.ticket)),
                "{msg}"
            );
            assert!(msg.contains(g.restriction), "{msg}");
            assert!(msg.contains(&g.ticket_url()), "{msg}");
            assert!(msg.contains(g.flag), "{msg}");
            assert!(
                g.ticket_url()
                    .starts_with("https://linear.app/econgraph/issue/ECO-"),
                "{msg}"
            );
        }
        assert_eq!(disabled_sources(), off);
        let excluded: Vec<String> = off.iter().map(|s| s.as_str().to_string()).collect();
        assert_eq!(claim_exclusions(), excluded);
    }

    #[test]
    fn a_test_unlock_enables_one_source_until_dropped() {
        {
            let _imf = testing::unlock(SourceId::Imf);
            assert!(allows(SourceId::Imf));
            assert!(check(SourceId::Imf).is_ok());
            assert!(!allows(SourceId::Boj));
            assert!(!disabled_sources().contains(&SourceId::Imf));
            assert!(!claim_exclusions().contains(&"IMF".to_string()));
        }
        assert!(!allows(SourceId::Imf));
    }
}

#[cfg(test)]
mod path_tests;
