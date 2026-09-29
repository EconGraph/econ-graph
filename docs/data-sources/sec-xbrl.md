# SEC EDGAR XBRL

Crawler: `backend/crates/econ-graph-sec-crawler/` (not a `SourceAdapter` in the main
crawler). Fixture: `test_data/sec_mock/submissions_CIK0009999901.json` (real shape,
invented company). No key; SEC requires a descriptive `User-Agent`. Background and the
full plan: [SEC EDGAR XBRL implementation plan](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md).

Unlike the other sources, SEC data **stays in Postgres** under the federation plan
(`companies`, `financial_statements`, `financial_line_items`, the `xbrl_*` taxonomy
tables). This file is here to show how its shape differs, not to propose an Iceberg table.

## What we fetch today

- **Submissions**: `GET https://data.sec.gov/submissions/CIK##########.json` for a company:
  company profile (name, SIC, tickers) and recent filings, stored in `companies`.
- **Filings**: for each new filing that passes the form and date filters, the XBRL instance
  from `https://www.sec.gov/Archives/edgar/data/{cik}/{accession without dashes}/`
  (`{stem}_htm.xml` for inline XBRL, `{accession}.xbrl` otherwise). The raw file is
  compressed and stored, with a `financial_statements` row per filing.
- **Parsing**: not part of the crawl. `parse_and_store_xbrl` exists and can parse an
  instance through Arelle, but nothing in the crate calls it, and it only logs fact and
  statement counts rather than storing them; `financial_line_items` is not filled.
- **Not used**: the `companyfacts` and `frames` JSON APIs, which already give parsed facts.

## Sample

Submissions (fixture shape, invented values), trimmed. Filings are column-wise arrays:

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

Company facts (**from public docs**, not used today):
`GET https://data.sec.gov/api/xbrl/companyfacts/CIK##########.json`

```json
{"cik": 320193, "entityName": "Apple Inc.", "facts": {"us-gaap": {"Revenues": {
  "label": "Revenues",
  "units": {"USD": [
    {"start": "2023-10-01", "end": "2023-12-30", "val": 119575000000,
     "accn": "0000320193-24-000006", "fy": 2024, "fp": "Q1", "form": "10-Q",
     "filed": "2024-02-02", "frame": "CY2023Q4"}
  ]}
}}}}
```

## Frequency and revisions

Quarterly (10-Q) and annual (10-K) filings, plus amendments (10-K/A, 10-Q/A). A fact is
identified by concept, unit, period (`start`/`end`, or an instant) and dimensions (XBRL
contexts with segments such as business line or geography). The same fact is reported
again in later filings as a comparative period, sometimes restated: each filing (`accn`,
`filed`) is a vintage. In `companyfacts`, `frame` marks the one value SEC picked per
calendar period.

## Flags and footnotes

Per fact: `decimals` (precision), sign and balance (debit or credit) from the taxonomy,
context segments, and XBRL footnotes linked to facts. `financial_line_items` has columns
for most of these (`decimals`, `is_credit`, `segment_ref`, `context_ref`).

## Relation to the dataset model

If SEC facts were ever exposed as series (for example "Apple revenue, quarterly"), the
dataset shape would be:

| Role | Columns |
|---|---|
| Dimensions | `cik`, `taxonomy`, `concept`, `unit`, `period_type` (duration or instant), segment members |
| Measures | `value` |
| Attributes | `form`, `fiscal_year`, `fiscal_period`, `decimals`, `frame` |
| Vintage | `filed` (with `accn`) as `revision_date` |

That is the same vintage model as the time series tables, which keeps the option open
without moving the data out of Postgres now.
