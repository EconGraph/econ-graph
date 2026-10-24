# Dataset definitions

One `<source>.toml` per source, named after the lowercase source id
(`fred.toml`, `world_bank.toml`, `census.toml`). The crawler reads these files at
startup, checks them against the dataset codes each adapter declares in
`SourceAdapter::datasets()`, and upserts them into the `datasets` table.

Every series belongs to a dataset (`economic_series.dataset_id` is `NOT NULL`), so every
adapter declares at least one. An adapter that declares none, a declared code without a
definition, or a definition that no adapter declares, stops the worker at startup. At crawl
time, a series that names an undeclared dataset, or whose dimension keys differ from the
definition, fails its job before anything is written.

```toml
[[dataset]]
code = "wdi"                 # unique within the source: letters, digits, _ and -
name = "World Development Indicators"
description = "World Bank development indicators by country"

# Dimensions, in the order the canonical external id lists their values:
# wdi/{indicator}.{area}, for example wdi/NY.GDP.PCAP.CD.USA
[[dataset.dimensions]]
name = "indicator"           # lowercase snake_case, the key in a series' dimensions
label = "Indicator"
type = "string"              # string (default), integer, decimal, date or boolean

[[dataset.dimensions]]
name = "area"
label = "Country or area"
codelist = "countries"       # a shared reference list (today only countries),
                             # resolved by the API, not the crawler; the source's
                             # own labels for some of its codes (World Bank
                             # aggregate names) come from the crawl, never this file

# Or list the codes inline (not a closed list), instead of codelist:
# codes = [{ code = "national", label = "United States" }, { code = "state", label = "State" }]

# Optional attributes (observation or series flags).
[[dataset.attributes]]
name = "obs_status"
label = "Observation status"
```

A dataset's code and its dimensions' names and declared order are part of every canonical
external id built from them (see below): never rename or reorder them once series exist under
that dataset, add a new dataset instead. `sync_datasets` never deletes a `datasets` row for a
code a definitions file drops; remove the row by hand if a dataset is retired.

In train 1 every dataset is stored long, because `data_points` holds one value per
observation. A dataset therefore has the single measure `value`, which is also its
`default_measure`. Both are the defaults, so leave them out. A source with several
measures publishes each one as its own series, with the measure as a dimension (for
example Census BDS `variable`).

A source with its own series key (FRED, BLS, SDMX) keeps it as the external id and
parses the dimension values from it. A source without one (Census BDS, WDI) builds ids
with `DatasetDef::external_id`, which formats the canonical id `{code}/{v1}.{v2}` in
declared dimension order and never needs to be written by hand.
