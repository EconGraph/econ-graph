# SEC EDGAR XBRL

SEC ingestion stores company profiles and raw filing documents in Postgres. It uses
`companies` and `financial_statements`, rather than the economic series and observation
tables used by the other sources.

The [SEC crawler crate](../../backend/crates/econ-graph-sec-crawler/) supplies
`SecFilingHandler`, which the shared `crawler-worker` runs for `(SEC, fetch_filing)`
queue jobs. Each job identifies a company by CIK. This handler uses the worker's shared
HTTP fetcher; it is separate from the main crawler's `SourceAdapter` interface. No API
key is required. SEC requires a descriptive `User-Agent`.

## What the crawl stores

The crawler fetches `https://data.sec.gov/submissions/CIK##########.json`, where the CIK
has ten digits, including leading zeros. It updates the company's `companies` row with
selected profile fields, including its name, SIC and first ticker. It then uses the
`filings.recent` arrays to select filings. It does not fetch the additional historical
submissions files listed in the response's `filings.files` array.

Default queued crawls select exactly `10-K` and `10-Q`. Form filters match exact strings,
so amendments (`10-K/A` and `10-Q/A`) require a configuration that includes those forms.
The crawler also applies any configured filing-date range, requires XBRL availability
and checks file-size limits. It skips accessions already stored for the company.

For each remaining filing, the crawler downloads its XBRL instance from
`https://www.sec.gov/Archives/edgar/data/{cik}/{accession without dashes}/`, using the
company's CIK without leading zeros. For inline XBRL with a `.htm` primary document,
it requests `{primary document stem}_htm.xml`; otherwise it requests `{accession}.xbrl`.
The raw document is compressed and stored with a `financial_statements` row for the
filing, including its accession, filing date and reporting period.

The crawl does not extract facts into `financial_line_items`. The optional
`xbrl-parser` feature, disabled by default, provides a native Rust parser and the
`sec-crawler parse` CLI subcommand. Under that feature, the crawler's
`parse_and_store_xbrl` helper retrieves and parses a stored instance, but only logs
statement and fact counts; it does not insert the parsed line items. The crawl does
not call this helper.

## Samples

Submissions ([fixture](../../backend/crates/econ-graph-sec-crawler/test_data/sec_mock/submissions_CIK0009999901.json)
shape, invented values), trimmed. Filings are column-wise arrays:

```json
{
  "cik": "9999901",
  "name": "Testco Holdings Inc.",
  "sic": "3571",
  "tickers": ["TSTC"],
  "fiscalYearEnd": "1231",
  "filings": {"recent": {
    "accessionNumber": ["0000950170-24-000001", "0009999901-24-000002"],
    "filingDate": ["2024-02-15", "2024-05-02"],
    "reportDate": ["2023-12-31", "2024-03-31"],
    "form": ["10-K", "10-Q"],
    "isInlineXBRL": [1, 1],
    "primaryDocument": ["tstc-20231231.htm", "tstc-20240331.htm"]
  }}
}
```

Company facts, trimmed from a [live public response](https://data.sec.gov/api/xbrl/companyfacts/CIK0000320193.json).
The crawler does not use this endpoint. The concept, accession and values below come
from a single record; that record has no `frame` field.

```json
{"cik": 320193, "entityName": "Apple Inc.", "facts": {"us-gaap": {
  "RevenueFromContractWithCustomerExcludingAssessedTax": {
    "label": "Revenue from Contract with Customer, Excluding Assessed Tax",
    "units": {"USD": [
      {"start": "2023-10-01", "end": "2023-12-30", "val": 119575000000,
       "accn": "0000320193-24-000006", "fy": 2024, "fp": "Q1", "form": "10-Q",
       "filed": "2024-02-02"}
    ]}
  }
}}}
```

SEC's [XBRL JSON APIs](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
aggregate standard-taxonomy facts that apply to the whole entity. They do not cover
all company extensions and segment facts available in full filing instances. The
crawler uses neither `companyfacts` nor `frames`.

## Reporting periods and revisions

Annual and quarterly filings contain facts for durations or instants. A fact's meaning
depends on its taxonomy concept, unit, period and context, including dimensions such
as business line or geography. Later filings can report the same period again as a
comparative, sometimes with restated values. The accession and filing date identify
the filing vintage, independently of the reporting period. The raw filing storage
retains separate accessions without turning these facts into observation rows.

In `companyfacts`, some records carry a calendar-frame label such as `CY2023Q4`.
It is distinct from the fiscal year and fiscal period: the sample is fiscal Q1 2024,
although its dates fall near calendar Q4 2023. The
[frames endpoint](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
selects, for each entity, the last-filed fact that most closely fits the requested
calendar period. Reporting dates can differ between entities in the same frame;
a frame does not imply identical fiscal calendars.

## Fact attributes and footnotes

In source XBRL, a numeric fact supplies its signed value and may carry a `decimals`
attribute describing rounding accuracy. Its context supplies the reporting period
and any segment or scenario dimensions. A taxonomy concept may declare a debit or
credit balance; that declaration is distinct from the sign of a reported value.
XBRL footnotes can link explanatory text to facts.

The crawl retains the raw instance containing these attributes and links. It does
not populate the fact schema: `financial_line_items` has columns for `value`,
`decimals`, `precision`, `context_ref`, `segment_ref`, `scenario_ref`, `is_credit`
and `is_debit`, but no structured footnote field. Those columns describe storage
capacity, rather than attributes extracted by the crawl. The optional parser also
leaves context segments unset and skips structural footnote content.
