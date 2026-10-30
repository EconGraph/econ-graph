# Rate Limit Sources and Verification

The main crawler's request rates are application policies. Provider quotas may also limit
requests per day, data volume, or traffic shared by an API account. This reference separates
those constraints from the rate metadata stored in source rows.

## Enforced crawler policies

The worker builds its HTTP client from each registered adapter's policy in
[`build_http`](../../backend/crates/econ-graph-crawler/src/cli.rs).
[`SourcePolicy::default_for`](../../backend/crates/econ-graph-crawler/src/policy.rs) supplies
the defaults; adapters can override them.
[`HttpFetcher`](../../backend/crates/econ-graph-crawler/src/http.rs) enforces a token bucket
and concurrency limit for each source within one process. Separate worker processes do not
share these limits.

The [production registry](../../backend/crates/econ-graph-crawler/src/sources/mod.rs) has seven
live adapters. Their effective policies are:

| Source | Sustained request rate | Burst | Maximum concurrent requests |
| --- | --- | --- | --- |
| FRED | 120 requests/minute | 4 | 4 |
| BLS | 25 requests/minute | 1 | 1 |
| Census | 40 requests/minute | 1 | 2 |
| BEA | 30 requests/minute | 1 | 1 |
| World Bank | 1 request/second | 1 | 2 |
| FHFA | 1 request/second | 1 | 2 |
| WTO | 1 request/second | 1 | 1 |

The FRED, BLS, Census, and BEA rates were inherited from the legacy scheduler, as recorded in
`policy.rs`. World Bank and FHFA use the general default. WTO follows the provider's published limit
(below). The BLS, World Bank, and FHFA
adapter overrides change batching without changing these request rates. These values govern
HTTP attempts, including retries; they do not count series or enforce daily or data-volume
quotas. A sustained rate and burst capacity do not guarantee compliance with a provider's
quota window or account-wide limits.

See [Per-source policy](./CRAWLER_POLITENESS.md#per-source-policy) for the other policy fields
and [Tuning](./CRAWLER_POLITENESS.md#tuning) for changing a policy and controlling worker
throughput. SEC EDGAR uses the separate
[SEC crawler crate](../../backend/crates/econ-graph-sec-crawler/README.md); the main crawler's
`SourceId::Sec` default is not evidence of that crawler's enforced rate.

## Provider evidence for live sources

The following evidence comes from public provider documentation, not live API tests or
provider approval of EconGraph's settings. Quotas apply to the services described by each
provider; an API quota should not be assumed to cover separate file-download services.

### FRED

The [FRED API errors reference](https://fred.stlouisfed.org/docs/api/fred/errors.html) allows
up to 120 requests/minute before returning HTTP 429 and warns that ignoring throttling can
cause a temporary block. That matches the crawler's sustained rate, but the cited page does
not specify whether the quota is aggregated by key, IP address, or another identity.

### BLS

The [BLS API FAQ](https://www.bls.gov/developers/api_faqs.htm) specifies 50 requests per
10 seconds for both registered and unregistered access. It separately limits registered
users to 500 queries/day and unregistered users to 25 queries/day. Registration also changes
the series-per-query limit from 25 to 50 and the years-per-query limit from 10 to 20.

The crawler's 25 requests/minute policy does not enforce either daily quota. The
[BLS adapter](../../backend/crates/econ-graph-crawler/src/sources/bls.rs) adjusts batching
according to whether `BLS_API_KEY` is set. Its discovery also reads files from
`download.bls.gov`; the API FAQ does not establish a quota for those downloads.

### Census

The [Census API query limits](https://www.census.gov/data/developers/guidance/api-user-guide.Query_Limits.html)
require an API key for all data queries and allow up to 50 variables per query. That page does
not state a numerical request-rate or daily quota. It therefore does not establish that the
crawler's 40 requests/minute policy is a provider-approved rate.

### BEA

The [BEA API user guide](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf),
under **API Calling Limits (rate limiting)**, specifies three limits for each API user
account: 100 requests/minute, 100 MB retrieved/minute, and 30 errors/minute. Exceeding any
limit triggers a timeout, described as one minute but subject to adjustment. Throttling
responses return HTTP 429 with a `Retry-After` header.

The crawler's 30 requests/minute policy is below the published request-rate limit. It does
not enforce the separate volume and error quotas or aggregate other clients using the same
BEA account.

### World Bank

The [World Bank API call structures guide](https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures)
describes API requests but does not state a numerical request-rate quota. The crawler's
1 request/second is an application default, not a quota established by that guide.

### WTO

The WTO Timeseries API's only subscription product on the
[WTO API portal](https://apiportal.wto.org/products), "Standard" (free), allows 10 calls/second
and at most 10,000 calls/hour per key, and 1 call/second for the time-series data endpoints
(`/data` and `/data_count`). The crawler holds every WTO request to 1 request/second with no
burst and one request at a time, which also keeps the hourly total at 3,600. These limits are
per key and the crawler enforces its rate per worker process, so each deployment needs its own
key. The
[WTO adapter](../../backend/crates/econ-graph-crawler/src/sources/wto.rs) makes two requests per
indicator for a discovery or a fetch batch, so a full crawl of its curated indicators takes a
few dozen requests. The portal answers an exhausted quota with HTTP 403; the adapter treats a
403 that mentions the quota as rate limiting rather than an authentication failure, and retries
after the replenishment time the message gives (an hour when it gives none).

### FHFA

FHFA publishes the [HPI master data as downloadable files](https://www.fhfa.gov/data/hpi/datasets).
The [FHFA adapter](../../backend/crates/econ-graph-crawler/src/sources/fhfa.rs) reads the master
CSV once per discovery and once per fetch batch. Every in-scope series shares a batch key,
and the batch capacity normally covers the whole catalog, so fetching all series normally
needs one file request. Reference-data refresh reads the file separately to obtain place
labels; discovery and fetch counts are not a bound on all requests in a refresh. Conditional
requests can avoid transferring an unchanged file, but still consume HTTP requests.

The dataset page does not state a numerical download quota. The crawler's 1 request/second
policy is an application default; the source-row API URL and rate metadata do not control
these file requests.

## Source-row metadata and catalog entries

[`data_source.rs`](../../backend/crates/econ-graph-core/src/models/data_source.rs) defines
`rate_limit_per_minute` metadata defaults: FRED 120, BLS and Census 500, WTO 60, and BEA,
FHFA, World Bank, IMF, ECB, OECD, Bank of England, Bank of Japan, Reserve Bank of Australia,
Bank of Canada, Swiss National Bank, UN Statistics Division, and ILO 1000. These defaults
are neither the main crawler's enforced HTTP policies nor evidence of provider quotas;
existing database rows can also differ from model defaults.

The main crawler provides development-only
[static catalogs](../../backend/crates/econ-graph-crawler/src/sources/static_catalogs.rs) for
ECB, OECD, Bank of England, Bank of Japan, Reserve Bank of Australia, Bank of Canada,
Swiss National Bank, UN Statistics Division, and ILO when `flag_static_catalogs` is enabled.
They do not make live provider requests. IMF has no adapter in the registry. A model row or
a default `SourcePolicy` for one of these sources does not imply HTTP traffic to that provider.
See the [data source reference](../data-sources/README.md) for source-specific documentation.

## Handling throttling

The HTTP fetcher makes up to two extra attempts for retryable failures, using exponential
backoff with jitter when no `Retry-After` is supplied. It waits in process for `Retry-After`
values of at most five seconds; longer values return the error immediately so the queue can
reschedule the job. The [worker](../../backend/crates/econ-graph-crawler/src/worker.rs)
reschedules rate-limited jobs using `Retry-After` when available, otherwise the source's queue
backoff, without consuming a failed attempt. Repeated rate-limit or authentication failures
can pause a source; see [Tuning](./CRAWLER_POLITENESS.md#tuning) and
[Retries](./CRAWLER_DEPLOYMENT_GUIDE.md#retries).

A 429 response is evidence of throttling for that request. Its absence does not establish a
provider's quota or justify raising throughput. Check the applicable provider terms and the
combined use of shared credentials before changing a policy.
