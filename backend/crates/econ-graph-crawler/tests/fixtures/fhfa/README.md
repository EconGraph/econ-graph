# FHFA fixtures

`hpi_master.csv` is **not a recording**. It was built from the layout FHFA documents for its
HPI master file (`HPI_master.csv`): the column names and their order, the `hpi_type`,
`hpi_flavor`, `frequency` and `level` values, `USA`, `DV_*` division ids and USPS state codes as
`place_id`, `period` as a month or a quarter, and `index_sa` left empty where FHFA publishes no
seasonally adjusted index. The values are made up.

It holds every train 1 series (purchase-only monthly and quarterly, all-transactions
quarterly, for the United States, the nine census divisions and the states and DC) with a few
periods each, plus out-of-scope rows (metros, expanded-data, non-metro, distress-free) that the
adapter must skip. A metro name contains a comma, so the CSV quoting is exercised.

Before release, QA checks the live file against it: the URL
(`https://www.fhfa.gov/hpi/download/monthly/hpi_master.csv`), the header, the division
`place_id` values, which series have `index_sa`, and the base periods the adapter states as units
(purchase-only January 1991 and 1991Q1, all-transactions 1980Q1). Replace this file with a trimmed recording
when network access allows.
