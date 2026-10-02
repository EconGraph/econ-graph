# World Bank (Indicators API v2)

Adapter: `backend/crates/econ-graph-crawler/src/sources/world_bank.rs`. Fixtures:
`tests/fixtures/world_bank/`. No key. Background:
[World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
and [integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md).

A live call on 2026-09-26 returned the same fields as the fixtures below, including
`obs_status` and `decimal`.

## What we fetch today

About 50 curated World Development Indicators, listed in
`backend/crates/econ-graph-crawler/data/wdi_indicators.csv`.

- **Discovery and fetch** both use one request per indicator for every area:
  `GET /country/all/indicator/{id}?format=json&per_page=20000[&page=N]`. One page holds a
  whole indicator today (about 266 areas times decades of history); further pages are
  followed when the response says so. Discovery lists the areas with at least one value for
  each indicator; fetching is batched by indicator, one request serving every area.
- **Series and ids**: one series per indicator and area, in dataset `wdi`
  (`data/datasets/world_bank.toml`) with dimensions `indicator` (the WDI code) and `area` (a
  key from the shared country reference: ISO alpha-3 for countries, the World Bank code for
  aggregates such as `EMU` or `WLD`). The external id is `wdi/{indicator}.{area}` (for
  example `wdi/NY.GDP.PCAP.CD.USA`). That is about 50 x 217 ≈ 11,000 series, not the
  roughly 1,400 x 217 ≈ 300,000 a crawl of all of WDI's indicators would be.
- **Kept**: indicator id and name (from `wdi_indicators.csv`), per-row `date`, `value`
  and the response's `lastupdated` (as the vintage date).
- **Dropped**: `obs_status` and `decimal` — `data_points` has no attribute columns in train
  1 (the dataset still declares them, for the schema). WDI leaves `obs_status` empty for
  nearly every row. Rows for an area outside the shared country table (regional aggregates
  not in it, the Channel Islands) are skipped, with one warning per request.

## Sample (recorded)

Every response is a two-element array `[meta, items]`. From `gdp_per_capita.json`,
trimmed:

```json
[
  {"page": 1, "pages": 1, "per_page": 20000, "total": 39, "sourceid": "2", "lastupdated": "2026-07-01"},
  [
    {
      "indicator": {"id": "NY.GDP.PCAP.CD", "value": "GDP per capita (current US$)"},
      "country": {"id": "ZH", "value": "Africa Eastern and Southern"},
      "countryiso3code": "AFE",
      "date": "2023",
      "value": 1520.1,
      "unit": "",
      "obs_status": "",
      "decimal": 1
    }
  ]
]
```

Errors are HTTP 200 with `[{"message": [{"id": "120", "key": "Invalid value", ...}]}]`.
`value` is a JSON number or `null` (dropped as a missing observation). `date` is `"2025"`
for annual data, and `"2025Q1"` or `"2025M01"` for the rare quarterly or monthly indicator
(from public docs).

## Frequency and revisions

Mostly annual. World Development Indicators (source 2) is updated several times a year, and
each update can revise past years. Every point's `revision_date` is the response's
`lastupdated` (the date the World Bank last updated the database), so each database update
is stored as a new vintage of the whole series — this is one of only two adapters (with
FRED) that store a real vintage rather than overwriting in place. `since` is ignored: the
full history comes back in the same single request regardless, which keeps revisions to old
years.

## Flags and footnotes

`obs_status` per observation (empty in the recorded samples; used for estimates and
forecasts in some sources) and `decimal` (display precision) are returned but dropped (see
above). Footnotes exist at the country-series level through the separate metadata API (from
public docs), not requested.

## Dataset mapping

One dataset for WDI, already in effect (`data/datasets/world_bank.toml`):

| Role | Columns |
|---|---|
| Dimensions | `indicator`, `area` |
| Measures | `value` (decimal) |
| Attributes | `obs_status` (string), `decimal` (small int) — declared, not yet populated |
| Shape | long, `revision_date` = `lastupdated` of the edition |

Covering all of WDI's roughly 1,400 indicators instead of the curated ~50 would multiply
series count toward 300,000 — the federation roadmap's "many small files" case, and a good
test for partitioning by `identity(series_id)` if WDI's scope grows past train 1.
