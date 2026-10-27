# World Bank API Experimental Findings

## Scope and provenance

These notes preserve an early experiment in finding economic indicators through the
World Bank Indicators API. The [original report](https://github.com/EconGraph/econ-graph/blob/55fa715da9a1ebe8899de721d32075cc0e22a27d/backend/docs/WORLD_BANK_API_EXPERIMENTAL_FINDINGS.md)
and its [discovery implementation](https://github.com/EconGraph/econ-graph/blob/55fa715da9a1ebe8899de721d32075cc0e22a27d/backend/src/services/series_discovery/world_bank.rs)
entered repository history on September 13, 2025. That is the document's provenance;
the report does not record a measurement date or raw responses for its catalog count.

The experiment combined topic lookup, known indicator IDs, and a bounded catalog search.
The current crawler selects indicators from a curated WDI list and discovers the areas
with observations for each one. The sections below distinguish the API interface, the
historical experiment, and the current implementation. See the
[World Bank source guide](../data-sources/world-bank.md) for current request, metadata,
and revision behavior.

## API structure

The API base is `https://api.worldbank.org/v2`. Append the paths below to that base;
they do not repeat `/v2`. Requests need no API key. Include `format=json` to request
JSON; the default is XML. The World Bank's
[basic-call reference](https://datahelpdesk.worldbank.org/knowledgebase/articles/898581-api-basic-call-structures)
describes format selection and the `page` and `per_page` parameters.

| Purpose | Path and example query |
|---|---|
| Indicator catalog across sources | `/indicator?format=json&per_page=100&page=1` |
| Metadata for one indicator | `/indicator/NY.GDP.MKTP.CD?format=json` |
| Topic categories | `/topic?format=json` |
| Indicators within Economy & Growth (topic 3) | `/topic/3/indicator?format=json` |
| Countries, territories, and aggregates | `/country?format=json&per_page=1000` |
| Observations for one country and indicator | `/country/USA/indicator/NY.GDP.MKTP.CD?format=json` |

Catalog metadata and observations are different resources: an indicator record names
and describes a measure, while an observation identifies an area, period, and value.
The [indicator reference](https://datahelpdesk.worldbank.org/knowledgebase/articles/898599-indicator-api-queries)
also explains how to select a source when an indicator belongs to several sources.
The unqualified catalog is broader than WDI (source 2), and broader still than
EconGraph's curated WDI list.

Successful JSON list responses use a two-element envelope, `[metadata, rows]`.
This abbreviated indicator example follows the official reference's single-indicator
response; descriptions are shortened and other fields omitted:

```json
[
  {"page": 1, "pages": 1, "per_page": "50", "total": 1},
  [
    {
      "id": "NY.GDP.MKTP.CD",
      "name": "GDP (current US$)",
      "source": {"id": "2", "value": "World Development Indicators"},
      "sourceNote": "GDP at purchaser's prices ...",
      "sourceOrganization": "World Bank national accounts data ..."
    }
  ]
]
```

The [topic reference](https://datahelpdesk.worldbank.org/knowledgebase/articles/898611-topic-api-queries)
describes topic and topic-indicator queries. Examples from the original report include
Agriculture & Rural Development (1), Aid Effectiveness (2), Economy & Growth (3),
Education (4), and Energy & Mining (5).

The country endpoint also includes territories and regional aggregates. These are
selected rows and fields from the original report's country example, with the envelope
omitted:

```json
[
  {"id": "ABW", "name": "Aruba"},
  {"id": "AFE", "name": "Africa Eastern and Southern"}
]
```

Aruba and the AFE aggregate illustrate why a country-list entry need not be a sovereign
state. The [country reference](https://datahelpdesk.worldbank.org/knowledgebase/articles/898590-country-api-queries)
shows the full envelope and explains the use of World Bank codes where ISO codes are
unavailable.

## Historical discovery experiment

### Strategies and coverage

The original report described three complementary approaches:

1. **Topic lookup** requested the Economy & Growth topic to narrow the catalog by subject.
   The implementation at the report's revision also requested Financial Sector (7) and
   attempted topic 11.
2. **Direct lookup** requested metadata for ten known economic indicators, avoiding a
   catalog scan for those IDs.
3. **Paginated search** requested the first ten catalog pages at 100 records per page and
   retained records matching economic keywords or ID patterns.

The same historical implementation also sampled five known indicators for 15 selected
countries. It treated a nonempty observation array as availability evidence, then combined
these candidates with the topic, direct-lookup, and paginated results. Candidates were
sorted and deduplicated by indicator ID, so this still produced an indicator catalog,
rather than a separate series for every country and indicator.

The report recorded **26,235 catalog entries across 263 pages at 100 per page**. This is
an undated reported snapshot, not a current catalog total, a WDI-only count, or a measured
count of EconGraph series. The ten-page search examined at most 1,000 catalog records
before filtering and deduplication. Topic and direct lookups could add candidates outside
those pages, but the combined strategy did not establish complete economic-indicator
coverage. Page counts depend on the requested page size and the catalog at request time.

### Direct-lookup indicators

The experiment used these identifiers:

| Indicator ID | Measure |
|---|---|
| `NY.GDP.MKTP.CD` | GDP (current US$) |
| `NY.GDP.MKTP.KD.ZG` | GDP growth (annual %) |
| `FP.CPI.TOTL.ZG` | Inflation, consumer prices (annual %) |
| `SL.UEM.TOTL.ZS` | Unemployment, total (% of total labor force) |
| `FR.INR.RINR` | Real interest rate (%) |
| `NE.TRD.GNFS.ZS` | Trade (% of GDP) |
| `GC.DOD.TOTL.GD.ZS` | Central government debt, total (% of GDP) |
| `GC.REV.XGRT.GD.ZS` | Revenue, excluding grants (% of GDP) |
| `GC.XPN.TOTL.GD.ZS` | Expense (% of GDP) |
| `BN.CAB.XOKA.GD.ZS` | Current account balance (% of GDP) |

### Keyword and ID filtering

`is_economic_indicator()` lowercased the name and ID, then accepted an indicator if
**either** contained a matching substring. Keyword matching and ID matching were
alternative inclusion tests, not two required stages.

The name keywords were:

- GDP, gross domestic product, inflation, unemployment, interest rate;
- exchange rate, trade, debt, revenue, expenditure, current account;
- balance of payments, economic, financial, monetary, fiscal;
- price, wage, income, consumption, investment, savings;
- export, import, balance, surplus, deficit, budget.

The ID substrings were:

- `ny.gdp`, `fp.cpi`, `sl.uem`, `fr.inr`, `ne.trd`;
- `gc.rev`, `gc.xpn`, `bn.cab`, `dt.dod`;
- `ic.tax`, `ic.bus`, `ic.reg`, `ic.gov`, `ic.lgl`;
- `ie.tic`, `ie.tra`, `ie.tec`, `ie.eng`, `ie.ene`, `ie.env`, `ie.hea`, `ie.edu`;
- `ie.agr`, `ie.fin`, `ie.inf`, `ie.urb`, `ie.rur`, `ie.gen`.

The original report summarized the last groups as `ic.*` and `ie.*`; the code matched
the specific substrings above, not every ID beginning with those prefixes. These
heuristics selected candidates by vocabulary, without proving relevance or data coverage.

### Historical storage and pacing

`discover_world_bank_series()` called the topic, direct-lookup, country-sampling, and
pagination helpers, then passed each retained indicator to `store_world_bank_series()`.
That helper used `EconomicSeries::get_or_create()` with the indicator ID as its external
ID. Its metadata helper supplied defaults such as annual frequency, units `Various`,
and a 1960 start date; these were not measurements of each indicator's history.

The implementation slept for 100 ms after each catalog page and after each sampled
country, rather than enforcing a delay after every HTTP request. Its data-source template
stored `rate_limit_per_minute = 1000`. Neither the sleeps nor that configuration value
establish a provider rate allowance or measured throughput.

## Current adapter and persistence

The [World Bank adapter](../../backend/crates/econ-graph-crawler/src/sources/world_bank.rs)
uses [wdi_indicators.csv](../../backend/crates/econ-graph-crawler/data/wdi_indicators.csv)
to select indicators. Full discovery requests observations for all areas of each selected
indicator with `/country/all/indicator/{id}?format=json&per_page=20000&page=N`.
It retains supported areas with at least one non-null value and emits one series per
indicator and area. The [WDI dataset](../../backend/crates/econ-graph-crawler/data/datasets/world_bank.toml)
defines dimensions `indicator` and `area`, giving canonical external IDs such as
`wdi/NY.GDP.PCAP.CD.USA`. The adapter follows observation pages up to `MAX_PAGES` (10),
a separate bound from the experiment's ten catalog pages. A response advertising more
observation pages is truncated with a warning.

Discovery returns series metadata, including title, description, units, frequency, data
URL, and dataset dimensions. The [worker](../../backend/crates/econ-graph-crawler/src/worker.rs)
persists those results through the shared
[persistence layer](../../backend/crates/econ-graph-crawler/src/persist.rs): discovery
upserts `series_metadata` by source and external ID; fetching upserts `economic_series`
and `data_points`. Indicator names and descriptions come from refreshed dataset codes,
with response-carried names as a fallback. The source guide explains the reference
refresh, unchanged-version probes, and storage of `lastupdated` as the observation vintage.

The adapter uses the World Bank defaults in
[`SourcePolicy`](../../backend/crates/econ-graph-crawler/src/policy.rs): a sustained
**1 request per second**, burst capacity 1, and at most 2 in-flight requests. These are
EconGraph's local controls. The legacy 1,000-per-minute data-source field is not the
crawler's request policy and is not a provider guarantee.

## Evidence and tests

The official references above establish request syntax and response fields. The
[checked-in fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/world_bank/)
and [adapter tests](../../backend/crates/econ-graph-crawler/src/sources/world_bank/tests.rs)
provide reproducible examples of current handling:

- `discover_lists_areas_with_values_indicator_by_indicator` uses mocked responses to
  check discovery, canonical IDs, titles, and omission of areas without values.
- `follows_pages_and_takes_lastupdated_from_the_first` and `page_count_is_bounded`
  check pagination and its cap against a local mock server.
- `response_shapes` checks fixture parsing, null observations, malformed responses, and
  API error messages. API errors can arrive inside HTTP 200 responses; an HTTP success
  status alone does not establish a successful data response.

The [policy tests](../../backend/crates/econ-graph-crawler/src/policy.rs) check the built-in
World Bank defaults. The [HTTP fetcher tests](../../backend/crates/econ-graph-crawler/src/http.rs)
exercise retry and concurrency behavior with a local mock server. The separate
[core World Bank integration tests](../../backend/crates/econ-graph-core/tests/world_bank_integration_tests.rs)
exercise database storage of the legacy data-source configuration, including the
1,000-per-minute field; they do not call the provider or validate its rate limits.
These checks describe local behavior, not live API availability or completeness of the
historical discovery experiment.
