# World Bank (Indicators API v2)

The [World Bank adapter](../../backend/crates/econ-graph-crawler/src/sources/world_bank.rs)
uses the public Indicators API v2 without an API key. Its
[fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/world_bank/)
cover observation and metadata responses. Background:
[World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
and [integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md).

## Coverage and requests

The adapter crawls the 158 curated World Development Indicators in
[wdi_indicators.csv](../../backend/crates/econ-graph-crawler/data/wdi_indicators.csv).
Discovery lists supported areas with at least one non-null value for each indicator.
Each series belongs to dataset `wdi`, defined in
[world_bank.toml](../../backend/crates/econ-graph-crawler/data/datasets/world_bank.toml),
with dimensions `indicator` (the WDI code) and `area` (a key from the
[shared country reference](../../backend/crates/econ-graph-core/data/countries.csv)).
Countries use ISO alpha-3 keys; aggregates use World Bank codes such as `EMU` or `WLD`.
The canonical external ID is `wdi/{indicator}.{area}`, for example
`wdi/NY.GDP.PCAP.CD.USA`. The catalog size depends on which supported areas have values
for each curated indicator.

When observations are needed, discovery and fetching use
`GET /country/all/indicator/{id}?format=json&per_page=20000&page=N`.
Fetching is batched by indicator, so the same download supplies all requested areas of
that indicator. One page commonly holds the whole indicator; the adapter follows the
response's page count up to ten pages, warning and skipping any remaining pages.
A public call for `NY.GDP.PCAP.CD` on 2026-10-26 returned 17,556 rows in one page;
this is an example, not a page-count guarantee for every indicator.

On the first discovery, the adapter downloads observations and builds a catalog version.
Only a discovery in which every indicator succeeds stores that version. With a stored
version, later discoveries first request `per_page=1` for each listed indicator.
If the combined version matches, discovery returns unchanged without downloading history.

A fetch batch can also use a one-row probe when every requested series for an indicator
has the same stored validators, including a version. If that version matches the probe,
the adapter marks those series crawled and keeps their stored points. The version includes
the response's `lastupdated`, the parser version, and a fingerprint of the cached indicator name and description. Discovery combines these
versions with the indicator IDs, so changes to the curated list also trigger discovery.
Missing or different validators, a changed version, or bypassing fetch validators causes a
full download. Authentication and rate-limit errors from a probe abort discovery or fail
the affected indicator's requested series; other probe failures fall back to full discovery
or fetching.

## Reference metadata and area names

Before discovery, the worker calls `refresh_reference_data`. It conditionally requests
`GET /indicator/{id}?format=json` for each curated indicator and
`GET /country?format=json&per_page=1000` for aggregate names, using stored HTTP validators.
Changed indicator names and descriptions (`sourceNote`) are merged into the `wdi`
dataset's `indicator` dimension codes. The adapter reloads its indicator metadata cache
from those codes after refresh and at the start of discovery and fetching. Those operations
use the cached values to construct series titles and descriptions; refreshing the codes
does not itself rewrite existing series.

The curated CSV carries indicator IDs and units, but no names or descriptions. Without
cached metadata, a series title uses the data row's `indicator.value`, falling back to the
indicator ID if that is also absent. The adapter also attempts to store a row-carried name
in the dataset's indicator codes. The data rows have no `sourceNote`, so without indicator
metadata the series gets a generic description identifying its indicator and area.

The shared country reference carries supported aggregates by code only. Reference refresh
stores the World Bank's names for those aggregates in the `wdi` dataset's `area` dimension
codes. EconGraph's API uses these dataset-specific labels for aggregates and the shared
ISO labels for countries. Series titles use a different source for aggregate names:
the `country.value` carried by observation rows. Country titles use the shared reference's
names. World Bank region and income-level fields in the country response are not stored.

Rows for areas outside the shared country reference, such as unsupported regional
aggregates and the Channel Islands, are skipped with a warning listing the unknown areas
for that indicator.

## Sample (checked-in fixture)

Successful observation responses have the shape `[meta, items]`. This excerpt retains the
metadata and first row of
[gdp_per_capita.json](../../backend/crates/econ-graph-crawler/tests/fixtures/world_bank/gdp_per_capita.json):

```json
[
  {"page": 1, "pages": 1, "per_page": 20000, "total": 39, "sourceid": "2", "lastupdated": "2026-07-01"},
  [
    {
      "indicator": {"id": "NY.GDP.PCAP.CD", "value": "GDP per capita (current US$)"},
      "country": {"id": "ZH", "value": "Africa Eastern and Southern"},
      "countryiso3code": "AFE",
      "date": "2023",
      "value": 1520.1,
      "unit": "",
      "obs_status": "",
      "decimal": 1
    }
  ]
]
```

API errors can arrive with HTTP 200 in a one-element envelope such as
`[{"message": [{"id": "120", "key": "Invalid value", ...}]}]`.
`value` is a JSON number or `null`; null values are dropped as missing observations.
WDI dates are annual strings such as `"2025"`. The adapter's date parser also accepts
quarterly (`"2025Q1"`) and monthly (`"2025M01"`) forms; the annual fixture above does not
illustrate those forms.

## Frequency and revisions

WDI observations are mostly annual. Database updates can revise past years.
Every point's `revision_date` is the response's `lastupdated`, the date the World Bank last
updated the database. A changed release date therefore creates a new vintage of the series
rather than overwriting the previous vintage. FRED and World Bank are the two adapters
that use source vintage dates. The adapter retrieves the current edition, not a
historical-vintage endpoint.

A release restates every observation, so persisting compares each one with the newest
stored row for its date at or before the release date:

| Stored row for the date | Written |
|---|---|
| None | a row at the release date, `is_original_release = true` |
| An earlier release, same value | nothing; reads take the value from the earlier row |
| An earlier release, different value | a row at the release date, `is_original_release = false` |
| The same release (re-read) | the existing row, updated in place if its value differs |

A WDI release therefore adds rows only for the values it changes or adds, rather than the
whole history of every series. A value the World Bank drops from a release keeps its last
stored row.

A date's original release is normally its earliest stored vintage, which `originalOnly`
reads return. A stale response, whose `lastupdated` is older than what is stored, writes
its values as original releases at that older vintage (see the first row of the table), so
such a date can have two original rows. Rows stored before these rules marked every release
as original. The data migration `2026-11-01-000100_world_bank_first_release` keeps the
earliest real vintage of each date as the original release and marks the later ones as
revisions. It deletes no row and changes no value. Synthetic legacy rows dated at their
observation date are left as they are; reads already skip them.

When a download is needed, `since` does not trim the requested history. Returning earlier
years preserves their revised values under the new release date, subject to the ten-page
limit above. An unchanged probe keeps the observations already stored.

## Flags and footnotes

Observation rows return `obs_status` (observation status) and `decimal` (display precision).
Both are dropped because `data_points` has no attribute columns, although the dataset
declares them. `obs_status` is empty in the checked-in sample.

The World Bank's [basic-call documentation](https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures)
describes `footnote=y` on observation requests. These footnotes apply to a country,
indicator and year. The adapter does not request this option or store observation footnotes.

## Dataset mapping

The [WDI dataset definition](../../backend/crates/econ-graph-crawler/data/datasets/world_bank.toml)
declares the following structure:

| Role | Columns |
|---|---|
| Dimensions | `indicator`, `area` |
| Measures | `value` (decimal) |
| Attributes | `obs_status` (string), `decimal` (integer) — declared; observation values are not stored |
| Shape | long, `revision_date` = the response's `lastupdated` |
