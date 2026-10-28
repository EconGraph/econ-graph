# Census fixtures

- `variables.json`, `geography.json`, `bds_estab_*.json`: BDS API responses.
- `state.txt`: a synthetic test fixture in the Census Bureau's
  [state FIPS reference file](https://www2.census.gov/geo/docs/reference/state.txt) format,
  with the pipe-delimited layout `STATE|STUSAB|STATE_NAME|STATENS`.
  The `STATENS` (GNIS) column is left empty because the adapter does not read it.
  The fixture includes the 50 states, DC and six territory rows to test territory exclusion.
  Discovery downloads the live file when the API lists state-level geography; this fixture
  is used only by tests.
