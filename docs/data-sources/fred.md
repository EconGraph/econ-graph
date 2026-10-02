# FRED (Federal Reserve Economic Data, St. Louis Fed)

Adapter: `backend/crates/econ-graph-crawler/src/sources/fred.rs`. Fixtures:
`tests/fixtures/fred/`. API key required (`FRED_API_KEY`).

## What we fetch today

- **Discovery**: looks up live metadata for each id in a curated list,
  `backend/crates/econ-graph-crawler/data/fred_series.csv` (headline series across GDP,
  employment, inflation, rates and more; 181 ids), via `GET /fred/series?series_id=ID`
  per id. A series whose `notes` match a known copyright/reproduction-restriction phrase
  (e.g. Coinbase, S&P/Case-Shiller) is dropped as a safety net before it is discovered. This
  replaced a `/series/search`-based crawl, which had no bound on how many series it could
  turn up.
- **Fetch**: `GET /fred/series/observations?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=100000[&offset=...]`,
  paged. A full fetch asks for every vintage FRED has
  (`realtime_start=1776-07-04`); an incremental fetch starts the real-time window the day
  before the newest stored vintage, so revisions to old dates still arrive, and re-reads
  that vintage in case it changed.
- **Kept**: title, notes, units, frequency, seasonal adjustment; per observation `date`,
  `value`, and `realtime_start` (as the vintage date).
- **Dropped**: `realtime_end` (derived from the next vintage), `last_updated`, `popularity`,
  the short-form fields, and the series' release and category.

## Sample (from public docs)

`observations_gdp.json` is built from FRED's documented `/fred/series/observations` response
format (real-time periods, ALFRED), not recorded from the live API; values are illustrative.
From it, trimmed — three vintages of the same quarter, then one more recent quarter:

```json
{
  "realtime_start": "1776-07-04",
  "realtime_end": "9999-12-31",
  "count": 6,
  "observations": [
    {"realtime_start": "2025-07-30", "realtime_end": "2025-08-27", "date": "2025-04-01", "value": "30331.117"},
    {"realtime_start": "2025-08-28", "realtime_end": "2025-09-24", "date": "2025-04-01", "value": "30353.902"},
    {"realtime_start": "2025-09-25", "realtime_end": "9999-12-31", "date": "2025-04-01", "value": "30485.729"},
    {"realtime_start": "2025-10-30", "realtime_end": "2025-12-22", "date": "2025-07-01", "value": "."},
    {"realtime_start": "2025-12-23", "realtime_end": "9999-12-31", "date": "2025-07-01", "value": "31095.089"},
    {"realtime_start": "2026-04-29", "realtime_end": "9999-12-31", "date": "2026-01-01", "value": "31722.514"}
  ]
}
```

From `series_gdp.json` (recorded):

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

Values are strings; `"."` means missing. Dates are the start of the period.

## Frequency and revisions

Any frequency from daily to annual, stated per series. FRED is the one source here with
real vintages through ALFRED: `realtime_start=1776-07-04&realtime_end=9999-12-31` returns
every vintage of every observation, each with the window in which it was the published
value. Each row becomes a point with `revision_date = realtime_start`; a date's earliest
row (the earliest vintage FRED has, which for most series is when ALFRED history starts, in
the 1990s, not necessarily the series' first ever publication) is marked
`is_original_release`.

For the federation model, `realtime_start` maps directly to `revision_date`, and
`realtime_end` is what the reader derives from the next vintage.

## Flags and footnotes

None per observation. Missing values are `"."`, which the adapter stores as `NULL`.
Series-level `notes` are free text.

## Dataset mapping

FRED series are single-valued and heterogeneous, so they are one dimensionless dataset,
`FRED` (`data/datasets/fred.toml`), keeping the FRED series id as the external id:

| Role | Columns |
|---|---|
| Dimensions | none beyond `series_id` (the FRED id stays the external id) |
| Measures | `value` (decimal) |
| Attributes | none |
| Shape | long (one value column), `date`, `revision_date = realtime_start` |

Frequency, units and seasonal adjustment stay series metadata in Postgres. Grouping FRED
series by release (for example "Gross Domestic Product", "Employment Situation") instead of
one flat dataset would need `GET /fred/series/release` per series, or a walk of
`/fred/releases` and `/fred/release/series`.
