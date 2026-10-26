# FRED (Federal Reserve Economic Data, St. Louis Fed)

Adapter: `backend/crates/econ-graph-crawler/src/sources/fred.rs`. Fixtures:
`tests/fixtures/fred/`. API key required (`FRED_API_KEY`).

## What we fetch today

- **Discovery**: bounded release/category catalogs selected by
  `backend/crates/econ-graph-crawler/data/catalog_selection/fred.toml`. Rules can constrain
  title, id prefix and frequency. All pages must agree on count and ordering before the
  snapshot supplies absence evidence; failed, truncated or changing pages fail discovery.
  Separate scan and selected-series bounds stop unexpected expansion before observation
  jobs are enqueued. A small include/exclude list handles explicit exceptions.
  The default scope is GDP release series plus headline overrides, including probes for two
  previously curated retired ids. `expected_missing` explicitly exempts those known retired
  probes from the unexpected-missing warning, while keeping their lifecycle evidence. An
  ordinary included id that disappears still raises the warning.
  Existing stored series outside that scope stay retained
  and scheduled; changing the discovery scope does not deactivate them. This is not a full
  FRED catalog crawl. Defaults are fixture-tested, not validated against the live API here.
- **Publication state**: complete catalogs track repeated absences separately from fetch
  failures. Restricted series are retained internally and hidden from public reads.
  Discontinued, missing and retired histories stay publicly reachable with a badge and
  lower search priority. Successor candidates require operator confirmation; histories
  are never merged. See the
  [catalog runbook](../monitoring/crawler-coverage-runbook.md#catalog-selection-and-retained-series).
- **Fetch**: `GET /fred/series/vintagedates?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=10000[&offset=...]`
  to list the vintage dates in the fetch's real-time window, then
  `GET /fred/series/observations?series_id=ID&realtime_start=..&realtime_end=..&limit=100000[&offset=...]`
  once per real-time window of at most 1,000 vintage dates (see
  [Vintage-date limit](#vintage-date-limit)), each paged. A full fetch asks for every
  vintage FRED has (`realtime_start=1776-07-04`); an incremental fetch starts the real-time
  window the day before the newest stored vintage, so revisions to old dates still arrive,
  and re-reads that vintage in case it changed. A series whose `last_updated` hasn't moved
  since the last fetch skips both requests.
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

### Vintage-date limit

One `/series/observations` request may cover at most 2,000 vintage dates in JSON. The
[`vintage_dates` parameter docs](https://fred.stlouisfed.org/docs/api/fred/series_observations.html)
say: "Your query will be limited to a specific number of vintage dates depending on the
series_id, file_type and output_type specified", with "any output_type or other series: 1000
csv, 1000 xlsx, 2000 json, 2000 xml". FRED applies the limit to a
`realtime_start`..`realtime_end` window too, and answers a window holding more with HTTP 400,
`Bad Request.  This exceeds the maximum number of vintage dates allowed (2000).` Daily series
such as DFF, DGS10 and EFFR have several thousand vintage dates.

- **Default batch**: every fetch lists the vintage dates in its window first and requests
  the observations in consecutive real-time windows of at most 1,000 vintage dates
  (`FRED_MAX_VINTAGE_DATES_PER_REQUEST`), half the documented limit. The margin covers what
  FRED's count may include beyond the listed dates (each window after the first starts the
  day before its first vintage, which can itself be a vintage date), dates the listing leaves
  out (it omits release dates on which the series didn't change), and a JSON limit lowered to
  the 1,000 FRED already uses for other formats. A series with at most 1,000 vintage dates is
  one observations request, as before; DFF's roughly 5,150 take six.
- **Windows**: the first window starts where the fetch does; each later one starts the day
  before its first vintage and ends the day before the next window's first vintage; the last
  ends at `9999-12-31`. A row is stored only from the window whose range, from its first
  vintage on, holds the row's `realtime_start`, so each vintage of each date is stored
  exactly once. The values FRED returns as already in effect on a window's first day (clamped
  to it) aren't stored, but mark those dates as already published, so original releases are
  the same as with one request.
- **Fallback**: if FRED still rejects a window for too many vintage dates (for example after
  lowering its limit), the crawler logs a warning, splits that window in half and retries,
  down to a single vintage date. Every other HTTP 400 fails the fetch as before.

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
