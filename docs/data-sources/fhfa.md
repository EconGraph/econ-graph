# FHFA House Price Index

The [FHFA adapter](../../backend/crates/econ-graph-crawler/src/sources/fhfa.rs) reads the
public HPI master CSV without an API key. Its [test fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/fhfa/README.md)
contain synthetic values; they are separate from the downloaded source rows shown below.

## Discovery and fetching

Discovery and series fetching use the same [HPI master CSV](https://www.fhfa.gov/hpi/download/monthly/hpi_master.csv),
which combines monthly and quarterly indexes. One read serves a whole fetch batch, and the
batch limit covers every series the adapter ingests. Columns are found by header name.
Reference-data refresh reads the file separately to label `place_id` codes from
`place_name`, using its own ETag cache.

Discovery and fetching use conditional requests with stored `ETag` or `Last-Modified`
values when available, with a SHA-256 body hash as a fallback. Without usable HTTP
validators, an unchanged check can still download the body, but the matching hash avoids
parsing it again. Discovery tracks the file under its own validator key; an unchanged
result leaves the catalog intact. A fetch batch uses stored validators only when every
member was fetched from the same file with the current parser version. An unchanged
result marks the series crawled and preserves its points. A new series, differing stored
validators, or an obsolete parser version requires a full read.

A process-local in-flight reservation keyed by the master URL prevents discovery and
fetch batches in the same process from downloading it concurrently. A second worker
receives retryable `Busy` instead of making a request. The reservation does not coordinate
separate worker processes or cover the reference-data request.

## Scope and series

The adapter selects rows with `hpi_type` equal to `traditional`, `hpi_flavor` equal to
`purchase-only` or `all-transactions`, `frequency` equal to `monthly` or `quarterly`, and
`level` equal to `USA or Census Division` or `State`. These cover the United States, the
nine census divisions, and the states and DC. Other rows, including metros, expanded-data,
non-metro and distress-free indexes, are skipped before their dates and values are parsed.

Selected rows supply five dimensions: `hpi_type`, `hpi_flavor`, `frequency`, `level` and
`place_id`. Each populated index column becomes a separate series through the sixth
dimension, `seasonal_adjustment`: `nsa` for `index_nsa` and `sa` for `index_sa`. A series
exists only when its column has at least one value. The adapter lowercases `level` and
replaces spaces with hyphens for the canonical dataset ID, for example
`fhfa_hpi/traditional.purchase-only.monthly.usa-or-census-division.USA.sa`.

Monthly coverage is purchase-only for the United States and census divisions. Quarterly
coverage includes both flavors at all supported geographic levels. Within this scope,
`index_sa` is populated only for purchase-only indexes, including state indexes; the
column remains present but empty for all-transactions rows.

## Source layout and sample

The downloaded master CSV has 12 columns. Checking its rows confirmed the selected
categories, the `USA` and nine `DV_*` division codes, the 51 state and DC postal codes, and
the seasonal-adjustment coverage described above. FHFA's [datasets page](https://www.fhfa.gov/data/hpi/datasets)
links the master file, its [dictionary](https://www.fhfa.gov/document/d/hpi/hpi_dictionary.xlsx)
and the [download specifications](https://www.fhfa.gov/document/d/hpi/hpi_specifications.xlsx).

These two rows are reproduced from the downloaded master CSV, with all columns retained:

```csv
hpi_type,hpi_flavor,frequency,level,place_name,place_id,yr,period,index_nsa,index_sa,rstderr,note
traditional,purchase-only,monthly,USA or Census Division,United States,USA,2024,10,429.51,427.19,,
traditional,purchase-only,monthly,USA or Census Division,East North Central Division,DV_ENC,2024,10,357.88,355.61,,
```

The master file contains no derived year-over-year or quarter-over-quarter measures.
Its `rstderr` and `note` columns carry standard errors and notes on some rows outside the
adapter's scope, including notes about insufficient records and suppressed indexes.
Both columns were empty in every in-scope row of the checked download. The adapter does
not retain either column as observation attributes.

## Dates, units and revisions

The adapter combines `yr` and `period` into the first day of the observation's month or
quarter. Purchase-only indexes use January 1991 = 100 for monthly series and 1991Q1 = 100
for quarterly series; all-transactions indexes use 1980Q1 = 100. The FHFA monthly and
quarterly workbooks linked from the datasets page confirm these base periods.

FHFA's repeat-sales estimates can change throughout the history as new sale pairs arrive.
The master file has no vintage column. When a file needs to be parsed, the adapter ignores
`since` and returns each requested series' complete history. It stores points with
`revision_date = date` and `is_original_release = true`, so changed estimates overwrite
stored values under the same revision key. Earlier release vintages are not retained.
An unchanged-file result preserves the existing values, as described above.

## Dataset mapping

All series belong to [`fhfa_hpi`](../../backend/crates/econ-graph-crawler/data/datasets/fhfa.toml).
The dataset stores one decimal value per observation, with the two source index columns
separated by `seasonal_adjustment`.

| Role | Mapping |
|---|---|
| Dimensions | `hpi_type`, `hpi_flavor`, `frequency`, `level`, `place_id`, `seasonal_adjustment` |
| Measure | `value` (decimal), from `index_nsa` or `index_sa` |
| Observation date | `yr` and `period`, mapped to the month's or quarter's first day |
| Attributes | None |
| Shape | Long; one value per series and observation date |
