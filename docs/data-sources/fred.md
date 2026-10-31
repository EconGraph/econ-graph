# FRED (Federal Reserve Economic Data, St. Louis Fed)

The [FRED adapter](../../backend/crates/econ-graph-crawler/src/sources/fred.rs)
requires `FRED_API_KEY`. Its [fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/)
include recorded series metadata and illustrative observation responses.

## Who can read it

FRED is readable by admins only: `catalog.fred:read` is granted to the `admin` composite
(and so to `super_admin`), not to `catalog-public` or the anonymous reader. Visitors and
signed-in users who aren't admins see no FRED source, series, search result, observation or
dashboard value. Admins read it as usual. An operator changes this on a running cluster with
`scripts/keycloak/set-catalog-access.sh`; see
[catalog access](../../config/keycloak/README.md#catalog-access-including-anonymous-visitors).

## What we fetch today

Discovery reads bounded release and category catalogs selected by
`backend/crates/econ-graph-crawler/data/catalog_selection/fred.toml`. Rules can constrain
title, id prefix and frequency. All pages must agree on count and ordering before the
snapshot supplies absence evidence; failed, truncated or changing pages fail discovery.
Separate scan and selected-series bounds stop unexpected expansion before observation
jobs are enqueued. A small include/exclude list handles explicit exceptions.

Discovery runs in chunks (the crawler's
[chunked catalog discovery](../../backend/crates/econ-graph-crawler/README.md#chunked-catalog-discovery)).
The `discover_catalog` job walks the rules' catalog pages and lists one unit per selected
series, carrying the metadata its catalog page returned, and one per include override the
rules didn't select. Each `discover_chunk` job handles 50 units. A rule-selected unit needs no
request; an include override needs one `/series` request. So a chunk makes at most 50
requests, under FRED's rate limit, which it shares with fetch jobs. One finalize job commits
the whole catalog. The run's catalog scope is the selection rules' fingerprint, the same one a
single walk reports, computed from `fred.toml` without a walk; if the rules can't be read the
discovery is refused and nothing changes. Setting `CRAWLER_SINGLE_JOB_DISCOVERY_SOURCES=FRED` on
the worker discovers FRED with a single job instead; see the
[crawler deployment guide](../technical/CRAWLER_DEPLOYMENT_GUIDE.md#run-the-worker).

The default scope is GDP release series plus headline overrides, including the NBER
recession indicator (`USREC`) and probes for two previously curated retired ids. `expected_missing` explicitly exempts those known retired
probes from the unexpected-missing warning, while keeping their lifecycle evidence. An
ordinary included id that disappears still raises the warning. Existing stored series
outside that scope stay retained and scheduled; changing the discovery scope does not
deactivate them. This is not a full FRED catalog crawl. Defaults are fixture-tested, not
validated against the live API.

Series whose `notes` match known copyright or reproduction-restriction phrases, such as
those on Coinbase and S&P/Case-Shiller series, are recorded as restricted and are not
fetched. Complete catalogs track repeated absences separately from fetch failures.
Absence counts only for ids the walked release or category no longer lists: a series
retitled or re-frequencied out of a rule stays published and scheduled, and is reported for
rule review. A `/series` response with no series is a failed fetch, not removal evidence.
Restricted series are retained internally and hidden from public reads. Discontinued,
missing and retired histories stay publicly reachable with a badge and lower search
priority. Successor candidates require operator confirmation; histories are never merged.
See the
[catalog runbook](../monitoring/crawler-coverage-runbook.md#catalog-selection-and-retained-series).

Each fetch requests series metadata. When observations are needed, it lists vintage dates with
`GET /fred/series/vintagedates?series_id=ID&realtime_start=..&realtime_end=9999-12-31&limit=10000[&offset=...]`.
It retrieves observations with
`GET /fred/series/observations?series_id=ID&realtime_start=..&realtime_end=..&limit=100000[&offset=...]`,
paged within real-time windows of at most 1,000 listed vintage dates; see
[Vintage-date limit](#vintage-date-limit). A full fetch starts at `1776-07-04` to request
all available vintages. An incremental fetch starts the day before the newest stored
vintage and re-reads that vintage. It does not restrict observation dates, so revisions
to older observations can arrive.

The adapter retains `last_updated` as fetch state in `Validators.version`, prefixed with
the parser version (`fred-1`). An incremental fetch skips the vintage-date and observation
requests when it has a known stored vintage, the new validator matches the stored one,
and validator bypass is disabled. Metadata is still refreshed. A missing validator, a
changed parser version, or a failure to read stored validators prevents the skip.

The adapter keeps title, notes, units, frequency and seasonal adjustment as series metadata.
Each observation supplies `date`, `value` and `realtime_start` (the revision date).
It does not store `realtime_end`, `popularity`, short-form metadata fields, or the series'
release and category. `last_updated` is retained only in the validator.

## Sample (from public docs)

The [GDP observation fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/observations_gdp.json)
is built from FRED's documented `/fred/series/observations` response format (real-time
periods, ALFRED), not recorded from the live API; values are illustrative. This trimmed
excerpt contains three vintages of one quarter, followed by observations for two later quarters:

```json
{
  "realtime_start": "1776-07-04",
  "realtime_end": "9999-12-31",
  "count": 6,
  "observations": [
    {"realtime_start": "2025-07-30", "realtime_end": "2025-08-27", "date": "2025-04-01", "value": "30331.117"},
    {"realtime_start": "2025-08-28", "realtime_end": "2025-09-24", "date": "2025-04-01", "value": "30353.902"},
    {"realtime_start": "2025-09-25", "realtime_end": "9999-12-31", "date": "2025-04-01", "value": "30485.729"},
    {"realtime_start": "2025-10-30", "realtime_end": "2025-12-22", "date": "2025-07-01", "value": "."},
    {"realtime_start": "2025-12-23", "realtime_end": "9999-12-31", "date": "2025-07-01", "value": "31095.089"},
    {"realtime_start": "2026-04-29", "realtime_end": "9999-12-31", "date": "2026-01-01", "value": "31722.514"}
  ]
}
```

From the [recorded GDP metadata fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/series_gdp.json):

```json
{
  "id": "GDP",
  "title": "Gross Domestic Product",
  "frequency": "Quarterly",
  "units": "Billions of Dollars",
  "seasonal_adjustment": "Seasonally Adjusted Annual Rate",
  "last_updated": "2026-08-28 07:54:02-05",
  "notes": "BEA Account Code: A191RC ..."
}
```

Values are strings; `"."` means missing. Dates are the start of the period.

## Frequency and revisions

Frequency is stated per series and ranges from daily to annual. FRED supplies historical
revisions archived by ALFRED. A full-history request uses
`realtime_start=1776-07-04&realtime_end=9999-12-31`; each response row describes a value
and the real-time interval during which it was published.

Each retained row becomes a point with `revision_date = realtime_start`. Before discarding
rows outside the retained ranges, the adapter finds the earliest returned `realtime_start`
for each observation date. It marks a retained row `is_original_release` only if it starts
on that earliest date. On a full fetch this means the earliest vintage FRED has, often
from when ALFRED history begins in the 1990s, rather than necessarily the observation's
first publication. The window handling below explains how earlier rows affect this flag
on incremental fetches.

For the federation model, `realtime_start` maps directly to `revision_date`. To reconstruct
an inclusive validity interval, use the day before the next revision of the **same
observation date** as `realtime_end`. The last revision has the open-ended convention
`9999-12-31`. A series vintage that revises other observation dates does not end this
observation's interval.

### Vintage-date limit

FRED's [observations parameter documentation](https://fred.stlouisfed.org/docs/api/fred/series_observations.html)
lists a limit of 2,000 vintage dates for JSON with the adapter's output type (the default,
`output_type=1`). The adapter also recognizes the HTTP 400 vintage-limit error for
`realtime_start`–`realtime_end` requests:
`Bad Request.  This exceeds the maximum number of vintage dates allowed (2000).`
Daily series such as DFF, DGS10 and EFFR can have several thousand vintage dates, so the
adapter splits long histories into windows.

The configured batch size, `FRED_MAX_VINTAGE_DATES_PER_REQUEST`, is 1,000 listed vintage
dates, half the documented JSON limit. This leaves room for the extra day at the start of
later requests and for differences between FRED's limit count and its vintage-date list.
The [vintage-date endpoint](https://fred.stlouisfed.org/docs/api/fred/series_vintagedates.html)
omits release dates on which a series did not change. The 1,000-date batch size also
matches the documented CSV and XLSX limit. The margin reduces the risk of a limit
rejection; it does not guarantee that FRED will count a window as expected. A series
with at most 1,000 listed vintages takes one observation window; for example, 5,150 listed
vintages would take six.

The first requested window begins at the fetch's real-time start. Every later window
begins one day before its group's first listed vintage. Each nonfinal window ends the day
before the next group's first vintage; the final window ends at `9999-12-31`. Requests
therefore overlap by one day, but the ranges of `realtime_start` retained for storage are
disjoint. A full fetch retains the first window's rows from its requested start. An
incremental fetch retains them from the newest stored vintage. Later windows retain rows
from their group's first listed vintage through the window's end.

Rows before those retention thresholds still take part in finding each observation
date's earliest returned row. They establish prior publication, preventing a later
revision from being marked `is_original_release`. The adapter assumes that FRED returns
values already in effect at a requested window's start with `realtime_start` clipped to
that start. The [synthetic fixtures](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/README.md)
model this behavior; authenticated API interval comparisons have not established it.
Consequently, agreement of original-release flags across full, incremental and windowed
fetches depends on this unverified response behavior.

If FRED rejects a window for too many vintage dates, the crawler logs a warning, splits
that window into smaller groups and retries, down to a single listed vintage. If the
window cannot be split further, the fetch fails. Every other HTTP 400 fails without this
splitting fallback.

## Flags and footnotes

None per observation. Missing values are `"."`, which the adapter stores as `NULL`.
Series-level `notes` are free text.

## Dataset mapping

FRED series are single-valued and heterogeneous, so they are one dimensionless dataset,
[`FRED`](../../backend/crates/econ-graph-crawler/data/datasets/fred.toml), keeping the FRED series id as the external id:

| Role | Columns |
|---|---|
| Dimensions | none beyond `series_id` (the FRED id stays the external id) |
| Measures | `value` (decimal) |
| Attributes | none |
| Shape | long (one value column), `date`, `revision_date = realtime_start` |

Frequency, units and seasonal adjustment stay series metadata in Postgres.
