# BEA (Bureau of Economic Analysis)

Adapter: `backend/crates/econ-graph-crawler/src/sources/bea.rs`. Fixtures:
`tests/fixtures/bea/` (catalog and an error only). API key required (`BEA_API_KEY`).

> **Release 1 (DATA-8):** a curated list of NIPA tables in a data file
> (for example T10101, T10105, T10106, T20100, T20600, T20804) and regional GDP by state.
> Discovery walks `GetParameterValuesFiltered` for their lines; fetch is `GetData` per
> table and frequency, scaled by `UNIT_MULT`. External ids become BEA's own table, line
> and frequency.

## What we fetch today

- **Discovery**: `GET https://apps.bea.gov/api/data?method=GetDatasetList`, then each
  dataset name is expanded from a hard-coded list (`known_series`): NIPA (GDP, GDP per
  capita, PCE), FixedAssets (net stock), ITA (exports, imports) and Regional (GDP by state).
  The ids (`NIPA_GDP_TOTAL`, `REG_GDP_TOTAL`, ...) are our own names; BEA does not know them.
- **Fetch**: not implemented. `fetch_series` fails; nothing has ever requested or parsed a
  BEA `GetData` response.

## Sample

Catalog (**recorded**, `dataset_list.json`, trimmed):

```json
{"BEAAPI": {"Results": {"Dataset": [
  {"DatasetName": "NIPA", "DatasetDescription": "Standard NIPA tables"},
  {"DatasetName": "Regional", "DatasetDescription": "Regional data sets"}
]}}}
```

Errors are HTTP 200 with `BEAAPI.Results.Error` (`error_invalid_userid.json`):

```json
{"Error": {"APIErrorCode": "3", "APIErrorDescription": "The UserId provided in the request is not valid."}}
```

NIPA data (**from public docs**, not recorded):
`GetData&DataSetName=NIPA&TableName=T10105&Frequency=Q&Year=2024` returns rows like (values illustrative)

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

Regional data (`DataSetName=Regional&TableName=SAGDP1&LineCode=1&GeoFips=STATE`) returns
rows with `GeoFips`, `GeoName`, `TimePeriod`, `DataValue`, `CL_UNIT`, `UNIT_MULT` and
`NoteRef` (from public docs).

Values are strings with thousands separators; `UNIT_MULT` is a power of ten (`6` =
millions). `TimePeriod` is `2024`, `2024Q1` or `2024M01`.

## Frequency and revisions

Annual, quarterly and monthly depending on the table. NIPA estimates are revised on a fixed
schedule: advance, second and third estimates for each quarter, annual updates that revise
the last few years, and comprehensive updates that can revise the whole history (from
public docs). The API returns current estimates only; vintages are published separately as
archived releases, not through the API. So a revision shows up as a changed value between
crawls, and the vintage date has to be the crawl date or the release date.

## Flags and footnotes

`NoteRef` on each row points into the `Notes` array (table notes, and per-cell notes such
as `(D)` suppressed in Regional data, from public docs). Nothing parses them yet.

## Proposed dataset mapping

A BEA series is naturally a table line, so one dataset per BEA dataset and table family:

| Dataset | Dimensions | Measures | Attributes |
|---|---|---|---|
| NIPA | `table_name`, `line_number` (with `series_code` kept as metadata), `frequency` | `value` | `note_ref` |
| Regional | `table_name`, `line_code`, `geo_fips` | `value` | `note_ref`, suppression marker |

Long shape (one value column): NIPA has thousands of lines across hundreds of tables. Scale
values by `UNIT_MULT` on ingest, or keep it as series metadata, but not per row. Building
this needs discovery to walk `GetParameterValues` (tables, lines) instead of the hard-coded
list, which also replaces our made-up ids with BEA's own.
