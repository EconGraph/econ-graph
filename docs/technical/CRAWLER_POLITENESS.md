# Crawler Politeness

Every outbound crawler request goes through one `HttpFetcher`
(`backend/crates/econ-graph-crawler/src/http.rs`). It applies, per source:

- **Rate limit**: a token bucket (`governor`) at `requests_per_second` with `burst`; callers wait
  for a permit rather than dropping requests.
- **Concurrency limit**: at most `max_concurrency` in-flight requests.
- **Timeout**: 30 s per request (`CRAWLER_HTTP_TIMEOUT_SECS` on the worker).
- **User-Agent**: `EconGraph/<version> (+https://econgraph.com; support@econgraph.com)`. download.bls.gov refuses a User-Agent that links to github.com. The SEC crawler sends `EconGraph support@econgraph.com` by default.
- **In-process retries**: up to 2 extra attempts for rate-limit and transient errors, jittered
  exponential backoff, honouring `Retry-After`. After that the queue reschedules the job
  (see [CRAWLER_DEPLOYMENT_GUIDE.md](./CRAWLER_DEPLOYMENT_GUIDE.md#retries)).
- **Secrets**: `api_key`, `registrationkey` and `UserID` query parameters are redacted in logs and errors.

## Per-source policy

Defaults from `SourcePolicy::default_for` (`src/policy.rs`); an adapter may override its own.

| Source | Rate | Burst | Concurrency | API key |
|---|---|---|---|---|
| FRED | 120/min | 4 | 4 | required |
| BLS | 25/min | 1 | 1 | optional (higher limits) |
| BEA | 30/min | 1 | 1 | required |
| Census | 40/min | 1 | 2 | required |
| SEC | 8/s (SEC allows 10/s) | 8 | 4 | none |
| all others | 1/s | 1 | 2 | none |

Queue retries: `max_retries` 3, backoff 30 s doubling up to 30 min.

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
- Throughput across sources: `CRAWLER_CONCURRENCY`. Per-source limits still cap each source.
- A source that keeps returning 429 / auth errors is paused for `CRAWLER_PAUSE_SECS` after
  `CRAWLER_PAUSE_AFTER` consecutive failures.
- Split sources across workers with `CRAWLER_SOURCES` only if they are separate deployments;
  rate limits are per process.
