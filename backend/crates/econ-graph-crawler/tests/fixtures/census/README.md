# Census fixtures

- `variables.json`, `geography.json`, `bds_estab_*.json`: BDS API responses.
- `state.txt`: the Census Bureau's state FIPS reference file
  (<https://www2.census.gov/geo/docs/reference/state.txt>), in its documented pipe-delimited
  layout `STATE|STUSAB|STATE_NAME|STATENS`, built for tests rather than recorded (the source is
  not reachable from CI). The `STATENS` (GNIS) column is left empty; the adapter does not read it.
  It lists the 50 states, DC and the six territory rows the adapter must drop. The crawler reads
  the live file on every crawl; this copy is for tests only.
