# BEA (Bureau of Economic Analysis)

Adapter: `backend/crates/econ-graph-crawler/src/sources/bea.rs`. Curated tables:
`backend/crates/econ-graph-crawler/data/bea_tables.csv`. Fixtures: `tests/fixtures/bea/`
(`GetData` for every curated NIPA table and frequency, SAGDP2N line codes, areas and line 1,
and an error). API key required (`BEA_API_KEY`); without it every call is `Permanent` before
any request.

## What we fetch today

- **Ids** are BEA's own codes: `bea_nipa/{table}.{line_number}.{frequency}` (for example
  `bea_nipa/T10105.1.Q`, GDP quarterly) and `bea_regional/{table}.{line_code}.{geo_fips}`
  (for example `bea_regional/SAGDP2N.1.06000`, California GDP).
- **Discovery**: for each NIPA table and frequency in `bea_tables.csv`, `GetData` for the last
  three years lists its lines (a table's line list isn't a `GetParameterValuesFiltered`
  parameter). For Regional, `GetParameterValuesFiltered` lists the table's `LineCode`s and
  `GeoFips`, kept to the US, states and BEA regions. Discovery is all or nothing: any failed
  request or empty table fails it, and a successful one is complete, so the worker retires
  series it no longer lists (dropped lines, and the old made-up ids such as `NIPA_GDP_TOTAL`).
- **Table titles** are BEA's own, from `GetParameterValues(DatasetName, ParameterName=TableName)`
  (one request per dataset, conditional `ETag`), not a curated column: `bea_tables.csv` only
  lists which tables and frequencies we crawl. Titles are kept in an in-process cache the worker
  fills from this call (so `discover`/`fetch` stay DB-free); until a worker process has run it at
  least once, every table's title is unknown. `fetch` on an unknown title still fetches real
  points but omits metadata so a previously stored good title isn't overwritten; `discover`
  fails outright on an unknown title, rather than persisting the bare table name as every one of
  that table's series' titles.
- **Fetch**: `GetData` per NIPA table and frequency, or per Regional table and line with the
  requested areas comma-separated, batched up to 64 series. Always the whole history (`Year`
  `X` or `ALL`), since revisions reach back years. Values are scaled by `UNIT_MULT`;
  suppressed or unavailable cells (`(D)`, `(NA)`, ...) become points with no value.
- **Errors**: BEA answers HTTP 200 with an error object. A `UserId` error is `Auth`, a rejected
  parameter (invalid, not valid, does not exist, missing, required) is `Permanent`, and
  anything else is `Transient`. The rate policy is 30 requests a minute.

## Sample

Errors are HTTP 200 with `BEAAPI.Results.Error` (`error_invalid_userid.json`):

```json
{"Error": {"APIErrorCode": "3", "APIErrorDescription": "The UserId provided in the request is not valid."}}
```

NIPA data (`nipa_t10105_q.json`, shaped from public docs; values illustrative):
`GetData&DataSetName=NIPA&TableName=T10105&Frequency=Q&Year=X` returns rows like

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

Regional data (`DataSetName=Regional&TableName=SAGDP2N&LineCode=1&GeoFips=STATE`) returns
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
crawls; since every fetch covers the whole history, the next crawl overwrites it. Quarterly
and monthly NIPA series are seasonally adjusted, and dollar levels are at annual rates.

## Flags and footnotes

`NoteRef` on each row points into the `Notes` array (table notes, and per-cell notes such
as `(D)` suppressed in Regional data, from public docs). Notes aren't stored; suppressed
cells become points with no value.

## Dataset mapping

A BEA series is a table line (`data/datasets/bea.toml`):

| Dataset | Dimensions | Measures |
|---|---|---|
| `bea_nipa` | `table_name`, `line_number`, `frequency` | `value` |
| `bea_regional` | `table_name`, `line_code`, `geo_fips` | `value` |

Long shape (one value column), values scaled by `UNIT_MULT` on ingest. More tables are rows
in `bea_tables.csv`.
