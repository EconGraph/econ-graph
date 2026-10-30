# Census BDS (Business Dynamics Statistics)

The [Census adapter](../../backend/crates/econ-graph-crawler/src/sources/census.rs)
uses the [Census fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/census/).
It requires `CENSUS_API_KEY`: the Census Data API rejects keyless data requests
("A valid key must be included with each data API request", checked live on 2026-09-26),
so a missing key is a crawler `Auth` error before any request. An Invalid Key HTML
page is also an `Auth` error. Background:
[Census BDS integration](../technical/CENSUS_BDS_INTEGRATION.md) and
[Census Bureau integration summary](../technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md).

## What we fetch today

Discovery requests `GET /data/timeseries/bds/variables.json` and `geography.json`.
Only measures become series (`is_economic_variable`). A variable must have a numeric
`predicateType` (`int` or `float`), and its name or label must match a keyword such as
`estab`, `firm`, `job` or `rate`. The numeric type leaves out:

- the classification codes (`EAGE`, `FAGE`, `EMPSZES`, `EMPSZFI`, `EMPSZESI`,
  `EMPSZFII`, `NAICS`, ...), whose `predicateType` is `string`. They are dimensions
  to group by, and a single-series request returns the code itself, 1, every year;
- the geography and predicate variables (`for`, `in`, `time`, `GEO_ID`, `STATE`, ...)
  and `YEAR`.

The keywords leave out the numeric non-measures `DENOM` and `INDLEVEL`. Discovery then
crosses the measures with the supported `us` and `state` levels listed in the geography
metadata. This produces one series per variable nationally and one per variable per
state and DC. When the metadata lists `state`, discovery downloads the Census Bureau's
[FIPS reference file](https://www2.census.gov/geo/docs/reference/state.txt) to obtain
the 50 states and DC, rather than reading previously stored state codes.

Before processing a discovery job, the worker separately refreshes reference data.
For Census, that refresh uses a conditional GET for the same FIPS file and merges
changed state names into the `bds` dataset's `state` dimension codes. A `304` response
or an unchanged body leaves the stored codes intact. Observation fetching does not
need the state file.

Each series belongs to the single `bds` dataset, defined in the
[dataset file](../../backend/crates/econ-graph-crawler/data/datasets/census.toml),
with dimensions `geo_level` (`national` or `state`), `state` (the two-digit FIPS
code, empty for national) and `variable`. External IDs are canonical dataset IDs:
`bds/national..ESTAB` for the nation and `bds/state.06.ESTAB` for California.
IDs from earlier versions (e.g. `CENSUS_BDS_*`) are no longer produced; a fetch for
one returns `Permanent` when the required key is configured.

Fetching requests `GET /data/timeseries/bds?get={VARIABLE},YEAR&for=us:*` (or
`state:{FIPS}`), with `key=` on each API request. Each request retrieves all years
for one variable and geography. The adapter keeps `YEAR` as January 1 and the value,
and assigns "Count" as the units for every series, rates included. It does not request county,
metro or CBSA series, or breakdowns by industry, firm age or firm size.

## Samples and fixtures

[`bds_estab_us.json`](../../backend/crates/econ-graph-crawler/tests/fixtures/census/bds_estab_us.json)
is a fixture with a JSON array of string rows, header first and geography last.

```json
[["ESTAB","YEAR","us"],
 ["7106316","2019","1"],
 ["7179420","2020","1"],
 ["","2021","1"],
 ["7324017","2022","1"]]
```

[`bds_estab_state_06.json`](../../backend/crates/econ-graph-crawler/tests/fixtures/census/bds_estab_state_06.json)
uses the same response shape with placeholder establishment counts for California:

```json
[["ESTAB","YEAR","state"],
 ["823456","2021","06"],
 ["841234","2022","06"]]
```

The trimmed [`variables.json` fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/census/variables.json)
holds a subset of the public metadata. Discovery selects six of its variables:
`ESTAB`, `ESTABS_ENTRY_RATE` (`float`), `FIRM`, `JOB_CREATION`,
`JOB_CREATION_CONTINUERS` and `JOB_DESTRUCTION_CONTINUERS`. It leaves out:

- the `string` classification codes `EAGE`, `FAGE`, `EMPSZES`, `EMPSZFI` and `NAICS`;
- `CBSA`, which has no `predicateType`;
- the predicate-only `for` and `in`;
- `YEAR`, which is an `int` in this fixture but matches no keyword.

Abridged:

```json
{"variables": {
  "EAGE": {"label": "Establishment age code", "predicateType": "string", "attributes": "EAGE_LABEL"},
  "ESTAB": {"label": "Number of establishments", "predicateType": "int"},
  "ESTABS_ENTRY_RATE": {"label": "Rate of establishments born during the last 12 months", "predicateType": "float", "attributes": "ESTABS_ENTRY_RATE_F"},
  "JOB_CREATION_CONTINUERS": {"label": "Number of jobs created from expanding establishments during the last 12 months", "predicateType": "int", "attributes": "JOB_CREATION_CONTINUERS_F"},
  "YEAR": {"label": "Year", "predicateType": "int"}
}}
```

In the observation samples, every cell is a string. The empty 2021 value in the
national sample is stored as `NULL`.

## Frequency and revisions

BDS is annual, from 1978, and is published once a year with a lag of about two years;
see the [BDS FAQ](https://www.census.gov/programs-surveys/bds/about/faq.html).
It is compiled from longitudinal records built from the Business Register. Releases
can revise earlier years, including the whole history; for example, the
[2018 release note](https://www2.census.gov/programs-surveys/bds/updates/bds2018-release-note-20201112-enty-exit-rate-fix.pdf)
describes changes applied across the time series. The API serves the latest release.
The adapter stores `revision_date = date`, so refreshing a year overwrites its
previous estimate rather than retaining release vintages.

## Flags and footnotes

The observation fixtures contain no flag columns. The public
[BDS variable metadata](https://api.census.gov/data/timeseries/bds/variables.html)
identifies flags such as `ESTAB_F`, `EMP_F` and `JOB_CREATION_F` as attributes of
measures. For example, [ESTAB metadata](https://api.census.gov/data/timeseries/bds/variables/ESTAB.json)
names `ESTAB_F` in its `attributes` field. Discovery reads top-level variable names
and labels; it does not expand those attribute references into series. Fetching
requests only the selected variable and `YEAR`, and the parser retains no separate
flag or footnote attributes.

The value parser stores empty strings, JSON `null` and markers without ASCII digits
as `NULL`. A cell containing digits must parse as a decimal within the observation
value limits; malformed or out-of-range numeric text fails the series with `Parse`
instead of becoming a missing value.

## Dataset mapping

The `bds` dataset stores observations in long form. Each variable and geography
combination has its own series, identified by the three dimensions below.

| Role | Columns |
|---|---|
| Dimensions | `geo_level` (national, state), `state` (FIPS; empty for national), `variable` (the BDS variable name) |
| Measures | `value` (decimal); each BDS variable (`ESTAB`, `FIRM`, `JOB_CREATION`, ...) is a separate series via the `variable` dimension rather than its own column |
| Attributes | none; separate source flags and footnotes are not retained |
| Shape | long; `revision_date = date` (no vintage column; each refresh overwrites) |
