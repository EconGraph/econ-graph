# Source notices

Several data sources' terms of use require a citation, a notice or both wherever their
data are shown or exported. EconGraph keeps that text in one table,
[`source_notices.rs`](../../backend/crates/econ-graph-graphql/src/source_notices.rs),
keyed by source code (`SourceId`). GraphQL serves it as `DataSourceType.notice`, and
every client reads it from there. The table lives in application code, not in the data
database.

| Field | Meaning |
|---|---|
| `citation` | The citation to show with the data; null when the terms prescribe none |
| `notice` | The statement the terms require, such as a no-endorsement notice |
| `termsUrl` | The provider's terms of use |
| `retrievalDateRequired` | Whether the date the data were retrieved from the source must be shown |

A source without a notice gets `null`, and clients show only the source name.

## Notices

| Source | Citation | Notice | Terms |
|---|---|---|---|
| BLS | Source: U.S. Bureau of Labor Statistics | "BLS.gov cannot vouch for the data or analyses derived from these data after the data have been retrieved from BLS.gov." The retrieval date is shown with it. | [BLS API terms](https://www.bls.gov/developers/termsOfService.htm) |
| BEA | Source: U.S. Bureau of Economic Analysis | "This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA." | [BEA API terms](https://apps.bea.gov/API/_pdf/bea_api_tos.pdf) |
| Census | Source: U.S. Census Bureau | "This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau." | [Census API terms](https://www.census.gov/data/developers/about/terms-of-service.html) |
| FHFA | Source: FHFA HPI® | "This product uses FHFA Data but is neither endorsed nor certified by FHFA." | [FHFA API terms](https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service) |
| World Bank | The World Bank: World Development Indicators | States that the data are licensed under CC BY 4.0 and that anyone reusing them must give the same attribution, in the format "The World Bank: Dataset name: Data source (if known)". | [World Bank dataset terms](https://www.worldbank.org/ext/en/legal/terms-conditions/datasets) |
| FRED | none | "This product uses the FRED® API but is not endorsed or certified by the Federal Reserve Bank of St. Louis." | [FRED API terms](https://fred.stlouisfed.org/docs/api/terms_of_use.html) |

The World Bank citation omits the per-indicator data source because the crawler does not
store it; the terms ask for it only "if known". The notice and terms link pass the
attribution requirement on to anyone who reuses the data from our CSV, GraphQL or MCP
output.

## Where notices appear

- **Series page:** below the chart, the source's citation, notice and a link to its terms.
  For BLS this includes the date EconGraph last retrieved the series (`lastUpdated`), or
  "Retrieval date not recorded" when that is unknown.
- **CSV export:** metadata rows `Citation`, `Notice`, `Retrieved from source` (BLS only;
  `not recorded` when unknown) and `Terms of use`, after the existing title, source,
  units, transformation and retrieval rows.
- **Dashboard:** each value card shows its source's notice and terms link below the card's
  link to the series.
- **World map:** the World Bank citation, notice and terms link below the map. Without a
  notice from the API, the caption cites "The World Bank: <dataset name>".
- **About page:** a Source Notices section listing every source that has a notice.
- **GraphQL:** `notice` on every `DataSourceType`, including `series.source` and
  `dataset.source`.
- **MCP:** the search, series metadata and series data tools, and the data sources and
  series catalog resources, return each source's `notice`. The chart tool lists each
  charted series' source, notice and `lastUpdated`.

No provider logos are shown, and no text implies endorsement.

## Transformed values

When the series page shows a transformation, such as a year-over-year percent change,
the page and the CSV (`Derived values` row) state that EconGraph computed the values
from the source's data and that they are not figures published by the source. The terms
of BLS, BEA, Census and FHFA forbid presenting modified data as theirs, and CC BY 4.0
requires indicating changes. The statement appears for every source, with or without a
notice.

## Adding a source

`notice` in the table is an exhaustive `match` on `SourceId`, so a new source code does
not compile until the match says whether it has a notice. A test reads the sources the
`catalog-public` role in [the Keycloak realm](../../config/keycloak/econ-graph-realm.json)
makes public and requires those without a notice to be exactly `PUBLIC_WITHOUT_NOTICE`.
Making another source public without a notice therefore fails the test, as does leaving a
source in `PUBLIC_WITHOUT_NOTICE` once it has a notice or is no longer public.
