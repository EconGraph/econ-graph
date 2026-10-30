# WTO Timeseries API

The [WTO adapter](../../backend/crates/econ-graph-crawler/src/sources/wto.rs) crawls a curated
set of World Trade Organization trade and tariff indicators from the
[WTO Timeseries API](https://apiportal.wto.org/api-details#api=version1) (v1), the API behind
the [WTO Stats](https://stats.wto.org) portal. The API requires a subscription key. Its
[fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/wto/) are synthetic and follow
the response shape the API portal documents; see their README for provenance.

## Subscription key

Every request needs a key: without one the API answers HTTP 401. The crawler reads it from
`WTO_API_KEY` and sends it as the `subscription-key` query parameter, whose value the crawler's
HTTP layer redacts from logs and errors.

While `WTO_API_KEY` is unset, the crawler worker leaves the WTO source out entirely: it runs no
WTO discovery or refreshes and publishes no WTO coverage gauges, and logs one warning at
startup, "source skipped: its API key is not set ...", with fields `source=WTO` and
`env=WTO_API_KEY`. `crawler sources` reports the key as `WTO_API_KEY NOT SET (required)`. A WTO job enqueued by hand
(`crawler discover --source WTO`, `crawler fetch --source WTO ...`) fails with an `Auth` error,
"WTO_API_KEY not set", before any request.

### Getting and installing a key

1. Create an account on the [WTO API portal](https://apiportal.wto.org/signup) and confirm the
   e-mail address.
2. On the portal's [Products](https://apiportal.wto.org/products) page, subscribe to
   **Standard**, the free product that covers the Timeseries API. Subscriptions need no
   approval. The portal shows a primary and a secondary key on the account's profile page;
   either works.
3. Store the key in the cluster with `scripts/deploy/set-crawler-keys.sh`, run in the
   operator's own terminal. It prompts for `WTO_API_KEY` with hidden input, writes it to the
   `wto-api-key` entry of Secret `crawler-api-keys`, and restarts the crawler worker. Never put
   the key on a command line or in a file in the repository.
4. Check the worker log for the absence of the warning above, and `crawler sources` for
   `WTO_API_KEY set (required)`. The scheduler enqueues the first WTO discovery on its next tick,
   because the source has no discovery history.

For local runs, export `WTO_API_KEY` in the environment of the worker or the `crawler` CLI.

## Terms of use and attribution

The API portal links its terms to the WTO website's
[Copyright and permissions](https://www.wto.org/english/info_e/copyrights_permissions_e.htm)
page and its [disclaimer](https://www.wto.org/english/info_e/disclaimer_e.htm); the Standard
product states no terms of its own. The copyright page:

- states that the site and its contents are the copyright of the WTO;
- allows material other than official documents to be reproduced for non-commercial
  purposes, provided attribution is given to the WTO and the WTO is informed;
- requires written permission from the WTO for commercial use (requests to
  `publications@wto.org`, answered by e-mail).

For non-commercial use, these terms allow WTO data to be shown on a public site with
attribution once the WTO has been informed; they do not forbid redistribution. Before enabling
the source on a public deployment, the operator informs the WTO of the use, and any commercial
use of the data needs the WTO's written permission first. Every WTO series description ends
with "Source: World Trade Organization, WTO Stats." for attribution.

The WTO's separate
[Tariff and Trade Data terms](https://ttd.wto.org/docs/Terms_of_Use_WTO_Tariff_and_Trade_Data.pdf),
which ask for the Secretariat's approval (`idb@wto.org`) before mass downloads or
re-dissemination, govern that platform's Integrated Database and Consolidated Tariff Schedules
data. The adapter does not use that platform; its tariff indicators are the Timeseries API's
headline tariff-profile averages.

## Rate limits

The Standard product allows 10 calls/second and 10,000 calls/hour per key, and 1 call/second
on the time-series data endpoints (`/data`, `/data_count`). The crawler's
[source policy](../../backend/crates/econ-graph-crawler/src/policy.rs) holds every WTO request
to 1 request/second with no burst and one request at a time (at most 3,600 an hour). A discovery
or fetch batch makes two requests per indicator. The portal answers an exhausted quota with HTTP
403 ("Out of call volume quota"); the adapter treats such a 403 as rate limiting and the job
is retried later. See [Rate limit sources](../technical/RATE_LIMIT_SOURCES.md#wto).

## Coverage and requests

The adapter crawls the indicators in
[wto_indicators.csv](../../backend/crates/econ-graph-crawler/data/wto_indicators.csv), each
with the product/sector and partner economy that select its headline series:

| Indicator | Product/sector (`pc`) | Partner (`p`) | Measures |
| --- | --- | --- | --- |
| `ITS_MTV_AX` | `TO`, total merchandise | `000`, World | Merchandise exports, annual value |
| `ITS_MTV_AM` | `TO`, total merchandise | `000`, World | Merchandise imports, annual value |
| `ITS_CS_AX6` | `S`, total services | `000`, World | Services exports (BPM6), annual value |
| `ITS_CS_AM6` | `S`, total services | `000`, World | Services imports (BPM6), annual value |
| `TP_A_0010` | none | none | Tariff profile headline average, all products |
| `TP_A_0030` | none | none | Tariff profile headline average, all products |

The last column is a summary. Each series' title, unit, frequency and description come from
the WTO's own indicator metadata, never from this file.

For each indicator, discovery and fetching make two requests:

- `GET /indicators?i={indicator}&lang=1` returns the indicator's `name`, `unitLabel`,
  `frequencyLabel` and `description`. An indicator the API doesn't list (an empty answer or HTTP
  400) is skipped by discovery with a warning, and counted in `crawler_curated_missing`; the
  worker then retires no series for that discovery.
- `GET /data?i={indicator}&r=all&ps=all&fmt=json&mode=full&head=M&lang=1&max=1000000`, plus
  `pc` and `p` where the list gives them, returns every reporting economy's full history in one
  response. The adapter allows that request 180 seconds, longer than the crawler's ordinary
  timeout, because the API can be slow to produce it. An empty `204` response means no data.

Every series belongs to dataset `timeseries`, defined in
[wto.toml](../../backend/crates/econ-graph-crawler/data/datasets/wto.toml), with dimensions
`indicator` (the WTO code) and `area` (a key of the
[shared country reference](../../backend/crates/econ-graph-core/data/countries.csv)). The
canonical external ID is `timeseries/{indicator}.{area}`, for example
`timeseries/ITS_MTV_AX.USA`. Fetching is batched by indicator: one pair of requests serves
every economy. Discovery lists every economy with at least one value and is complete, so
series of an economy or indicator the WTO stops publishing are retired.

Discovery also merges each indicator's name, unit and description into the dataset's
`indicator` codes, and the WTO's names for the world and the European Union into its `area`
codes.

## Reporting economies

Rows name the reporting economy by the WTO's numeric code (`ReportingEconomyCode`). The adapter
looks a code up as an ISO 3166-1 numeric code among the shared reference's countries, and maps
`000` (World) to `WLD` and `918` (European Union) to `EUU`. Rows for other economies, such as
regional and economic groups or territories the reference doesn't list, are skipped with one
warning per request naming them.

A series title is the indicator's name and the economy's: the shared reference's name for a
country, the WTO's name (`ReportingEconomy`) for an aggregate. The description adds the
reporting economy, the product/sector and partner the series covers, and the source.

## Sample (synthetic fixture)

The `/data` response is `{"Dataset": [row, ...]}`. One row of
[data_its_mtv_ax.json](../../backend/crates/econ-graph-crawler/tests/fixtures/wto/data_its_mtv_ax.json),
whose values are synthetic:

```json
{
  "IndicatorCode": "ITS_MTV_AX",
  "ReportingEconomyCode": "840",
  "ReportingEconomy": "United States of America",
  "PartnerEconomyCode": "000",
  "PartnerEconomy": "World",
  "ProductOrSectorCode": "TO",
  "ProductOrSector": "Total merchandise",
  "PeriodCode": "A",
  "FrequencyCode": "A",
  "UnitCode": "USM",
  "Unit": "Million US dollar",
  "Year": 2024,
  "ValueFlagCode": "E",
  "ValueFlag": "Estimation",
  "TextValue": null,
  "Value": 2020000
}
```

The full row also carries the indicator's category and the product classification. Codes and
years may arrive as JSON strings or numbers, and the adapter reads both. It also accepts the
camelCase field names of the portal's published schema.

## Frequency and revisions

The curated indicators are annual. `Year` and `PeriodCode` give each observation's date: `A`
is 1 January; `Q1`–`Q4`, `M01`–`M12` and `S1`/`S2` are stored as their period's first day.
A `null` value is no observation and is dropped. Values are read from the response text digit
for digit.

The API publishes no release dates or vintages. Each point is stored with
`revision_date = date` and `is_original_release = true`, so a revised value overwrites the
stored one. Every fetch reads the full history in one request; `since` does not trim it.

## Flags

`ValueFlagCode` and `ValueFlag` mark values such as estimates. The dataset declares a
`value_flag` attribute, but `data_points` has no attribute columns, so the flags are not
stored.

## Dataset mapping

| Role | Columns |
|---|---|
| Dimensions | `indicator`, `area` |
| Measures | `value` (decimal) |
| Attributes | `value_flag` (string), declared; not stored |
| Shape | long, `revision_date` = observation date |
