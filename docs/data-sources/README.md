# Data sources

These guides compare the data publishers return with what EconGraph discovers, fetches
and stores. Each source page covers response samples, frequency and revisions, retained
and dropped flags, and the current dataset mapping. The seven main-crawler adapters store
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

The other nine sources in `SourceId` (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN_STATS)
are [static catalogs](../../backend/crates/econ-graph-crawler/src/sources/static_catalogs.rs):
fixed series lists without HTTP requests, whose `fetch_series` always fails. They are
disabled in release builds by the `static_catalogs` build flag and available in development
builds. There is no IMF adapter. IMF, BOJ, SNB and UN_STATS are also
[licence-gated](../../backend/crates/econ-graph-crawler/README.md#licence-gate): no build
crawls them unless its `crawl_<source>` flag is on.

## Publisher shapes and stored datasets

All seven main-crawler adapters use the [dataset contracts](../../backend/crates/econ-graph-crawler/data/datasets/README.md).
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
A conflicting observation receives the new value. BLS, Census BDS, BEA, FHFA and WTO set
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
