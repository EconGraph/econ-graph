# Dataset definitions

Each source has a `<source>.toml` file named after its lowercase source id
(`fred.toml`, `world_bank.toml`, `census.toml`). At startup, the worker loads the file
for each registered adapter, checks its definitions against that adapter’s
`SourceAdapter::datasets()` declarations, and upserts them into the `datasets` table.
Files for adapters outside the registry are not loaded.

Every series belongs to a dataset (`economic_series.dataset_id` is `NOT NULL`), so every
registered adapter must declare at least one. Startup fails if a registered adapter
declares none, a declared code has no definition, or its file defines a code that the
adapter does not declare. At crawl time, a series that names an undeclared dataset,
or whose dimension keys differ from the definition, fails its job before anything is
written.

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
codelist = "countries"       # shared reference list resolved by the API

# Or list the codes inline (not a closed list), instead of codelist:
# codes = [{ code = "national", label = "United States" }, { code = "state", label = "State" }]

# Optional attributes (observation or series flags).
[[dataset.attributes]]
name = "obs_status"
label = "Observation status"
```

Use either `codes` or `codelist` for a dimension. Inline codes supply labels without
restricting the dimension to those codes. A `codelist` names a shared reference list,
resolved by the API; `countries` is the only supported list. Source-specific labels
for codes in that list, such as World Bank aggregate names, arrive through crawling
and must not be defined in this file.

Generated external IDs contain the dataset code and dimension values in declared
order. Dimension names are the keys stored in each series’ dimensions. Once series
exist, keep the dataset code, dimension names, and dimension order unchanged; define
a new dataset for a different schema. `sync_datasets` never deletes a `datasets` row
for a code a definitions file drops; remove the row by hand if a dataset is retired.

Every dataset is stored long, because `data_points` holds one value per
observation. A dataset therefore has the single measure `value`, which is also its
`default_measure`. Both are the defaults, so leave them out. A source with several
measures publishes each one as its own series, with the measure as a dimension (for
example Census BDS `variable`).

Sources with their own series keys (FRED, BLS, SDMX) keep them as external IDs.
Structured keys, such as those covered by BLS layouts, supply dimension values;
FRED’s opaque keys belong to a dataset with no dimensions. Sources without their
own keys (Census BDS, WDI) use `DatasetDef::external_id` to build IDs of the form
`{code}/{v1}.{v2}...`, with values in declared dimension order. Adapters should use
this method instead of formatting IDs by hand.
