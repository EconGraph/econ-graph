# GraphQL API Documentation

## Overview

The EconGraph backend now uses GraphQL instead of REST, providing efficient data fetching with comprehensive N+1 problem prevention through DataLoaders.

## N+1 Problem Prevention

### DataLoader Implementation

We've implemented several DataLoaders to batch database queries and prevent N+1 problems:

1. **DataSourceLoader** - Batches data source lookups by ID
2. **EconomicSeriesLoader** - Batches series lookups by ID  
3. **SeriesBySourceLoader** - Batches series lookups by source ID (one-to-many)
4. **DataPointsBySeriesLoader** - Batches data point lookups by series ID (one-to-many)
5. **LatestDataPointsBySeriesLoader** - Optimized loader for recent data points
6. **DataPointsDateRangeLoader** - Custom loader for filtered date ranges
7. **DataPointCountLoader** - Batches count queries for series

### Field Resolvers with DataLoaders

Instead of using `SimpleObject`, our GraphQL types use `#[Object]` with field resolvers that leverage DataLoaders:

```rust
#[Object]
impl EconomicSeriesType {
    // Efficient source lookup using DataLoader
    async fn source(&self, ctx: &Context<'_>) -> Result<Option<DataSourceType>> {
        let loaders = ctx.data::<DataLoaders>()?;
        let source_uuid = Uuid::parse_str(&self.source_id)?;
        
        match loaders.data_source_loader.load_one(source_uuid).await? {
            Some(source) => Ok(Some(source.into())),
            None => Ok(None),
        }
    }
    
    // Batched data point retrieval
    async fn recent_data_points(&self, ctx: &Context<'_>, limit: i32) -> Result<Vec<DataPointType>> {
        let loaders = ctx.data::<DataLoaders>()?;
        let series_uuid = Uuid::parse_str(&self.id)?;
        
        let data_points = loaders
            .latest_data_points_loader
            .load_one(series_uuid)
            .await?
            .unwrap_or_default();
        
        Ok(data_points.into_iter().take(limit as usize).map(DataPointType::from).collect())
    }
}
```

## GraphQL Schema

### Queries

#### Core Data Queries
- `series(id: ID!)` - Get a specific economic series
- `seriesList(filter: SeriesFilter, pagination: PaginationInput)` - List series with filtering
- `searchSeries(query: String!, ...)` - Full-text search across series
- `dataSource(id: ID!)` - Get a specific data source
- `dataSources` - List all data sources
- `seriesData(seriesId: ID!, filter: DataFilter, transformation: DataTransformation)` - Get time series data

#### Monitoring Queries
- `crawlerStatus` - Get crawler status information
- `queueStatistics` - Get queue processing statistics

### Mutations

- `triggerCrawl(input: TriggerCrawlInput!)` - Manually trigger data crawling

### Types

#### Core Types
```graphql
type EconomicSeries {
  id: ID!
  sourceId: ID!
  externalId: String!
  title: String!
  description: String
  units: String
  frequency: String!
  seasonalAdjustment: String
  lastUpdated: DateTime
  startDate: Date
  endDate: Date
  isActive: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Efficient relationship fields using DataLoaders
  source: DataSource
  recentDataPoints(limit: Int = 100): [DataPoint!]!
  dataPointCount: Int!
  dataPoints(filter: DataFilter, transformation: DataTransformation): [DataPoint!]!
}

type DataSource {
  id: ID!
  name: String!
  description: String
  baseUrl: String!
  apiKeyRequired: Boolean!
  rateLimitPerMinute: Int!
  createdAt: DateTime!
  updatedAt: DateTime!
  
  # Batched series lookup
  series(first: Int = 50, after: String): SeriesConnection!
  seriesCount: Int!
}

type DataPoint {
  id: ID!
  seriesId: ID!
  date: Date!
  value: Decimal
  revisionDate: Date!
  isOriginalRelease: Boolean!
  createdAt: DateTime!
  updatedAt: DateTime!
}
```

#### Transformation Support
```graphql
enum DataTransformation {
  NONE
  YEAR_OVER_YEAR
  QUARTER_OVER_QUARTER
  MONTH_OVER_MONTH
  PERCENT_CHANGE
  LOG_DIFFERENCE
}

type TransformedDataPoint {
  date: Date!
  originalValue: Decimal
  transformedValue: Decimal
  transformation: DataTransformation!
  revisionDate: Date!
  isOriginalRelease: Boolean!
}
```

#### Pagination
```graphql
type SeriesConnection {
  nodes: [EconomicSeries!]!
  totalCount: Int!
  pageInfo: PageInfo!
}

type PageInfo {
  hasNextPage: Boolean!
  hasPreviousPage: Boolean!
  startCursor: String
  endCursor: String
}
```

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

### Multiple Series Efficiently (No N+1)
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

1. **Batched Database Queries** - DataLoaders automatically batch multiple requests into single queries
2. **Query Complexity Analysis** - Built-in protection against expensive queries
3. **Caching** - DataLoaders provide per-request caching
4. **Efficient Pagination** - Cursor-based pagination for large result sets
5. **Selective Field Loading** - Only requested fields are processed
6. **Custom Loaders** - Specialized loaders for complex filtering scenarios

## Development Tools

- **GraphQL Playground** - Available at `/graphql/playground` in development
- **Introspection** - Full schema introspection support
- **Query Validation** - Automatic query validation and error reporting

## Migration from REST

The GraphQL API provides all functionality previously available through REST endpoints with improved efficiency and flexibility. Legacy REST endpoints are maintained for backward compatibility during the transition period.
