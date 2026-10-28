# GraphQL API Guide

EconGraph's GraphQL API lets applications discover economic series and datasets,
read observations and historical revisions, and work with annotations and chart sharing.
The [schema reference](../../backend/crates/econ-graph-graphql/schema.graphql) lists
all fields, arguments, enums, and scalars. Dates use `NaiveDate` (`YYYY-MM-DD`),
timestamps use `DateTime` (RFC 3339), and observation values use `BigDecimal`.

## Send a request

Send a JSON body containing `query` and, when needed, `variables` and `operationName`,
using `POST` with `Content-Type: application/json`. Application operations, including
annotations, sharing, accounts, and administration, use `/graphql`. Economic data
reads also use `/graphql` in deployments that serve both kinds of operations there.
Deployments serving economic data separately expose `/data/graphql` for those reads.
If `/graphql` returns `errors[].extensions.code: DATA_SERVED_REMOTELY`, send the data
operation to `/data/graphql`.

For example, this body discovers datasets for a source. Replace the sample UUID
with a source ID returned by `dataSources`, and use the deployment's economic data endpoint:

```json
{
  "operationName": "FindDatasets",
  "query": "query FindDatasets($sourceId: ID!) { datasets(sourceId: $sourceId) { id code name defaultMeasure } }",
  "variables": { "sourceId": "00000000-0000-0000-0000-000000000001" }
}
```

Check the response's `errors` as well as its `data`. Endpoint selection, available
data, and account permissions are separate concerns: signing in does not fix a
`DATA_SERVED_REMOTELY` error or make an absent dataset available.

## Access and sign-in

The economic data reads in this guide are available anonymously. For application
requests that need an identity, send an identity-provider access token in
`Authorization: Bearer <token>`. Sign-in identifies the viewer for reads as well as
writes. Each protected operation also checks its required permission, and some
operations check ownership or a role on the individual chart.

Sign-in must be available in the deployment. Where sign-in is disabled, sending a
token establishes no identity. Where token verification is configured, an invalid
or expired token can reject even an otherwise public request with HTTP 401 and
`UNAUTHENTICATED`; a suspended account gets HTTP 403 and `ACCOUNT_SUSPENDED`.
If sign-in verification is temporarily unavailable, a request may run anonymously,
so public reads can succeed while protected operations fail or viewer-specific
results are absent.

### Application reads

| Operation | Access and visibility |
| --- | --- |
| `annotationsForSeries` | Public annotations, plus the signed-in caller's own private annotations. The deprecated `userId` argument is ignored; it cannot select another viewer. |
| `commentsForAnnotation` | Comments are readable only when the parent annotation is public or belongs to the caller. |
| `chartCollaborators` | Requires sign-in. Only the chart's collaborators see the list; other callers receive an empty list. |
| `user` | Requires sign-in. Reading the caller's own record is allowed; reading another user's requires `admin.users:read`. |
| `users` | Requires `admin.users:read`. |
| `crawlerStatus`, `queueStatistics`, `systemHealth` | Require `admin.system:read`. |
| `securityEvents`, `securityEventsPage` | Require `admin.security:read`. |
| `auditLogs` | Requires `admin.audit:read`. |

### Writes

All mutations require sign-in. Permissions below are the fine-grained roles granted
to the caller; a valid token alone does not grant them. Administrative permissions
do not bypass private-annotation visibility.

| Operation | Additional requirements |
| --- | --- |
| `createAnnotation` | `annotation:create`; the caller becomes the author. New annotations are private unless `isPublic: true` is requested. |
| `updateAnnotation` | `annotation:create` and authorship. |
| `addComment` | `annotation:comment` and visibility of the annotation. A chart annotation also requires an `admin` role on that chart. |
| `shareChart` | `chart:share` and an existing `admin` role on the chart. |
| `deleteAnnotation` | Authorship, or an `admin` role on the annotation's chart if the annotation is public. Another author's private annotation remains inaccessible. |
| `triggerCrawl` | `admin.crawlers:manage`; returns the number of jobs enqueued, not completed observations. |
| `updateUser` | `admin.users:update`; changing `isActive` also requires `admin.users:suspend`. |
| `deleteUser` | `admin.users:delete`; cannot delete the caller's own account. |
| `suspendUser`, `activateUser` | `admin.users:suspend`; callers cannot suspend themselves. |

Updating, deleting, suspending, or activating another user's account also requires
every `admin.*` permission:

- `admin.users:read`, `admin.users:create`, `admin.users:update`,
  `admin.users:delete`, and `admin.users:suspend`;
- `admin.sessions:read` and `admin.sessions:revoke`;
- `admin.crawlers:manage` and `admin.system:read`;
- `admin.security:read` and `admin.audit:read`.

A caller with only `admin.users:update` can update their own record; changing its
`isActive` field still requires `admin.users:suspend`. Callers cannot delete or
suspend their own account, including by setting `isActive: false` through `updateUser`.

Available source data and product features vary by deployment. Use discovery queries
to find the data present at the selected endpoint; availability does not imply an
account entitlement, and a missing source does not by itself indicate a permission denial.

## Discover sources and series

Start with `dataSources`, which returns sources by name and includes only sources
with at least one series containing observations. `dataSource(id: ...)` can still
return a source omitted from that listing. Use the returned source UUID to narrow
series or dataset discovery.

`seriesList` accepts a `SeriesFilter` with `sourceId`, `frequency`, `isActive`, and
`searchQuery`. An omitted `isActive` selects active series; `false` selects inactive
ones. Results omit series without observations and are ordered by title, with ID
breaking ties. `pagination.first` defaults to 50 and is clamped to 1–100. Omit
`pagination.after` initially, then pass `pageInfo.endCursor` while `hasNextPage` is
true. These cursors are decimal offsets; use the same filter for each page.

```graphql
query ListSeries($filter: SeriesFilter, $after: String) {
  seriesList(filter: $filter, pagination: { first: 50, after: $after }) {
    nodes { id title frequency }
    totalCount
    pageInfo { hasNextPage endCursor }
  }
}
```

For direct lookup, `series(id: ...)` returns null for an unknown series or one
without observations. `seriesByExternalId(sourceName: ..., externalId: ...)` resolves
a source's native series ID; use the exact source name returned by `dataSources`
(for example, `Federal Reserve Economic Data (FRED)` with external ID `GDP`).
It returns null when no such series exists. `seriesExists(id: ...)` checks existence
even when there are no observations.

### Read a series and recent observations

`recentDataPoints(limit: ...)` returns the latest revision for each selected
observation date, newest date first. `dataPointCount` counts all stored point rows,
including revisions, so it can exceed the number of distinct observation dates.

```graphql
query GetSeries($id: ID!) {
  series(id: $id) {
    id
    title
    description
    units
    frequency
    source {
      name
      description
    }
    recentDataPoints(limit: 10) {
      date
      value
      isOriginalRelease
    }
    dataPointCount
  }
}
```

### Search with pagination

`searchSeries` returns active series with observations in relevance order. Its
optional `source` argument is a source UUID, despite having type `String`; invalid
UUIDs return `BAD_USER_INPUT`. The optional `frequency` uses `SeriesFrequency`
(for example, `MONTHLY` or `ANNUAL`). `searchSuggestions(partialQuery: ...)` offers
query completions and corrections.

Search text must be 1–500 characters. Omit `after` for the first search page.
`first` defaults to 50; supplied values must be between 1 and 1,000.
`totalCount` counts all matching series. For the next page, set `after` to the number
of series already read as a decimal string: after reading 50 results, send
`"after": "50"` with the same query and filters. Add each page's returned series count
to that offset and stop when it reaches `totalCount`. An `after` offset must be an
integer from 0 through 2,147,483,647. Malformed cursors and offsets outside that
range return `BAD_USER_INPUT`. Unlike
`seriesList` and `seriesData`, search results have no `pageInfo` or `endCursor`.

```graphql
query SearchSeries($query: String!, $source: String, $frequency: SeriesFrequency, $first: Int, $after: String) {
  searchSeries(query: $query, source: $source, frequency: $frequency, first: $first, after: $after) {
    series {
      id
      title
      description
      source {
        name
      }
    }
    totalCount
    query
    tookMs
  }
}
```

## Read observations and revisions

A data point's `date` is the observation date; `revisionDate` is the publication
date of that version. A date can have several stored revisions, and `value` can be
null when the source publishes an observation without a number.

`seriesData` accepts inclusive `startDate` and `endDate` bounds in its `filter`.
Without a transformation or revision filter, it returns all stored revisions in
ascending observation date, then revision date, then ID order. Choose a revision
mode explicitly to control which versions are included:

- `latestRevisionOnly: true` selects each date's newest revision.
- `originalOnly: true` restricts results to original releases.
- `asOf` selects each date's newest revision published on or before the supplied
  date and omits observations first published later. It takes precedence over
  `latestRevisionOnly`. If combined with `originalOnly: true`, only original releases
  are eligible; `asOf` does not remove that restriction.

Historical reads reflect the vintages available in EconGraph; they cannot
reconstruct revisions the source no longer provides and EconGraph never captured.

### Transform values

Set `transformation` on `seriesData` to return transformed results in `value`, with
the point's dates and other fields retained. `NONE` leaves values unchanged. Other
transformations default to latest revisions unless the filter chooses `asOf`,
`originalOnly: true`, or an explicit `latestRevisionOnly` value. Setting
`latestRevisionOnly: false` alone allows every stored revision into the transformation.
Date bounds apply before transformation, so a comparison outside the selected range
is unavailable.

| Transformation | Returned value |
| --- | --- |
| `YEAR_OVER_YEAR` | Percentage change from a reading 360–370 days earlier. |
| `QUARTER_OVER_QUARTER` | Percentage change from a reading 85–95 days earlier. |
| `MONTH_OVER_MONTH` | Percentage change from a reading 28–32 days earlier. |
| `PERCENT_CHANGE` | Percentage change from the filtered series' earliest non-null, nonzero value. |
| `LOG_DIFFERENCE` | `ln(value) - ln(previous value)`, comparing consecutive selected points; both values must be positive. |

The period comparisons use the earliest observation within the stated window.
A missing comparison, null input, or zero percentage-change denominator yields a
null value. Rows remain in the result even when their transformed value is null.

```graphql
query GetSeriesData($seriesId: ID!, $transformation: DataTransformation) {
  seriesData(
    seriesId: $seriesId
    filter: { startDate: "2020-01-01", latestRevisionOnly: true }
    transformation: $transformation
  ) {
    nodes {
      date
      value
      revisionDate
    }
  }
}
```

### Read every point of a long series

`seriesData` returns at most 10,000 points per page (`first` defaults to and is capped
at 10,000; negative values are rejected). `totalCount` counts matching point rows,
including any selected revisions. Pass the previous page's `endCursor` as `after`
until `hasNextPage` is false. A cursor is the number of points up to and including
the one it names. Keep filters and transformation the same between pages. A transformed
page has the values it would have in the whole filtered series, so pages can be concatenated.

```graphql
query GetSeriesPage($seriesId: ID!, $after: String) {
  seriesData(seriesId: $seriesId, filter: { latestRevisionOnly: true }, first: 10000, after: $after) {
    totalCount
    pageInfo {
      hasNextPage
      endCursor
    }
    nodes {
      date
      value
    }
  }
}
```

### Read data as known on a past date

This request selects the revisions known by March 15, 2024, rather than restricting
observations to that date:

```graphql
query GetSeriesAsOf($seriesId: ID!) {
  seriesData(seriesId: $seriesId, filter: { asOf: "2024-03-15" }) {
    nodes {
      date
      value
      revisionDate
    }
  }
}
```

## Discover datasets and build cross-sections

Use `datasets(sourceId: ...)` to find a source's datasets, or omit `sourceId` to list
all datasets. Results are grouped by source and ordered by code within each source;
the list is unpaginated. Use a returned ID with `dataset(id: ...)` to inspect its
dimensions, allowed codes and labels, measures, and `defaultMeasure`.

```graphql
query InspectDataset($id: ID!) {
  dataset(id: $id) {
    id
    code
    name
    dimensions {
      name
      label
      codelist
      codes { code label unit }
    }
    measures { name label unit }
    defaultMeasure
  }
}
```

Dimension `name` and code values supply the inputs for `crossSection`; labels are
for display. `codelist` identifies a shared list such as `countries`. A non-null
`codelist` with empty `codes` means that list could not be resolved, rather than
that the dimension has no codes. On an individual series, `dimensions` supplies
its dimension values and labels, and `defaultMeasure` gives its measure override
or the dataset's default.

### Cross-section for a map

After finding the World Development Indicators dataset and inspecting its
`indicator` and `area` dimensions, pass its ID as `$wdi`. Omitting `measure` selects
the dataset's `defaultMeasure`.

```graphql
query GdpPerCapitaByCountry($wdi: ID!) {
  crossSection(
    datasetId: $wdi
    filter: [{ dimension: "indicator", value: "NY.GDP.PCAP.CD" }]
    across: "area"
    latest: true
  ) {
    key
    area { name iso3 isoNumeric kind }
    seriesId
    date
    value
  }
}
```

`filter` pins every dataset dimension except `across`. Give exactly one of `date` and
`latest: true`. With `latest`, each key gets its most recent non-null value with its own
date, since the latest year is often missing for many countries. Every active matching
series is returned, ordered by key; `value` (and with `latest`, `date`) is null where the
series has no value. `area` is set when `across` uses the `countries` code list, and `kind` tells
countries from aggregates such as `WLD`. `flags` is empty. An unknown dataset is a
`NOT_FOUND` error; an invalid request is `BAD_REQUEST`.

`asOf` selects each observation's newest revision published on or before that date;
observations first published later are excluded. Combine it with `date` to read that
observation date as it was known at `asOf`, or with `latest: true` to find each key's
most recent non-null value among the revisions known at `asOf`. Omit `asOf` to use
current revisions.

### Latest observations across a source

`latestObservation` selects the newest observation date at that date's newest
revision. The field is null if there are no points; its `value` can be null even
when an older date has a number. This differs from `crossSection(latest: true)`,
which seeks each key's most recent non-null value.

```graphql
query GetMultipleSeries($sourceId: ID!) {
  dataSource(id: $sourceId) {
    name
    series(first: 100) {
      nodes {
        id
        title
        dataset {
          code
          source { name }
        }
        latestObservation {
          date
          value
          revisionDate
        }
      }
    }
  }
}
```

## Request limits and exploration

Queries are limited to depth 15 and complexity 1,000 (one unit per selected field);
requests exceeding either limit are rejected. Request only the fields needed by the application, and use
pagination for large series lists and observation histories.

Schema introspection is supported. The optional GraphQL Playground at `/playground`
is available when enabled for the deployment. See the
[backend API documentation](../../backend/README.md#api-documentation) for health,
metrics, and playground routes. The API uses identity-provider access tokens;
there is no `/auth/login` route or legacy `/api/v1/series` endpoint.
