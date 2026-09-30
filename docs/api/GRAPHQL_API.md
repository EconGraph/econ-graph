# GraphQL API Documentation

## Overview

The EconGraph backend exposes a GraphQL API. DataLoaders batch selected relationship queries; they do not cover every resolver.

## N+1 Problem Prevention

### DataLoader Implementation

The current [DataLoaders implementation](../../backend/crates/econ-graph-graphql/src/graphql/dataloaders.rs) has these fields:

- `data_source_loader` — source lookups by ID
- `data_points_by_series_loader` — data points by series ID
- `data_point_count_loader` — data point counts by series ID
- `series_by_source_loader` — series by source ID
- `series_count_loader` — series counts by source ID
- `user_loader` — user lookups by ID
- `latest_observation_loader` — latest observations by series ID (non-cached)

### Field Resolvers with DataLoaders

[EconomicSeriesType resolvers](../../backend/crates/econ-graph-graphql/src/graphql/types.rs)
obtain loaders from `ctx.data::<SchemaResources>()?.data_loaders` and call
`.load(key).await`. For example, `source` uses `data_source_loader`, and
`recentDataPoints` uses `data_points_by_series_loader` before taking the requested limit.
The limit does not bound the loader's database fetch.

The filtered `EconomicSeriesType.dataPoints` resolver queries the database directly for
each series. Selecting it across several series therefore performs per-series queries;
there is no filtered-date-range loader.

Cached loaders are created with the schema and stored in its `SchemaResources`, so
cache lifetime follows schema lifetime. The backend's [`/graphql` route](../../backend/crates/econ-graph-backend/src/main.rs)
builds a fresh schema for every HTTP request, making those caches request-scoped.
Callers that retain a schema, such as the MCP and security servers, retain its cached
loaders across requests. The latest-observation loader is explicitly non-cached so
retained schemas can read observations updated by a crawl.

## GraphQL Schema

### Queries

#### Core Data Queries
- `series(id: ID!)` - Get a specific economic series
- `seriesList(filter: SeriesFilter, pagination: PaginationInput)` - List series with filtering
- `searchSeries(query: String!, ...)` - Full-text search across series
- `dataSource(id: ID!)` - Get a specific data source
- `dataSources` - List all data sources
- `seriesData(seriesId: ID!, filter: DataFilter, transformation: DataTransformation, first: Int, after: String)` - Get time series data, one page at a time
- `crossSection(datasetId: ID!, measure: String, filter: [DimensionFilterInput!]! = [], across: String!, date: NaiveDate, latest: Boolean)` - One measure of a dataset for every value of one dimension (e.g. every country), at a date or at each key's latest value

#### Monitoring Queries
- `crawlerStatus` - Get crawler status information
- `queueStatistics` - Get queue processing statistics

### Mutations

- `triggerCrawl(input: TriggerCrawlInput!)` - Manually trigger data crawling

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
countries from aggregates such as `WLD`. `flags` stays empty until datasets store
observation attributes, and `asOf` is not supported yet. An unknown dataset is a
`NOT_FOUND` error; an invalid request is `BAD_REQUEST`.

### Multiple Series with Batched Relationship Fields
```graphql
query GetMultipleSeries($sourceId: ID!) {
  dataSource(id: $sourceId) {
    name
    series(first: 100) {
      nodes {
        id
        title
        dataPointCount
        recentDataPoints(limit: 5) {
          date
          value
        }
      }
    }
  }
}
```

## Performance Features

1. **Batched Database Queries** - Selected relationship fields use DataLoaders; filtered `dataPoints` remains a direct per-series query
2. **Query Complexity Analysis** - Built-in protection against expensive queries
3. **Caching** - Cache lifetime follows schema lifetime: request-scoped on the backend `/graphql` route, retained when callers reuse a schema; latest-observation loading is non-cached
4. **Efficient Pagination** - Cursor-based pagination for large result sets
5. **Selective Field Loading** - Only requested fields are processed
6. **Schema Reference** - The generated SDL and snapshot test keep the full API reference aligned with runtime types

## Development Tools

- **GraphQL Playground** - Served at `/playground` only when the backend runs with `ENABLE_GRAPHQL_PLAYGROUND=true` (docker-compose sets it; deployed environments do not)
- **Introspection** - Full schema introspection support
- **Query Validation** - Automatic query validation and error reporting

## Migration from REST

The GraphQL API provides all functionality previously available through REST endpoints with improved efficiency and flexibility. Legacy REST endpoints are maintained for backward compatibility during the transition period.
