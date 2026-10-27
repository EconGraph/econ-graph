# Crawler Politeness

Every outbound crawler request goes through the shared
[`HttpFetcher`](../../backend/crates/econ-graph-crawler/src/http.rs). It tracks rate and
concurrency limits per source and applies the following policies:

- **Rate limit**: a token-bucket-style GCRA limiter at `requests_per_second` with
  `burst`; callers wait for a permit rather than dropping requests.
- **Concurrency limit**: at most `max_concurrency` in-flight requests.
- **Timeout**: ordinary requests default to 30 s (`CRAWLER_HTTP_TIMEOUT_SECS` on the worker).
  Reference-file downloads use the greater of 120 s and the configured timeout.
- **User-Agent**: the worker sends
  `EconGraph/<version> (+https://econgraph.com; support@econgraph.com)`, including for SEC jobs.
  download.bls.gov refuses a User-Agent that links to github.com. The standalone SEC crawler
  defaults to `EconGraph support@econgraph.com`.
- **In-process retries**: the fetcher makes up to two extra attempts for rate-limit and
  transient errors. It uses exponential backoff with jitter and honors `Retry-After`.
  When `Retry-After` exceeds 5 s, the fetcher returns the error immediately so the queue can
  defer the job.
  Reference-file downloads make no extra attempts, avoiding repeated long downloads within
  one discovery job. Errors returned to the worker follow the queue retry rules below.
- **Secrets**: `api_key`, `registrationkey` and `UserID` query parameters are redacted in logs and errors.

## Per-source policy

Defaults come from
[`SourcePolicy::default_for`](../../backend/crates/econ-graph-crawler/src/policy.rs);
an adapter may override its source's policy.

| Source | Rate | Burst | Concurrency | API key |
|---|---|---|---|---|
| FRED | 120/min | 4 | 4 | required |
| BLS | 25/min | 1 | 1 | optional (higher limits) |
| BEA | 30/min | 1 | 1 | required |
| Census | 40/min | 1 | 2 | required |
| SEC | 8/s (SEC allows 10/s) | 8 | 4 | none |
| all others | 1/s | 1 | 2 | none |

Queue jobs default to `max_retries` 3: a job fails when its counted failures reach three.
Transient failures count toward that limit; rate-limit deferrals do not. Queue backoff starts
at 30 s and doubles up to 30 min. A supplied `Retry-After` sets the rate-limit deferral instead.
See [Retries](./CRAWLER_DEPLOYMENT_GUIDE.md#retries) for the queue's error classification.

## Request patterns

- **FRED**: each series fetch whose `last_updated` changed is one `/series` request, one
  `/series/vintagedates` request, and one paged `/series/observations` request per real-time
  window of at most 1,000 vintage dates (half FRED's documented 2,000). Most series are a
  single window; a daily series with its full revision history takes a few (DFF about six).
  The windows run one after another under the FRED rate limit. A window FRED still rejects
  for too many vintage dates is split in half and retried with a warning logged; no other
  HTTP 400 is retried. See [FRED](../data-sources/fred.md#vintage-date-limit).

## Tuning

- Change a default in `SourcePolicy::default_for`, or override `SourceAdapter::policy` in the
  source's adapter. Rebuild and redeploy the worker.
- `CRAWLER_CONCURRENCY` controls concurrent jobs across sources. Per-source limits still cap
  each source.
- A source that keeps returning 429 / auth errors is paused for `CRAWLER_PAUSE_SECS` after
  `CRAWLER_PAUSE_AFTER` consecutive failures.
- To split sources across workers, use separate deployments with nonoverlapping
  `CRAWLER_SOURCES` filters. Rate limits are per process: replicas serving the same source each
  enforce the full limit independently, multiplying the possible traffic to that source.
