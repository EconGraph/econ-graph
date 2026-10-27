# BEA (Bureau of Economic Analysis)

The [adapter](../../backend/crates/econ-graph-crawler/src/sources/bea.rs) crawls the
NIPA and Regional tables selected in the [curated table list](../../backend/crates/econ-graph-crawler/data/bea_tables.csv).
The [BEA fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/bea/) cover
`GetData` for every curated NIPA table and frequency, Regional SAGDP2 line codes, areas
and line 1, table titles, and an error response. The adapter requires `BEA_API_KEY`;
without it, every call fails as `Permanent` before making a request.

## What we fetch today

Series IDs use BEA's own codes: `bea_nipa/{table}.{line_number}.{frequency}` (for example
`bea_nipa/T10105.1.Q`, GDP quarterly) and `bea_regional/{table}.{line_code}.{geo_fips}`
(for example `bea_regional/SAGDP2.1.06000`, California GDP).

### Table titles and discovery

Before discovery, the worker refreshes table titles with
`GetParameterValues(DatasetName, ParameterName=TableName)`, one request per dataset.
These titles come from BEA; the curated CSV selects tables and frequencies. The refresh
merges titles into the stored `table_name` dimension labels and reloads those labels into
an in-process cache, even if the upstream refresh fails. Previously cached titles are kept
if no stored labels are available. The cache is empty until the worker process first runs
the refresh; `discover` and `fetch` then read it without querying the database.
Refreshes use conditional `ETag` requests when this process retains a successfully
parsed, nonempty title list from the last refresh.

For each curated NIPA table and frequency, discovery calls `GetData` for the last three
years to list its lines: NIPA has no line parameter for `GetParameterValuesFiltered`.
For Regional, `GetParameterValuesFiltered` lists the table's `LineCode`s and `GeoFips`,
keeping the US, states and BEA regions. A failed request or empty table fails the whole
discovery. Normally, a successful discovery is complete, so the worker retires series
that it no longer lists, such as dropped lines or areas.

The exception is a curated table absent from BEA's list after a successful title refresh.
Discovery skips that table, even if an old title remains stored, and continues with the
other tables. It logs a warning naming the skipped table, and the worker reports the count
in `crawler_curated_missing{source="BEA"}`. The worker retires no series from that
discovery, preserving the skipped table's stored series.

An unknown title is different from a table absent from a successfully refreshed list.
If a table has no cached title and no successful list establishes that it is absent,
discovery fails as `Transient` so it can retry. It never saves a bare table name as the
title of every series in that table. A failed or empty title refresh can still leave usable
stored labels in the cache; it does not by itself prevent discovery.

### Fetching and errors

Fetch calls `GetData` per NIPA table and frequency, or per Regional table and line with
the requested areas comma-separated, batching up to 64 series. Every fetch requests the
whole history (`Year=X` for NIPA, `Year=ALL` for Regional), since revisions reach back
years. Values are scaled by `UNIT_MULT`; suppressed or unavailable cells (`(D)`, `(NA)`,
...) become points with no value. If the cached table title is unknown, fetch still returns
the real points but omits metadata, preserving a previously stored title.

BEA can return HTTP 200 with an error object. The adapter classifies a `UserId` error as
`Auth`, a rejected parameter (invalid, not valid, does not exist, missing, required) as
`Permanent`, and anything else as `Transient`. The crawler's rate policy is 30 requests
a minute.

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
illustrates `DataSetName=Regional&TableName=SAGDP2&LineCode=1&Year=ALL` with
`GeoFips=00000,01000,06000,36000,56000`, selecting the US, Alabama, California, New York
and Wyoming. Rows contain `GeoFips`, `GeoName`, `TimePeriod`, `DataValue`, `CL_UNIT`,
`UNIT_MULT` and optional `NoteRef` fields. Its values are also illustrative. For an
all-states request, the guide documents `GeoFips=STATE`.

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

Both datasets use a long shape with one `value` measure, scaled to base units by
`UNIT_MULT` on ingest. To select another table, add its dataset, table name and crawl
frequencies to the curated table list.
