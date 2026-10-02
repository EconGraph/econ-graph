# FHFA House Price Index

Adapter: `backend/crates/econ-graph-crawler/src/sources/fhfa.rs`. Fixtures:
`tests/fixtures/fhfa/`. No key.

## What we fetch today

- **Discovery and fetch**: both download the HPI master CSV,
  `GET https://www.fhfa.gov/hpi/download/monthly/hpi_master.csv` — one file holding every
  HPI series and period. Columns are found by header name. Discovery and every fetch batch
  download the same file; a process-local in-flight reservation keyed by the URL stops two
  workers downloading it at once (the second gets a retryable `Busy` instead of making a
  redundant request).
- **Scope (train 1)**: rows with `hpi_type` `traditional`, `hpi_flavor` `purchase-only` or
  `all-transactions`, `frequency` `monthly` or `quarterly`, and `level`
  `USA or Census Division` or `State` — the United States, the nine census divisions, and
  the states and DC. Every other row (metros, expanded-data, non-metro, distress-free, ...)
  is skipped without being parsed.
- **Series and ids**: each series is in dataset `fhfa_hpi`
  (`data/datasets/fhfa.toml`), with dimensions `hpi_type`, `hpi_flavor`, `frequency`,
  `level`, `place_id` and `seasonal_adjustment` (`nsa` for the `index_nsa` column, `sa` for
  `index_sa`) — the two value columns become separate series. Ids are canonical dataset
  ids, e.g. `fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa`. A
  series exists only when its column has at least one value (`index_sa` is empty for many
  series). The made-up `{CODE}HPI` ids (e.g. `USHPI`, `CAHPI`) are gone.
- **Kept**: `yr` + `period` (as the month's or quarter's first day), `index_nsa`,
  `index_sa`.
- **Dropped**: nothing published per observation beyond the two index columns; there are no
  derived year-over-year or quarter-over-quarter fields in the master file.

**The master CSV's layout has never been verified against a live download.** The column
names and order, the `hpi_type`/`hpi_flavor`/`frequency`/`level` values, and the division
and state `place_id`s are carried over from FHFA's public documentation, not a recording;
see `tests/fixtures/fhfa/README.md` for the exact pre-release checks planned (URL, header,
`place_id` values, which series have `index_sa`, and the base periods the adapter states as
units).

## Sample

Assumed file layout (`hpi_master.csv`, **not a recording**; values illustrative):

```csv
hpi_type,hpi_flavor,frequency,level,place_name,place_id,yr,period,index_nsa,index_sa
traditional,purchase-only,monthly,USA or Census Division,United States,USA,2024,10,203.17,203.78
traditional,purchase-only,monthly,USA or Census Division,East North Central Division,DV_ENC,2024,10,206.34,206.96
```

`index_sa` is only present for purchase-only series (every level, including states); it is
empty, not omitted, for all-transactions series.

## Frequency and revisions

Monthly (purchase-only, national and census divisions) and quarterly (all levels). The
index is a repeat-sales estimate, so every release re-estimates the whole history as new
sale pairs arrive (from public docs): each release is effectively a new vintage of every
observation. The file has no vintage column; a fetch ignores `since` and returns every
observation, so each refresh overwrites the stored values with the latest estimate
(`revision_date = date`, `is_original_release = true`, as for the other sources without
vintage history).

## Flags and footnotes

None per observation in the published file. Series with too few transactions are omitted
rather than flagged.

## Dataset mapping

One dataset for the HPI master file, already in effect (`data/datasets/fhfa.toml`):

| Role | Columns |
|---|---|
| Dimensions | `hpi_type`, `hpi_flavor`, `frequency`, `level`, `place_id`, `seasonal_adjustment` |
| Measures | `value` (decimal) |
| Attributes | none |
| Shape | long; the file's two value columns (`index_nsa`, `index_sa`) become separate series via the `seasonal_adjustment` dimension rather than a wide row; `revision_date = date` (no vintage column; each refresh overwrites) |
