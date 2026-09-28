# Census BDS (Business Dynamics Statistics)

Adapter: `backend/crates/econ-graph-crawler/src/sources/census.rs`. Fixtures:
`tests/fixtures/census/`. Key required (`CENSUS_API_KEY`), although the adapter on
`main` still treats it as optional. Background:
[Census BDS integration](../technical/CENSUS_BDS_INTEGRATION.md) and
[Census Bureau integration summary](../technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md).

> **Release 1 (#207, open):** a key is required. The API rejects keyless requests as of
> 2026: a keyless live BDS request on 2026-09-26 returned "A valid key must be included
> with each data API request". The key is sent as `key=` on data and metadata requests. A
> missing key is a crawler `Auth` error, and so is an Invalid Key HTML page.

## What we fetch today

- **Discovery**: `GET /data/timeseries/bds/variables.json` and `geography.json`. Variables
  are filtered by keyword (`is_economic_variable`) and crossed with the `us` and `state`
  levels: one series per variable nationally (`CENSUS_BDS_{VARIABLE}_us`) and one per
  variable per state and DC (`CENSUS_BDS_{VARIABLE}_state_{FIPS}`, states from
  `backend/crates/econ-graph-crawler/data/us_states.csv`).
- **Fetch**: `GET /data/timeseries/bds?get={VARIABLE},YEAR&for=us:*` (or `state:{FIPS}`),
  all years in one request. One request per variable per geography.
- **Kept**: `YEAR` (as January 1) and the value. Units are always "Count".
- **Dropped**: nothing else is requested. County, metro and CBSA levels, and the
  breakdowns BDS offers (industry, firm age, firm size), are not used.

## Sample (recorded)

`bds_estab_us.json`: a JSON array of string rows, header first, geography last.

```json
[["ESTAB","YEAR","us"],
 ["7106316","2019","1"],
 ["7179420","2020","1"],
 ["","2021","1"],
 ["7324017","2022","1"]]
```

`bds_estab_state_06.json` (real shape; the establishment counts are placeholders, far
below California's actual figure of roughly 900,000):

```json
[["ESTAB","YEAR","state"],
 ["823456","2021","06"],
 ["841234","2022","06"]]
```

`variables.json`, trimmed:

```json
{"variables": {
  "ESTAB": {"label": "Number of establishments", "predicateType": "int"},
  "FIRM": {"label": "Number of firms", "predicateType": "int"},
  "JOB_CREATION": {"label": "Number of jobs created from expanding and opening establishments during the last 12 months", "predicateType": "int"},
  "NAICS": {"label": "2017 NAICS code", "predicateType": "string"},
  "YEAR": {"label": "Year", "predicateType": "int"}
}}
```

Every value is a string. An empty string (2021 above) is a missing value, stored as `NULL`.

## Frequency and revisions

Annual, from 1978, published once a year with a lag of about two years. Each release is
rebuilt from the Business Register and can revise the whole history (from public docs);
the API serves only the latest release. The adapter stores `revision_date = date` and
overwrites.

## Flags and footnotes

None in the recorded responses. Suppressed or unavailable cells come back as empty strings
or non-numeric markers, which the adapter turns into `NULL`. Many Census API datasets pair
a variable `X` with annotation variables such as `X_F` (flag); whether BDS has them should
be checked in the full `variables.json` (the fixture is trimmed).

## Proposed dataset mapping

This is the case that motivated datasets: many measures published together for the same
place and year. One dataset for BDS by geography:

| Role | Columns |
|---|---|
| Dimensions | `geo_level` (us, state), `geo_code` (FIPS; `1` for us) |
| Measures | `firm`, `estab`, `emp`, `job_creation`, `job_destruction`, `net_job_creation`, `reallocation_rate`, `estabs_entry`, `estabs_exit`, ... (integers or decimals, from `variables.json`) |
| Attributes | per-measure flags if BDS has `_F` variables; otherwise none |
| Shape | wide (a few dozen measures), `revision_date` = release year or crawl date |

With the dataset shape the fetch also gets cheaper: one request
`get=FIRM,ESTAB,EMP,...,YEAR&for=state:*` returns every measure for every state, instead
of one request per variable per state today.

Breakdowns (NAICS sector, firm age, firm size) would be further dimensions in a separate
dataset, since they multiply the series count.
