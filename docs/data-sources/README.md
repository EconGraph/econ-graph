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
state fixture and FHFA's master CSV).

## Sources

| Source | File | Adapter today | Sample |
|---|---|---|---|
| FRED | [fred.md](fred.md) | Discovery (curated list) and fetch, with ALFRED vintages | Metadata recorded, observations from public docs |
| BLS | [bls.md](bls.md) | Discovery (data file) and fetch | Recorded |
| Census BDS | [census-bds.md](census-bds.md) | Discovery and fetch (national and per state) | Recorded |
| BEA | [bea.md](bea.md) | Discovery (curated tables) and fetch | Catalog recorded, data from public docs |
| FHFA HPI | [fhfa.md](fhfa.md) | Discovery and fetch from the HPI master CSV | File layout from public docs, not a recording |
| World Bank | [world-bank.md](world-bank.md) | Discovery (curated indicators) and fetch | Recorded |
| IMF | [imf.md](imf.md) | No adapter (removed in #217) | n/a |
| SEC XBRL | [sec-xbrl.md](sec-xbrl.md) | Separate crawler (`econ-graph-sec-crawler`) | From public docs |

Each page describes the adapter as it ships in release 1 (release/v4.0). Status (draft,
merged, blocked) is tracked in Linear (linear.app/econgraph, prefix ECO), not here.

The other ten sources in `SourceId` (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS, WTO)
are static catalogs (`static_catalogs.rs`): hard-coded series lists, no HTTP, and
`fetch_series` always fails. They have no data shape to document yet, and are gated off in
release builds by the `static_catalogs` build flag (dev builds only). ECB, OECD, ILO and
the UN publish SDMX, so they would look like [imf.md](imf.md).

## Findings that matter for the data model

1. **Most adapters still don't keep vintages.** Every adapter that fetches sets
   `revision_date = date` and `is_original_release = true`, so a re-crawl overwrites values
   in place (`persist.rs` upserts on `(series_id, date, revision_date,
   is_original_release)`), **except FRED and World Bank WDI**, which now store a real
   vintage date (`revision_date = realtime_start` / `lastupdated`). Under the federation
   model a revision is an append keyed by `(series_id, date, revision_date)`; with
   `revision_date = date`, a revised value hits the same key and becomes a correction
   (row-level delete plus new row), and the history is lost. The remaining adapters need a
   real vintage date too: the crawl date or the source's release date.
2. **Several sources revise their whole history at each release**: Census BDS (annual
   release), FHFA HPI (every release), BEA (annual and comprehensive updates), and the
   World Bank WDI (several updates a year). WDI now appends a new vintage of every
   observation on each such release; the others still overwrite (see finding 1).
3. **Flags are mostly still dropped.** BLS footnotes (`P` preliminary, `X` unavailable) are
   now parsed and logged, but not yet stored as a column (`data_points` has no attribute
   columns in train 1). World Bank `obs_status`, BEA `NoteRef` and SDMX `OBS_STATUS` are
   still not parsed. FRED has no per-observation flags (missing is `"."`). These become
   attributes.
4. **Dimensions are now mostly real columns.** Census BDS (`geo_level`, `state`,
   `variable`), BLS (per-survey layouts for CU/CE/LN/LA, e.g. `CUUR0000SA0` splits into
   survey `CU`, seasonal `U`, area `0000`, item `SA0`), BEA, FHFA and World Bank all carry
   real dimension columns via the dataset contract (`data/datasets/*.toml`), parsed out of
   the external id or the source's own fields. FRED stays dimensionless (a FRED series id is
   opaque, not a combination of dimension values); any series outside a known layout still
   falls back to a dimensionless `other` dataset.
5. **Catalogs are curated, not hard-coded ids.** FRED, BLS, BEA and World Bank discovery now
   reads a data file of real ids (FRED series ids, BLS series ids, BEA table/line codes, WDI
   indicator codes) instead of calling a listing endpoint and expanding it to made-up ids.
   FHFA downloads the source's own master file instead of a curated id list. IMF is deleted;
   its ids were made up and never mapped to a real API.
6. **Wide vs long falls out per source.** Census BDS and FHFA HPI publish a handful of
   measures together (wide). World Bank WDI has about 50 curated indicators (long:
   indicator is a dimension, one value column). FRED and BLS are single value series; a
   FRED "dataset" is best a release or a category, not the whole of FRED.

## Release 1 changes

Delivered by the data area for release 1 (release/v4.0); each page above describes the
adapter as it ships. Status going forward is tracked in Linear (linear.app/econgraph,
prefix ECO), not here.

| Change | Source | PR |
|---|---|---|
| The ten static catalogs are gated off release builds by the `static_catalogs` build flag (dev builds only). The IMF adapter is deleted (its ids were made up); `SourceId::Imf` stays for a later SDMX adapter | Static catalogs, IMF | #217, #311 |
| A Census key is required, and a missing key fails before any request | Census BDS | merged |
| FRED discovery reads a curated list of series ids and records ALFRED vintages: `revision_date = realtime_start` | FRED | #214 |
| The series list moved to a data file of 291 series (no discovery request), fetched 50 per request (25 keyless); footnotes are parsed and logged but not yet stored | BLS | #235 |
| The unverified API client was replaced by the HPI master CSV, with the file's columns as dimensions (dataset `fhfa_hpi`) | FHFA | #240 |
| Real NIPA table and line ids, plus regional GDP by state; discovery and fetch both implemented | BEA | DATA-8 |
| About 50 WDI indicators fetched for all countries: dataset `wdi` with dimensions `area` and `indicator`, `revision_date = lastupdated` | World Bank | DATA-9 |
| The scheduler fetches series that were discovered but never fetched | All | #228 |

Together these address finding 1 above for FRED and WDI, the two sources that now publish a
revision date (crawl-date vintages for the other sources come later), and finding 5 for
FRED, BLS, BEA and World Bank (curated id lists) and FHFA (a real master file instead of
made-up ids).

## Related notes

These older notes are kept where they are and linked, not duplicated:

- [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md)
- [Census BDS integration](../technical/CENSUS_BDS_INTEGRATION.md) and
  [Census Bureau integration summary](../technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md)
- [World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
  and [World Bank integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md)
- [SEC EDGAR XBRL implementation plan](../archive/development/SEC_EDGAR_XBRL_IMPLEMENTATION_PLAN.md)
- [Rate limits by source](../technical/RATE_LIMIT_SOURCES.md)
