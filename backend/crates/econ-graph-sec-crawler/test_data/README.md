# XBRL Test Data

This directory contains XBRL instances, taxonomy material, and mock submissions for SEC EDGAR crawler and parser tests. The inventory distinguishes synthetic data, real filing instances, and saved error responses; a filename extension alone does not identify the response body.

## Files

### [sample_10k.xml](sample_10k.xml) (6.3 KB)

- **Type**: Synthetic XBRL instance document
- **Company identifiers**: Apple Inc. (CIK: 0000320193)
- **Period**: 2023 Annual and Q4 2023
- **Purpose**: Controlled parser input and a mock filing response for handler tests
- **Content**: Balance sheet, income statement, and cash flow facts, with contexts and units

### [apple_2025_q3_10q.xml](apple_2025_q3_10q.xml) (760 KB)

- **Type**: Real SEC EDGAR XBRL instance
- **Company**: Apple Inc. (CIK: 0000320193)
- **Filing**: 10-Q for Q3 2025 (period ending June 28, 2025)
- **Accession number**: 000032019325000073
- **Purpose**: Parser checks against a quarterly filing instance with multiple contexts, dimensions, and financial facts

### [chevron_2025_q2_10q.xml](chevron_2025_q2_10q.xml) (1.8 MB)

- **Type**: Real SEC EDGAR XBRL instance
- **Company**: Chevron Corporation (CIK: 0000093410)
- **Filing**: 10-Q for Q2 2025 (period ending June 30, 2025)
- **Accession number**: 0000093410-25-000062
- **Purpose**: Parser checks against an oil and gas company's quarterly instance, including industry concept names

### [apple_2023_q4.xsd](apple_2023_q4.xsd) and [apple_2023_q4_htm.xml](apple_2023_q4_htm.xml)

Both files contain XML `Error` responses with code `NoSuchKey`, naming missing objects under `edgar/data/320193/000032019323000006/`. Neither is a usable XBRL instance or taxonomy schema. The crate's current tests do not read these saved response bodies; their presence does not establish error-handling coverage.

### [sec_mock/submissions_CIK0009999901.json](sec_mock/submissions_CIK0009999901.json)

This synthetic EDGAR submissions response describes Testco Holdings Inc. (CIK: 0009999901). Its recent filings contain a 10-K, a 10-Q, and an 8-K. Submissions tests decode and split those rows; crawler tests use them for date filtering and archive URL construction; handler tests serve them through a mock EDGAR endpoint.

### [taxonomies/](taxonomies/)

The directory contains several kinds of material:

| Files | Contents |
| --- | --- |
| [aapl-20250628.xsd](taxonomies/aapl-20250628.xsd), [cvx-20250630.xsd](taxonomies/cvx-20250630.xsd) | Small issuer schemas declaring selected Apple and Chevron concepts and importing local schema filenames |
| [aapl-20241228-real.xsd](taxonomies/aapl-20241228-real.xsd), [cvx-20240930-real.xsd](taxonomies/cvx-20240930-real.xsd) and their corresponding `_cal.xml`, `_def.xml`, `_lab.xml`, `_pre.xml` files | Earlier-period issuer schemas and calculation, definition, label, and presentation linkbases |
| [country-2025.xsd](taxonomies/country-2025.xsd), [dei-2024.xsd](taxonomies/dei-2024.xsd), [ecd-2024.xsd](taxonomies/ecd-2024.xsd) | SEC taxonomy schemas |
| [xbrl-instance-2003-12-31.xsd](taxonomies/xbrl-instance-2003-12-31.xsd), [xbrldi-2006.xsd](taxonomies/xbrldi-2006.xsd), [xl.xsd](taxonomies/xl.xsd), [xlink.xsd](taxonomies/xlink.xsd) | XBRL instance, dimensions, and linking schemas |
| [srt-2024.xsd](taxonomies/srt-2024.xsd), [us-gaap-2024.xsd](taxonomies/us-gaap-2024.xsd) | HTML `404 Not Found` response bodies, despite their `.xsd` extensions |

The parser fixture tests do not load this directory or configure it as a taxonomy search path. These files therefore do not establish complete offline taxonomy resolution or schema validation of the 2025 instances.

## Usage in Tests

The [parser tests](../src/xbrl_parser_tests.rs) require the `xbrl-parser` feature, which is off by default. From the repository root, run:

```bash
cd backend/crates/econ-graph-sec-crawler
cargo test --features xbrl-parser --lib xbrl_parser_tests -- --nocapture
```

The synthetic sample supports parsing, prefixed context and unit extraction, validation, and cache tests. Validation tests distinguish instances with and without facts and assert a warning that fact-level validation is unimplemented. Cache tests check retained facts and line items, content and path changes, and unreadable cache entries. The sample parsing test fails on a parser error, but returns without asserting if its file is missing; other sample-consuming tests read or parse the file with `unwrap` or `expect`.

The Apple and Chevron parsing tests assert nonempty facts, contexts, and units and check the issuer's CIK when parsing succeeds. The Chevron test also looks for an oil and gas concept-name match. Both tests return without failing if the instance is missing or parsing returns an error, so a passing run alone does not prove successful parsing. Use the test output to distinguish a successful parse from a logged skip. Document-type tests likewise check only files that exist. These checks do not establish complete financial statement mapping, taxonomy validation, or filing-size performance.

The same module names an optional `jpmorgan_2025_q2_10q.xml` for JPMorgan Chase & Co. (CIK: 0000019617), Q2 2025 ending June 30, 2025, accession 0000019617-25-000615. That file is not included, so its parsing test returns without exercising bank concepts. If the file is supplied, the test also accepts parsing errors. The module's taxonomy-cache performance test exercises generated concepts rather than large filing instances. Its validation-report serialization test checks valid empty reports and invalid reports with 1,000 errors and warnings, preserving every diagnostic and its order without a timing threshold.

The [submissions tests](../src/submissions.rs), [crawler tests](../src/crawler.rs), and [handler tests](../src/handler/tests.rs) use the mock submissions without the parser feature. From the same crate directory:

```bash
cargo test --lib submissions::tests
cargo test --lib crawler::tests
cargo test --lib handler::tests
```

Handler tests use local mock HTTP endpoints. Their database-backed cases require `DATABASE_URL` and return early when it is unset; a run without that variable does not verify persistence. They serve `sample_10k.xml` as mock filing bytes, which does not by itself verify parser behavior. See the [crate's test instructions](../README.md#running-tests) for other test targets.

## Data Sources

The real Apple and Chevron instances originate from SEC EDGAR and are retained for testing against filing data. The sample instance and mock submissions are synthetic inputs for controlled tests. The taxonomy directory includes issuer and standards schemas, linkbases, and saved error responses as described above; it is not a complete taxonomy bundle.
