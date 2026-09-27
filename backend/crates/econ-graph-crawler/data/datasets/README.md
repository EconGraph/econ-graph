# Dataset definitions

One `<source>.toml` per source, named after the lowercase source id
(`fred.toml`, `world_bank.toml`, `census.toml`). The crawler reads these files at
startup, checks them against the dataset codes each adapter declares in
`SourceAdapter::datasets()`, and upserts them into the `datasets` table.

A declared code without a definition, or a definition that no adapter declares, stops
the worker at startup. At crawl time, a series that names an undeclared dataset, or whose
dimension keys differ from the definition, fails its job before anything is written.

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
codelist = "countries"       # values from a shared reference list: countries or us_states

# Or label the codes inline (not a closed list), instead of codelist:
# codes = { national = "United States", state = "State" }

# Optional attributes (observation or series flags).
[[dataset.attributes]]
name = "obs_status"
label = "Observation status"
```

In train 1 every dataset is stored long, because `data_points` holds one value per
observation. A dataset therefore has the single measure `value`, which is also its
`default_measure`. Both are the defaults, so leave them out. A source with several
measures publishes each one as its own series, with the measure as a dimension (for
example Census BDS `variable`).

A dataset with no dimensions, such as FRED, keeps the source's own series ids. A
dataset with dimensions builds ids with `DatasetDef::external_id`, which formats the
canonical id `{code}/{v1}.{v2}` and never needs to be written by hand.
