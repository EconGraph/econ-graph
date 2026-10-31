# EconGraph SEC Crawler

This crate retrieves SEC EDGAR company submissions and stores the XBRL instance documents
for matching filings in PostgreSQL. It updates the company record and skips filings already
stored for that company. The crawl reads the submissions document's recent filings; it does
not follow the separate submission-history files.

## Crawling through the worker

`SecFilingHandler` processes `(SEC, fetch_filing)` jobs in `crawler-worker`. Each job identifies
a company by CIK and, by default, selects 10-K and 10-Q filings. Requests use the worker's shared
`HttpFetcher`, which applies SEC rate limits, concurrency limits, retries, and metrics.

From `backend/`, enqueue a company with:

```bash
cargo run -p econ-graph-sec-crawler --bin sec-crawler -- enqueue --cik 0000320193
```

The CLI requires `DATA_DATABASE_URL` pointing to a data database with migrations applied.
Enqueuing writes a job; a running worker performs the crawl. See the
[Crawler Deployment Guide](../../../docs/technical/CRAWLER_DEPLOYMENT_GUIDE.md)
for database setup, worker configuration, and queue operation, and
[Crawler Politeness](../../../docs/technical/CRAWLER_POLITENESS.md) for HTTP policies.

## Optional XBRL processing

The default build, including the deployed `crawler-worker`, stores filing documents without
parsing financial facts or calculating ratios. The `xbrl-parser` Cargo feature is off by
default. It exposes the XBRL parser, DTS (Discoverable Taxonomy Set) download, financial ratio
calculator, their `config/*.json` loading, and the `sec-crawler parse` and `validate`
subcommands. It also enables `econ-graph-core/financial-ratios`.

With this feature enabled, the crawl attempts to download referenced taxonomy components
after storing each instance document; taxonomy download failures are logged without failing
the filing. Enabling the feature does not make the crawl parse facts or calculate ratios.
The separate `parse_and_store_xbrl` method parses a stored document and logs the result, but
does not persist parsed statements or facts. The `validate` command checks parsing and the
presence of facts; fact-level validation rules are not implemented.

## Testing

Default-feature tests cover utilities, submissions decoding, filing URLs and filters, and
handler behavior against a local `MockSource` HTTP server. Mock handler tests exercise HTTP
error mapping and rate limiting. Database-backed handler tests also check filing storage,
repeated crawls, and queue behavior against mocked EDGAR responses. They return early when
`DATABASE_URL` is unset, so a passing default run can omit those checks.

Feature-enabled tests add parser fixtures, ratio-calculator and JSON configuration checks,
and taxonomy storage tests. The `xbrl_financial_integration_tests` target inserts test records
to exercise database storage and DTS references; its parser integration check only verifies
initialization. These suites do not establish a live SEC crawl or end-to-end financial
analysis accuracy.

### Database prerequisites

Use a disposable PostgreSQL database for database-backed tests. Handler tests apply migrations
and delete SEC queue rows and the test company's records. Feature-enabled tests use
`econ_graph_core::test_utils::TestContainer` as a dependency: in this mode it connects to
`DATABASE_URL`, falling back to `postgres://localhost/econ_graph_test`; it does not start a
Docker container. Its cleanup drops and recreates the entire `public` schema and then applies
migrations. Set `DATABASE_URL` explicitly and run these suites without other tests or services
using that database. See [Database Testing](../../../docs/testing/DATABASE_TESTING.md)
for the shared helper and database setup.

### Running tests

Run these commands from `backend/`. The first command deliberately omits database-backed
handler coverage:

```bash
env -u DATABASE_URL cargo test -p econ-graph-sec-crawler
```

After setting `DATABASE_URL` to the disposable database described above, run the default
suite with database coverage, or enable all crate features:

```bash
cargo test -p econ-graph-sec-crawler -- --test-threads=1
cargo test -p econ-graph-sec-crawler --all-features -- --test-threads=1
```

For focused feature-enabled checks, use the module filters or the integration target:

```bash
cargo test -p econ-graph-sec-crawler --features xbrl-parser xbrl_parser -- --test-threads=1
cargo test -p econ-graph-sec-crawler --features xbrl-parser financial_ratio_calculator -- --test-threads=1
cargo test -p econ-graph-sec-crawler --features xbrl-parser --test xbrl_financial_integration_tests -- --test-threads=1
```

The storage-statistics test is ignored in ordinary runs. It requires `DATABASE_URL` and uses
a temporary table on one connection. Run it explicitly with:

```bash
cargo test -p econ-graph-sec-crawler --lib storage::tests::storage_stats_handle_empty_mixed_and_large_totals -- --ignored --exact
```

## License

This project is licensed under the Microsoft Reference Source License (MS-RSL).
See the [license](../../LICENSE) for complete terms and conditions.

## Nightly companyfacts (Train 5)

`sec-companyfacts-ingest` loads the SEC's official nightly
[companyfacts and submissions bulk archives](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
into dedicated company-fact and filing-metadata tables. It leaves the existing raw XBRL crawler
and company-page feature gate unchanged. Run data migrations before using it. The crawler-worker
image includes `/app/sec-companyfacts-ingest`; the optional manifest
`k8s/optional/sec-companyfacts.yaml` starts suspended and must be enabled together with its
freshness check only after the platform and ingest acceptance gates pass.

The command uses `DATA_DATABASE_URL` and SEC's shared HTTP rate policy and contact User-Agent.
It downloads sequentially to private temporary files, verifies both complete ZIPs and payload
CRCs before writing, and parses one company at a time through a bounded channel. Defaults
limit each ZIP to 16 GiB, each JSON member to 128 MiB, total decompressed bytes per archive
to 128 GiB and members to one million. These are capacity limits, not claims about current
SEC payload sizes. Allow 32 GiB staging space plus database growth; the compressed archives
are never buffered in RAM. The monitor rejects upstream Last-Modified timestamps older than
24 hours. SEC publishes at approximately 03:00 ET; the optional schedule runs at 09:00 UTC.

Each company's metadata, all available submissions-history chunks and facts commit in one
transaction. Repeated loads preserve IDs and accession-specific revisions. A changed value
under an already stored accession fails instead of silently replacing evidence. An interrupted
run can leave earlier companies committed, but never marks the global run successful. Rerun
both archives from the beginning; no byte-range resume or destructive replacement is needed.
The dedicated database connection releases its advisory lock even after process termination.

For operator-staged archives, run `sec-companyfacts-ingest --companyfacts-zip /path/companyfacts.zip
--submissions-zip /path/submissions.zip`. This explicit offline mode checks ZIP integrity but
cannot prove archive publication freshness, so it is for fixtures or separately verified staging.
No supplied archive is deleted. Network mode deletes its own temporary files on exit.
`--check-freshness` fails for missing runs, a latest staged/failed/interrupted run, a successful
load older than 24 hours, or upstream archive timestamps older than 24 hours; `--evidence` prints the last 30 recorded runs, with source hashes and
completion counts. Seven distinct nightly runs must be observed; repeated fixture loads do not
satisfy release acceptance.

`SecCompanyfactsLoadFailed` and `SecCompanyfactsStale` use existing kube-state-metrics and
Prometheus rules. They remain inactive while the optional CronJobs are absent/suspended.
As with the existing crawler rules, Prometheus visibility does not itself configure an external
notification receiver. Validate notification routing before relying on paging.

The ingestion database test is opt-in via `TRAIN5_INGEST_TEST_DATABASE_URL` and requires an
isolated disposable data database. It applies migrations, then truncates only the three
companyfacts tables in that database; never point it at production.
