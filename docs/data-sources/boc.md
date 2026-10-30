# Bank of Canada Valet

The [Bank of Canada adapter](../../backend/crates/econ-graph-crawler/src/sources/boc.rs) reads
series from [Valet](https://www.bankofcanada.ca/valet/docs), the Bank of Canada's public JSON
API, without an API key. Its [test fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/boc/README.md)
are unedited recordings of Valet responses.

## Discovery and fetching

Discovery reads a curated list of Valet series names,
[`boc_series.csv`](../../backend/crates/econ-graph-crawler/data/boc_series.csv), and requests
`/valet/series/{name}/json` for each one. The list holds only names. Each series' title,
description, unit and frequency come from that response: the title is Valet's `label`, the
description its `description`, and the units and frequency are `metadata.unit` and
`metadata.frequency`. Valet leaves `metadata` empty for the monetary aggregates; those series
have no units, and their frequency is Valet's `dateFormat` (`monthly`) with its first letter
capitalised.

Valet gives the aggregates' units nowhere in a structured field. Their series details have empty
`metadata`, and so does the group that holds them (`/valet/groups/E1_MONTHLY/json`, "Selected
monetary aggregates and their components"), whose per-series entries carry no metadata at all.
Only that group's free-text `description`, "Month-end, Millions of dollars", states them. The
adapter does not parse prose into units, so M2 and M2++ are stored with empty units.

A name Valet doesn't know (HTTP 404) or answers malformed is skipped and reported as a missing
curated entry; the rest are still discovered. A rate limit, server error or authentication
failure stops the discovery at once, without looking up the remaining names, and the queue
retries it later. The list is the adapter's
whole catalog: once a discovery succeeds with no skipped names, a stored Bank of Canada series
the list no longer names is marked inactive.

Fetching requests `/valet/observations/{name}/json`, one request per series. A refresh sends
`start_date` set to the scheduler's revision window (365 days before the latest stored
observation); a first fetch sends no date and receives the whole history. The response carries
the series' details again, so a fetch also refreshes the title, description, units and
frequency. Valet sends no `ETag` or `Last-Modified`, so there is no conditional request.

Only names made of ASCII letters, digits, `.`, `_` and `-` are requested; any other external id
is rejected as not found without a request.

## Scope and series

The curated list covers headline series the Bank of Canada itself publishes:

| Group | Valet names |
|---|---|
| Policy and money-market rates | `V39079` (target for the overnight rate), `AVG.INTWO` (CORRA), `V80691311` (prime rate) |
| Exchange rates | `FXUSDCAD`, `FXEURCAD`, `FXGBPCAD`, `FXJPYCAD`, `FXCNYCAD` |
| Government of Canada benchmark bond yields | `BD.CDN.2YR.DQ.YLD`, `BD.CDN.5YR.DQ.YLD`, `BD.CDN.10YR.DQ.YLD`, `BD.CDN.LONG.DQ.YLD` |
| Core inflation | `CPI_TRIM`, `CPI_MEDIAN`, `CPI_COMMON` |
| Monetary aggregates | `V41552796` (M2 (gross), seasonally adjusted), `V41552801` (M2++ (gross), seasonally adjusted) |

The descriptions in parentheses are Valet's labels, shortened. The rates and yields are
business daily, the prime rate weekly, and the inflation measures and aggregates monthly.
Valet's credit aggregates (household and business credit) end in September 2020, so the list
has none. Statistics Canada's own series that Valet republishes, such as total CPI, are not
listed.

## Source layout and sample

This excerpt is reproduced from `GET /valet/observations/FXUSDCAD/json?start_date=2026-10-28`,
with all fields retained:

```json
{
    "terms": {
        "url": "https://www.bankofcanada.ca/terms/"
    },
    "seriesDetail": {
        "FXUSDCAD": {
            "label": "USD/CAD",
            "description": "Daily average exchange rate of the US dollar in Canadian dollars.",
            "dimension": {
                "key": "d",
                "name": "Date"
            },
            "dateFormat": "daily",
            "metadata": {
                "unit": "Canadian dollars per US dollar",
                "frequency": "Business daily"
            }
        }
    },
    "observations": [
        {
            "d": "2026-10-28",
            "FXUSDCAD": {
                "v": "1.4036"
            }
        },
        {
            "d": "2026-10-29",
            "FXUSDCAD": {
                "v": "1.3985"
            }
        }
    ]
}
```

`metadata` may also hold `unit_symbol` (`%`), which the adapter does not store. An unknown name
returns HTTP 404 with `{"message": "Series NAME not found.", "docs": "..."}`.

## Dates, units and revisions

Each observation's `d` is its date. Daily and weekly series use the observation day; monthly
series use the first of the month. The adapter accepts only `YYYY-MM-DD` dates and fails the
fetch on any other form. Values are strings, stored digit for digit with their written scale
(`0.008860` stays `0.008860`). An observation with no entry for the series, or an empty or
null `v`, is stored as a missing value. Any other value that isn't a decimal, such as an `NA`
marker, fails the whole fetch rather than being skipped: none of the listed series uses such
markers, so one would mean Valet's format changed, and a failed job makes that visible.

Valet publishes no vintages. The adapter stores points with `revision_date = date` and
`is_original_release = true`, so a revised value within the refresh window overwrites the
stored one. Earlier values are not retained.

## Dataset mapping

All series belong to [`valet`](../../backend/crates/econ-graph-crawler/data/datasets/boc.toml),
a dataset with no dimensions: a Valet name is an opaque key, kept as the external id.

| Role | Mapping |
|---|---|
| Dimensions | None |
| Measure | `value` (decimal), from `v` |
| Observation date | `d` |
| Attributes | None |
| Shape | Long; one value per series and observation date |

## Terms of use and request rate

Valet's responses link the Bank of Canada's [terms of use](https://www.bankofcanada.ca/terms/),
which the [Valet documentation](https://www.bankofcanada.ca/valet/docs) also points to. The
terms permit using, copying, distributing and transmitting the site's content on these
conditions:

- the Bank of Canada is attributed as the source, with an indication if changes were made
  (EconGraph lists it among its data sources and stores values unchanged);
- due diligence is exercised in reproducing the content accurately;
- content provided through paid services, or in documents for sale, must come with a notice
  that it is available free of charge on the Bank's website.

The exceptions are bank note images, the Bank's logo and wordmark, and third-party content,
which is why the list holds only series the Bank produces. The terms also state that Bank of
Canada exchange rates are released for statistical and analytical purposes only.

The terms forbid circumventing the Bank's limits on the number or frequency of requests,
including retrieval through Valet, and access that could overburden the site. Neither the
terms, the Valet documentation nor the [Valet how-to guide](https://www.bankofcanada.ca/valet-api-how-to/)
gives a numeric limit; the guide asks clients to start slowly, wait before retrying a failed
request, and cache data that changes once a day. The site's `robots.txt` disallows only
`/wp-admin/`.

The crawler's Bank of Canada policy is 1 request/second with a burst of 1 and one request in
flight ([`policy.rs`](../../backend/crates/econ-graph-crawler/src/policy.rs)). The source's
`data_sources` row records the same rate, 60 requests/minute. A full discovery
is one request per listed name, and the scheduler refreshes each daily series at most once a
day, each weekly or monthly series at most once a week.
