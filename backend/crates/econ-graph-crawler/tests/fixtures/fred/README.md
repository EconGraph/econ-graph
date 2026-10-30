# FRED fixtures

The `observations_gdp*.json` files use FRED's documented
[`/fred/series/observations` response format](https://fred.stlouisfed.org/docs/api/fred/series_observations.html)
for [real-time periods](https://fred.stlouisfed.org/docs/api/fred/realtime_period.html)
(ALFRED). They are synthetic, with illustrative values, rather than recordings of the live API.
`observations_gdp.json` represents a full vintage fetch (`realtime_start=1776-07-04`,
`realtime_end=9999-12-31`). `observations_gdp_since_2026-04-28.json` represents the incremental
fetch that follows it, with a known vintage of 2026-04-29. The adapter starts the request window
one day earlier, on 2026-04-28, so it can reread the known vintage itself.

The incremental fixture models rows already in effect on 2026-04-28 with `realtime_start`
clamped to that date, followed by a later revision to an old quarter. Clamping is an assumption
of these synthetic fixtures; they do not establish how the live API handles a window that
begins during an existing real-time interval. The adapter discards rows whose `realtime_start`
is earlier than the known vintage.

`vintagedates_gdp.json` uses the documented
[`/fred/series/vintagedates` response format](https://fred.stlouisfed.org/docs/api/fred/series_vintagedates.html)
and lists every vintage date used by the full, incremental, and end-to-end GDP observation
fixtures. One file therefore serves all three: the adapter keeps only the dates inside the
requested real-time window. `vintagedates_multipage_e2e.json` lists the two vintages used by
`observations_multipage_e2e.json`.

`observations_gdp_e2e.json` contains the rows from `observations_gdp.json` plus two quarters
needed by the [release end-to-end tests](../../../../../../frontend/tests/e2e/release/README.md):
a missing observation (2025-10-01, value `.`, with no later vintage) and a new quarter
(2026-04-01). The release stack's `seed-fixtures` command uses this file through the
[seed manifest](../e2e-seed.json). It calls `fetch_series` once per series, so it needs every
vintage in one response and does not exercise incremental fetching. The separate fixture lets
the adapter's full-fetch unit tests retain their original, smaller `observations_gdp.json`.

`series_multipage_e2e.json` and `observations_multipage_e2e.json` define a synthetic daily series
(`EGE2EMULTIPAGE`, "Multi-Page Daily Test Series") for ECO-261’s
[multipage end-to-end spec](../../../../../../frontend/tests/e2e/release/series-ui/multipage-series.spec.ts).
The series has 22,000 daily observation dates, so the frontend reads it in three `seriesData`
pages with a page size of 10,000. Missing values occur on both sides of each page boundary,
and each page contains revised dates. The [fixture generator](make_observations_multipage.py)
writes the observations file and documents its shape; edit and rerun the generator rather
than editing the JSON. The rows omit `realtime_end`, which the adapter does not read, to keep
the file small.

`release_series_53.json` uses the documented
[`/fred/release/series` response format](https://fred.stlouisfed.org/docs/api/fred/release_series.html)
for one catalog page of release 53 (Gross Domestic Product), in `series_id` order. It is
synthetic: three series that a `title_contains = "gross domestic product"` rule matches (one of
them titled `(DISCONTINUED)`) and one that it does not. The `crawler check-selection` tests
(`src/check_selection/tests.rs`) read it.
