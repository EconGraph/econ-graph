# Roadmap: global analysis (world map and cross-country data)

Status: proposed (2026-09-26). Replaces the plan in
[GLOBAL_ANALYSIS_ROADMAP.md](../development/GLOBAL_ANALYSIS_ROADMAP.md) and the UI plans
that copy it. Reviews open PR #154.

## Goal

Let a user pick an indicator and a date, see it for every country on a map, select a few
countries and compare them over time, and see how they move together. Every number shown
comes from a crawled source. Nothing on the page is sample data.

The older plan aimed for "Bloomberg Terminal-level" analysis in 16 weekly phases: news-fed
events, machine-learning impact prediction, real-time updates, collaboration, a REST API,
Redis and a mobile mode. This roadmap drops most of that (see [Not planned](#not-planned)).
None of it is useful before the map shows real data, and today it shows none.

## Where things stand today (main at `140fadf`)

| Area | State |
|---|---|
| Tables | `countries`, `global_economic_indicators`, `global_indicator_data`, `country_correlations`, `trade_relationships`, `global_economic_events`, `event_country_impacts`, `leading_indicators` (consolidated initial migration). Only 20 countries and 5 events are seeded. No crawler writes to any of them; the only write is the mocked correlations described in the next row. |
| Service | `econ-graph-services/src/services/global_analysis_service.rs`. `calculate_pairwise_correlation` returns a hard-coded 0.75 with p = 0.01 for every pair, and `calculate_country_correlations` stores those values. `get_correlation_network` filters `indicator_category` against itself (the parameter is `_indicator_category`), and `BigDecimal::from(min_correlation as i64)` truncates a 0.3 threshold to 0. `get_countries_with_economic_data` runs several queries per country. |
| GraphQL | `GlobalAnalysisQuery` (`econ-graph-graphql/src/graphql/global_analysis.rs`) is never merged into the root `Query`, so nothing can call it. `calculateCountryCorrelations` writes to the database but is a query. `CountryType` returns `None` for GDP and currency and `Utc::now()` for timestamps. |
| Frontend queries | `frontend/src/utils/graphql.ts` defines three global queries that no component uses. Their argument types and field shapes don't match the backend's (flat edge lists versus nested nodes, `Int` versus `Float`, `isoAlpha2` versus `isoCode2`). |
| Frontend | `/global` has three tabs. The map (`pages/GlobalAnalysisDemo.tsx` with `InteractiveWorldMap`, `useWorldMap`, `useCountryData`, D3) draws `data/sampleCountryData.ts`. `MultiCountryDashboard` (Chart.js) and `GlobalEventsExplorer` use sample arrays defined in the component. `GlobalEconomicNetworkMap` is imported nowhere. The world outline is fetched at runtime from `cdn.jsdelivr.net`. |
| Sources | The World Bank and IMF adapters only discover; `fetch_series` returns an error. The OECD, ECB, central bank (BOC, BOE, BOJ, RBA, SNB), ILO, UN and WTO catalogs are hard-coded lists that can't fetch. No adapter knows which country a series belongs to. See the data source docs added by PR #186 (`docs/data-sources/`). |
| Tests | 11 backend service tests, one of which passes only because of the mocked 0.75. Five Playwright specs (`frontend/tests/e2e/global-analysis/`) that run nightly and on manual dispatch, not on PRs. No unit tests for `components/global`. |

In short, the page is a D3 demo on sample data whose world outline never loads (decision 7),
and the backend beneath it is unreachable and partly fake.

## PR #154

PR #154 ("Enhance Global Analysis UI", 7,780 lines, 30 files, opened 2025-10-04) adds an
`EnhancedInteractiveWorldMap` next to the existing map, with map controls, a color legend,
a country selection panel, a statistics panel, accessibility, performance and
error-handling utilities, 1,000 lines of component docs, tests, and GraphQL response
fixtures.

Assessment:

- **It doesn't change what users see.** The new map is used only by
  `examples/GlobalAnalysisExample.tsx`. `/global` still renders the old map.
- **It still runs on sample data.** Its fixtures mock the three frontend queries, which
  don't match the backend and which the backend doesn't serve.
- **It duplicates code.** A second map component, a second legend (`ColorLegend` next to
  `MapLegend`) and a second controls panel (`MapControls` next to `WorldMapControls`).
- **It is stale.** Its base is a year old, and its last CI run failed Quality Checks.
- **It is worth mining.** The hook tests (`useCountryData`, `useWorldMap`),
  `test-utils/d3-testing-utils.ts`, the keyboard and ARIA handling, and the
  `useWorldMap` fix (a fresh projection object per call instead of mutating a shared
  one) are useful. The rest is generic scaffolding.

| Option | Cost | Result |
|---|---|---|
| Rebase and merge | Resolve a year of drift, fix CI, review 7,800 lines | Two maps, still on sample data |
| **Close it and port the useful parts into the phase 3 PRs (recommended)** | A few small PRs that each touch the one map | One map, tested, on real data |
| Leave it open | None | Keeps misleading anyone who reads the PR list |

## Decisions

Each decision lists the options and a recommendation. Joe decides.

### 1. Where cross-country data lives

The global tables are a second time series store beside `economic_series` and
`data_points`, with their own indicator catalog and their own observation table. The
[federation roadmap](https://linear.app/econgraph/document/federation-roadmap-design-record-from-pr-178-a1ebb66c538d) models data as **datasets** with SDMX-style
dimensions, and the World Bank, IMF and OECD all publish in that shape: a GDP value is one
cell of a dataset keyed by indicator and country.

| Option | For | Against |
|---|---|---|
| A. Keep the global tables and write a loader for them | Fastest way to fill the current schema | Two stores for the same kind of data; federation step 7 would then have to migrate them; no revisions, flags or `asOf` |
| **B. Country is a dimension of a dataset (recommended)** | One store, one crawler path, one read API; revisions, `asOf` and Iceberg come for free; the same map works for US states | Waits on the dataset columns in `economic_series` |
| C. Put country on each series with a naming convention (as the World Bank adapter's "{indicator} for {country}" titles do) | No schema change | Parsing names to find a country; no way to ask "this indicator for every country" |

With option B, "global analysis" stops being a subsystem. It becomes a set of queries over
any dataset with a geographic dimension. `global_economic_indicators`,
`global_indicator_data`, `country_correlations` and `leading_indicators` are then deleted,
not migrated. This replaces federation's step 7 ("bring `global_indicator_data` onto the
same format").

B does not need Iceberg first. The dataset metadata (the `datasets` table and the
`economic_series` dataset and dimension columns) is Postgres-only, so it can land before
the storage work in federation phases 3 to 5. The observations go wherever
`TimeSeriesStore` puts them at the time.

### 2. Country reference data

The map needs ISO 3166 alpha-2, alpha-3 and numeric codes (the world outline is keyed by
numeric code), a name, a region and an income group. Today it is 20 rows in a migration.

Recommendation: a shared reference data file (`countries.csv` in `econ-graph-core`'s data
directory, added by MAP-2, #205) loaded at runtime, like the state list from PR #175. It is built from ISO 3166,
with the World Bank country API (`/v2/country?format=json&per_page=400`) filling regions and
income groups. That API also marks the aggregates ("World", "Euro area", "High income" and so on) with the region
`Aggregates` (from public docs; the cloud environment can't reach the API to confirm).
Aggregates are valid series but never map cells.

The file is the one place any source's area code is resolved, so it is shared with the
SDMX sources in the data source coverage roadmap (PR #192) rather than duplicated. Each
row is an area with a kind (`country` or `aggregate`), and its codes are columns: ISO
alpha-2, alpha-3 and numeric, the World Bank code, and the SDMX `REF_AREA` code where it
differs (for example `U2` for the euro area in ECB data). Aggregates get their own rows
with no ISO codes, keyed by their World Bank code (`EMU` for the euro area, `EUU` for the
EU, `WLD`). An adapter maps its source's code to the row's key (alpha-3 for a country)
when it writes a series. The loader lives in `econ-graph-core`, so the crawler and
GraphQL read the same file, and GraphQL serves country names and codes from it. The
`countries` table is not needed as a lookup; it is dropped with the other global tables
(phase 9). Its `population` and `gdp_usd` columns were never a good fit anyway: population
and GDP are indicators, not attributes of a country.

Known mismatches to handle in the file, not in code: Kosovo (`XKX` in the World Bank, no
ISO code), Taiwan (absent from the World Bank, present in the IMF), and the small states
the 1:110m outline leaves out (drawn as dots or with the 1:50m outline).

### 3. The read API for the map

What the map needs is a cross-section: one measure of one dataset, for every value of one
dimension, at one date.

| Option | Shape |
|---|---|
| A. Register `GlobalAnalysisQuery` as it is | Fake correlations, a write disguised as a query, filter bugs, a country type with invented fields |
| B. Fix `GlobalAnalysisQuery` and register it | Keeps a bespoke `countriesWithEconomicData` that returns GDP, unemployment and inflation only |
| **C. A generic `crossSection` query (recommended)** | `crossSection(datasetId, measure, filter: {indicator: "NY.GDP.PCAP.CD"}, across: "area", date, latest, asOf)` (a fixed `date` or `latest: true`) returns `[{key, area, seriesId, date, value, flags}]` as plain JSON. `key` is the `across` dimension's value (so US states work too); `area` adds names and ISO codes when that dimension uses the countries list |

The roadmap index (PR #179) lists registering `GlobalAnalysisQuery` as step one. This
roadmap disagrees: registering it exposes the fake correlations to the frontend and to
MCP. Recommendation: C, then delete `GlobalAnalysisQuery`, the service and its tests.
`latest: true` means each country's most recent non-null value, with its own date,
because annual data for the latest year is often missing for half the countries. The
legend says so.

`crossSection` is also what an MCP tool for "compare this across countries" needs.

### 4. Correlations

Today's design precomputes a correlation for every pair of countries per indicator
category and stores it. Apart from being mocked, that has two problems:

- **Correlating levels is misleading.** Most macro series trend, so almost any two
  countries' GDP levels correlate above 0.9. Correlation has to be on changes (growth
  rates or differences), with a minimum overlap, and p-values need a
  multiple-comparison correction when showing a matrix of hundreds of pairs.
- **Precomputing is unnecessary.** Annual WDI series are a few dozen points. A
  30 by 30 correlation matrix on them is microseconds of work.

| Option | For | Against |
|---|---|---|
| A. Precompute pairs into `country_correlations` | Fast reads | Stale after every crawl; one fixed transform and window; a table that grows as countries squared times indicators |
| **B. Compute on request in the backend (recommended)** | Always current; the user picks transform and window; same result for UI and MCP | Needs a cap on countries per request |
| C. Compute in the browser | No backend work | MCP and the API get a different answer, or none |

Recommendation: B, as a `correlationMatrix(seriesIds, transform, start, end)` query using the
existing series transformations (`DataTransformationType`), capped at around 50 series. Drop `country_correlations`. A
network view (`GlobalEconomicNetworkMap`) can come later on top of the same query, and
until then it is deleted rather than kept as orphaned code.

### 5. Global events

The old plan filled events from news APIs and predicted their impact with machine
learning. The tables hold an "impact magnitude" and "confidence" per country with no stated
method.

| Option | For | Against |
|---|---|---|
| A. News API ingestion | Automatic | Noisy; licensing; still no impact method |
| **B. A short curated list as reference data, shown as chart annotations (recommended)** | Honest; a few dozen well-known events cover the use case | Maintained by hand |
| C. Drop events | Least work | Loses useful context on charts |

Recommendation: B. Events are a reference data file (name, dates, type, affected
countries, source link) in the data plane. The frontend shows them as shaded ranges on
charts, the same way chart annotations are drawn. The `global_economic_events` and `event_country_impacts` tables are dropped. If
impact is ever shown, it is computed from the data (change over a window against the prior
trend) and labeled as that calculation, never stored as an editorial number.

### 6. Trade

Bilateral trade is a dataset with two country dimensions (reporter, partner), so it fits
decision 1 without a special table. Candidate sources are the IMF's International Trade
in Goods (the successor to Direction of Trade Statistics), UN Comtrade (API key, strict
rate limits) and the WTO. Recommendation: defer until the IMF adapter is rebuilt on the
IMF's current SDMX API, then use IMF trade data. Drop `trade_relationships` with the other
tables.

### 7. The world outline

The map fetches `world-atlas@3/world/110m.json` from jsDelivr on every visit. That version
was never published (the latest `world-atlas` is 2.x), so the request fails and the map has
never rendered on `main` (found by MAP-3). A CDN fetch would also fail offline and in tests,
need a CSP exception, and tell a third party who uses the site.
Recommendation: add `world-atlas` as a dependency and import the JSON so it is bundled (about
100 KB, cacheable).

### 8. Which data first

Recommendation: the World Bank's World Development Indicators, but a curated set of about
50 indicators (GDP, GDP per capita, growth, inflation, unemployment, population, debt,
current account, trade share and so on), not all ~1,400. That is about 11,000 series,
fetched with one request per indicator (`/country/all/indicator/{id}?per_page=20000`). It
exercises the dataset model and the map without the full 300,000 series "many small files"
case, which can follow once federation's storage is measured. IMF World Economic Outlook
comes second because it adds forecasts and editions (vintages).

## Phases

Each numbered item is one small PR, stacked where it depends on the one before.

1. **Stop showing fake numbers.** Delete `calculate_pairwise_correlation` and
   `calculateCountryCorrelations`, and the test that relies on the mock. No dependencies.
2. **Country reference data.** The countries file and its loader in `econ-graph-core`,
   built from ISO 3166 with the World Bank country list filling regions and income groups.
3. **Datasets metadata, early.** The `datasets` table and the `economic_series` dataset
   and dimension columns from federation phase 3 (Postgres only, no storage change). Owned
   by the federation roadmap; this roadmap needs it early.
4. **World Bank fetch.** Implement `fetch_series` for the curated WDI set, writing one
   dataset `wdi` with dimensions `indicator` (inline codes with a unit each) and `area`
   (the `countries` codelist, keyed by the reference file's `key`); measure `value`;
   attributes `obs_status`, `decimal`. Vintages wait for train 3.
5. **`crossSection` query** with `latest`, and tests on a seeded dataset. The filter pins
   every dimension other than `across`. `asOf` follows once the SQL `asOf` work (#184) merges.
   Indicator names and units come from the datasets GraphQL API (`DatasetDimension.codes`).
   `GlobalAnalysisQuery` stays unregistered and goes with the legacy tables in phase 9.
6. **Map on real data.** Point the map at `crossSection`; bundle `world-atlas`; delete
   `sampleCountryData.ts`, the sample wiring in `GlobalAnalysisDemo.tsx`, the orphaned
   `GlobalEconomicNetworkMap` and the unused frontend queries; port the hook tests from PR #154
   and close it; show each value's date in the tooltip. The comparison and events tabs are
   compiled out behind the build flag `global_analysis_tabs` until phases 7 and 10.
7. **Country comparison.** Replace `MultiCountryDashboard`'s sample data with real series
   on Chart.js, selected from the map.
8. **Correlation matrix** on request (decision 4), shown as a heat map beside the
   comparison.
9. **Drop the legacy tables**: `countries`, `global_economic_indicators`, `global_indicator_data`,
   `country_correlations`, `trade_relationships`, `event_country_impacts`,
   `leading_indicators`, and the global service. `global_economic_events` goes with phase 10,
   once the events reference file replaces it.
10. **Events** as reference data and chart annotations (decision 5).
11. **Later.** US state maps for Census BDS (PR #175) on the same `crossSection` query
    with a US states outline; IMF WEO; trade (decision 6); a correlation network.

Phases 1 and 2 can start now. Phase 4 onward depends on phase 3.

## Release 1 (train 1)

Train 1 ([releases.md](./releases.md)) ships phases 1, 2, 5 and 6, with phase 3 and 4 from
the datasets and data areas. The PR map with branches and dependencies is kept in the
project's release-1 notes; this table is updated as the PRs move.

| Id | Phase | PR | Depends on | State |
|---|---|---|---|---|
| MAP-1 | 1: delete the fake correlations | #216 | none | In review |
| MAP-2 | 2: country reference file (`econ_graph_core::reference`) | #205 | none | In review |
| MAP-3 | 6: bundle `world-atlas` 2.x | #202 | none | In review |
| MAP-4 | 5: `crossSection` query | #239 | MAP-2, datasets table (#220) | Draft |
| MAP-5 | 6: compile out the other tabs | not open yet | build flags (#215) | Not started |
| MAP-6 | 6: map on real data, close #154 | not open yet | MAP-3, MAP-4, MAP-5, datasets GraphQL | Not started |
| MAP-7 | release spec: the map shows a value for every seeded country | not open yet | MAP-6, WDI fixture | Not started |

The datasets GraphQL API and the crawler's codelist resolution also read the countries file
through MAP-2's loader, so its `key` column stays stable.

## Not planned

- **Forecasting and machine-learning impact prediction.** Show source forecasts (IMF WEO)
  instead of producing our own.
- **Real-time updates.** The data is annual, quarterly or monthly.
- **Collaboration, sharing and comments specific to global analysis.** Charts already have
  them.
- **A separate REST API, Redis caching and materialized views.** GraphQL and MCP cover
  access; add caching when a measured query needs it.
- **A mobile-specific version.** The map should stay usable on a phone, but not as its own
  project.

## Open questions

1. Keep global events at all (decision 5, option B), or drop them (option C)? Not needed
   before train 3.

Settled for release 1 (2026-09-26): PR #154 is closed once MAP-6 ports its tests; the
`datasets` metadata lands in train 1; the data area picks the curated WDI indicators.
