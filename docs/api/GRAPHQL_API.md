# GraphQL API Documentation

## Overview

The EconGraph backend exposes a GraphQL API. DataLoaders batch selected relationship queries; they do not cover every resolver.

## N+1 Problem Prevention

### DataLoader Implementation

The [DataLoaders implementation](../../backend/crates/econ-graph-graphql/src/graphql/dataloaders.rs)
provides cached loaders for sources, data points, data point counts, series by source,
and series counts (`data_source_loader`, `data_points_by_series_loader`,
`data_point_count_loader`, `series_by_source_loader`, and `series_count_loader`).
It also provides four non-cached loaders: `dataset_loader`,
`series_dataset_fields_loader`, `dataset_source_loader`, and `latest_observation_loader`.
A loader's presence does not mean every related field uses it.

### Field Resolvers with DataLoaders

The active [EconomicSeriesType resolvers](../../backend/crates/econ-graph-graphql/src/types.rs)
batch `dataset`, `dimensions`, and `defaultMeasure` through the dataset and series-dataset
loaders, and `latestObservation` through the latest-observation loader.
[`DatasetType.source`](../../backend/crates/econ-graph-graphql/src/graphql/datasets.rs)
uses `dataset_source_loader`. These resolvers obtain loaders through
`crate::graphql::context::data_loaders(ctx)?` and call `.try_load(key).await`.
The helper reads the optional loaders in `SchemaResources`; without a configured data
database, it returns `DATA_SERVED_REMOTELY`, directing callers to `/data/graphql`.

`EconomicSeriesType.source`, `recentDataPoints`, `dataPointCount`, and
filtered `dataPoints` query the database separately for each series.
`recentDataPoints(limit: ...)` applies a SQL limit and returns the latest revision of
each selected observation, ordered from newest observation date to oldest.
`DataSourceType.series` also queries directly, loading the source's active series with
observations before paging them in memory; its `first` argument limits the response,
not the database fetch.

Cached loaders are created with the schema and stored in its `SchemaResources`, so
cache lifetime follows schema lifetime. The backend's [`/graphql` route](../../backend/crates/econ-graph-backend/src/main.rs)
builds a fresh schema for every HTTP request, making those caches request-scoped.
Callers that retain a schema, such as the MCP and security servers, retain its cached
loaders across requests. The dataset and latest-observation loaders are non-cached so
retained schemas can read dataset edits and observations updated by a crawl.

## GraphQL Schema

### Queries

#### Core Data Queries

- `series(id: ID!)` - Get a specific economic series
- `seriesList(filter: SeriesFilter, pagination: PaginationInput)` - List series with filtering
- `searchSeries(query: String!, ...)` - Full-text search across series
- `dataSource(id: ID!)` - Get a specific data source
- `dataSources` - List sources with at least one series containing observations
- `seriesData(seriesId: ID!, filter: DataFilter, transformation: DataTransformation, first: Int, after: String)` - Get time series data, one page at a time
- `crossSection(datasetId: ID!, measure: String, filter: [DimensionFilterInput!]! = [], across: String!, date: NaiveDate, latest: Boolean, asOf: NaiveDate)` - One measure of a dataset for every value of one dimension (e.g. every country), at a date or at each key's latest value

`series(id: ...)` returns null for an unknown series or one without observations.
`seriesList` and `searchSeries` omit series without observations and, by default,
inactive series. `dataSource(id: ...)` can still return a source omitted by `dataSources`.

#### Monitoring Queries

- `crawlerStatus` - Get crawler status information; requires `admin.system:read`
- `queueStatistics` - Get queue processing statistics; requires `admin.system:read`

### Mutations

- `triggerCrawl(input: TriggerCrawlInput!)` - Enqueue crawl jobs; requires `admin.crawlers:manage`

For these protected operations, send an identity-provider access token in the
`Authorization: Bearer <token>` header. Authorization checks the token's roles claim;
a valid token must also grant the operation's required permission.

### Types

The complete type, field, argument, enum, and scalar reference is the committed
[generated schema](../../backend/crates/econ-graph-graphql/schema.graphql), checked by the
[schema snapshot test](../../backend/crates/econ-graph-graphql/tests/schema_snapshot.rs)
against the runtime schema. Use it rather than a separate handwritten SDL copy.

In particular, the series and source types are `EconomicSeriesType` and
`DataSourceType`; dates use `NaiveDate`, and numeric observation values use
`BigDecimal`. Transformations return data points through the existing `value` field;
there is no separate `TransformedDataPoint` output type.

## Example Queries

### Basic Series Query with Related Data

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

### Search with Pagination

Omit `after` for the first page. `first` defaults to 50 and is capped at 1,000.
`totalCount` counts all matching series. For the next page, set `after` to the number
of series already read, expressed as a decimal string: after reading 50 results, send
`"after": "50"` with the same query and filters. Add each page's returned series count
to that offset and stop when it reaches `totalCount`. Malformed cursors, negative
offsets, and offsets outside the i32 range are rejected with `BAD_USER_INPUT`.
Unlike `seriesData`, this result has no `pageInfo` or `endCursor`.

```graphql
query SearchSeries($query: String!, $first: Int, $after: String) {
  searchSeries(query: $query, first: $first, after: $after) {
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

### Data with Transformation

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

### Every Point of a Long Series

`seriesData` returns at most 10,000 points per page (`first` defaults to and is capped at 10,000).
`totalCount` counts every point matching the filter. Pass the previous page's `endCursor` as
`after` until `hasNextPage` is false. A cursor is the number of points up to and including the
one it names. A transformed page has the values it would have in the whole series, so pages can
be concatenated. `LOG_DIFFERENCE` is `ln(value) - ln(previous value)`, empty unless both are
positive.

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

### Data as Known on a Past Date

`asOf` returns each observation's newest revision published on or before that date, and omits
observations first published later. It takes precedence over `latestRevisionOnly`.

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

### Cross-section for a map

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

### Multiple Series with Batched Dataset and Observation Fields

In this example, `dataset` and `latestObservation` batch their lookups across the
returned series. The enclosing `DataSourceType.series` field queries directly, as
explained above.

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

## Performance Features

1. **Batched Database Queries** - Selected relationship fields use DataLoaders; `source`, `recentDataPoints`, `dataPointCount`, and filtered `dataPoints` query per series
2. **Query Complexity Analysis** - Built-in protection against expensive queries
3. **Caching** - Cache lifetime follows schema lifetime: request-scoped on the backend `/graphql` route, retained when callers reuse a schema; dataset and latest-observation loading is non-cached
4. **Pagination** - `seriesData` and `seriesList` page in the database; `DataSourceType.series` pages in memory
5. **Selective Field Loading** - Only requested fields are processed
6. **Schema Reference** - The generated SDL and snapshot test keep the full API reference aligned with runtime types

## Development Tools

- **GraphQL Playground** - Served at `/playground` only when the backend runs with `ENABLE_GRAPHQL_PLAYGROUND=true` (docker-compose sets it; deployed environments do not)
- **Introspection** - Full schema introspection support
- **Query Validation** - Automatic query validation and error reporting

## Migration from REST

Economic series queries use `POST /graphql`. The current server does not expose
legacy `/api/v1/series` or `/auth/login` endpoints; authentication uses
identity-provider access tokens. See the [backend API documentation](../../backend/README.md#api-documentation)
for the server's health, metrics, and optional playground routes.
