# World Bank (Indicators API v2)

Adapter: `backend/crates/econ-graph-crawler/src/sources/world_bank.rs`. Fixtures:
`tests/fixtures/world_bank/`. No key. Background:
[World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
and [integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md).

> **Release 1 (DATA-9):** about 50 curated WDI indicators, one request per
> indicator for all countries (`/country/all/indicator/{id}`). One series per indicator
> and area, in dataset `wdi` with dimensions `indicator` (the WDI code) and `area` (a key
> from the shared country reference: ISO alpha-3 for countries, the World Bank code for
> aggregates such as `EMU` or `WLD`). `revision_date` is the source's `lastupdated`. That
> is about 11,000 series rather than the 300,000 estimated below for all of WDI.
>
> A live call on 2026-09-26 returned the same fields as the fixture below, including
> `obs_status` and `decimal`; Japan's recent values were `null` there, and `lastupdated`
> was 2026-07-13.

## What we fetch today

- **Discovery**: four strategies against `https://api.worldbank.org/v2`: indicators of
  topics 3, 7 and 11; ten key indicators by id; a probe of 15 countries x 5 indicators; and
  up to 10 pages of the full indicator list, keyword-filtered. At most 110 requests.
- **Fetch**: not implemented. A discovered indicator id is not tied to a country, so there
  is no single series to fetch; `fetch_series` fails.
- **Kept**: indicator id, name, `sourceNote` (description), `unit`; frequency is assumed
  "Annual".

## Sample (recorded)

Every response is a two-element array `[meta, items]`. Data for one country and indicator
(`country_indicator.json`, trimmed; real shape, but the value and `lastupdated` are
placeholders — the live call below returned different ones for the same country and
indicator):

```json
[
  {"page": 1, "pages": 65, "per_page": 1, "total": 65, "sourceid": "2", "lastupdated": "2026-07-01"},
  [
    {
      "indicator": {"id": "GC.DOD.TOTL.GD.ZS", "value": "Central government debt, total (% of GDP)"},
      "country": {"id": "JP", "value": "Japan"},
      "countryiso3code": "JPN",
      "date": "2025",
      "value": 215.3,
      "unit": "",
      "obs_status": "",
      "decimal": 1
    }
  ]
]
```

Indicator metadata (`indicator_single.json`, trimmed):

```json
{"id": "FR.INR.RINR", "name": "Real interest rate (%)", "unit": "",
 "source": {"id": "2", "value": "World Development Indicators"},
 "sourceOrganization": "International Monetary Fund, International Financial Statistics ...",
 "topics": [{"id": "3", "value": "Economy & Growth "}]}
```

Errors are HTTP 200 with `[{"message": [{"id": "120", "key": "Invalid value", ...}]}]`.
`value` is a JSON number or `null`. `date` is `"2025"` for annual data, and `"2025Q1"` or
`"2025M01"` in the few quarterly or monthly sources (from public docs).

## Frequency and revisions

Mostly annual. World Development Indicators (source 2) is updated several times a year, and
each update can revise past years. `lastupdated` in the meta element dates the source's
last update. The API serves current values only; the WDI Database Archives source keeps
past editions (from public docs), which could backfill vintages if ever needed.

## Flags and footnotes

`obs_status` per observation (empty in the recorded sample; used for estimates and
forecasts in some sources) and `decimal` (display precision). Neither is parsed, since
fetching values is not implemented. Footnotes exist at the country-series level through
the separate metadata API (from public docs).

## Proposed dataset mapping

One dataset per World Bank source (WDI is source 2), long:

| Role | Columns |
|---|---|
| Dimensions | `indicator_id`, `country_iso3` |
| Measures | `value` (decimal) |
| Attributes | `obs_status` (string), `decimal` (small int) |
| Shape | long (WDI has about 1,400 indicators), `revision_date` = `lastupdated` of the edition |

This makes one series per country and indicator: roughly 1,400 x 217 = 300,000 series for
WDI, each only a few dozen annual rows. That is the case the federation roadmap's
"many small files" risk describes, so WDI is a good test for partitioning by
`identity(series_id)`. Fetching per indicator for all countries
(`/country/all/indicator/{id}?per_page=20000`) keeps the request count at one per
indicator.
