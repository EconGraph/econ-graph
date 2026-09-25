// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

//! The single HTTP client every adapter uses.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::CrawlError;
use crate::policy::SourcePolicy;
use crate::rate_limit::SourceRateLimiter;
use crate::source::SourceId;

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

/// Rate-limited, retrying HTTP client shared by all adapters. Cheap to clone.
///
/// Every request method:
/// - waits on the source's rate limiter and concurrency semaphore ([`SourceRateLimiter::acquire`]);
/// - retries [retryable](CrawlError::is_retryable) failures in-process up to 2 extra times with
///   jittered exponential backoff, honouring `Retry-After`;
/// - maps non-success statuses with [`CrawlError::from_status`] and body decode failures to
///   [`CrawlError::Parse`];
/// - records metrics, and redacts `api_key`, `registrationkey` and `UserID` query parameters from
///   anything it logs or puts in an error.
#[derive(Clone, Debug)]
pub struct HttpFetcher {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    #[allow(dead_code)] // used by the request methods (A3)
    client: reqwest::Client,
    #[allow(dead_code)] // used by the request methods (A3)
    config: HttpConfig,
    /// Complete: contains an entry for every `SourceId`.
    policies: HashMap<SourceId, SourcePolicy>,
    #[allow(dead_code)] // used by the request methods (A3)
    limiter: SourceRateLimiter,
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
        let _ = (source, url, query);
        todo!("A3")
    }

    /// GET `url` with `query` appended and deserialize the JSON body.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        source: SourceId,
        url: &str,
        query: &[(&str, &str)],
    ) -> Result<T, CrawlError> {
        let _ = (source, url, query);
        todo!("A3")
    }

    /// POST `body` as JSON to `url` and deserialize the JSON response.
    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        source: SourceId,
        url: &str,
        body: &B,
    ) -> Result<T, CrawlError> {
        let _ = (source, url, body);
        todo!("A3")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
