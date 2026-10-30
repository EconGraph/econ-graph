# BLS (Bureau of Labor Statistics, Public Data API v2)

The [BLS adapter](../../backend/crates/econ-graph-crawler/src/sources/bls.rs) discovers
curated series and fetches their observations. `BLS_API_KEY` is optional; it enables
larger batches, longer request windows and catalog metadata. Test payloads are in the
[BLS fixture directory](../../backend/crates/econ-graph-crawler/tests/fixtures/bls/).
For historical API experiments, see [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md).

## Discovery and fetching

The [curated list](../../backend/crates/econ-graph-crawler/data/bls_series.csv) selects
425 series: headline CPI-U (CU), CES payrolls and earnings (CE), CPS labor force (LN),
LAUS state unemployment rates and labor force (LA) for the states and DC, JOLTS job
openings, hires, quits, layoffs and separations (JT) for total nonfarm and major
industries, PPI final demand, intermediate demand and key commodity indexes (WP), and
import/export price indexes by end use and by origin (EI). BLS's
survey series files (`cu.series`, `ce.series`, `la.series`, `ln.series`, `jt.series`,
`wp.series` and `ei.series`, under `https://download.bls.gov/pub/time.series/`) supply each
series' title (`series_title`) and frequency (from its latest period, `end_period`).
`jt.series` has no title column, so a JOLTS (JT) series' title joins BLS's labels of its
codes, in this order: data element, rate or level, industry, state, area, size class and
seasonal adjustment (for example "Job openings, Rate, Total nonfarm, Total US, All areas,
All size classes, Seasonally adjusted"). A fetch with a key replaces it with the API
catalog's title. Units come from the `base_period` of CPI (such as "Index 1982-84=100") or
of import/export prices (EI, such as "Index 2000=100"), the CE data type label in
`ce.datatype`, the LA measure label, or the JT rate/level label in `jt.ratelevel` ("Rate"
or "Level - In Thousands"). LN's and PPI's (WP) flat files provide no unit wording; their
units come only from an API fetch with a key.

Before discovery, the worker runs the adapter's reference refresh. The shared
`reference_file::refresh` mechanism uses conditional GETs (`ETag` or `Last-Modified`)
and an unchanged-body hash check to avoid reapplying unchanged files. It caches the
curated series' rows in `reference_file_cache.payload`. It also refreshes the code label
files of CU, CE, LA, JT, WP and EI, which name dimension values and supply JT titles and
units. The refresh also derives LN's
`series_code` labels from `ln.series`. Seed migrations supply code lists, but neither
these series rows nor LN labels, so a new database needs a successful refresh before
it can list the curated series.

The adapter's discovery method reads the cached rows without making an API request;
the worker persists the discovered descriptions in `series_metadata`. A curated ID
whose survey file has not loaded, or that the file does not list, or (JT) one of whose
codes has no stored label, is omitted from that discovery with a warning. If no curated series has a cached row, discovery returns a
transient error. The scheduler enqueues the first observation fetch for newly listed
series; no manual `crawler enqueue` is needed.

Fetching uses `POST /timeseries/data/`, for example:

```json
{
  "seriesid": ["CUUR0000SA0"],
  "startyear": "2023",
  "endyear": "2024",
  "catalog": true
}
```

The adapter adds `registrationkey` when `BLS_API_KEY` is set. It batches up to 50 series
per request with a key or 25 without, splitting history into year windows of up to 20
years with a key or 10 without, newest first.

BLS allows 500 requests a day with a key and 25 without. With a key, the curated list is
9 requests (9 batches of 50) for the first 20-year fetch and 9 for each refresh, whose
5-year revision lookback fits one window. Without a key it is 17 batches of 25: 17
requests for each refresh, but 34 for the first fetch (two 10-year windows), so the first
fetch spans two days: requests past the daily limit fail as rate-limited and the worker
retries them later with backoff. Set a key.

With a key, the response's `catalog` supplies the title, survey name, units
(`measure_data_type`) and seasonality. Frequency comes from the observation periods.
Without a key, BLS omits the catalog even when requested: the fetch reports frequency
and the seasonal adjustment encoded in the ID, while persistence retains the title and
units from discovery. Missing catalog fields also retain existing metadata.

## Test-fixture samples

The following excerpts show response shapes and test cases, not uniformly recorded
observations. In particular, the November 2023 shutdown entry in
[cpi_monthly.json](../../backend/crates/econ-graph-crawler/tests/fixtures/bls/cpi_monthly.json)
is an illustrative missing-value case. A public keyless request for `CUUR0000SA0` for
2023 returns November's value as `307.051` with `[{}]` footnotes, matching the
[original November 2023 CPI release](https://www.bls.gov/news.release/archives/cpi_12122023.htm).
The catalog below is fixture content; that keyless check does not verify keyed catalog
responses.

Trimmed from `cpi_monthly.json`:

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

From [eci_quarterly.json](../../backend/crates/econ-graph-crawler/tests/fixtures/bls/eci_quarterly.json), a preliminary value used by tests:

```json
{"year": "2024", "period": "Q02", "value": "4.1", "footnotes": [{"code": "P", "text": "preliminary"}]}
```

BLS can return errors as HTTP 200 with `status` other than `REQUEST_SUCCEEDED` and a
`message` array, as illustrated by
[not_processed_threshold.json](../../backend/crates/econ-graph-crawler/tests/fixtures/bls/not_processed_threshold.json)
and [series_does_not_exist.json](../../backend/crates/econ-graph-crawler/tests/fixtures/bls/series_does_not_exist.json).

## Frequency and revisions

The adapter converts `year` and `period` to the period's start date: monthly codes
`M01`–`M12` become the first of each month; quarterly codes `Q01`–`Q04` become January 1,
April 1, July 1 and October 1; semiannual codes `S01` and `S02` become January 1 and July 1;
and annual code `A01` becomes January 1. It skips annual averages `M13`, `Q05` and `S03`,
and unknown period codes.

The API returns current values rather than historical vintages. A later crawl can
receive a changed value or a disappearing `P` (preliminary) footnote; annual benchmark
revisions to payrolls and seasonal factors can change several years at once. The adapter
sets `revision_date = date` and `is_original_release = true` for every observation, so
persistence overwrites the same observation on subsequent crawls and retains no revision
history.

## Parsed and stored observations

The raw response's `DataPoint` contains `year`, `period`, `value` and `footnotes`.
Each footnote has optional `code` and `text` fields; `[{}]` means no footnotes. The
fixtures include `P` (preliminary) and `X` (data unavailable, paired with a `"-"` value).
Other surveys use additional codes.

The adapter converts each retained raw point into a `FetchedPoint` with `date`, numeric
`value` (or a missing value), `revision_date` and `is_original_release`. These become the
persisted observation fields. A `"-"` value becomes `NULL`. Footnotes are parsed but are
not persisted because `data_points` has no footnote column. For missing values, the
adapter debug-logs the raw value and footnote summary; preliminary points contribute to
a per-series debug count. It does not log every observation's footnotes, and the stored
missing value does not retain its reason.

The adapter ignores `latest`, `periodName`, and catalog fields such as `area` and `item`.
BLS also documents `calculations`, `annualaverage` and `aspects` request options in its
[v2 API signatures](https://www.bls.gov/developers/api_signature_v2.htm). The adapter does
not request them. Aspects can carry extra observation values such as standard errors
for some surveys.

## Dataset mapping

Each survey with a known ID layout has its own dataset: CU (CPI-U), CE (CES national),
LN (CPS), LA (LAUS), JT (JOLTS), WP (PPI commodity data) and EI (import/export price
indexes). The layouts are in `SERIES_ID_LAYOUTS` in the adapter, and the
dataset dimensions are defined in
[bls.toml](../../backend/crates/econ-graph-crawler/data/datasets/bls.toml). A series ID
consists of the two-letter survey prefix followed by fixed-width fields that become
dimension values. A series from any other survey, or whose ID does not fit its survey's
layout, belongs to the dimensionless `other` dataset, keyed by its BLS ID like a FRED
series.

For CPI (CU), ID `CUUR0000SA0` has the following mapping:

| Role | Columns |
|---|---|
| Dimensions | `seasonal` = `U` (S/U), `periodicity` = `R`, `area` = `0000` (4 chars), `item` = `SA0` (the rest) |
| Measures | `value` (decimal) |
| Attributes | No persisted footnote attributes |
| Shape | Long (one value column); `revision_date = date`, so refreshes overwrite observations |

The other layouts split IDs the same way:

| Dataset | Example ID | Dimensions |
|---|---|---|
| CE | `CES0000000001` | `seasonal` (1), `industry` (8), `data_type` (2) |
| LN | `LNS14000000` | `seasonal` (1), `series_code` (8) |
| LA | `LASST060000000000003` | `seasonal` (1), `area` (15), `measure` (2) |
| JT | `JTS000000000000000JOR` | `seasonal` (1), `industry` (6), `state` (2), `area` (5), `size_class` (2), `data_element` (2), `rate_level` (1) |
| WP | `WPSFD4`, `WPU01` | `seasonal` (1), `group` (2), `item` (the rest, empty for a group index such as `WPU01`) |
| EI | `EIUCOCHNTOT`, `EIUIR` | `seasonal` (1), `index` (2), `item` (the rest, empty for an index's all-commodities series such as `EIUIR`) |

Annual averages are excluded from this mapping. Keeping their separate period codes
out of the observation dates avoids collisions with regular periods. Footnote codes,
preliminary status and missing-value reasons cannot be retrieved from stored observations.
