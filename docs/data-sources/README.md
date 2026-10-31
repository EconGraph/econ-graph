# Data sources

These guides compare the data publishers return with what EconGraph discovers, fetches
and stores. Each source page covers response samples, frequency and revisions, retained
and dropped flags, and the current dataset mapping. The eight main-crawler adapters store
series and observations in Postgres; SEC ingestion stores company profiles and raw
filings separately.

## Sources and sample evidence

Samples have different origins. A **fixture excerpt** shows checked-in test input;
its existence alone does not establish that it was captured from a live response.
**Synthetic examples** use invented values, sometimes in a documentation-derived response
shape. **Downloaded source excerpts** reproduce rows from a public response or file.
The source pages identify the particular files and endpoints behind their samples.

| Source | Guide | Discovery and fetching | Sample evidence |
|---|---|---|---|
| FRED | [FRED](fred.md) | Curated series IDs enriched with live metadata; observations with ALFRED vintages | Metadata fixture excerpt; synthetic observations based on API documentation |
| BLS | [BLS](bls.md) | Curated series IDs described by cached BLS survey files; batched API fetches | Observation and catalog fixture excerpts |
| Census BDS | [Census BDS](census-bds.md) | API variable and geography discovery; national and state series fetched separately | National and metadata fixture excerpts; synthetic state counts |
| BEA | [BEA](bea.md) | Curated tables and frequencies expanded into lines and areas through API calls; full-history fetches | Illustrative NIPA and Regional fixtures; error fixture excerpt |
| FHFA HPI | [FHFA HPI](fhfa.md) | Series enumerated and fetched from the publisher's HPI master CSV | Downloaded 12-column source excerpt; test fixture values are synthetic |
| World Bank WDI | [World Bank](world-bank.md) | Curated indicators expanded into supported countries and aggregates through API calls | Checked-in observation fixture excerpt; response fields also checked live |
| WTO Timeseries | [WTO](wto.md) | Curated indicators expanded into reporting economies through keyed API calls | Synthetic fixtures in the response shape the API portal documents |
| Bank of Canada | [Bank of Canada Valet](boc.md) | Curated Valet series names enriched with live metadata; one observations request per series | Recorded fixtures; downloaded source excerpt |
| SEC XBRL | [SEC XBRL](sec-xbrl.md) | Separate SEC filing handler fetches submissions and raw XBRL documents | Synthetic submissions fixture; downloaded companyfacts excerpt from an endpoint the crawler does not use |

The downloaded FHFA file includes `rstderr` and `note` alongside the two index columns;
these attributes are not retained. The SEC companyfacts sample illustrates a public fact
response, rather than the raw filing storage used by the crawler. Successful BEA data
examples remain illustrative, rather than live recordings.

FRED's synthetic incremental fixture assumes that the observations API clamps
`realtime_start` to the requested window's start for a value already in effect then.
That specific behavior remains unverified against an authenticated API response;
public ALFRED downloads use different interval semantics and do not establish it. See
the [FRED fixture guide](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/README.md)
for the modeled window and filtering behavior.

Eight other sources in `SourceId` (BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS)
are [static catalogs](../../backend/crates/econ-graph-crawler/src/sources/static_catalogs.rs):
fixed series lists without HTTP requests, whose `fetch_series` always fails. They are
disabled in release builds by the `static_catalogs` build flag and available in development
builds. There is no IMF adapter. IMF, BOJ, SNB and UN_STATS are also
[licence-gated](../../backend/crates/econ-graph-crawler/README.md#licence-gate): no build
crawls them unless its `crawl_<source>` flag is on.

## Publisher shapes and stored datasets

All eight main-crawler adapters use the [dataset contracts](../../backend/crates/econ-graph-crawler/data/datasets/README.md).
Every dataset is stored long: `data_points` holds one `value` per observation. Named
dimensions identify series within a dataset; their values are stored in the JSONB
`dimensions` fields on `series_metadata` and `economic_series`, with definitions in
`datasets`. They are not separate physical SQL columns for each dimension.

| Source | Publisher shape | Current dataset mapping |
|---|---|---|
| FRED | Single-valued series with opaque IDs | One dimensionless `FRED` dataset; frequency, units and seasonal adjustment stay series metadata |
| BLS | Single-valued series whose IDs encode survey-specific fields | `CU`, `CE`, `LN` and `LA` datasets parse their known ID layouts; other layouts use dimensionless `other` |
| Census BDS | Header-first arrays; a request can select multiple variables | One `bds` dataset with `geo_level`, `state` and `variable`; the adapter requests one variable per geography |
| BEA | NIPA rows by table line and period; Regional rows also identify an area | `bea_nipa` uses `table_name`, `line_number`, `frequency`; `bea_regional` uses `table_name`, `line_code`, `geo_fips` |
| FHFA HPI | CSV rows with `index_nsa` and `index_sa` value columns | `fhfa_hpi` uses `hpi_type`, `hpi_flavor`, `frequency`, `level`, `place_id`, `seasonal_adjustment`; each index column becomes a separate series |
| World Bank WDI | Response metadata followed by rows for an indicator and area | One `wdi` dataset with `indicator` and `area` dimensions |
| WTO Timeseries | Rows for an indicator, reporting economy, partner, product and period | One `timeseries` dataset with `indicator` and `area` dimensions; the curated list fixes each indicator's product and partner |
| Bank of Canada | Single-valued series with opaque Valet names | One dimensionless `valet` dataset; units and frequency stay series metadata |

For example, BLS's `CUUR0000SA0` splits into survey `CU`, seasonal adjustment `U`,
periodicity `R`, area `0000` and item `SA0`. Census variables and FHFA's two index columns
likewise become distinct series under named dimensions, preserving a single stored value
column even when the publisher can return several measures together.

SEC uses `companies` and `financial_statements` instead of these dataset contracts.
Separate accessions retain filing vintages; the crawl stores raw documents without
populating `financial_line_items` with parsed facts.

## Revisions and observation attributes

The [main crawler's persistence code](../../backend/crates/econ-graph-crawler/src/persist.rs)
upserts observations on `(series_id, date, revision_date, is_original_release)`.
A conflicting observation receives the new value. BLS, Census BDS, BEA, FHFA, WTO and the
Bank of Canada set
`revision_date = date` and `is_original_release = true`, so revised estimates overwrite
that key rather than retaining earlier releases. This matters for sources that revise
past observations, including Census annual releases, BEA updates and FHFA re-estimates.

FRED and WDI retain different kinds of vintages. FRED maps ALFRED's `realtime_start`
to `revision_date` and marks the earliest available vintage of each observation as its
original release. WDI uses the response's database-level `lastupdated` date and fetches
the full history. When a fetched response has a new `lastupdated`, each observation whose
value is new or changed is stored under that new vintage; an unchanged value keeps the
row of the vintage that first carried it. WDI therefore retains updates actually fetched;
it does not recover missed editions or provide an upstream archive comparable to ALFRED.

Observation attributes are distinct from dimensions and vintage dates. BLS parses footnotes
such as `P` (preliminary) and `X` (unavailable), but does not persist them.
BEA's `NoteRef`, World Bank's `obs_status` and `decimal`, FHFA's `rstderr` and `note`, and
WTO's `ValueFlagCode` are also not stored as observation attributes. WDI and WTO declare
attributes in their dataset contracts, but those declarations do not populate them. FRED has no per-observation flags;
its `"."` missing-value marker becomes `NULL`.

## Units

Each series stores its units three ways on `economic_series`:

- `units`: the text shown with the series, describing the values as stored.
- `units_raw`: the source's units text exactly as received, or NULL when it was not recorded
  (rows no fetch has written since the column was added). It differs from `units` only for
  BEA, whose `units` drops the `Millions of` (or other scale) that `UNIT_MULT` already
  multiplied into the values.
- `units_source`: `structured` when the units come from a field of the source's own that
  holds them (FRED's `units`, BLS's catalog and code files, BEA's `CL_UNIT` and
  `METRIC_NAME`, the World Bank, Census and FHFA units that the adapters set from their
  curated lists), `parsed_from_text` when the crawler parsed them out of free text such as a
  description, and `unknown` when the series has no units. NULL on a row no writer has set it
  on; readers then derive it: `unknown` when `units` is missing or only whitespace, else
  `structured`. Every current adapter's units are
  `structured`.

`units_parsed` holds the units in a fixed vocabulary (the
[units module](../../backend/crates/econ-graph-crawler/src/units.rs)): a kind (currency,
constant-price currency, percent, percent change, percentage points, index, count, rate,
years), the scale of the stored values (units, thousands, millions, billions, trillions), and,
where stated, an ISO 4217 currency code, an index base such as `2017=100`, and the price
reference year of a constant-price amount. The parser recognises only listed phrases,
normalising case and whitespace and nothing else; anything ambiguous or unlisted (`Rate`,
`Level`, `Percent or rate`, `+1 or 0`, `Mt CO2e`) is left unparsed. A bare currency word
(`dollars`, `$`) gets the currency its adapter states for the source: US dollars for FRED,
BEA and BLS. The World Bank's units name their currency (`current US$`), and the Bank of
Canada, WTO, Census, FHFA and the static catalogs state none. Text that names a currency
(`Canadian dollars`, `euro`) always keeps it, and other kinds never get a currency. GraphQL
exposes these as
`unitsSource`, `unitsRaw` and `parsedUnit`. For a series whose `units_parsed` is not stored,
`parsedUnit` parses `units` with the same source currency a fetch uses, so it answers the
same before and after the next fetch writes `units_parsed`.

When a fetched series is stored, the crawler parses its units text and, where the adapter
also states units from structured fields (BEA: values are in base units, since they are
multiplied by `10^UNIT_MULT`), compares the two. The structured units win in `units_parsed`,
and the check never changes the stored text or values. A fetch that gives no units keeps the
stored ones. It counts by source (at least once per stored fetch: a retried persist counts
again) in
`crawler_units_unrecognised_total`, `crawler_units_mismatch_total` and
`crawler_units_agree_total`, and logs one line per series: `error` for a mismatch or for
unparsed text next to structured units, `debug` for other unparsed text. The
`CrawlerUnitsMismatch` alert fires for a day after any mismatch.

## Checking selections against the live catalogs

`crawler check-selection` ([crawler guide](../technical/CRAWLER_DEPLOYMENT_GUIDE.md#check-curated-lists-and-selection-rules),
[code](../../backend/crates/econ-graph-crawler/src/check_selection.rs)) downloads the catalog
each adapter selects from and applies the current curated list or selection rules, read-only
and without a database. `scripts/deploy/check-selection.sh --namespace NS` runs it in the
cluster with the crawler's API keys. A licence-gated source is left out of the default run, and
naming one with `--source` is refused with exit status 64, as is naming a source with nothing to
check.

| Source | Catalog checked | Selection checked |
|---|---|---|
| FRED | Release and category series pages, plus `/series` for each include override | `catalog_selection/fred.toml` |
| BEA | `GetParameterValues(TableName)` for NIPA and Regional, then each selected table (NIPA `GetData`, Regional line codes and areas) | `catalog_selection/bea.toml` |
| BLS | The survey series files (`cu.series`, `la.series`, ... under download.bls.gov) | `bls_series.csv` |
| World Bank WDI | Each indicator's `/indicator/{id}` | `wdi_indicators.csv` |
| Census BDS | `variables.json` and `geography.json`, and the state file | The adapter's economic-variable keyword rules |
| FHFA HPI | The HPI master CSV (its required columns and in-scope rows) | The adapter's fixed scope |

Problems, each of which makes the command exit nonzero:

- **missing**: a curated entry the source no longer lists (a FRED include override FRED reports
  as nonexistent, a BLS id absent from its survey's series file, a World Bank indicator the API
  rejects), or a rule that matches nothing in the current catalog;
- **discontinued**: a FRED include override titled `(DISCONTINUED)`, a BLS series whose latest
  period is too old, a World Bank indicator whose source is an archive, or a BEA table that BEA
  lists but serves no rows or lines for (the BLS and World Bank rules are heuristics, below);
- **restricted**: a FRED include override whose notes carry a copyright restriction;
- **over-broad**: a rule that matches more catalog entries than its optional `max_matches`;
- **over-cap**: a selection past `max_series`, `max_tables` or `max_catalog_series`, which would
  stop discovery;
- **over-quota**: a refresh needing more requests than the source's daily quota (BLS: 500 a day
  with a key, 25 without);
- **unfetchable**: a World Bank indicator listed under a source other than WDI whose data request
  fails. The adapter's `/country/all/indicator/{id}` names no source, and the API answers it only
  for WDI indicators (message 175, "not found", for one such as `DT.AMT.MLTC.GG.CD` in
  International Debt Statistics). The check makes that request, for one row, for each indicator
  not listed under WDI;
- **rejected**: a selected BEA table whose request BEA refuses as invalid ("rejected by BEA").

BLS and World Bank publish no discontinued flag, so the check infers one:

- **BLS**: the series file's `end_year`, the year of the latest published period, is before
  last year. Annual and semi-annual series, which BLS publishes with a lag, get one more year.
- **World Bank**: `/indicator/{id}` names an archive as the indicator's source (a source whose
  name contains "Archive", such as 57, "WDI Database Archives"). An indicator listed under
  another live source is unfetchable (above) unless its data request works, which is a notice.

Rule-selected FRED series that are discontinued or restricted, and FRED's known-retired include
probes (`expected_missing`), are reported as notices: discovery keeps them on purpose. Each
source also reports the selected series count against its limits, how many entries each rule
matched, and the requests one refresh of every selected series takes at its policy rate.

`max_matches` is optional on each rule in `catalog_selection/fred.toml` (`[[rules]]`) and
`catalog_selection/bea.toml` (`[[rule]]`). Discovery does not enforce it and it is not part of
the selection fingerprint; it states how many entries the rule is meant to match, so a rule that
starts matching more is reported.

## Attribution

The citation and notice each source's terms require, and where they appear, are in
[Source notices](source-notices.md).

## Related notes

- [BLS API experimental findings](../technical/BLS_API_EXPERIMENTAL_FINDINGS.md)
- [Census BDS integration](../technical/CENSUS_BDS_INTEGRATION.md) and
  [Census Bureau integration summary](../technical/CENSUS_BUREAU_INTEGRATION_SUMMARY.md)
- [World Bank API experimental findings](../technical/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
  and [World Bank integration post-mortem](../archive/technical/WORLD_BANK_INTEGRATION_POST_MORTEM.md)
- [Rate limits by source](../technical/RATE_LIMIT_SOURCES.md)
