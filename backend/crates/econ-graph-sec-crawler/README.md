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
