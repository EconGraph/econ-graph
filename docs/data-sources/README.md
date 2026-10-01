# Data sources

What the data from each source looks like, to inform the federation data model: one
Iceberg table per dataset, with SDMX-style dimensions, measures and attributes.

Each file covers:

- what the crawler adapter fetches today;
- a trimmed sample payload;
- frequency, and how revisions and vintages show up;
- the flags and footnotes the source provides, and where our adapter drops them;
- a proposed dataset mapping (dimensions, measures, attributes; wide or long).

Written from the adapters in `backend/crates/econ-graph-crawler/src/sources/`, their test
fixtures in `backend/crates/econ-graph-crawler/tests/fixtures/`, and public API
documentation. The live APIs could not be called from the environment these notes were
written in (its network policy blocks them); two calls that did get through are noted
where they matter. Samples marked **recorded** come from the
fixtures (real responses, trimmed); samples marked **from public docs** show the documented
shape of an endpoint we have never recorded, and should be checked against a live response
before building on them. Values in those samples are illustrative. "Recorded" means the
fixture has the real shape of a response; some fixture values are themselves
placeholders rather than genuine recordings (noted where that's true, e.g. Census BDS'
state fixture and one World Bank row).

## Sources

| Source | File | Adapter today | Sample |
|---|---|---|---|
| FRED | [fred.md](fred.md) | Discovery and fetch | Recorded |
| BLS | [bls.md](bls.md) | Discovery (hard-coded list) and fetch | Recorded |
| Census BDS | [census-bds.md](census-bds.md) | Discovery and fetch (national and per state) | Recorded |
| BEA | [bea.md](bea.md) | Discovery only (hard-coded list) | Catalog recorded, data from public docs |
| FHFA HPI | [fhfa.md](fhfa.md) | Static catalog and fetch against an unverified API | Fixture shape unverified; files from public docs |
| World Bank | [world-bank.md](world-bank.md) | Discovery only | Recorded (one data row) |
| IMF | [imf.md](imf.md) | No adapter (removed in #217) | n/a |
| SEC XBRL | [sec-xbrl.md](sec-xbrl.md) | Separate crawler (`econ-graph-sec-crawler`) | From public docs |

Each page describes the adapter on `main`. Release 1 changes that are still in open pull
requests are listed below and in a short "Release 1" note at the top of each page.

The other ten sources in `SourceId` (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO)
are static catalogs (`static_catalogs.rs`): hard-coded series lists, no HTTP, and
`fetch_series` always fails. They have no data shape to document yet, and release 1
keeps them out of release builds (#217). ECB, OECD, ILO and
the UN publish SDMX, so they would look like [imf.md](imf.md).

## Findings that matter for the data model

1. **No adapter keeps vintages.** Every adapter that fetches sets
   `revision_date = date` and `is_original_release = true`, so a re-crawl overwrites values
   in place (`persist.rs` upserts on `(series_id, date, revision_date,
   is_original_release)`). Under the federation model a revision is an append keyed by
   `(series_id, date, revision_date)`; with `revision_date = date`, a revised value hits the
   same key and becomes a correction (row-level delete plus new row), and the history is
   lost. Adapters need a real vintage date: FRED gives one (`realtime_start`); for the
   others it has to be the crawl date or the source's release date.
2. **Several sources revise their whole history at each release**: Census BDS (annual
   release), FHFA HPI (every quarter), BEA (annual and comprehensive updates), and the
   World Bank WDI (several updates a year). With real vintages, each such release appends a
   new vintage of every observation, not a few new rows.
3. **Flags are dropped everywhere.** BLS footnotes (`P` preliminary, `X` unavailable) are not
   even deserialized; World Bank `obs_status`, BEA `NoteRef` and SDMX `OBS_STATUS` are not
   parsed. FRED has no per-observation flags (missing is `"."`). These become attributes.
4. **Dimensions are hidden in external ids.** `CENSUS_BDS_ESTAB_state_06`, `CAHPI`, and BLS
   ids such as `CUUR0000SA0` (survey `CU`, not seasonally adjusted, area `0000`, item `SA0`)
   all encode dimensions in a string. The dataset model makes them columns.
5. **Most catalogs are hard-coded.** BLS, BEA and IMF discovery calls a listing endpoint
   and then expands it to a fixed handful of series; the BEA and IMF ids
   (`NIPA_GDP_TOTAL`, `IFS_US_PCPI_IX`) are our own names and do not map to anything the
   API accepts. FHFA's endpoint has never been verified against a live server.
6. **Wide vs long falls out per source.** Census BDS and FHFA HPI publish a handful of
   measures together (wide). World Bank WDI and IMF IFS have hundreds to thousands of
   indicators (long: indicator is a dimension, one value column). FRED and BLS are single
   value series; a FRED "dataset" is best a release or a category, not the whole of FRED.

## Release 1 changes in progress

Planned by the data area. None of these has merged yet; each page is updated as its PR
lands. Current status (draft, merged, blocked) is tracked in Linear (linear.app/econgraph,
prefix ECO), not here.

| Change | Source | PR |
|---|---|---|
| The ten static catalogs are registered only with the `static-catalogs` cargo feature, so they are absent from release builds (later replaced by the `static_catalogs` build flag, #311). The IMF adapter is deleted (its ids were made up); `SourceId::Imf` stays for a later SDMX adapter | Static catalogs, IMF | #217 |
| A Census key is required, and a missing key fails before any request | Census BDS | #207 |
| FRED records ALFRED vintages: `revision_date = realtime_start` | FRED | #214 |
| The series list moves to a data file of 291 series (no discovery request), fetched 50 per request (25 keyless), footnotes parsed and logged but not stored | BLS | #235 |
| The unverified API client is replaced by the HPI master CSV, with the file's columns as dimensions (dataset `fhfa_hpi`, 203 series) | FHFA | #240 |
| Real NIPA table and line ids, plus regional GDP by state | BEA | DATA-8 |
| About 50 WDI indicators fetched for all countries: dataset `wdi` with dimensions `area` and `indicator`, `revision_date = lastupdated` | World Bank | DATA-9 |
| The scheduler fetches series that were discovered but never fetched | All | #228 |

Together these address finding 1 above for FRED and WDI, the two sources that publish a
revision date (crawl-date vintages for the other sources come later), and finding 5 for
BLS, FHFA and BEA.

## Related notes

These older notes are kept where they are and linked, not duplicated:

- [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md)
- [Census BDS integration](../technical/CENSUS_BDS_INTEGRATION.md) and
  [Census Bureau integration summary](../technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md)
- [World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
  and [World Bank integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md)
- [SEC EDGAR XBRL implementation plan](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md)
- [Rate limits by source](../technical/RATE_LIMIT_SOURCES.md)
