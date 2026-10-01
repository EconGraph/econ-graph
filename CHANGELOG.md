# Changelog

All notable changes to EconGraph are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Releases are tagged `vX.Y.Z`; see
[the release process](docs/development/RELEASE_PROCESS.md) for how a train reaches a tag.

## [4.0.0] - Unreleased

Train 1: broad real data behind pages that work. See
[the train 1 roadmap](docs/roadmap/releases.md) for the full scope and what was deliberately
left for later trains.

### Added

- Real data end to end for the core loop: search a series, open it and read a correct, current
  chart, with real revisions and vintages (FRED/ALFRED and World Bank WDI).
- Six data sources crawl real data: FRED, BLS (CPI, CES and LAUS), Census BDS, FHFA HPI (rebuilt
  on the published master CSV), BEA (real NIPA table and line IDs) and World Bank WDI (about 50
  curated indicators).
- A world map showing the latest value of a chosen indicator for every country the source
  reports it for.
- Sign-in through Keycloak, with public and private annotations on a series chart.
- A real dashboard and data-sources page, replacing hard-coded sample values.
- Datasets metadata (country, indicator and table dimensions) for the new multi-dimensional
  sources.

### Changed

- The ten static development-only catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats,
  WTO) and IMF discovery are excluded from release and production builds; search in those
  builds never returns a series that can't have data.

### Removed

- `/analysis`, `ProfessionalChart` and `ChartCollaboration`, which ran on mock data and a
  made-up chart ID.
- `imf.rs` and other mock/hard-coded data paths (fake correlations, the dashboard's hard-coded
  indicator cards, `SeriesDetail`'s generated mock points).
