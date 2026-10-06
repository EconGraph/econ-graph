# BLS (Bureau of Labor Statistics, Public Data API v2)

Adapter: `backend/crates/econ-graph-crawler/src/sources/bls.rs`. Fixtures:
`tests/fixtures/bls/`. Key optional (`BLS_API_KEY`); with a key a request may span 20
years, without one 10. Background: [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md).

## What we fetch today

- **Discovery**: the 291 series ids in
  `backend/crates/econ-graph-crawler/data/bls_series.csv` (headline CPI-U (CU), CES payrolls
  and earnings (CE), CPS labor force (LN), and LAUS state unemployment rates and labor force
  (LA) for the states and DC), described from BLS's own survey series files
  (`https://download.bls.gov/pub/time.series/{cu,ce,la,ln}/*.series`): title
  (`series_title`), frequency (from the latest period, `end_period`), and units from CPI's
  `base_period` ("Index 1982-84=100"), the CE data type label (from `ce.datatype`) or the LA
  measure label. LN's flat files have no units, so LN series have units only after a fetch
  with a key. Before each scheduled discovery the worker re-fetches each file by conditional
  GET (`ETag`/`Last-Modified`, or an unchanged body hash) and stores the curated series' rows (`reference_file_cache.payload`);
  discovery reads those rows and makes no API call. The files go through the shared
  reference-file refresh (`reference_file::refresh`); no seed migration carries them (seeds
  hold code lists only), so a new database lists these series after its first successful
  refresh. `ln.series` also supplies LN's `series_code` labels, so those aren't seeded
  either. A curated id whose file hasn't loaded yet, or that BLS's file doesn't list, is left
  out of that discovery with a warning.
- **Fetch**: `POST /timeseries/data/` with `{"seriesid": [ids..], "startyear", "endyear",
  "catalog": true, "registrationkey"?: key}`, batched at 50 series per request with a key
  (25 without), in year windows (20 years per window with a key, 10 without), newest first.
- **Kept**: `year` + `period` (as the period's start date) and `value`. With a key, the
  `catalog` gives the series' title, survey name, units (`measure_data_type`) and
  seasonality; frequency comes from the periods. Without a key there is no catalog: the fetch
  reports the frequency and the seasonal adjustment the id encodes, and the series keeps the
  title and units discovery stored from the series files. Each
  observation's `footnotes` (`{code, text}`) are parsed and logged — a `P` (preliminary)
  footnote and an `X` ("data unavailable") footnote are recognised — but not yet stored:
  `data_points` has no footnote column in train 1.
- **Dropped**: `latest`, `periodName`, the catalog's `area` and `item`; annual averages
  `M13`, `Q05` and `S03` are skipped.
- Until the scheduler picked up discovered-but-never-fetched series (#228, now merged),
  a newly listed series needed a manual `crawler enqueue --source BLS --series <ids>`; the
  scheduler now does this on its own.

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

Footnotes are now parsed (`code`, `text`) and logged per observation, but `DataPoint`'s
stored fields are still only `year`, `period` and `value` — there is no footnote column to
write them to yet. A `"-"` value becomes `NULL` with no stored record of why, beyond the log
line.

## Dataset mapping

One dataset per survey with a known id layout — CU (CPI-U), CE (CES national), LN (CPS) and
LA (LAUS) — coded by the two-letter series id prefix and defined in `data/datasets/bls.toml`
(`SERIES_ID_LAYOUTS` in `bls.rs`). A series id is that prefix followed by fixed-width
fields, split into the dataset's dimensions; a series of any other survey, or whose id
doesn't fit its survey's layout, goes in the dimensionless `other` dataset (like FRED). For
CPI (CU), id `CUUR0000SA0`:

| Role | Columns |
|---|---|
| Dimensions | `seasonal` (S/U), `periodicity`, `area` (4 chars), `item` (the rest) |
| Measures | `value` (decimal) |
| Attributes | none yet — footnotes are parsed and logged (see above) but not stored |
| Shape | long (one value column); `revision_date = date` (no vintage column; each refresh overwrites) |

Annual averages (`M13`) are derived; keep skipping them, or store them as a separate
periodicity rather than colliding with `M01`. A footnote attribute column (codes,
`preliminary` bool) would let the parsed-but-dropped footnotes above actually be stored.
