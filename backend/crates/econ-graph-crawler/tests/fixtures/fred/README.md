# FRED fixtures

The `observations_gdp*.json` files are built from FRED's documented `/fred/series/observations`
response format (real-time periods, ALFRED), not recorded from the live API. Their values are
illustrative. `observations_gdp.json` is a full vintage fetch (`realtime_start=1776-07-04`,
`realtime_end=9999-12-31`); `observations_gdp_since_2026-04-28.json` is the incremental fetch
that follows it (known vintage 2026-04-29, so the window starts on 2026-04-28), with rows in
effect on 2026-04-28 clamped to that `realtime_start` and a later revision to an old quarter.

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
