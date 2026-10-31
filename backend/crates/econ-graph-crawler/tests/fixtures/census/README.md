# Census fixtures

- `variables.json`, `geography.json`, `bds_estab_*.json`: BDS API responses.
- `bds_estabs_entry_rate_us.json`: a BDS API response shape holding the national
  establishment entry rates for 2021–2023 copied from the Census Bureau's public
  [bds2023.csv](https://www2.census.gov/programs-surveys/bds/tables/time-series/2023/bds2023.csv)
  (`estabs_entry_rate`), the same release the API serves. It shows that rates are percents.
- `state.txt`: a synthetic test fixture in the Census Bureau's
  [state FIPS reference file](https://www2.census.gov/geo/docs/reference/state.txt) format,
  with the pipe-delimited layout `STATE|STUSAB|STATE_NAME|STATENS`.
  The `STATENS` (GNIS) column is left empty because the adapter does not read it.
  The fixture includes the 50 states, DC and six territory rows to test territory exclusion.
  Discovery downloads the live file when the API lists state-level geography; this fixture
  is used only by tests.
