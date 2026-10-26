# FRED fixtures

The `observations_gdp*.json` files are built from FRED's documented `/fred/series/observations`
response format (real-time periods, ALFRED), not recorded from the live API. Their values are
illustrative. `observations_gdp.json` is a full vintage fetch (`realtime_start=1776-07-04`,
`realtime_end=9999-12-31`); `observations_gdp_since_2026-04-28.json` is the incremental fetch
that follows it (known vintage 2026-04-29, so the window starts on 2026-04-28), with rows in
effect on 2026-04-28 clamped to that `realtime_start` and a later revision to an old quarter.

`vintagedates_gdp.json` is a `/fred/series/vintagedates` response in FRED's documented format
listing every vintage date the GDP observation fixtures use (the full, incremental and e2e
ones), so one file serves each of them; the adapter only keeps the dates inside the real-time
window it asked about. `vintagedates_multipage_e2e.json` lists the two vintages of
`observations_multipage_e2e.json`.

QA must check against the live API that FRED clamps `realtime_start` to the requested window
start for rows already in effect on it, as these fixtures assume. The adapter drops rows starting
before the known vintage either way.

`observations_gdp_e2e.json` is `observations_gdp.json` plus the two quarters the e2e specs
(`frontend/tests/e2e/release/`) expect that the adapter's own unit tests don't need: a genuinely
missing observation (2025-10-01, value `.`, with no later vintage) and a new quarter
(2026-04-01). It backs the release end-to-end stack's single-shot seed
(`tests/fixtures/e2e-seed.json`, run by `seed-fixtures`), which only ever calls `fetch_series`
once and so never exercises the incremental path — it needs a fixture that already has every
vintage in one response. It's kept separate from `observations_gdp.json` so the adapter's own
full-fetch unit tests keep their original, smaller fixture.

`series_multipage_e2e.json` and `observations_multipage_e2e.json` are a synthetic daily series
(`EGE2EMULTIPAGE`, "Multi-Page Daily Test Series") for ECO-261's multi-page end-to-end spec
(`frontend/tests/e2e/release/series-ui/multipage-series.spec.ts`): 22,000 daily observations, so
the frontend reads it in three `seriesData` pages of 10,000, with missing values on both sides of
each page boundary and revised dates on every page. `make_observations_multipage.py` writes the
observations file and documents its shape; edit and re-run it rather than editing the JSON. The
rows omit `realtime_end`, which the adapter doesn't read, to keep the file small.
