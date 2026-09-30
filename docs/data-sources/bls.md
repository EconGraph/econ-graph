# BLS (Bureau of Labor Statistics, Public Data API v2)

Adapter: `backend/crates/econ-graph-crawler/src/sources/bls.rs`. Fixtures:
`tests/fixtures/bls/`. Key optional (`BLS_API_KEY`); with a key a request may span 20
years, without one 10. Background: [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md).

> **Release 1 (#235, draft):** discovery reads 291 series from
> `econ-graph-crawler/data/bls_series.csv` with no HTTP call, replacing the `/surveys`
> discovery below. The list covers CPI-U, CES, CPS and LAUS for the states and DC.
> Fetches are batched at 50 series per request with `BLS_API_KEY`, 25 without. Footnotes
> are parsed and logged, but not yet stored. The rest of this page describes `main`.

## What we fetch today

- **Discovery**: `GET /surveys` lists surveys, not series. Each survey abbreviation is then
  expanded from a hard-coded table (`known_series_for_survey`) to four series in total:
  `CUUR0000SA0`, `CUUR0000SA0L1E` (CPI), `CES0000000001` (nonfarm payrolls) and
  `LNS14000000` (unemployment rate).
- **Fetch**: `POST /timeseries/data/` with `{"seriesid": [id], "startyear", "endyear",
  "catalog": true}`, in year windows, newest first; 20 years of history on a first fetch.
- **Kept**: `year` + `period` (as the period's start date) and `value`; from `catalog`,
  the title, survey name, `measure_data_type` (as units) and seasonality.
- **Dropped**: `footnotes` (not even deserialized), `latest`, `periodName`, the catalog's
  `area` and `item`; annual averages `M13`, `Q05` and `S03` are skipped.

## Sample (recorded)

From `cpi_monthly.json`, trimmed:

```json
{
  "status": "REQUEST_SUCCEEDED",
  "message": [],
  "Results": { "series": [ {
    "seriesID": "CUUR0000SA0",
    "catalog": {
      "series_title": "All items in U.S. city average, all urban consumers, not seasonally adjusted",
      "seasonality": "Not Seasonally Adjusted",
      "survey_abbreviation": "CU",
      "measure_data_type": "Index 1982-1984=100",
      "area": "U.S. city average",
      "item": "All items"
    },
    "data": [
      {"year": "2024", "period": "M03", "periodName": "March", "latest": "true", "value": "312.332", "footnotes": [{}]},
      {"year": "2023", "period": "M13", "periodName": "Annual", "value": "304.702", "footnotes": [{}]},
      {"year": "2023", "period": "M11", "periodName": "November", "value": "-",
       "footnotes": [{"code": "X", "text": "Data unavailable due to the lapse in appropriations."}]}
    ]
  } ] }
}
```

From `eci_quarterly.json`, a preliminary value:

```json
{"year": "2024", "period": "Q02", "value": "4.1", "footnotes": [{"code": "P", "text": "preliminary"}]}
```

Errors come back as HTTP 200 with `status` other than `REQUEST_SUCCEEDED` and a `message`
array (`not_processed_threshold.json`, `series_does_not_exist.json`).

## Frequency and revisions

Monthly (`M01`-`M12`, `M13` annual average), quarterly (`Q01`-`Q04`, `Q05` annual),
semiannual (`S01`, `S02`, `S03` annual) and annual (`A01`). The API has no vintages: it
returns current values only, and revised values replace earlier ones. Revisions show up as
the `P` (preliminary) footnote disappearing and the value changing between crawls, plus
annual benchmark revisions (payrolls, seasonal factors) that rewrite several years at once.
The adapter stores `revision_date = date` and overwrites.

## Flags and footnotes

Each observation has a `footnotes` array of `{code, text}`; `[{}]` means none. Codes seen in
fixtures: `P` preliminary, `X` data unavailable (the value is then `"-"`). Other surveys use
more codes. The v2 API also offers `calculations`, `annualaverage` and `aspects` request
options (from public docs; not requested today); aspects carry extra per-observation values
such as standard errors for some surveys.

The adapter drops footnotes entirely: `DataPoint` in `bls.rs` deserializes only `year`,
`period` and `value`. A `"-"` value becomes `NULL` with no record of why.

## Proposed dataset mapping

One dataset per survey (CU, CE, LN, ...), since each survey's series id is a fixed layout
of that survey's dimensions (documented in the survey's mapping files, for example
`https://download.bls.gov/pub/time.series/cu/cu.series`, from public docs). For CPI (CU):

| Role | Columns |
|---|---|
| Dimensions | `seasonal` (S/U), `periodicity`, `area_code`, `item_code` |
| Measures | `value` (decimal) |
| Attributes | `footnote_codes` (list of strings), `preliminary` (bool, from `P`) |
| Shape | long (one value column); `revision_date` = crawl date |

Annual averages (`M13`) are derived; keep skipping them, or store them as a separate
periodicity rather than colliding with `M01`. Decoding ids into dimensions needs each
survey's mapping files, which is new reference data (a shared data file, like
`us_states.csv`).
