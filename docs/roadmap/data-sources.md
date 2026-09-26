# Roadmap: data source coverage

Status: proposed (2026-09-26), for Joe to decide the forks marked below.

This roadmap covers which data sources EconGraph supports, in what order, and how the
international statistical sources get built on one generic SDMX adapter. It picks up the
ten static catalogs and the IMF adapter that train 1 removes from the build (see the
[release trains](./releases.md)). The SDMX adapter is train 3 work.

What each source's data looks like today is documented in
[docs/data-sources/](../data-sources/README.md), and this doc doesn't repeat it. The
storage model is the [federation roadmap](./federation.md)'s: one Iceberg table per
dataset, with SDMX-style dimensions, measures and attributes, and the `datasets` metadata
in Postgres landing first.

Facts about upstream APIs below come from their public documentation. The cloud
environment's network policy blocks the source hosts (checked again on 2026-09-26 for the
ECB, OECD, Eurostat, ILO, IMF, Bank of Canada and World Bank APIs), so none of them has
been checked against a live response. The one exception is the Census key requirement,
which the data source docs thread saw in a live response. Each source's first PR records a
real response as a fixture before building on it.

## Where things stand

Checked on `main` at `c92bdaf`, in `backend/crates/econ-graph-crawler/src/`:

| Source | Adapter | Fetches data | Notes |
|---|---|---|---|
| FRED | `sources/fred.rs` | Yes | Needs `FRED_API_KEY` |
| BLS | `sources/bls.rs` | Yes | Hard-coded series list |
| Census BDS | `sources/census.rs` | Yes | National and per state. The API now rejects requests without a key |
| FHFA HPI | `sources/fhfa.rs` | No | Fetches from an endpoint that probably never existed |
| BEA | `sources/bea.rs` | No | Made-up series ids; in `FETCH_UNIMPLEMENTED` (`scheduler.rs:61`) |
| World Bank | `sources/world_bank.rs` | No | In `FETCH_UNIMPLEMENTED` |
| IMF | `sources/imf.rs` | No | Made-up series ids, and the legacy `dataservices.imf.org` base URL; in `FETCH_UNIMPLEMENTED` |
| BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats, WTO | `sources/static_catalogs.rs` | No | 47 hard-coded entries, no HTTP. `fetch_series` always fails |

The static catalogs are registered in `default_registry()` (`sources/mod.rs:35`), so
discovery writes their entries into `series_metadata`, but the scheduler never fetches
them (`supports_fetch`, `scheduler.rs:438`). Every one of those series is a catalog entry
that can never get data.

What the catalog ids are worth:

- **Look like real upstream codes** (unverified): two of the four ECB entries are
  plausible SDMX series keys (`ICP.M.U2.N.000000.4.ANR` is euro area HICP inflation,
  though the catalog labels it the main refinancing rate), BOC's `V39079` is a Valet
  series, BOE's `IUDBEDR` is an IADB code, and RBA's `F1.1` and `G1` are its statistical
  table numbers.
- **Out of date**: most OECD entries use the old OECD.Stat dataset names (`SNA_TABLE1`,
  `PRICES_CPI`), which the OECD's 2024 SDMX API replaced. One of them repeats its dataset name
  four times (`TIS_GOODS_SERVICES.TIS_GOODS_SERVICES...`).
- **Made up**: BOJ (`BOJ_UNRATE`), ILO (`ILO_UNEMPLOYMENT`), SNB (`ir`, `gdp`), UN Stats
  (`UN_GDP`) and WTO (`MT_GOODS_EXP`).

So the static catalogs can't serve real data. They are still useful in development,
where they fill the catalog with more sources to build and test the UI against (Joe,
2026-09-26). The few real ids are listed above, and the SDMX configuration below
rediscovers them from the source.

## Decisions to make

### Decision 1: the static catalogs in development only

**Decided (Joe, 2026-09-26): keep them for development, and leave them out of release and
production builds.** The options were to delete them or keep them in some form. Joe
chose to keep them because they are useful in a development environment.

So train 1 gates them rather than deleting them. The static-catalog adapters and their
`SourceId` variants are built only in development, and release builds don't register
them or offer their sources. Search in a release build can't return a series that has
no data. `imf.rs` is different: it has made-up ids and no development use, so it's
deleted. The `Imf` source id stays for the SDMX adapter.

Each source leaves the static catalog when a real adapter for it lands (ECB, OECD and
ILO on the SDMX adapter in train 3, and the central banks through BIS, decision 3).

### Decision 2: which SDMX publisher leads the adapter?

Joe decided on 2026-09-26 that SDMX doesn't need to be in the first release: FRED, BLS,
Census BDS, FHFA, BEA and World Bank WDI are already a lot of useful data, and WDI covers
the map. So the SDMX adapter and all its publishers are train 3 work. The earlier release
plan had put an IMF-only SDMX adapter in train 1. In train 1 the IMF adapter is removed
from the build like the static catalogs, because it can't fetch anything (made-up ids,
legacy endpoint), and it comes back on the SDMX adapter.

Which publisher the adapter is built against first still matters. The IMF is the least
stable SDMX publisher to build against. It has been moving its data from the legacy SDMX
2.0 JSON service (the one `imf.rs` targets) to a new data portal and SDMX 2.1 and 3.0 API
at `api.imf.org`, and it reorganized datasets on the way, for example splitting IFS into
topic datasets (from public docs). The ECB's API (`data-api.ecb.europa.eu`) has served
stable SDMX 2.1 for years (its host moved from `sdw-wsrest.ecb.europa.eu` in 2023, and the
static catalog's URLs still use the old one), needs no key, and supports every query
feature the adapter needs.

| Option | For | Against |
|---|---|---|
| A. IMF first | IMF covers every country | The adapter's first target is the one most likely to change under it. Bugs in the adapter and changes in the API look the same |
| B. ECB alone first, then the others one by one | Most stable target | One publisher doesn't prove the adapter is generic |
| **C. Build against ECB and IMF together, with ECB as the reference (recommended)** | Two publishers prove the adapter is generic. ECB validates the adapter code; IMF shows that a second publisher needs only configuration. If the IMF's API is still moving, IMF waits and ECB ships | Two sets of fixtures and catalog choices in the adapter's first PRs |

The release plan's train 3 row still says "the IMF first, then the ECB"; it changes to
match once this decision is taken.

### Decision 3: bespoke adapters for the central banks, or BIS?

BOC, BOE, BOJ, RBA and SNB each publish in their own format: Valet JSON, IADB CSV, flat
files (the BOJ may now have an API; check), CSV tables and SNB's cube API respectively
(from public docs). That's five bespoke adapters. What users most want from them is policy
rates, exchange rates and perhaps property prices. The BIS publishes exactly those, for
about 40 central banks, over SDMX (`stats.bis.org`: central bank policy rates, effective
exchange rates, residential property prices, credit to the private sector). The ECB covers
euro exchange rates, and FRED already carries some central-bank series.

| Option | For | Against |
|---|---|---|
| A. A bespoke adapter per central bank | The full depth of each bank's data | Five adapters and five formats to maintain, for data few users go deep on |
| **B. BIS on the SDMX adapter for policy rates, exchange rates, property prices and credit; bespoke adapters only when someone needs a specific bank's detail (recommended)** | One configuration file instead of five adapters, with consistent definitions across countries | Less depth for each bank. BIS lags some banks by days or weeks |
| C. Rely on FRED's copies | No new work | Spotty coverage, and FRED's terms restrict redistributing some third-party series |

BIS isn't a `SourceId` yet, so B adds `Bis`.

### Decision 4: UN Stats and WTO

| Source | Recommendation | Why |
|---|---|---|
| UN Stats | Drop, with no plan | The static catalog's population, GDP and life expectancy are all in World Bank WDI. The SDG database can come later if someone needs SDG indicators |
| WTO | Unscheduled | The WTO Timeseries API needs a free subscription key (from public docs). The [global analysis roadmap](./global-analysis.md) defers trade to IMF bilateral trade data (IMTS, formerly DOTS), which the SDMX adapter covers |

## The generic SDMX adapter

One adapter implementation, instantiated once per publisher from a data file. Adding a
publisher or a dataset is a configuration change plus a recorded fixture, not new code.

### Protocol and formats

- **SDMX REST 2.1** for every publisher. ECB, Eurostat, OECD, ILO, BIS and the IMF's new
  API all serve it. Some also serve 3.0, but 2.1 is the common subset.
- **Structure** (dataflows, data structure definitions, code lists): SDMX-ML 2.1 XML.
  Every publisher serves it. SDMX-JSON structure support varies between them.
- **Data**: SDMX-CSV, requested with the `application/vnd.sdmx.data+csv` Accept header,
  which is the portable way. Publishers' `format` parameters differ (`csvdata` at the ECB
  and BIS, others elsewhere), so any override is set per publisher in the config. One row
  per observation, with a `DATAFLOW` column, one column per dimension, `TIME_PERIOD`,
  `OBS_VALUE` and one column per attribute. That's already the row shape of a federation
  dataset table, and it streams without building a tree. Publishers that don't serve
  SDMX-CSV fall back to SDMX-ML generic data.

### Configuration

Per Joe's rule on reference data, the publisher list and the chosen datasets live in a
data file read at runtime (`econ-graph-crawler/data/sdmx_providers.toml`, next to
`us_states.csv` and loaded through `reference.rs`), not in code. For each publisher:

- the `SourceId`, base URL and agency id;
- the rate policy: requests per second, concurrency, and a maximum series count per
  request;
- whether it supports `updatedAfter` and `startPeriod`;
- the datasets to crawl. Each entry is a dataflow (agency, id, version) and a key filter
  selecting the series, such as `M..EUR.SP00.A` for the ECB's monthly euro reference
  rates, plus which dimension is the country, if any.

Key filters keep the crawl to curated slices. Whole dataflows are huge: the ECB's exchange
rate dataflow alone has thousands of series.

### Mapping SDMX to the federation data model

| SDMX | EconGraph |
|---|---|
| Dataflow | A row in `datasets`, and later one Iceberg table |
| DSD dimensions (except `TIME_PERIOD`) | Dataset dimensions. `FREQ` stays a dimension, because one dataflow mixes frequencies |
| `TIME_PERIOD` | `date`, the period start. The parser handles `2024`, `2024-Q1`, `2024-01`, `2024-S1`, `2024-W05`, `2024-01-15` and the IMF's `2024-M01` |
| Primary measure (`OBS_VALUE`) | Measure `value`, the dataset's default measure |
| Observation-level attributes (`OBS_STATUS`, `OBS_CONF`, comments) | Dataset attributes |
| Series-level attributes (`UNIT_MULT`, `UNIT`, `DECIMALS`, `TITLE`) | Series metadata in `economic_series` |
| Series key | `external_id`, written as `{dataflow}/{key}`, such as `EXR/M.USD.EUR.SP00.A`. The source column already says which publisher |
| Code lists | Labels for dimension values ("Germany" for `DE`). Stored with the dataset metadata so the UI can filter and label without calling the publisher |

**Country codes differ across publishers.** The ECB and Eurostat use two-letter codes with
exceptions (Eurostat's `EL` for Greece, the ECB's `U2` for the euro area). The OECD, ILO,
World Bank and the IMF's new API use three-letter codes (the IMF's legacy service used
two-letter ones). The map and the global analysis `crossSection` query need one country
dimension, so each publisher's country codes map onto one shared country reference file
(ISO 3166 alpha-3 plus aggregates), loaded like `us_states.csv`. Global analysis phase 1
already plans that file; the SDMX adapter reuses it rather than adding a second one.

### Discovery

Discovery lists series without downloading observations. For each configured dataset it
requests the key filter with `detail=serieskeysonly`, or the publisher's availability
endpoint where that isn't supported. It reads the DSD and code lists once per dataset.
Titles are the series' `TITLE` attribute where the publisher provides one, and otherwise
are built from the code list labels ("Euro area, HICP, overall index, annual rate of
change, monthly").

### Fetching in batches

Today's adapter contract fetches one series per call (`fetch_series(external_id)` in
`adapter.rs`). With SDMX that is wasteful: one request with a key filter returns every
matching series, so a dataset of 500 series is one request, not 500. The OECD and Eurostat
also limit request rates per client, so one request per series would take hours.

**Proposed:** train 1 adds a batch fetch to the adapter contract (a batch key per series,
and one adapter call for the queued jobs that share it). The SDMX adapter uses a dataset
slice as the batch key, so one request fetches the whole slice. This is also the shape the
federation roadmap's Iceberg writer wants: it commits one append per dataset batch.

Incremental crawls send `updatedAfter` (the last successful crawl time) where the
publisher supports it, which returns only changed series, and otherwise `startPeriod` with
the source's revision lookback (`policy.rs`).

Responses that are too large (HTTP 413, or a publisher-specific "too many results" error)
split the key filter on its first wildcard dimension and retry. A 404 with "no results" is
an empty result, not an error: SDMX 2.1 uses 404 for an empty query.

### Vintages

SDMX 2.1 has no vintage in the data. Two things give one:

- **Crawl-date vintages.** A value that differs from the latest stored value for the same
  `(series_id, date)` is appended with `revision_date` set to the crawl date. Unchanged
  values aren't rewritten. The other adapters get this in train 3, and the SDMX adapter
  does it from the start rather than stamping `revision_date = date`.
- **Edition vintages.** Where the publisher releases editions (the IMF World Economic
  Outlook each April and October), the edition's date is the vintage.

### Testing

Each publisher gets a recorded structure fixture (DSD and code lists) and a recorded data
fixture for one dataset slice, under `tests/fixtures/sdmx/<publisher>/`, following the
testkit's `new(base_url)` convention. A test checks that every configured key filter is
valid against its recorded DSD (right number of dimensions, known codes), so a typo in
`sdmx_providers.toml` fails CI instead of the crawl.

## Keys, rate limits and licenses by source

All from public documentation; none checked live. "Default" means the crawler's fallback
policy of 1 request a second with concurrency 2 (`policy.rs`).

| Source | Key | Rate limit | Redistribution | Policy today |
|---|---|---|---|---|
| FRED | Required, free | 120 requests a minute | Allowed for most series. Some third-party series are marked copyrighted and need citation or permission | 120 a minute |
| BLS | Optional. Without one, 25 queries a day; a free version 2 key gives 500 a day and 50 series a query | Daily query quota | Public domain | 25 a minute |
| Census | Required: keyless requests are rejected (checked live on 2026-09-26) | Per-key daily limit | Public domain | 40 a minute |
| BEA | Required, free | 100 requests and 100 MB a minute | Public domain | 30 a minute |
| FHFA | None (CSV file) | None stated | Public domain | Default |
| World Bank | None | None stated | CC BY 4.0 | Default |
| IMF (new API) | None for the public SDMX endpoints; check whether the new API gateway needs a subscription key | Not stated; the API gateway throttles | Free reuse with attribution under the IMF's terms | Default |
| ECB | None | None stated | Free reuse with attribution | Default |
| Eurostat | None | Large queries are served asynchronously | Free reuse with attribution | Not a `SourceId` yet |
| OECD | None | Published per-client limits, changed since the 2024 API launch | CC BY 4.0 since 2024 | Default |
| ILO | None | None stated | CC BY 4.0 | Default |
| BIS | None | None stated | Free reuse with attribution (BIS terms) | Not a `SourceId` yet |
| WTO | Required, free subscription | Per-key quota | WTO terms | Default |

Every "free reuse with attribution" source needs its attribution on the series page and in
the CSV download (release plan, train 1 features 7 and 11). `data_sources` should carry
the attribution text and license, so the page shows it without code changes.

API keys for FRED, BEA and Census belong in the crawler's Kubernetes Secret (release plan,
train 1 feature 2), never in the config map.

## Order

Each item is its own PR, per project convention. Items in the same train can proceed in
parallel unless they say otherwise.

### Train 1 (`v4.0.0`)

No SDMX work. Each row is one PR. This table is kept current as the PRs land (state as of
2026-09-27).

| Work | What it changes | PR |
|---|---|---|
| Static catalogs out of release builds | Decision 1: available in development only. `imf.rs` is deleted (made-up ids); the `Imf` source id stays for the SDMX adapter. The ten static catalogs (BOC, BOE, BOJ, ECB, ILO, OECD, RBA, SNB, UN Stats, WTO) register only with the `static-catalogs` cargo feature. There is no migration: their `data_sources` rows are created only by the crawler, so a release database never has them | #217 (draft) |
| Batch fetch | The worker claims queued jobs that share a batch key and makes one adapter call for them (`batch_key`, `fetch_batch`, `SourcePolicy::max_batch`). Pulled forward from the SDMX work because BLS allows 25 requests a day without a key, and World Bank, FHFA and BEA return many series per request. After the first rate-limit error, the default `fetch_batch` makes no more upstream calls. The SDMX adapter later uses a dataset slice as the batch key | #213 |
| Datasets on the crawler side | Adapters declare datasets (`data/datasets/<source>.toml`) and give each series its dimension values, on the federation roadmap's `datasets` table. Owned by the datasets work | #225 (draft) |
| Fetch discovered series | Discovery writes only `series_metadata` and the scheduler only refreshed `economic_series`, so a discovered series was never fetched. The scheduler now picks up discovered series | #228 |
| Census key | The Census API now rejects requests without a key, so the key is required and a missing key fails before any request. The key goes on every request, including metadata | #207 |
| FRED vintages | `realtime_start` from ALFRED as the revision date. The other sources keep `revision_date = date` until train 3, except the World Bank (`lastupdated`). The first vintage is the original release. Later fetches start from the newest stored vintage and page past ALFRED's 100,000-row limit | #214 (draft) |
| BLS list | 291 series from `data/bls_series.csv`: headline CPI-U, CES payrolls and earnings, CPS labor force, and LAUS rates and labor force for the states and DC. Discovery makes no request. Batched 50 per request with `BLS_API_KEY`, 25 without. Footnotes are parsed but not stored | #235 (draft) |
| FHFA | The HPI master CSV instead of an API that probably never existed. Real dimensions replace the made-up `{CODE}HPI` ids | #240 (draft) |
| BEA | Real NIPA table and line ids and regional GDP by state, instead of made-up ids | Not opened yet |
| World Bank WDI | About 50 indicators for every country, with `area` (the shared country reference) and `indicator` as dimensions | Not opened yet |
| Coverage and freshness | A per-source coverage report and Prometheus alerts, for the release plan's exit criteria 2 and 3. The alert rules now actually load; there is no Alertmanager yet, so a crawl failure fires in Prometheus. `crawler coverage [--fail-under 95]` prints the report, the worker exports `crawler_coverage_*` gauges, and a runbook (`docs/monitoring/crawler-coverage-runbook.md`, added by the PR) covers the alerts | #210 |
| Hide series without data | Search and listings skip series with no data points, and the sources listing skips sources with none | Not opened yet |
| Crawler deployment | Source keys from the `crawler-api-keys` Secret, and the data files in the image | Not opened yet |

### Train 3 (`v4.2.0`)

1. **Crawl-date vintages for the other adapters**, as the SDMX adapter does from the
   start.
2. **SDMX core**: structure parsing, SDMX-CSV parsing, the period parser, discovery,
   batching by dataset slice and crawl-date vintages, tested against recorded ECB
   fixtures.
3. **ECB on the SDMX adapter**: exchange rates (`EXR`), HICP (`ICP`), policy rates (`FM`)
   and monetary aggregates (`BSI`), euro area and member states. A few hundred series.
4. **IMF on the SDMX adapter**: the World Economic Outlook headline indicators (GDP
   growth, inflation, unemployment, current account, government debt) for every country,
   plus the monthly CPI and exchange rate datasets. Moves to a later train if the IMF API
   isn't settled.
5. **BIS**: policy rates, effective exchange rates, residential property prices and
   credit, for every country BIS covers (decision 3).
6. **Eurostat** (adds a `Eurostat` source id): quarterly national accounts, HICP and
   unemployment for EU members. Needs the asynchronous download path for large queries.
7. **OECD**: composite leading indicators, key short-term indicators and quarterly
   national accounts. Its rate limit sets the policy.
8. **ILO**: unemployment and labour force participation by country, sex and age.

Items 2 and 3 come first, then 4 if the IMF API has settled; 1 and 5 to 8 are each mostly
configuration and can land in any order or slip to a later train without holding train 3.

### Unscheduled

- Bespoke central bank adapters (BOC Valet first, as the simplest) when someone needs a
  bank's detail beyond BIS.
- WTO, when trade data beyond IMF bilateral trade is needed.
- The UN SDG database.
- Growing the curated key filters toward whole dataflows. That's a scale question for the
  Iceberg store (federation phases 3 to 5), not the adapter.

## Open questions for Joe

1. Decisions 2 to 4 above: ECB alongside IMF when SDMX starts, BIS for central banks,
   dropping UN Stats and leaving WTO unscheduled. Decision 1 is decided.
2. **Network access and keys.** Every source in this doc needs its host allowed in the
   cloud environment's network policy before a Claude session can record fixtures. The
   SDMX hosts are `data-api.ecb.europa.eu`, `api.imf.org`, `stats.bis.org`,
   `ec.europa.eu`, `sdmx.oecd.org` and `sdmx.ilo.org`.
3. **Curation.** The dataset lists above are a proposal. Is there a list of indicators you
   want first (for example, what a FRED user would search for)?
