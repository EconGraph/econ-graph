# FHFA fixtures

`hpi_master.csv` is **not a recording**. Its rows and values are made up to exercise the
[FHFA adapter](../../../src/sources/fhfa.rs). It uses the ten columns the adapter reads from
FHFA's [master CSV](https://www.fhfa.gov/hpi/download/monthly/hpi_master.csv), in the same
order. The live file also has `rstderr` and `note` columns, which the fixture omits and the
adapter ignores; the adapter finds columns by name.

The fixture contains monthly purchase-only series for the United States and the nine census
divisions. Its quarterly purchase-only and all-transactions series also cover the 50 states
and DC. Each series has a few periods. Purchase-only rows have both `index_nsa` and
`index_sa`; all-transactions rows leave `index_sa` empty. These combinations match the live
file's 203 in-scope series, counting the two index columns as separate series.

The fixture uses FHFA's `hpi_type`, `hpi_flavor`, `frequency` and `level` values, with `USA`,
`DV_*` division ids and USPS state codes as `place_id`. The `period` column holds a month or
quarter number according to `frequency`. Out-of-scope rows (metros, expanded-data, non-metro
and distress-free) exercise the adapter's filters. A metro name contains a comma to exercise
CSV quoting.

FHFA's [data dictionary](https://www.fhfa.gov/document/d/hpi/hpi_dictionary.xlsx) describes
the master columns, and its
[file specifications](https://www.fhfa.gov/document/d/hpi/hpi_specifications.xlsx) list the
division ids and quarterly index bases. The adapter's units match the live file's base
values: purchase-only January 1991 and 1991Q1 equal 100; all-transactions 1980Q1 equals 100.
The synthetic values test parsing and metadata; they do not reproduce FHFA observations.
