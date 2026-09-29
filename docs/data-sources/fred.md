# FRED (Federal Reserve Economic Data, St. Louis Fed)

Adapter: `backend/crates/econ-graph-crawler/src/sources/fred.rs`. Fixtures:
`tests/fixtures/fred/`. API key required (`FRED_API_KEY`).

> **Release 1 (#214, open):** observations are fetched with
> `realtime_start=1776-07-04&realtime_end=9999-12-31` (every vintage), then incrementally
> with `realtime_start` = the newest stored vintage minus one day and no
> `observation_start`, so revisions to old dates still arrive. `limit=100000` with offset
> paging. `revision_date = realtime_start`, and `is_original_release` marks the earliest
> vintage FRED has, which for most series is when ALFRED history starts, in the 1990s.
> The rest of this page describes `main`.

## What we fetch today

- **Discovery**: `GET /fred/series/search`, one page of the 100 most popular series, then
  up to 5 pages of 1,000 for each of 15 search terms (GDP, unemployment, inflation, ...),
  de-duplicated by id. At most 76 requests.
- **Fetch**: `GET /fred/series?series_id=ID` for metadata, then
  `GET /fred/series/observations?series_id=ID[&observation_start=...]` for current values.
- **Kept**: title, notes, units, frequency, seasonal adjustment; per observation `date` and
  `value`.
- **Dropped**: `realtime_start`/`realtime_end` on each observation, `last_updated`,
  `popularity`, the short-form fields, and the series' release and category.

## Sample (recorded)

From `observations_gdp.json`, trimmed:

```json
{
  "realtime_start": "2026-09-25",
  "realtime_end": "2026-09-25",
  "units": "lin",
  "count": 5,
  "observations": [
    {"realtime_start": "2026-09-25", "realtime_end": "2026-09-25", "date": "2025-07-01", "value": "31095.089"},
    {"realtime_start": "2026-09-25", "realtime_end": "2026-09-25", "date": "2025-10-01", "value": "."},
    {"realtime_start": "2026-09-25", "realtime_end": "2026-09-25", "date": "2026-01-01", "value": "31722.514"}
  ]
}
```

From `series_gdp.json`:

```json
{
  "id": "GDP",
  "title": "Gross Domestic Product",
  "frequency": "Quarterly",
  "units": "Billions of Dollars",
  "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
  "last_updated": "2026-08-28 07:54:02-05",
  "notes": "BEA Account Code: A191RC ..."
}
```

Values are strings; `"."` means missing. Dates are the start of the period. In a
current-values response every row's window is the request date. (The fixture's last row,
dated 2026-04-01, has an earlier window, 2026-04-30 to 2026-05-28; it looks hand-edited.
The tests only count all 5 rows, not their content, so it is left out of the sample here.)

## Frequency and revisions

Any frequency from daily to annual, stated per series. FRED is the one source here with
real vintages: through ALFRED, `realtime_start=1776-07-04&realtime_end=9999-12-31` (or
`vintage_dates=`) returns every vintage of every observation, each with the window in which
it was the published value (from public docs). We request current values only, so every
row carries today's `realtime_start`; the adapter therefore ignores it and stores
`revision_date = date` (see the comment in `parse_observation`).

For the federation model, `realtime_start` maps directly to `revision_date`, and
`realtime_end` is what the reader derives from the next vintage.

## Flags and footnotes

None per observation. Missing values are `"."`, which the adapter stores as `NULL`.
Series-level `notes` are free text.

## Proposed dataset mapping

FRED series are single-valued and heterogeneous, so one table for all of FRED works
schema-wise but makes a single huge dataset. Group by FRED **release** (for example
"Gross Domestic Product", "Employment Situation"), which also matches how FRED publishes
updates:

| Role | Columns |
|---|---|
| Dimensions | none beyond `series_id` (the FRED id stays the external id) |
| Measures | `value` (decimal) |
| Attributes | none |
| Shape | long (one value column), `date`, `revision_date = realtime_start` |

Frequency, units and seasonal adjustment stay series metadata in Postgres. Knowing a
series' release needs `GET /fred/series/release`, one more request per series, or a walk
of `/fred/releases` and `/fred/release/series` instead of search-term discovery.
