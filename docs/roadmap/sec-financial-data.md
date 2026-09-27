# Roadmap: SEC financial data (companies, statements and ratios)

Status: agreed with Joe (2026-09-26). Replaces
[SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md).
Covers the financial demo page that [release train 1](./releases.md) removes from the
build and the unrouted `frontend/src/components/financial` components. The link to
`federation.md` resolves once PR #178 merges.

## Goal

Let a user find a company that reports to the SEC, chart its reported fundamentals over
time (revenue, net income, assets, cash flow), read its income statement, balance sheet
and cash flow statement for any fiscal year (and any quarter for companies that file
quarterly), and compare a few ratios with peers in the same industry. Every number comes
from an SEC filing, and the page shows which filing and on what date. Users can attach
notes to any statement line or reported fact and share them, the base for collaborative
analysis later.

The old plan aimed much higher: a three-layer GraphQL API, real-time collaborative
annotation with assignments and approvals, an education system with badges, 25 or more
ratios including valuation multiples, DCF and scenario modeling, and its own XBRL
processing through Arelle. It also marks most of that as complete. Almost none of it
exists (see below). This roadmap keeps the goal of collaborative financial analysis,
but builds it in order: real numbers first, notes next, collaboration later (see
[Later](#later-collaborative-analysis) and [Not planned](#not-planned)).

## Where things stand today (main at `c92bdaf`)

The live path is: fetch the submissions list, store each filing's raw XBRL file, stop.
Nothing is parsed, nothing is computed, and nothing is served.

| Area | State |
|---|---|
| Crawl | `SecFilingHandler` runs in `crawler-worker` for `(SEC, fetch_filing)` jobs (`econ-graph-crawler-worker/src/main.rs:224-227`). The scheduler never creates these jobs (`econ-graph-crawler/src/scheduler.rs:437-439`). They come only from the `sec-crawler enqueue` and `sec-company-crawler --enqueue` CLIs. Rate limit and User-Agent go through the shared `HttpFetcher` (8 requests per second, `econ-graph-crawler/src/policy.rs:80-84`) |
| What is fetched | The submissions JSON, and one XBRL instance file per 10-K and 10-Q. Only `filings.recent` is read (`src/submissions.rs:33-39`, `src/crawler.rs:236`), so older filings are never seen. SEC's `companyfacts` and `frames` APIs are not used; `build_company_facts_url` (`src/utils.rs:106`) has no caller |
| Storage | `companies` upserted by CIK. One `financial_statements` row per filing holding the zstd-compressed instance in a `bytea` column (`src/storage.rs:37-39,260`). `filing_type` is hard-coded to `"10-K"`, even for 10-Qs (`src/storage.rs:250`). Files over 100 MB compressed get a placeholder large-object id `12345` and their content is dropped (`src/storage.rs:95,160-165`) |
| XBRL parsing | Not on the live path. `parse_and_store_xbrl` (`src/crawler.rs:815`) has no caller and stores nothing. The default parser shells out to a Python Arelle script and fills in a random company id, accession `"unknown"` and today's date as the period end (`src/xbrl_parser.rs:532-570`). The native parser drops dimensions, hard-codes the unit to USD and maps facts to no statement (`src/xbrl_parser.rs:1004-1016,1864-1865,1921-1930`). Inline XBRL handling ignores `scale` and `sign` |
| Taxonomy tables | `xbrl_taxonomy_schemas`, `xbrl_taxonomy_linkbases`, `xbrl_taxonomy_concepts`, `xbrl_dts_dependencies`, `xbrl_instance_dts_references` and `xbrl_processing_logs` exist. DTS discovery never matches a real `link:schemaRef` (`src/crawler.rs:554-566`), so they stay empty |
| Line items and ratios | `financial_line_items` and `financial_ratios` are never written. `financial_ratio_calculator.rs` implements five ratios (ROE, ROA, ROIC with a hard-coded 25% tax rate, current ratio, debt to equity); 23 more return `None`. Its only callers are its own tests. `econ-graph-core/src/models/financial_ratios.rs` has a second calculator, also test-only |
| Concept mapping | `config/concept_mappings.json` has 34 US GAAP and 15 IFRS mappings, loaded only by `config_loader.rs`, whose only user is the test-only ratio calculator. Some targets don't exist (`us-gaap:EBITDA`, `us-gaap:FreeCashFlow`), shares outstanding are `dei:` concepts, and the ASC 606 revenue concept is missing |
| API | None. Nothing in `econ-graph-graphql`, `econ-graph-mcp`, `econ-graph-services` or `econ-graph-backend` touches companies, statements, ratios or financial annotations |
| Annotations | `financial_annotations`, `annotation_replies`, `annotation_assignments` and `annotation_templates` exist, with foreign keys into `financial_statements` and `financial_line_items`. Nothing reads or writes them |
| Frontend | About 6,100 lines in `components/financial` plus `pages/FinancialComponentsDemo.tsx` (excluding tests and stories), none routed. The components import their GraphQL documents from `test-utils/mocks/graphql/*` and query operations the backend doesn't serve. `TrendAnalysisChart` and `PeerComparisonChart` render "Coming Soon" where the chart should be |
| Tests | CI runs `cargo test -p econ-graph-sec-crawler --lib` (`.github/workflows/ci-core.yml:477`). The handler tests against a mock EDGAR are solid. `tests/xbrl_financial_integration_tests.rs` runs only in a step allowed to fail and asserts things like `is_ok() \|\| is_err()` |
| Fake data | `load_sp500_ciks` (`src/crawler.rs:748-759`) hard-codes 10 CIKs; 7 carry the wrong company name (one of them repeats Microsoft's CIK) |

What is worth keeping: the `crawl_queue` wiring and its error mapping, the SEC fetch
policy, the submissions decoder, the `companies` upsert and the handler tests.

## Decisions

### 1. Where the numbers come from

The old plan parses every filing's XBRL itself. That is the hardest part of the whole
feature, and SEC already does it: EDGAR publishes the facts from every XBRL filing as
JSON.

| Option | For | Against |
|---|---|---|
| A. Parse filings ourselves (fix the native parser, or run Arelle) (later: phases 7 to 9) | Every fact, including dimensional facts (segments, geographic splits) and company-specific concepts. Keeps the as-filed statement layout from the presentation linkbase | Inline XBRL, scale and sign, contexts, dimensions and calculation linkbases are months of work before one correct number reaches a user. Arelle means Python in the crawler image. Raw files have to be stored somewhere |
| **B. SEC's `companyfacts` data, bulk file nightly plus the per-company API for updates (first: phases 1 to 6)** | Already parsed by SEC (facts deduplicated within each filing; the `frame` field marks one value per calendar period). One nightly bulk file (`companyfacts.zip`) covers every XBRL filer, so full coverage costs one download, not 10,000 companies' worth of filings. Every fact carries the accession number, form and date filed of the filing that reported it, which gives real vintages. The `frames` API gives the same concept for every company in one calendar period, which is the peer comparison | Only facts without dimensions, and only standard taxonomies (US GAAP, IFRS, DEI, SRT), so no segment breakdowns and no company-specific extension concepts. No statement layout: statements are built from a standard concept list, not as filed |
| C. SEC's Financial Statement data sets (`sub`, `num`, `tag`, `pre` files) | Includes the presentation (`pre`) file, so statements can be shown as filed, line order and all | Quarterly (monthly for the larger Financial Statement and Notes sets), so new filings arrive late. Large tab-separated files to load each period |

**Decided (Joe, 2026-09-26): B first, then A.** SEC's JSON gets the basic features
(company page, statements from standard lines, ratios, peers) working for every filer.
Our own parser comes later, in its own phases, for the richer data the JSON leaves out:
segment and geographic breakdowns, company-specific concepts, and statements laid out as
filed. C stays a fallback for the as-filed layout if the parser is slow to arrive.

Both paths write the same `company_facts` table (decision 2), so the page and the API
don't care which one supplied a value. Where both have a fact, the parser's value must
match SEC's, which makes the JSON a free test oracle for the parser.

**The JSON stays after the parser lands (Joe, 2026-09-26).** Even with our own parser
and segments, the nightly `companyfacts` crawl keeps running as an independent second
source and a quality check. Each night a reconciliation compares every fact both sources
have for the same filing (same concept, unit, period and no dimensions). A mismatch or a
fact only one side has is listed by a check, like train 1's coverage check, and a jump
in mismatches raises an alert. A mismatch usually means a parser bug (scale, sign, a
context read wrong), and occasionally an SEC processing quirk worth knowing about. The
page shows the parser's value only when it agrees with the JSON's or the JSON has no
such fact. Otherwise the JSON's value is shown, until the mismatch is resolved.

**How far back the JSON goes.** It covers every XBRL filing, and XBRL is where history
starts. SEC phased XBRL in by filer size: companies with a public float over $5 billion
from fiscal periods ending after 2009-06-15, other large accelerated filers from
2010-06-15, everyone else (including smaller reporting companies) from 2011-06-15, and
foreign private issuers on IFRS only from fiscal periods ending on or after 2017-12-15
(SEC did not accept the IFRS taxonomy until 2017, even though the 2009 phase-in rule
nominally placed them in the 2011-12-15 group). The first XBRL 10-K also tagged its
comparative columns (three years of income statement for large filers, two for smaller
ones), so the earliest values reach roughly 2007 to 2010 depending on the company. IFRS
filers' history starts later, around 2015 to 2016, once SEC accepted the IFRS taxonomy.
Before that, SEC's APIs expose no machine-readable financial data: the 2005-2009 voluntary XBRL
program's exhibits are not in `companyfacts`, and everything else is HTML and text.
Earlier history would need a commercial source or text extraction from old filings,
which is not planned. Unlike the submissions API, which the crawler reads today and
which lists only the most recent filings inline, `companyfacts` holds each company's
full XBRL history in one document.

The details above come from SEC's published documentation for these APIs and the XBRL
phase-in rule, not from live responses: the cloud environment's network policy blocks
`data.sec.gov` and `www.sec.gov`. Phase 1 starts by downloading the bulk file and
recording its real size, fact count, field list and earliest period per company in a
new `docs/data-sources/sec-edgar.md`.

Consequences:

- The native parser, the DTS manager and the DTS download in `crawler.rs` stay in the
  tree, compiled out behind a cargo feature (decision 9), until the richer-data phases
  pick them up. The six `xbrl_*`
  taxonomy tables stay for the same reason.
- The Arelle path is deleted. It is known bad: it fills in a random company id, the
  accession `"unknown"` and today's date as the period end.
- Raw filing storage is not needed for the JSON path. The parser phases need the raw
  files and store them in object storage, not Postgres. Until then, the existing
  `bytea` columns stay, except the large-object branch, which is known bad (it records a
  placeholder id and drops the file) and is deleted.

### 2. Data model

SEC and XBRL data stays in Postgres (federation decision, [federation.md](./federation.md)),
on the data side of the plane split.

- `companies`: keep. Drop what submissions doesn't provide (`industry`, `sector`,
  `entity_size` are never filled). Add the full ticker list and the exchanges.
- `filings` (replaces `financial_statements`): accession number, company, form, filed
  date, report period, fiscal year and period of the filing, amendment flag. Raw files
  move to object storage (a key and a hash) in phase 7.
- `company_facts`: one row per fact per filing that reported it: company, taxonomy,
  concept, unit, period start and end, value, accession number, form, filed date, and the
  filing's `filing_fy` and `filing_fp`. Those two describe the filing, not the fact: a
  prior-year comparative in the FY2023 10-K carries fy=2023, fp=FY. The fact's own fiscal
  period is derived in phase 2 from its dates and the company's fiscal year end. A value
  restated in a later 10-K is a second row with a later filed date. That is the same
  revision model as the time series: `filed` plays the part of `revision_date`, and
  "latest known value" and `asOf` work the same way. Two more columns are there from the
  start so the parser phases need no reshaping: `dimensions` (axis and member pairs, empty
  for the JSON's facts) and `source` (SEC's JSON or our parser).
- Phase 1 only adds tables. The old ones, with their `econ-graph-core` models and
  `schema.rs` entries, are dropped by the phase that retires the code
  still compiled against them (decision 9): `financial_ratios` and `company_comparisons`
  in phase 5 (ratios on request), the financial annotation tables in phase 6, and
  `financial_statements`, `financial_line_items` (which has a foreign key into
  `financial_statements`) and its `bytea` files together in phase 7 (object storage; the
  parser in phase 8 writes `company_facts` instead of `financial_line_items`).
  `xbrl_processing_logs`, also foreign-keyed to `financial_statements`, is retargeted to
  `filings` in the same phase-7 migration, since it stays (decision 9) as the parser's log
  table.

**Size is the open risk.** Every fact is repeated in each later filing that reports it
again (a balance sheet figure shows up in the next year's 10-K as the comparative
column), so the row count is several times the number of distinct facts. It is likely
tens of millions of rows for all filers, and it could be more. That is fine for Postgres
with an index on `(company, concept, period end, filed)`, but it has to be measured on the
real bulk file in phase 1.

| Option | For | Against |
|---|---|---|
| **A. `company_facts` in Postgres (decided, federation)** | Matches the federation decision. Queries are per company and small. Joins with `companies` and `filings` stay plain SQL | If the row count turns out to be in the hundreds of millions, this is the "observations in Postgres" problem federation exists to avoid |
| B. A `sec_company_facts` dataset in Iceberg (dimensions company, concept, unit; measure value), per the federation dataset model | The same storage as every other time series. A company's revenue becomes a series that the series page and `crossSection` handle without new code | Iceberg doesn't exist until train 7. Revisits a decision Joe already made |

A stands, per the federation decision. Revisit only if phase 1's measurement comes out
far above the estimate.
Either way, model the facts so that a company plus a concept reads like a series (dated
values with vintages), because that is what the chart and the CSV export already
understand.

### 3. Standard concepts

A statement or a ratio needs "revenue", but filers tag it at least four different ways
(`Revenues`, `RevenueFromContractWithCustomerExcludingAssessedTax`, `SalesRevenueNet`
before 2018, and IFRS `Revenue`). The standard concept list maps each standard line to
the ordered list of tags that can supply it, plus the statement and position it appears
in.

- It is reference data, so it lives in a shared data file loaded at runtime, not in code
  (project rule for reference data).
- It replaces `config/concept_mappings.json`, fixing the wrong targets listed above.
- It starts with about 40 lines: the main lines of the income statement, balance sheet and
  cash flow statement, plus shares outstanding.
- Each value shown says which tag it came from, so a wrong mapping is visible.

### 4. Ratios

Compute ratios when asked, from the standard concepts, and store nothing. The old plan's
stored `financial_ratios` rows go stale on every restatement, and a ratio is one division.

Start with ratios that need only the filings: gross, operating and net margin, ROE, ROA,
current ratio, debt to equity, and year-over-year growth of revenue, net income and
operating cash flow. Anything with a market price in it (P/E, EV/EBITDA, FCF yield) needs
a price source that EconGraph doesn't have, so it waits. The hard-coded 25% tax rate in
ROIC goes; the effective rate comes from the filing.

Peer comparison uses the `frames` shape: one concept for every company for one calendar
period (`CY2023`, `CY2023Q4`, `CY2023Q4I` for instants), filtered by SIC code from
`companies`, from which the percentile is computed on request. Peers with different
fiscal year ends are compared on calendar periods. No stored benchmarks: the
`config/*.json` files and `config_loader.rs` stay compiled out with the old calculator
(decision 9) and are removed in phase 5.

### 5. The API

Plain GraphQL, returning plain arrays like the rest of the site.

- `companies(search)`: by name, ticker or CIK, reusing the pg_trgm search from #165.
- `company(cik)` or `company(ticker)`: header data and the filing list.
- `companyFacts(cik, concepts, period: ANNUAL | QUARTERLY, asOf)`: dated values per
  concept with the accession number and filed date, shaped like `seriesData` so the
  existing chart and CSV code can take it.
- `financialStatement(cik, kind, fiscalYear, fiscalPeriod)`: the standard lines for one
  period, with the previous period beside it.
- `companyRatios(cik, ratios, period)` and `peerRatios(cik, ratio, period)`.

The operations go into the committed `schema.graphql` (release train 1, item 18), so the
frontend can't drift from the backend again the way the financial components did. No
subscriptions.

### 6. Annotations

**Decided (Joe, 2026-09-26): notes on details stay.** The platform is heading towards
collaborative analysis, so a user must be able to attach a note to a single statement
line or a single reported fact, not only to a chart, even though the collaboration
features around those notes come later.

**Deferred (Joe, 2026-09-26): whether notes share one annotation system with series
charts or get their own.** It is decided when phase 6 starts. The analysis workspace
roadmap (#191) proposes one `annotations` table with an anchor kind, which is the
leading candidate. Either way, these requirements hold:

- A note can be anchored to a company chart, a statement line (CIK, statement, standard
  line and period), a single fact (CIK, concept, unit, period and dimensions, optionally
  pinned to the accession number of the filing that reported it, so a note on a restated
  value says which version it was about), or a filing (CIK and accession number).
- Anchors use natural keys, never data-side row ids, so notes survive a data reload and
  the plane split.
- The existing `financial_annotations`, `annotation_replies`, `annotation_assignments`
  and `annotation_templates` are not built on. They have foreign keys into
  `financial_statements` and `financial_line_items`, which the app side can't reference
  after the plane split and which this roadmap replaces (decision 2). No annotation data
  exists, so they are replaced by a migration, not converted.

Assignments, templates and review workflows belong to the collaborative analysis work
(see [Later](#later-collaborative-analysis)).

### 7. The frontend

The `components/financial` components were written against a mock API that will not
exist in this form. Their props, queries and types all assume the old plan's schema, and
they import their GraphQL documents from `test-utils/mocks`.

**Decided (Joe, 2026-09-26): compile out rather than delete, unless known bad.** So:

- In train 1, `FinancialComponentsDemo.tsx` and `components/financial` are compiled out
  of the build behind a build-time flag that is off in release and development builds
  (see the feature flags roadmap, #195), not deleted. The flag's removal is tied to
  phase 4. Their unit tests and enabled stories keep running, so the code keeps
  compiling. The mock GraphQL documents they import (`test-utils/mocks/graphql/financial-queries`
  and `ratio-queries`) stay until phase 4 for the same reason.
- The company page (phase 4) is built on the series page's chart, transformations and
  CSV export, and pulls in the pieces worth keeping as it goes: most likely
  `FinancialStatementViewer`'s table layout and `AnnotationPanel` for line notes. Each
  piece is switched from the mock documents to the real API when it is used.
- A component is deleted only in the PR that replaces it, or when it is found to be
  known bad. The repo-root `PERFORMANCE_TODO.md`, which is about these components, stays
  until then.

The company page has three parts: a header with the company's filings, a chart of chosen
concepts over time (annual or quarterly), and statement tables with a period picker, with
notes on lines and facts (decision 6). Ratios and peers come as a fourth part in phase 5.

### 8. Which companies

Every company in `companyfacts` that files 10-K, 10-Q, 20-F or 40-F reports, which
includes foreign private issuers reporting under IFRS. With the bulk file (decision 1)
that costs the same as the S&P 500, and a curated list would have to be maintained.
The hard-coded `load_sp500_ciks` list is deleted: it is known bad (7 of its 10
entries carry the wrong company name, and one of them repeats Microsoft's CIK).

### 9. What is compiled out and what is deleted

Joe's rule (2026-09-26): compile out, and delete only what is known bad.

| Code | Treatment | Why |
|---|---|---|
| Native XBRL parser, DTS manager, DTS discovery and download in `crawler.rs` (`download_dts_components`, `discover_dts_references`, taken off `crawl_company`), `parse_and_store_xbrl`, `xbrl_*` taxonomy tables | Compiled out behind a cargo feature (`xbrl-parser`, off by default). CI builds and tests the crate with the feature on, so it doesn't rot | The parser phases build on it (decision 1) |
| `financial_ratio_calculator.rs`, `config_loader.rs` and `config/*.json`, and the core `FinancialRatioCalculator` | Compiled out behind the same feature, and a matching feature on `econ-graph-core` for the core copy, until phase 5 picks one and removes the rest | Five working ratios, but two copies and a hard-coded 25% tax rate |
| `tests/xbrl_financial_integration_tests.rs` | Behind the same feature | Weak assertions, fixed when the parser phases start |
| Arelle path in `xbrl_parser.rs` (and `use_arelle`, `arelle_path`, `python_env` in its config), `scripts/*.py` | Deleted | Known bad: made-up company id, accession and period end |
| Large-object branch in `storage.rs` | Deleted | Known bad: stores a placeholder id and drops the file |
| `load_sp500_ciks`, the stubs that return empty results as if they had worked (`get_crawl_progress`, `get_companies_by_sic`, `get_recent_filings`, `crawl_recent_filings`), and their callers `crawl_sp500_companies` and `crawl_companies_by_industry` | Deleted | Known bad: they report success with nothing done |
| `components/financial`, `FinancialComponentsDemo.tsx` | Compiled out behind a build-time flag (decision 7) | Parts are reused in phase 4 |

## Phases

Each phase is its own PR, or a short stack. Phases 1 to 6 run on SEC's JSON; phases 7 to
9 add our own parser for the richer data.

0. **Cleanup.** Decision 9: the cargo feature, and deleting what is known bad. No
   user-visible change. Can merge any time.
   Release 1 (train 1) carries it as three PRs: SEC-1, delete the known-bad code (#206,
   in review); SEC-2, the `xbrl-parser` cargo feature (in progress); SEC-3, the
   `financial_components` build flag (starts after the flags PR #215).
1. **Ingest.** Download `companyfacts.zip` and `submissions.zip` nightly as a scheduled
   crawler job, reading every page in `submissions.zip`, including the `-submissions-NNN`
   overflow files, so the full filing list is there. Load `companies`, `filings` (keyed by
   accession number) and `company_facts` (keyed by accession, taxonomy, concept, unit,
   period and dimensions) idempotently. Use the per-company API only to catch up on the
   day's filings. Document the source in `docs/data-sources/sec-edgar.md` with measured
   sizes and the earliest period per company. The worker's existing SEC policy, error
   mapping and tests carry over.
2. **Standard concepts and periods.** The shared concept file and the Rust code that picks
   a value for each standard line. Quarterly values that filings don't report directly are
   derived and marked as derived: the fourth quarter (a 10-K reports only the full year,
   so for income and cash flow lines Q4 is the year less the first three quarters; a
   balance sheet's Q4 is the year-end value), single quarters of cash flow (10-Q cash flow
   statements are year to date), and trailing twelve months. Each fact's own fiscal year
   and period are derived from its dates and the company's fiscal year end, not from the
   reporting filing's `filing_fy` and `filing_fp`. Tests on a few real companies' facts
   saved as fixtures.
3. **API.** Decision 5, without ratios. `schema.graphql` updated.
4. **Company page.** Search results include companies, and the company page from decision
   7 with CSV download. Routed only once the Playwright suite covers it (release train 1's
   "nothing broken" bar).
5. **Ratios and peers.** Decision 4, on the page and in the API.
6. **Notes and MCP.** Notes on company charts, statement lines, facts and filings
   (decision 6). MCP tools for company facts and statements, after MCP OAuth (auth phase
   6).
7. **Raw filings.** Fetch each filing's XBRL instance (the crawler already does) and store
   it in object storage instead of `bytea`, for every filing in the full list from phase
   1.
8. **Parser.** Turn the `xbrl-parser` feature on: inline XBRL with scale, sign and
   formats, contexts with dimensions, units from the filing. Facts land in `company_facts`
   with `source = parser` and their dimensions, replacing `financial_line_items` (dropped
   with `financial_statements` in phase 7). The nightly reconciliation against the
   JSON (decision 1) starts here and never stops.
9. **Richer pages.** Segment and geographic breakdowns, company-specific concepts, and
   statements laid out as filed, from each filing's presentation linkbase.

A price source for market cap and valuation ratios is deferred (see
[Scope calls](#scope-calls-for-fully-useful)).

## Release trains

**Decided (Joe, 2026-09-26): company pages get a train of their own, train 4 (`v4.3.0`),
and it ships only when the page is fully useful.** Phase 0 lands in train 1, where it
compiles the financial code out of the build, which train 1's "nothing fake" check needs
anyway. Phase 1 can merge any time, because it changes nothing a user sees, so the data is
ready when the page is. Train 4 runs after train 2 (its notes need train 1's sign-in and
train 2's sharing, and its MCP tools need train 2's MCP OAuth) and can run in parallel
with train 3.

### What "fully useful" means

A company page that only charts a few numbers is a demo. The train carries phases 1 to
6 and leaves when someone who would otherwise use a free fundamentals site could use
this one instead:

1. **Find any company.** Every company in scope (decision 8), by ticker, name or CIK, from
   the site's search.
2. **The company at a glance.** Name, tickers and exchanges, SIC industry, fiscal year
   end, and the filing list with links to each document on EDGAR.
3. **All three statements, every period.** Income statement, balance sheet and cash flow,
   annual, quarterly and trailing twelve months, several periods side by side, from the
   first XBRL filing to the latest. No missing fourth quarters (phase 2). Quarterly and
   trailing twelve months apply to 10-Q filers; 20-F and 40-F filers report annually.
4. **Per-share data.** Reported EPS, shares outstanding and weighted average shares.
5. **Any line as a chart,** over time, with CSV download, using the series page's chart.
6. **Ratios and peers.** Phase 5's ratios, and the company's percentile among its SIC
   peers.
7. **Restatements visible.** Where a number was restated, the page shows the value as
   first reported and as restated, with both filings.
8. **Fresh.** A new 10-K, 10-Q, 20-F or 40-F (or an amendment) shows up within a day
   of filing.
9. **Notes** on lines, facts and filings for signed-in users (phase 6).

Exit criteria, in the same spirit as train 1's:

- CI green; the Playwright suite opens 20 fixed companies across sectors and exercises
  every control on the page.
- For every company with a balance sheet, total assets equal total liabilities plus
  temporary (redeemable) equity plus total equity including noncontrolling interest,
  within rounding, in at least 99% of periods. Totals derived from their components count
  when a filer doesn't tag them; a reported `LiabilitiesAndStockholdersEquity` alone
  doesn't, because it matches assets by construction. The remaining periods are listed by
  a check, like train 1's coverage check.
- For the 20 fixed companies, revenue, net income, total assets and operating cash flow
  for three years match the filings, checked by hand once and then kept as fixtures.
- The nightly load has run for seven days in a row with no manual step, and a failed load
  raises an alert.

### Scope calls for "fully useful"

**Market prices: noted, deferred (Joe, 2026-09-26).** Most company pages lead with the
share price, market cap and P/E. SEC has no prices, so train 4 ships without market cap,
valuation ratios or a price chart, and the page says the numbers are fundamentals from
filings. Adding prices later means choosing an end-of-day source whose terms allow
showing prices publicly. Free feeds usually forbid redistribution, so it likely costs
money, and the terms have to be read before anything is built. Prices are not part of
"fully useful" for train 4.

**Segments: train 5 (`v4.4.0`), right after train 4 (Joe, 2026-09-26).** Revenue and
profit by business segment and by region are what make the platform powerful: they are the
detail analysts dig for, and where notes on details matter most. SEC's JSON has no
dimensional facts, so segments need our own parser (phases 7 to 9). They are not part of
train 4 so the largest and least certain piece doesn't hold the company page. They get a
firm slot, not "someday":

- the parser phases are train 5, with segments as its headline;
- `company_facts` carries a `dimensions` column from phase 1 (decision 2), so segment
  facts need no reshaping;
- phase 7 (raw filings in object storage, full filing history) changes nothing a user
  sees, so it can start while train 4 is still in progress.

## Later: collaborative analysis

Joe's direction (2026-09-26) is that the platform moves towards collaborative analysis.
Phase 6 gives it the base: notes anchored on series, company charts, lines and facts,
with comments and sharing. What the old plan sketched on top of that belongs to that
later work, for series and companies alike, rather than being SEC-only:

- mentions, assignments, reviews and approvals on notes;
- note templates;
- presence and live cursors;
- saved multi-company comparisons shared with a team (with the analysis workspace
  roadmap, since saved charts live there).

## Not planned

- Arelle (decision 9).
- A three-layer GraphQL API with raw XBRL access, and subscriptions.
- The education system: learning paths, exercises, badges.
- Valuation multiples, DCF, scenario and forecast modeling, until there is a price source.
- Financial alerts, a separate mobile view, and a separate export component. The company
  page uses the site's responsive layout and CSV export.
- 8-K and other non-periodic filings. Only 10-K, 10-Q, 20-F, 40-F and their amendments
  (decision 8).
- Financial data from before XBRL (roughly 2007 to 2010 depending on the company,
  decision 1).

## Open items

- Network access: phase 1 needs `data.sec.gov` and `www.sec.gov` allowed in the cloud
  environment to measure the bulk file and test the adapter against real data.
- The notes design (one annotation system or two) is decided when phase 6 starts
  (decision 6).
