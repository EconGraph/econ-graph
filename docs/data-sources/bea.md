# BEA (Bureau of Economic Analysis)

The [adapter](../../backend/crates/econ-graph-crawler/src/sources/bea.rs) uses
[selection rules](../../backend/crates/econ-graph-crawler/data/catalog_selection/bea.toml).
The [BEA fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/bea/) cover
(`GetData` for six NIPA tables at each frequency, short extracts of BEA's NIPA and Regional
table-title lists, SAGDP2 line codes, areas and line 1, and an error). API key required
(`BEA_API_KEY`); without it every call is `Permanent` before
any request.

## What we fetch today

- **Ids** are BEA's own codes: `bea_nipa/{table}.{line_number}.{frequency}` (for example
  `bea_nipa/T10105.1.Q`, GDP quarterly) and `bea_regional/{table}.{line_code}.{geo_fips}`
  (for example `bea_regional/SAGDP2.1.06000`, California GDP).
- **Tables**: 25, one rule each: NIPA headline tables from sections 1 to 7 (GDP and prices,
  GDI, personal income and consumption, government, foreign transactions, saving and
  investment, compensation and corporate profits by industry, per capita series), Regional
  GDP by state (SAGDP2) and the state personal income summary (SAINC1). A full refresh is
  about 300 requests, about ten minutes at the 30-a-minute policy rate. A test checks the
  rules against BEA's published table lists
  (`tests/fixtures/bea/published_table_names.txt`).
- **Discovery**: rules select dataset, table family prefix, optional title text and frequencies
  from BEA's current `TableName` catalogs. NIPA `GetData` for the last three years lists lines;
  Regional `GetParameterValuesFiltered` lists `LineCode` and `GeoFips`, limited to the US,
  states and BEA regions. Table and expanded-series limits reject surprise growth before
  observation jobs are enqueued. A failed or empty required table fails discovery.
- **Table titles** come from `GetParameterValues(DatasetName, ParameterName=TableName)`.
  Immutable snapshots keyed by source, reference URL and body hash let workers reuse the
  exact current catalog after a conditional response, even when another worker refreshed it.
  Historical dimension labels remain additive: removing a table upstream never deletes its
  saved name. An unknown title after a failed refresh does not overwrite a stored good title.
- **Publication state** is reconciled only after complete successful discovery under unchanged
  selection rules. Three consecutive absences mark a retained series missing. A sudden loss
  above the disappearance threshold is held for operator review. A table that BEA still lists
  but the rules no longer select keeps its series' state. A fetch response that omits a
  requested line or area (including an empty response) is not catalog removal evidence. For
  a series the current catalog does not list outright, such as a line under a table the rules
  no longer select, six such responses in a row (ten refresh intervals apart) stop routine
  fetches, and it is then only probed occasionally. Histories remain public and
  badged; candidate successors need human confirmation and values are never merged. See the
  [catalog runbook](../monitoring/crawler-coverage-runbook.md#catalog-selection-and-retained-series).
- **Fetch metadata**: fetches read the worker's in-process table-title cache. An unknown
  title makes fetch omit metadata while still returning real points; persistence preserves
  previously stored metadata rather than replacing a good title with a bare table name.
- **Fetch**: `GetData` per NIPA table and frequency, or per Regional table and line with the
  requested areas comma-separated, batched up to 64 series. Always the whole history (`Year`
  `X` or `ALL`), since revisions reach back years. Values are scaled by `UNIT_MULT`;
  suppressed or unavailable cells (`(D)`, `(NA)`, ...) become points with no value.
- **Units**: `CL_UNIT`, or `METRIC_NAME` when `CL_UNIT` is `Level`. The stored `units` drops
  a `Thousands of` / `Millions of` / `Billions of` prefix that `UNIT_MULT` already applied
  (`Millions of current dollars` with `UNIT_MULT` 6 is stored as `Current dollars`), and
  `units_raw` keeps BEA's text. The adapter's structured scale comes from `UNIT_MULT`, which
  is treated as authoritative: since the values are multiplied by `10^UNIT_MULT`, they are in
  base units. A stored units text that disagrees with that scale raises `CrawlerUnitsMismatch`
  (see [Units](README.md#units)). Bare `dollars` are US dollars.
- **Errors**: BEA answers HTTP 200 with an error object. A `UserId` error is `Auth`, a rejected
  parameter (invalid, not valid, does not exist, missing, required) is `Permanent`, and
  anything else is `Transient`. The rate policy is 30 requests a minute.

## Sample

The [BEA API guide](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf#page=4)
documents the `BEAAPI.Results.Error` envelope. The
[invalid-UserID fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/bea/error_invalid_userid.json)
contains this `Results` excerpt:

```json
{"Error": {"APIErrorCode": "3", "APIErrorDescription": "The UserId provided in the request is not valid."}}
```

The [NIPA fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/bea/nipa_t10105_q.json)
illustrates the documented row shape (API guide, [Appendix B](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf#page=23));
its values are illustrative, not a live response recording. A full-history request is
`GetData&DataSetName=NIPA&TableName=T10105&Frequency=Q&Year=X`. The fixture contains
rows like:

```json
{"BEAAPI": {"Results": {
  "Data": [
    {"TableName": "T10105", "SeriesCode": "A191RC", "LineNumber": "1",
     "LineDescription": "Gross domestic product", "TimePeriod": "2024Q1",
     "METRIC_NAME": "Current Dollars", "CL_UNIT": "Level", "UNIT_MULT": "6",
     "DataValue": "28,624,069", "NoteRef": "T10105"}
  ],
  "Notes": [{"NoteRef": "T10105", "NoteText": "Table 1.1.5. Gross Domestic Product ..."}]
}}}
```

The guide's [GetData examples](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf#page=16)
and [Appendix N](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf#page=60)
document Regional data and request parameters. The
[Regional fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/bea/regional_sagdp2_line1.json)
illustrates `DataSetName=Regional&TableName=SAGDP2&LineCode=1&GeoFips=STATE&Year=ALL`,
with `GeoFips`, `GeoName`, `TimePeriod`, `DataValue`, `CL_UNIT`, `UNIT_MULT` and optional
`NoteRef` fields. Its values are also illustrative.

Values are strings with thousands separators; `UNIT_MULT` is a power of ten (`6` =
millions). `TimePeriod` is `2024`, `2024Q1` or `2024M01`.

## Frequency and revisions

Frequency is annual, quarterly or monthly depending on the table. BEA's
[GDP release guidance](https://www.bea.gov/news/gdp-release-additional-information)
describes advance, second and third quarterly GDP estimates, annual updates that revise
the last few years, and comprehensive updates that can revise the whole history.

The API returns current estimates only; BEA publishes vintages separately through its
[previously published estimates](https://www.bea.gov/data/gdp/gross-domestic-product).
The adapter sets `revision_date = date` and `is_original_release = true`, so a revision
appears as a changed value between crawls; since every fetch covers the whole history,
the next crawl overwrites the stored value. The quarterly and monthly NIPA series crawled
here are seasonally adjusted, and dollar levels are at annual rates.

## Flags and footnotes

A row's `NoteRef`, when present, refers to entries in the `Notes` array. Notes can apply
to a table or to individual cells, such as `(D)` for suppressed Regional data; see the guide's
[GetData notes explanation](https://apps.bea.gov/api/_pdf/bea_web_service_api_user_guide.pdf#page=21).
Notes aren't stored; suppressed cells become points with no value.

## Dataset mapping

The [dataset contract](../../backend/crates/econ-graph-crawler/data/datasets/bea.toml)
defines a NIPA series by table line and frequency, and a Regional series by table line
and area:

| Dataset | Dimensions | Measures |
|---|---|---|
| `bea_nipa` | `table_name`, `line_number`, `frequency` | `value` |
| `bea_regional` | `table_name`, `line_code`, `geo_fips` | `value` |

Long shape (one value column), values scaled by `UNIT_MULT` on ingest. Additional families are declarative rules
in `catalog_selection/bea.toml`, subject to its table and series bounds.
